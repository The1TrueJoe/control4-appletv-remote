#!/usr/bin/env python3
"""Minimal c4vc client — push Opus microphone audio to Control4's `appled`.

Reimplements the voice_apple -> appled:9200 wire protocol recovered in
PROTOCOL.md, so our Siri Remote audio can reach the Apple TV through the
controller's existing (already HomeKit-paired) appled, bypassing voiced.

Two modes:
  --handshake-only   connect, handshake, report accept/reject, disconnect.
                     Makes the Apple TV do nothing; safe to probe.
  (default)          handshake, then stream a WAV as 20 ms Opus frames, finish.
                     This WILL trigger Siri on the paired Apple TV.

Wire protocol (all big-endian):
  handshake (12B):  "C4VC" | u16 ver=1 | u16 rsvd=0 | u32 stream_id(!=0)
  accept iff appled echoes the same stream_id back in its 12B reply.
  audio pkt:        0x01 | 0x00 | u16 len=12+N | u32 ms | f64 rms_db | opus[N]
  finish:           0x02 | u8 success | 0x00 0x00

Usage:
  c4vc_push.py <appled-host> --handshake-only
  c4vc_push.py <appled-host> --wav utterance.wav            # 16 kHz mono
  c4vc_push.py <appled-host> --wav x.wav --realtime         # 20 ms paced
"""
import argparse
import ctypes
import math
import os
import random
import socket
import struct
import sys
import time
import wave

PORT = 9200
MARKER = b"C4VC"
FRAME_MS = 20
RATE = 16000
SAMPLES_PER_FRAME = RATE * FRAME_MS // 1000  # 320

# --- libopus (encode PCM -> 20 ms Opus frames) --------------------------------
OPUS_APPLICATION_VOIP = 2048


def load_opus():
    for name in ("libopus.so.0", "libopus.so", "libopus.0.dylib", "libopus.dylib"):
        try:
            return ctypes.CDLL(name)
        except OSError:
            continue
    # Homebrew on macOS may need an explicit path.
    for p in ("/opt/homebrew/opt/opus/lib/libopus.dylib", "/usr/local/opt/opus/lib/libopus.dylib"):
        if os.path.exists(p):
            return ctypes.CDLL(p)
    sys.exit("libopus not found (brew install opus, or apt install libopus0)")


class OpusEncoder:
    def __init__(self):
        self.lib = load_opus()
        self.lib.opus_encoder_create.restype = ctypes.c_void_p
        self.lib.opus_encode.argtypes = [
            ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int,
            ctypes.c_char_p, ctypes.c_int,
        ]
        err = ctypes.c_int()
        self.st = self.lib.opus_encoder_create(RATE, 1, OPUS_APPLICATION_VOIP, ctypes.byref(err))
        if not self.st or err.value != 0:
            sys.exit(f"opus_encoder_create failed: {err.value}")

    def encode(self, pcm_frame: bytes) -> bytes:
        out = ctypes.create_string_buffer(4000)
        n = self.lib.opus_encode(self.st, pcm_frame, SAMPLES_PER_FRAME, out, len(out))
        if n < 0:
            sys.exit(f"opus_encode failed: {n}")
        return out.raw[:n]


def rms_db(pcm_frame: bytes) -> float:
    if not pcm_frame:
        return -120.0
    n = len(pcm_frame) // 2
    samples = struct.unpack(f"<{n}h", pcm_frame[: n * 2])
    acc = sum(s * s for s in samples) / n
    if acc <= 0:
        return -120.0
    return 20.0 * math.log10(math.sqrt(acc) / 32768.0)


# --- c4vc protocol ------------------------------------------------------------
def handshake(sock: socket.socket) -> int:
    stream_id = random.randint(1, 0x7FFFFFFF)
    sock.sendall(MARKER + struct.pack(">HHI", 1, 0, stream_id))
    reply = b""
    while len(reply) < 12:
        chunk = sock.recv(12 - len(reply))
        if not chunk:
            sys.exit("appled closed the connection during handshake")
        reply += chunk
    if reply[:4] != MARKER:
        sys.exit(f"bad handshake reply marker: {reply[:4]!r}")
    ver, rsvd, echoed = struct.unpack(">HHI", reply[4:12])
    print(f"  reply: ver={ver} rsvd={rsvd} id={echoed} (sent {stream_id})")
    if echoed == 0:
        sys.exit("appled REJECTED the handshake (id zeroed)")
    if echoed != stream_id:
        sys.exit(f"appled returned a different id ({echoed} != {stream_id})")
    print("  handshake ACCEPTED")
    return stream_id


def audio_packet(opus: bytes, ms: int, db: float) -> bytes:
    payload = struct.pack(">I", ms) + struct.pack(">d", db) + opus
    return struct.pack(">BBH", 0x01, 0x00, len(payload)) + payload


def finish_packet(success: bool = True) -> bytes:
    return struct.pack(">BBH", 0x02, 1 if success else 0, 0)


def stream_wav(sock, path, realtime):
    w = wave.open(path, "rb")
    if w.getframerate() != RATE or w.getnchannels() != 1 or w.getsampwidth() != 2:
        sys.exit(f"need 16 kHz mono 16-bit WAV; got {w.getframerate()} Hz "
                 f"{w.getnchannels()}ch {w.getsampwidth() * 8}-bit")
    enc = OpusEncoder()
    sent = 0
    while True:
        pcm = w.readframes(SAMPLES_PER_FRAME)
        if len(pcm) < SAMPLES_PER_FRAME * 2:
            break  # drop the short trailing frame
        sock.sendall(audio_packet(enc.encode(pcm), FRAME_MS, rms_db(pcm)))
        sent += 1
        if realtime:
            time.sleep(FRAME_MS / 1000.0)
    sock.sendall(finish_packet(True))
    print(f"  streamed {sent} frames ({sent * FRAME_MS / 1000:.2f}s), sent finish")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("host", help="appled host (a non-Director controller)")
    ap.add_argument("--port", type=int, default=PORT)
    ap.add_argument("--handshake-only", action="store_true",
                    help="probe the handshake only; the Apple TV does nothing")
    ap.add_argument("--wav", help="16 kHz mono 16-bit WAV to stream")
    ap.add_argument("--realtime", action="store_true", help="pace at 20 ms/frame")
    args = ap.parse_args()

    print(f"connecting to appled {args.host}:{args.port}")
    with socket.create_connection((args.host, args.port), timeout=10) as sock:
        sock.settimeout(10)
        handshake(sock)
        if args.handshake_only:
            print("  handshake-only: done, not streaming")
            return
        if not args.wav:
            sys.exit("give --wav <file> to stream, or --handshake-only")
        stream_wav(sock, args.wav, args.realtime)


if __name__ == "__main__":
    main()
