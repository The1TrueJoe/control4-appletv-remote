# ESP32 Siri Remote → Control4 bridge (Zephyr firmware)

A self-contained bridge: BLE-connects an Apple TV Siri Remote, joins your WiFi,
auto-appears in Control4, and forwards buttons/swipes as room commands and Siri
voice to the Apple TV via `appled`. No host needed. Target: **ESP32-S3** (FPU +
dual core; required for A1513 Speex→Opus transcode). C on Zephyr.

The BLE/protocol half lives in the reusable `siri_remote` Zephyr module
(`github.com/The1TrueJoe/siri-remote`, `zephyr/`); this app adds WiFi, mDNS, a
web UI, the forwarder, and the voice relay.

## Build

```sh
# one-time workspace
mkdir siri-ws && cd siri-ws
west init -l /path/to/control4-appletv-remote/firmware
west update
west zephyr-export

# build for a board
west build -b xiao_esp32s3/esp32s3/procpu \
  /path/to/control4-appletv-remote/firmware
# or: esp32s3_devkitm/esp32s3/procpu
west flash
```

## First boot (provisioning)

1. The bridge starts a SoftAP `SiriBridge-setup`. Join it.
2. Browse `http://192.168.4.1/`, enter WiFi, the Control4 target URL
   (the native driver's URL, or the roomcontrol-webhook `http://<ctrl>:5080`),
   the room, and the remote's BD address. Save → it reboots and joins WiFi.
3. It advertises `_appletv-bridge._tcp`; the Control4 driver discovers it.
4. Press a button on the remote to wake/pair it.

## Status / what works

| Part | State |
|------|-------|
| `siri_remote` module (BLE central, unlock, 0x98, parse) | written; **untested on hardware** |
| WiFi STA + provisioning AP | written; Zephyr WiFi-mgmt API **UNVERIFIED** per release |
| mDNS advertise | DNS-SD with static instance + HTTP `/health` for room/model (see mdns_adv.c) |
| Web UI (form + /health) | written; Zephyr `http_server` API **UNVERIFIED** per release; POST parsing is a TODO |
| Forwarder (buttons → /command) | map done; HTTP-client POST is a sketch (TODO), per docs/FORWARDER.md |
| Voice → appled (A1962 Opus) | c4vc binary framing done; the appled `:9200` session setup is a **TODO** (gated on the live bring-up) |
| Voice A1513 (Speex→Opus) | **stubbed** behind `CONFIG_SIRI_BRIDGE_TRANSCODE`; needs an S3 benchmark |

This firmware is authored but **not built or run** (no SDK/board at authoring
time). Every spot relying on a version-specific Zephyr API is marked
`UNVERIFIED`/`TODO` in the source. CI (`west build`) is the first gate; a real
S3 is needed for the rest.
