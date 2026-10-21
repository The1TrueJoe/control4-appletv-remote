# Control4 voice path (`voiced`)

How a Control4 EA controller takes microphone audio from a remote and gets it
to a target device (Apple TV, cable box, etc.). Pulled from a stock EA-1 on OS
`3.3.3.660628-res` (Front-Garage-EA1, 192.168.1.139), 2026-10-20.

## The players

| Process | Port | Role |
|---|---|---|
| `remote_keyd` | tcp 5115 | Receives key/voice events from the physical remote (Zigbee via `/dev/c4devices/button_master`). |
| `voiced` | tcp 9250, 9252 | Voice daemon. C++ (boost::asio/beast). Decodes/encodes Opus, runs one process per **voice service**, pushes audio to a target. |
| `audio3server/client/streamer` | — | The music/streaming path (AirPlay-style), **not** the remote-voice path. Separate. |

Binaries saved under `bin/` (both ELF 32-bit i386, stripped):
`voiced` (4.9 MB, Jan 2023), `remote_keyd` (80 KB, Apr 2023).
Source tree on the box: `/source/control4/servers/voiced/`
(`daemon.cpp`, `httpserver.cpp`, `lib/voicesrv.cpp`, `lib/base.cpp`).
Links `libopus` and `libspeex`/`libspeexdsp` (both present in `/control4/lib`).

## The protocol (JSON over a socket, port 9250)

`voiced` speaks line/JSON via an `AJsonMessenger`. Commands (from the embedded
help text, verbatim):

```jsonc
// Start a management session. Only one at a time; when the management
// connection drops, voiced STOPS ALL SERVICE INSTANCES.
{"method":"manage","args":{}}

// Start a service instance in its own process.
// voice_target_config is "<id>@<ip>:<port>" — the TARGET audio is pushed to.
{"method":"start","args":{"name":"apple1","service":"apple",
  "voice_target_config":"1@192.168.1.145:9201","data":{}}}

{"method":"status","args":{}}
{"method":"stop","args":{"name":"comcast1"}}
{"method":"clear","args":{"name":"comcast1","data":{"all":true}}}
{"method":"help","args":{}}
```

Known service types: **`apple`**, **`comcast`** (from the help examples).
A service is a process that knows how to hand audio to one kind of target.

## Within a service: mapping a remote to a target, then pushing audio

Once a service is started, a device (a c4id — the Control4 proxy id of the
AppleTV/cable device in the project) is bound to it and audio is streamed:

- `map_c4id {internal, encoding}` — bind a Control4 device id to the service.
  `encoding` is the audio encoding (opus / pcm / passthrough). Logs
  `map_c4id mapping`, `mapped c4id`, `Internal mapping for c4id`.
- `unmap_c4id`, `abort_c4id {internal}` — unbind / abort a push.
- Audio push lifecycle (log phrases): `Starting audio input for c4id` →
  `Audio push to <target>` → frames over a **c4vc data stream** →
  `Finish audio push for c4id`.
- `voice_server::client::acceptAudioStream(..., encoding_type)` and
  `VDaemon::handleEnablePushVoice(c4id, cb(bool, encoding_type))` are the
  internal entry points; `encoding_type` ∈ {opus, pcm, passthrough}.
- `Registering wakeword for ...` exists — services can carry a wakeword.

## The last hop: `appled` is a HomeKit accessory

`voice_apple` (`AppleVService`) does **not** talk to the Apple TV directly. It
**only accepts Opus** ("Unexpectedly received encoding other than opus") and
hands each packet to **`appled`** over a local c4vc stream
(`AppledConnection`, `appled_stream_id`, "packet to send to appled").

`appled` (1.8 MB, `/source/control4/servers/appled/`) is built on **Apple's
official HAP ADK** (HomeKit Accessory Development Kit). Control4 pairs with the
Apple TV as a **HomeKit Target Control (remote) accessory** and streams Siri
audio over a HomeKit DataStream:

- `HAPPlatformMicrophoneStartOpusStream` — mic audio is Opus over the stream.
- `HAPDataStreamProtocols+TargetControl.c` — the HomeKit remote/button protocol.
- `HAPDataStreamProtocols+DataSend.c` — the Siri-audio DataSend protocol.
- `HAPDataStream+TCP.c` / `+HAP.c` — the transport.
- Log phrases: "No siri channel is active on remote", "Siri for Apple TV
  control is allowed for remote accessories", "Siri session invalidated on
  remote before voice input stop".
- Standard HomeKit pair-setup / pair-verify / pairings with a setup code.

So the Apple TV sees Control4 as an Apple-TV-remote HomeKit accessory, exactly
like the Remote widget or a licensed third-party remote, and Siri audio rides
Apple's own DataSend protocol.

## Full chain

```
Siri Remote (BLE)
  → remote_keyd  (Zigbee/BLE key+voice events, tcp 5115)
  → voiced       (JSON control, tcp 9250; routes by c4id to a service)
  → voice_apple  (AppleVService; Opus only; per-service process)
  → appled       (HomeKit HAP ADK: Target Control + Siri DataSend)
  → Apple TV     (paired HomeKit accessory, Opus over HomeKit DataStream)
```

Services present: `voice_apple` (Apple TV / HomeKit), `voice_amazon` (Alexa),
`voice_comcast` (cable). **There is no Roku voice service** in these binaries —
Roku is not a Control4 voice target here, so remote voice goes to the Apple TV
(or Alexa/cable), not to a Roku.

## What this means for our remote

The path that gets remote mic audio to an Apple TV / Roku is:

1. A **voice service** (`apple`) is running in `voiced`, pointed at the target
   with `voice_target_config = <id>@<targetIP>:<port>`.
2. Our remote's **c4id** is mapped into that service (`map_c4id`, encoding
   `opus`).
3. We **push Opus frames** into the c4vc stream for that c4id.

Our `siri-remote` work already produces exactly what `voice_apple` requires:
16 kHz mono audio it can hand on as Opus (the A1962 is Opus natively; the A1513
is Speex, so decode → re-encode Opus). So "getting into the voice path" =
speaking `voiced`'s JSON protocol, mapping our remote's c4id into the `apple`
service with `encoding:"opus"`, and pushing Opus frames — `appled` then carries
it to the Apple TV over HomeKit, with no Apple-side work on our part.

Two ways to drive it:
- **On an openHC controller** we own: run/drive `voiced` + `appled` directly.
- **Against a stock controller**: speak the `voiced` JSON protocol as Director
  does. Needs the c4id↔device binding and the apple service's
  `voice_target_config` (next section).

## Safety note while investigating

**Do not open a `manage` session against a live controller.** The help text is
explicit: when the management connection drops, `voiced` stops every service
instance — i.e. it would kill voice for the whole house when we disconnect.
Inspect config/registration on disk and in `/proc` instead; only drive the
protocol against a controller we have taken over (openHC) or a dedicated test
unit.

## The cleaner injection point: `appled` on tcp 9200

Observed live (read-only) on Front-Garage-EA1: `appled` runs continuously and
**listens on tcp 9200**, while **no `voice_apple` process exists** until a
Siri press spins one up. So the stable, always-present piece is `appled` — it
holds the HomeKit pairing to the Apple TV and waits on :9200. `voice_apple` is
a short-lived shim `voiced` spawns, which connects to `appled:9200` and feeds
it Opus.

That means `appled:9200` is a better target than `voiced`: it sidesteps the
management-session singleton (and its "disconnect stops all services" hazard)
entirely. `appled` speaks two protocols:

1. **Inbound c4vc (tcp 9200)** — from `voice_apple`. Handshake marker **`C4VC`**,
   then a JSON **`audio_info`** (`deserialize_audioinfo(Json::Value, ...)`;
   fields include encoding/codec, sample rate, channels — "channels! expected
   only mono!"), then **Opus** packets, then a finish packet. Log phrases:
   "c4vc handshake was accepted for c4vc stream", "Failed to decode audio info",
   "Ignoring data packet, stream was not started for internal". **This is our
   injection target.**
2. **Outbound HomeKit DataSend/Siri** — to the Apple TV. `appled` negotiates the
   Siri audio codec over HAP (`AudioCodecTypeIsSupportedForSiri`,
   `AudioSampleRateIsSupportedForSiri`) and owns the pairing. We do not touch
   this; `appled` handles it once it has our Opus.

### CORRECTION (verified live 2026-10-21): `:9200` is a JSON control plane

A live probe showed `appled:9200` is **not** the raw c4vc stream — it's a
newline-JSON messenger (`{"method":...,"args":...}\n`; a binary blob gets
`{"error":{"code":3,"message":"Failed to parse request"}}`). The c4vc binary
stream runs on a **separate, on-demand port** that `appled` opens only after a
remote is activated ("Voice server is accepting cv4c connections on port <P>";
examples use `:9201`). See PROTOCOL.md §0.

So the Control4-native sequence to get Siri audio to the Apple TV is heavier
than "stream Opus". On `:9200`, in order:

1. `provision` → `bridge` → `remote {id,data}` — register/start a HomeKit
   remote; this opens the voice-server port P.
2. `activate {identifier,timeout}` — HomeKit TargetControl (drives the Apple TV).
3. `button {button,state:"down",timestamp}` — the Siri press that opens the
   "siri channel" (`appled` refuses audio otherwise: "No siri channel is active
   on remote").
4. open a **new** TCP connection to `appled_ip:P` and speak the c4vc binary
   (handshake §1a → audio §4 → finish §6).

**Risk:** steps 1–3 mutate the controller's HomeKit bridge state and actively
drive the paired Apple TV — not harmless probing. Several sub-schemas
(`provision`/`bridge`/`remote.data`, and which response returns P) are still
unknown, and there is no native C4 voice remote in this house to capture a
reference from, so finishing this means experimenting against a live `appled`.
Do it only on a controller whose Apple TV is expendable for testing, never on
Director (Living-room EA-1).

`c4vc_push.py` here implements the c4vc binary half (handshake + Opus + finish);
it is correct but cannot be used until the `:9200` setup above opens port P.
`appled_ctl.py` drives the `:9200` JSON control plane one command at a time
(status / remote / activate / button / raw; provision & bridge are guarded) so
the bring-up can be run deliberately with each reply visible.

### Live-controller facts to finish this (gating a bring-up)

- **Never send `provision`/`bridge` to the garage appled.** It is already
  provisioned, bridged and paired to its Apple TV; those are first-time-setup
  commands (`provision` resets appled's HAP identity/token, `cmdProvision`
  `0x080c7010`) and can disturb the existing pairing. Live bring-up uses only
  `remote` → `activate` → `button down` → stream → `button up` → finish →
  `activate timeout:0` → `remote` with flags false (cleanup).
- **`status` returns nothing** to a fresh `:9200` connection (observed
  2026-10-21), even reading to the newline. The decode shows it builds
  `data:{provisioned,bridged}` unconditionally, so the reply path likely needs a
  session/context we haven't established. Unresolved — needs a live iterative
  session or more RE.
- **Port P is not returned over `:9200`** — only in appled's log line "Voice
  server is accepting cv4c connections on port <P>". Read it via the flasher
  (`ohc-flash sh <ip> 'netstat -lnp | grep appled'`) right after `remote`
  starts the voice server.
- **The Siri `button` id is accessory-specific** (set at provisioning,
  validated against the Apple TV's button config) — not a constant; discover it
  live (try the TargetControl button ids, watch for "Target does not have
  button" errors).
- Adding a `remote` exposes a new HomeKit TargetControl remote on that appled's
  bridge; reversibility (cleanup via flags-false / ABridge `ignore`/`stop`) is
  believed but unverified. So the first bring-up must be on a controller whose
  Apple TV is expendable, with the TV in view.

**Original (pre-correction) plan:**
1. Nail the exact `appled:9200` c4vc wire format — the `C4VC` handshake framing
   and the `audio_info` JSON schema (shape known from strings; exact bytes still
   needed).
2. Build a small c4vc client that connects to `appled:9200`, handshakes, sends
   `audio_info` (opus, 16 kHz, mono), streams our Opus frames for a c4id, and
   finishes.
3. Feed it from the `siri-remote` stack, which already produces 16 kHz mono
   Opus-capable audio.

There is no native Control4 voice remote in this house (only the Apple Siri
Remote we're building), so there's no reference Siri press to capture. The
protocol is instead being recovered by disassembling `voice_apple` (the client
side of `:9200`) with radare2.

### Handshake, confirmed from `voice_apple` disassembly (fcn @ 0x08092676)

The outbound c4vc buffer is built as:

```
offset 0..3 : "C4VC"            marker
offset 4    : 0x00
offset 5    : 0x01              version (?)
offset 6    : 0x00
offset 7    : 0x00
offset 8..11: <id>, big-endian  4-byte stream/c4id (from the connection object)
offset 12.. : std::string payload appended next (the audio_info JSON — see
              deserialize_audioinfo(Json::Value, audio_info))
```

Then Opus packets are streamed (`queueAudioPacket` / `writeQueuedAudioPackets`),
and a finish packet ends it (`finishAudioPush`). Still to pin down: the exact
length framing around the JSON and each Opus packet, the `audio_info` JSON
field names, and `appled`'s accept/reject reply ("c4vc handshake was accepted
for c4vc stream"). All recoverable from the same two binaries.

Final verification needs a live `appled:9200` on a **non-Director** controller
(Front- or Rear-Garage EA-1) with its paired Apple TV visible, to confirm Siri
actually triggers. Director (Living-room EA-1, 192.168.1.172) is read-only —
never interrupt it.

## Open questions

- What `voice_target_config` do the live `apple`/cable services actually use —
  i.e. what IP:port is the Apple TV / Roku audio sink, and what protocol does
  that sink speak (9201)? Needs the live service config (registry/driver), read
  only.
- Who normally calls `manage` + `start` + `map_c4id`? Likely Director, from the
  room/endpoint driver. Confirm the c4id ↔ device binding source.
- `remote_keyd` → `voiced` handoff: how a Siri press on a C4 remote reaches
  `handleEnablePushVoice` (shared pipe? the sockets on voiced fd 7/8/11?).
- Port 9252 vs 9250: which is the JSON control port and which carries audio
  (httpserver.cpp suggests 9252 may be HTTP).
