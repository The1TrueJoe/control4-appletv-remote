# control4-appletv-remote

Drive a Control4 room from an Apple TV Siri Remote: buttons, swipes and
hold-to-ramp volume become room commands, through the Room Control Webhook
driver.

The remote itself — protocol, raw ATT transport, gestures, audio and the
`siri-events` CLI — lives in
[siri-remote](https://github.com/The1TrueJoe/siri-remote). This repo is the
Control4 side, and depends on it as a pinned git dependency.

## What's here

| Path | What it is |
|---|---|
| `crates/siri-c4d` | The Control4 bridge: web UI, multi-remote support and a systemd unit. |
| `examples/python/control4_glue.py` | The same mapping in ~100 lines of Python over `siri-events`, for when you'd rather not run the daemon. |
| `scripts/install.sh` | Installs `siri-c4d` and `siri-events` on a Pi. |
| `dist/` | Prebuilt aarch64 binaries. |

## Quick start

```bash
sudo ./scripts/install.sh --binary-dir ./dist    # or --build
```

The installer sets up `siri-c4d` and prints a URL: open it, enter your
controller address, then scan and pair. It maps buttons and swipes to room
commands, handles several remotes at once (each optionally driving a different
room), and serves its own UI from inside the binary.

It also installs `siri-events`, so glue scripts work too:

```bash
siri-events listen DC:2B:2A:E0:18:71 \
  | examples/python/control4_glue.py --host 10.0.0.20 --room "Living Room" --dry-run
```

The event format, voice options and library API are documented in
[siri-remote](https://github.com/The1TrueJoe/siri-remote#readme).

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

Plain `cargo build --release`. Cargo fetches `siri-remote` at the tag pinned
in `crates/siri-c4d/Cargo.toml`; to build against a local checkout instead,
uncomment the `[patch]` block in the root `Cargo.toml`.

The only wrinkle is that the build links C libraries — `libspeex` and
`libopus` for microphone decoding and `libdbus` for pairing — which makes
cross-compilation awkward, since it needs those libraries for the target
architecture rather than the host.

Two options, in order of convenience:

**On the Pi itself.** Slowest but simplest, and always correct — it is the
target. A Pi 3 wants `-j2` so the linker does not exhaust memory.

```bash
sudo apt install build-essential pkg-config libdbus-1-dev libopus-dev libspeex-dev
cargo build --release -j2
```

**In a container.** Fast on an Apple Silicon Mac, where `linux/arm64` runs
natively rather than emulated:

```bash
docker build --platform linux/arm64 -t siri-build -f scripts/Dockerfile.build scripts/
docker run --rm --platform linux/arm64 -v "$PWD":/w -w /w siri-build cargo build --release
```

`siri-c4d` is Linux-only. For a fast edit-test loop on the remote protocol
itself, work in the `siri-remote` repo, whose library builds and tests on macOS.

## Credits

The remote protocol work and its credits are in
[siri-remote](https://github.com/The1TrueJoe/siri-remote#credits).
