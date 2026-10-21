# Forwarder contract

What a bridge (Pi `siri-c4d` or ESP firmware) sends to Control4. Keeping this
one contract lets both bridges and both targets (the native `.c4z` driver and
the fallback `roomcontrol-webhook`) stay interchangeable.

## Targets

A bridge has one or more **forwarder targets**, each with a `kind`:

| kind | endpoint | used for |
|------|----------|----------|
| `webhook` | `http://<controller>:5080` (roomcontrol-webhook) | buttons/room-control (fallback, proven) |
| `driver` | `http://<controller>:<driverPort>` (native `.c4z` listener) | buttons/room-control (native, default) |
| `appled` | `<controller>:9200` + c4vc stream | **voice** only (separate hop, binary) |

`webhook` and `driver` speak the **same HTTP/JSON command API** below, so the
forwarder code is identical for both — only the base URL differs. `appled` is a
different, binary protocol (see `docs/c4-voice-path/PROTOCOL.md`).

## Command API (HTTP/JSON, shared by `webhook` and `driver`)

Auth: optional `Authorization: Bearer <password>` when a password is set.
Room: numeric id, room name (case-insensitive), comma list, or `all`; omit to
let the target infer the single bound room.

```
POST /command  {"command":"PLAYPAUSE","room":"Living Room"}
POST /preset   {"preset":1,"room":123}
POST /button   {"button":"Party","action":"tap","room":123}
GET  /health            -> {"ok":true,...}
GET  /rooms|/commands|/presets|/buttons
```

### Button/swipe → command mapping

The **bridge** owns the mapping (configurable in its web UI); it POSTs resolved
commands, so the target never sees raw remote buttons. An action is one of:

- `command{command}` — fire once on press.
- `hold{start,stop}` — `start` on press, `stop` on release (volume ramps etc.).
- `preset{preset}` — fire a room preset on press.
- `button{button,action}` — trigger a room custom button.
- `none` — unmapped.

Default Siri-remote → Control4 map (matches `siri-c4d` `config.rs`):

| control | action |
|---------|--------|
| `MENU` | command `MENU` |
| `PLAY_PAUSE` | command `PLAYPAUSE` |
| `VOLUME_UP` | hold `START_VOL_UP` / `STOP_VOL_UP` |
| `VOLUME_DOWN` | hold `START_VOL_DOWN` / `STOP_VOL_DOWN` |
| `TOUCHPAD` | command `ENTER` |
| `TOUCHPAD_2` | command `BACK` |
| `AIRPLAY` | command `ROOM_OFF` |
| `SWIPE_UP/DOWN/LEFT/RIGHT` | command `UP`/`DOWN`/`LEFT`/`RIGHT` |
| `SIRI` | voice (appled), not a room command |

## Voice (`appled` target)

`SIRI` does not map to a room command. On Siri-hold the bridge opens the
`appled` voice path and streams Opus (A1513: transcode Speex→Opus first; A1962:
relay). Full binary protocol: `docs/c4-voice-path/PROTOCOL.md`.

## Discovery

A bridge advertises `_appletv-bridge._tcp` over mDNS with TXT keys
`name`, `room` (if configured), `port` (its web UI / status port), `model`
(a1513/a1962), `id`. The native `.c4z` driver browses this to list and bind
bridges. A bridge reaches its Control4 target by the controller address
configured in its web UI (or SDDP discovery of the controller).
