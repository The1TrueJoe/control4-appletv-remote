# control4-appletv-remote

Use an Apple TV Siri Remote as a Bluetooth input device on Linux — buttons,
touchpad, swipes, battery, and the microphone stream — and route it wherever
you like.

The remote publishes a deliberately null HID descriptor, so the kernel cannot
drive it and BlueZ refuses the write that unlocks it. This talks to it directly
over a raw ATT channel instead. See
[docs/siri-remote-gen1-findings.md](docs/siri-remote-gen1-findings.md) for how
the protocol was worked out.

## What's here

| Crate / path | What it is |
|---|---|
| `crates/siri-remote` | **The library.** Protocol, transport, gestures, Opus audio. Produces events; decides nothing. |
| `crates/siri-events` | **The seam.** Streams events as newline-delimited JSON so glue can be written in any language. |
| `crates/siri-c4d` | **A worked example.** Full Control4 bridge with a web UI, multi-remote support and a systemd unit. |
| `examples/` | Glue scripts — Python for Control4 and for live audio, shell for MQTT. |

The split is deliberate. The library and `siri-events` know nothing about
Control4; `siri-c4d` is one consumer of them, not the point. To drive something
else, write glue against the event stream and leave the rest alone.

**Speech recognition is out of scope.** The remote's microphone is 16 kHz mono
PCM — exactly what offline recognisers want — so this project hands you the
audio and gets out of the way.

## Quick start

```bash
sudo ./scripts/install.sh --binary-dir ./dist    # or --build

siri-events scan                                 # press a button while it runs
siri-events pair DC:2B:2A:E0:18:71
siri-events listen DC:2B:2A:E0:18:71 | examples/python/minimal.py
```

For the Control4 bridge, the installer also sets up `siri-c4d` and prints a URL:
open it, enter your controller address, then scan and pair. It maps buttons and
swipes to room commands, handles several remotes at once (each optionally
driving a different room), and serves its own UI from inside the binary.

## Event stream

One JSON object per line on stdout; diagnostics go to stderr.

```json
{"event":"connected","address":"DC:…","mtu":104,"audio_available":true}
{"event":"button","button":"MENU","state":"down"}
{"event":"swipe","direction":"SWIPE_LEFT"}
{"event":"battery","percent":27}
{"event":"voice_start"}
{"event":"voice_end","seconds":1.04,"wav":"/tmp/voice/siri-….wav"}
```

Controls: `MENU`, `PLAY_PAUSE`, `VOLUME_UP`, `VOLUME_DOWN`, `AIRPLAY`, `SIRI`,
`TOUCHPAD`, `TOUCHPAD_2`, and `SWIPE_{UP,DOWN,LEFT,RIGHT}`.

## Voice

Holding **Siri** streams Opus at 16 kHz mono in 20 ms frames, decoded to PCM.

```bash
# whole utterances as WAV files
siri-events listen <addr> --audio-dir /tmp/voice

# live PCM while the button is held, for streaming recognisers
mkfifo /tmp/siri.pcm
siri-events listen <addr> --pcm-fifo /tmp/siri.pcm &
examples/python/voice_stream.py /tmp/siri.pcm
```

**Audio needs a Bluetooth 4.2+ adapter.** Frames are 101 bytes, so the ATT MTU
must be at least 104, and the remote will not grant that to a controller
without Data Length Extension. A Raspberry Pi 3's onboard chip is 4.1 and gets
23 — buttons and touch work fine, audio does not, and you get an explicit
`audio_unavailable` event rather than silence. A cheap RTL8761B USB dongle
fixes it.

## Using the library directly

```rust
use siri_remote::{Session, SessionConfig, Event};

Session::run(&SessionConfig::new("DC:2B:2A:E0:18:71"), |event| {
    if let Event::ButtonDown(b) = event {
        println!("pressed {}", b.as_str());
    }
    true
})?;
```

```bash
cargo run -p siri-remote --example print_events -- DC:2B:2A:E0:18:71
```

Features: `opus` (microphone decoding, on by default) and `bluez` (discovery
and pairing, off by default — the rest of the crate is synchronous and has no
D-Bus dependency).

## Web UI

The UI is TypeScript, compiled and embedded in the `siri-c4d` binary at build
time — so deployment stays a single file with no asset directory to ship.

```
crates/siri-c4d/web/
├── index.html          markup only, no inline styles or scripts
├── css/app.css
├── src/*.ts            one module per concern
└── dist/               compiled output, embedded via rust-embed (gitignored)
```

`src/types.ts` mirrors the daemon's Rust structs, so a renamed field fails the
build rather than silently rendering `undefined`. `build.rs` runs the compile
automatically when sources change and Node is available; if `dist/` is already
current it does nothing, which keeps offline and release builds working.

```bash
cd crates/siri-c4d/web
npm ci
npm run build        # compile into dist/
npm run watch        # recompile on change
npm run typecheck    # types only, no output
```

## Building

Builds are plain `cargo build --release`. Compiling on a Pi 3 works but is
slow; cross-building in a container is faster, and on Apple Silicon it runs
natively:

```bash
docker build --platform linux/arm64 -t siri-build -f scripts/Dockerfile.build scripts/
docker run --rm --platform linux/arm64 -v "$PWD":/w -w /w siri-build cargo build --release
```

## Credits

The `0xAF` unlock byte, the button bitmask and the touch decoding come from
[Yanndroid/SiriRemote-Linux](https://github.com/Yanndroid/SiriRemote-Linux) and
[wiz78/SiriRemote-HomeAssistant](https://github.com/wiz78/SiriRemote-HomeAssistant).
Opus framing was cross-checked against captures from
[Jack-R1/SiriRemoteVoiceDecoder](https://github.com/Jack-R1/SiriRemoteVoiceDecoder).
