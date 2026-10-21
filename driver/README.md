# Apple TV Siri Remote Bridge — Control4 driver

Native Control4 DriverWorks driver that represents a Siri Remote **bridge** (a
Pi or ESP32 running the control4-appletv-remote firmware) and turns the remote's
buttons/swipes into Control4 **room commands**.

It is **not** a cable/source proxy. A Siri Remote is an input device that sends
commands, so this driver binds to a **Room** and issues commands with
`C4:SendToDevice`, reusing the command set and approach of the Room Control
Webhook suite. Voice is a **separate** path (bridge → `appled`), not handled by
this driver — see `docs/c4-voice-path/`.

## How it works

```
bridge --HTTP/JSON--> this driver --C4:SendToDevice--> bound Room
         (docs/FORWARDER.md command API)
```

- The driver runs a small HTTP listener on **Listen Port** (default 5090). The
  bridge's forwarder "driver" target POSTs `/command`, `/preset`, `/button`
  (and `GET /health`, `/rooms`, `/commands`) — the same shape as the webhook
  target, so they are interchangeable.
- Commands are validated against the Control4 room command allowlist (transport,
  volume with START_/STOP_ hold pairs, nav, power, source) and forwarded to the
  room(s) this driver is bound to.
- **mDNS discovery**: *Discover Bridges* browses `_appletv-bridge._tcp` and
  fills the *Discovered Bridges* list; pick one to set its IP (persisted).

## Install (Composer Pro)

1. Build: `./build.sh` → `apple_tv_remote_bridge.c4z`.
2. Composer → Driver → Add/Update Driver → select the `.c4z`.
3. Add the "Apple TV Siri Remote Bridge" device to the project.
4. **Bind** its Room connection(s) to the room(s) it should control.
5. Set **Listen Port** (or leave 5090) and optionally a **Password**; enter the
   same host/port/password in the bridge's web UI as its Control4 "driver"
   target.
6. Run *Discover Bridges* (optional) to confirm the bridge is seen.

## Files

- `driver.xml` — device metadata, properties, actions, room-control bindings
  (facing 6, type 1, class `APPLETV_REMOTE_ROOM`), events, icon refs.
- `driver.lua` — HTTP listener, command allowlist + room routing
  (`SendToDevice`), mDNS discovery, persistence.
- `json.lua`, `mdns.lua` — dependency-free helpers (required by `driver.lua`).
- `icons/` — original line-art remote icon (not Apple's asset) + SVG source.
- `build.sh` — packages the `.c4z`.

## Uncertainties (verify in Composer; I could not test here)

- **TCP server client API**: modeled on the Room Control Webhook suite's
  `C4:CreateTCPServer()` + `:OnListen/:OnError/:OnAccept` and a client with
  `:OnData/:Write/:Close`. The exact client callback names may differ by OS
  version — adjust `HandleClient` if needed.
- **mDNS over UDP**: uses `C4:CreateNetworkConnection(binding, addr, "udp")` +
  `NetConnect`/`SendToNetwork` and `ReceivedFromNetwork`. Multicast join/TTL
  behaviour is not verified; discovery is a convenience (manual IP always works).
- **Room id resolution**: bound room device ids come from `OnBindingChanged`'s
  `otherDeviceID`; name lookup uses `C4:GetDeviceDisplayName` when present.
- **Preset/custom-button** semantics are install-specific; `/preset` forwards a
  best-effort `SELECT_PRESET` and `/button` is a stub to wire per room.
- **Icon XML**: referenced via `<small>/<large>` in `driver.xml`; if Composer
  ignores them, move the PNGs to the form Composer expects for your OS.
