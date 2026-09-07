#!/usr/bin/env python3
"""Glue: Siri Remote events -> Control4 Room Control Webhook.

This is the pattern the library is designed around. `siri-events` decides
nothing; it prints what the remote did. This script decides what that means.
Swap it for your own and the same event stream drives anything.

    siri-events listen DC:2B:2A:E0:18:71 | ./control4_glue.py \
        --host 10.0.0.20 --password secret --room "Living Room"

Volume uses Control4's START_/STOP_ command pairs mapped onto button down/up,
so holding the button ramps the way a real remote does.
"""
import argparse
import json
import sys
import urllib.error
import urllib.request

# Control name -> what to send. A tuple means (on_press, on_release).
MAPPING = {
    "MENU": "MENU",
    "PLAY_PAUSE": "PLAYPAUSE",
    "TOUCHPAD": "ENTER",
    "TOUCHPAD_2": "BACK",
    "AIRPLAY": "ROOM_OFF",
    "VOLUME_UP": ("START_VOL_UP", "STOP_VOL_UP"),
    "VOLUME_DOWN": ("START_VOL_DOWN", "STOP_VOL_DOWN"),
    "SWIPE_UP": "UP",
    "SWIPE_DOWN": "DOWN",
    "SWIPE_LEFT": "LEFT",
    "SWIPE_RIGHT": "RIGHT",
}


class Control4:
    def __init__(self, host, port, password, room, timeout=2.5):
        self.base = f"http://{host}:{port}"
        self.password = password
        self.room = room
        self.timeout = timeout

    def command(self, command):
        body = {"command": command}
        # The driver infers the room when only one is linked.
        if self.room:
            body["room"] = self.room
        req = urllib.request.Request(
            f"{self.base}/command",
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        if self.password:
            req.add_header("Authorization", f"Bearer {self.password}")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as r:
                return True, r.status
        except urllib.error.HTTPError as e:
            return False, f"HTTP {e.code}"
        except Exception as e:
            return False, str(e)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--host", required=True, help="Control4 controller address")
    ap.add_argument("--port", type=int, default=5080)
    ap.add_argument("--password", default="", help="root driver password, if set")
    ap.add_argument("--room", default="", help="room id or name; blank if only one is linked")
    ap.add_argument("--dry-run", action="store_true", help="print instead of sending")
    args = ap.parse_args()

    c4 = Control4(args.host, args.port, args.password, args.room)

    def send(command, why):
        if args.dry_run:
            print(f"    would send {command}  ({why})", file=sys.stderr)
            return
        ok, detail = c4.command(command)
        print(f"    {command} {'ok' if ok else 'FAILED: ' + str(detail)}", file=sys.stderr)

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue

        kind = ev.get("event")

        if kind == "button":
            action = MAPPING.get(ev["button"])
            if action is None:
                continue
            if isinstance(action, tuple):
                # Hold-style: ramp while held.
                send(action[0] if ev["state"] == "down" else action[1], ev["button"])
            elif ev["state"] == "down":
                send(action, ev["button"])

        elif kind == "swipe":
            action = MAPPING.get(ev["direction"])
            if action:
                send(action, ev["direction"])

        elif kind == "voice_end":
            # Audio is handed to you, not interpreted. Point a recogniser at
            # the WAV, or read live PCM from siri-events --pcm-fifo.
            wav = ev.get("wav")
            print(
                f"    voice: {ev['seconds']:.2f}s"
                + (f" -> {wav}" if wav else " (not saved)"),
                file=sys.stderr,
            )

        elif kind == "connected":
            note = "" if ev.get("audio_available") else "  (no microphone at this MTU)"
            print(f"connected, MTU {ev['mtu']}{note}", file=sys.stderr)

        elif kind == "disconnected":
            print(f"disconnected: {ev.get('reason', '')}", file=sys.stderr)


if __name__ == "__main__":
    main()
