# The `c4vc` wire protocol (voice_apple → appled)

Recovered by static disassembly (radare2) of the stock EA-1 binaries
`bin/voice_apple` (the client) and `bin/appled` (the server). No binaries
were run and no network was touched. All multi-byte integers on the wire are
**big-endian**. Byte offsets below are 0-based from the start of each message.

> **IMPORTANT (corrected by a live probe + re-analysis): tcp 9200 is NOT the
> c4vc binary stream.** Port 9200 is a JSON line-messenger (`AJsonMessenger` /
> `ABase::Server`) — sending the raw 12-byte `C4VC…` blob there yields
> `{"error":{"code":3,"message":"Failed to parse request","type":"Invalid
> request"}}`. The c4vc **binary** protocol documented in §1b–§6 runs on a
> **separate, on-demand TCP port** (the voice server), not on 9200. See §0 for
> the JSON control plane and how the binary port is reached.

Reference bases used for address resolution (so every claim can be re-checked):

| binary | `.text` vaddr/fileoff | `.rodata` vaddr/fileoff | GOT base (`ebx`) |
|---|---|---|---|
| `voice_apple` | `0x08066000` / `0x1e000` | `0x082b1b00` / `0x269b00` | `0x08384800` |
| `appled` | `0x0806fcb0` / `0x27cb0` | `0x0818e820` / `0x146820` | `0x082084d8` |

Both binaries are non-PIE but compiled PIC-style: string/data operands are
loaded as `lea reg, [ebx ± disp]` where `ebx` = the GOT base above, so a string
at vaddr `V` is referenced with `disp = V - GOT`.

Key functions (demangled):

- `voice_apple` `fcn.08092650` — `AppledConnection::connect()/sendHandshake` (builds the 12-byte handshake).
- `voice_apple` `fcn.08092bd0` — `AppledConnection<AppleVService>::queueAudioPacket(audio_info&, uint8_t*, size_t)` (builds an audio-data packet).
- `voice_apple` `fcn.08093240` — `AppledConnection<AppleVService>::finishAudioPush(bool, ...)` (builds the finish packet).
- `voice_apple` `fcn.08099d90` — `AppledConnection<AppleVService>::handleData(...)` (parses appled's replies; the accept/reject authority).
- `voice_apple` `fcn.08099470` — `c4::voice_control::transfer_one_packet::operator()` (the stream framing / match-condition).
- `voice_apple` `fcn.08073ad0` — `c4::vservice::deserialize_audioinfo(const Json::Value&, audio_info&)` (the JSON schema — **upstream** protocol, see §3).
- `appled` `fcn.08110820` — `c4::voice_server::client::handleHeaderRead(...)` (validates the 12-byte handshake).
- `appled` `fcn.08111df0` — `c4::voice_server::client::handlePacketRead(...)` (parses audio-data / finish packets).
- `appled` `fcn.08114f90` — builds the handshake **rejection** reply.

appled JSON control-plane functions:

- `appled` `fcn.080ad030` — `ABase::Server` request parser: reads `{"method","args"}`, then dispatches (virtual `handleCommand` at `vtable+0x1c`, then a `std::map<string,handler>` walk).
- `appled` `fcn.080b9d30` — error-response builder: `{"error":{"code":N,"type":…,"message":…}}` (matches the observed reply).
- `appled` `0x080dc500` — command registration helper (`addCommand(name, handler)`).
- `appled` command handlers: `ADaemon::cmdProvision 0x080c7010`, `cmdBridge 0x080d1e30`, `cmdStatus 0x080c6e50`, `cmdRemote 0x080ccbd0`; `ARemote::cmdActivate 0x080cf1c0`, `cmdButton 0x080cfe70`; `ABridge::cmdStop 0x080ced10`, `cmdChangeProp 0x080c90f0`, `cmdFactoryReset 0x080c9a10`.
- `appled` `fcn.080cf840` — `ABridge::startRemoteDevice`: starts the c4vc **voice server** (dynamic port) once a remote is activated.

---

## 0. Transport model and the `:9200` JSON control plane

appled exposes **two** TCP services:

1. **tcp 9200 — JSON control plane** (`ABase::Server`, an `AJsonMessenger`).
   Newline-delimited JSON requests `{"method":"<name>","args":{…}}\n`; replies
   are JSON (`{"error":{"code":N,"type":"…","message":"…"}}` on failure). This
   is what Director/voiced drive. It never carries audio.
2. **The c4vc binary voice server — a separate, on-demand TCP port** (the
   `c4::voice_server` with `handleHeaderRead`/`handlePacketRead`). It is **not**
   bound at startup (so `netstat` shows only 9200 when idle); it is created by
   `ABridge::startRemoteDevice` (`fcn.080cf840`) when a remote device is
   started, and logs `"Voice server is accepting cv4c connections on port <N>"`
   (`0x080cfd81`). The port is read back from the bound acceptor
   (`movzx esi, word [[esi+0x20]+0xc]`, `0x080cfd7d`) — i.e. it is assigned at
   bind time (the `voice_target_config` examples use `:9201`), **not** a
   compile-time constant, and is reported to voiced out of band. §1b–§6 describe
   what flows on **this** port.

**There is no in-band JSON→binary upgrade of the 9200 socket.** The two are
different sockets on different ports. `voice_apple` confirms this: on TCP
connect it writes the 12-byte `C4VC` binary handshake **immediately**, with no
JSON prefix — `AppledConnection::handleStateChange` (`fcn.08093c10`), on state
`"Connected"`, calls the handshake builder `fcn.08092650` directly (call at
`0x08093d85`). `voice_apple` connects to `appled_ip:appled_port` using
`appled_stream_id`, all supplied by voiced in the `start` command's
`voice_target_config` (voice_apple strings `"appled_port:"`,
`"appled_stream_id:"`, `"AppleVService::handleStart - appled_ip_v4:"`).

### JSON method vocabulary (`:9200`)

Registered names (via `addCommand`, `fcn` around `0x080cca30` calling
`0x080dc500`): **`bridge`, `provision`, `remote`, `status`, `activate`,
`button`** (plus `ABridge`: `stop`, `changeprop`, `factoryreset`, `ignore`).
`args` keys recovered from each handler's `operator[]`/`get` string literals:

| method | handler | `args` keys (type) | purpose / notes |
|---|---|---|---|
| `provision` | `ADaemon::cmdProvision` | (HAP setup token args) | set up the HomeKit pairing/token |
| `bridge` | `ADaemon::cmdBridge` | bridge/device config | start the HAP bridge + devices |
| `remote` | `ADaemon::cmdRemote` | `id` (uint), `data` (obj) | add/start a remote device → `startRemoteDevice` → **opens the voice-server port**; errors "Invalid id value", "Remote already exists", "Failed to start remote" |
| `activate` | `ARemote::cmdActivate` | `identifier` (uint), `timeout` (uint) | **activate the remote to control a target** (HomeKit TargetControl); logs "Activating remote X to control #N"; errors "Invalid identifier value", "Failed to activate remote", "Bridge not running or remote not found" |
| `button` | `ARemote::cmdButton` | `button` (uint), `state` (`"up"`/`"down"`/`"press"`), `timestamp` (uint), `duration` (uint) | inject a remote button event; the Siri button (`state:"down"`) opens the Siri voice channel on the Apple TV; errors "No active target", "Target does not have button" |
| `status` | `ADaemon::cmdStatus` | — | returns `data` with `provisioned`, `bridged` (booleans) and remote state |
| `stop` / `changeprop` / `factoryreset` | `ABridge::*` | — | bridge lifecycle / property change / reset |

Error codes (the `code`/`type` in the error reply): `0 Success`, `1 General
error`, `2 Invalid request`, `3 Unknown method`?/`Invalid request`,
`4 Aborted`, `5 Invalid argument`, `6 Access denied`, … (enum names at rodata
`0x081974e0`–; the observed `code:3` carried type "Invalid request" / message
"Failed to parse request").

### Precondition for audio: the remote must be activated (the "Siri channel")

`ABridge::startRemoteDevice` refuses to start the voice server unless the remote
is **activated** — it logs `"Remote <id>: not activated"` and bails otherwise
(`fcn.080cf840 @ 0x080cf932`). Activation is `ARemote::cmdActivate` selecting a
target ("Activating remote X to control #N" — HomeKit TargetControl). So the
Siri-channel precondition ("No siri channel is active on remote") is opened by
`activate` (and a Siri `button` `down` event), after which appled will accept
c4vc audio for that remote and forward it over the HomeKit DataSend/Siri stream.

### Corrected end-to-end setup sequence

```
(all JSON on :9200, newline-terminated)
1. {"method":"provision","args":{…HAP token…}}        → provision HomeKit
2. {"method":"bridge","args":{…devices…}}             → start bridge
3. {"method":"remote","args":{"id":<n>,"data":{…}}}   → add remote; appled now
      opens the c4vc voice-server port (log: "accepting cv4c connections on
      port <P>") — learn <P> (status/response/log)
4. {"method":"activate","args":{"identifier":<n>,"timeout":<ms>}}
                                                       → TargetControl: remote
      controls the chosen Apple TV (opens the Siri channel precondition)
5. {"method":"button","args":{"button":<siri>,"state":"down","timestamp":<t>}}
                                                       → begin Siri voice input
6. open a NEW TCP connection to appled_ip:<P> and speak the c4vc BINARY
   protocol (§1a handshake with the remote's stream id, §4 audio packets,
   §6 finish). This is exactly what voice_apple does.
```

---

## 1. Framing

The connection carries two kinds of message with **different** framing:

### 1a. The handshake (first message, each direction) — fixed 12 bytes, no length prefix

appled reads the handshake with boost::asio `transfer_exactly(12)` and
`handleHeaderRead` begins with `cmp dword [arg_10h], 0xc` (`appled`
`0x08110850`) — i.e. the header is **exactly 12 bytes**; a different length is
rejected ("Got c4vc handshake response with unexpected size", client side
`voice_apple 0x08099e58`: `cmp dword [edi-0xc], 0xc`). There is **no length
field**; the marker sits at offset 0.

```
offset  size  field              value / encoding
0       4     marker             ASCII "C4VC"   (bytes 43 34 56 43)
4       2     protocol version   BE16, must be 0x0001
6       2     reserved/flags     BE16, must be 0x0000
8       4     stream id          BE32, must be non-zero
```

Evidence:
- `voice_apple fcn.08092650` builds the buffer at `[esp+0x3c]`:
  `mov dword [esp+0x3c], 0x43563443` ("C4VC"); `[esp+0x40]=0x00`,
  `[esp+0x41]=0x01`, `[esp+0x42]=0x00`, `[esp+0x43]=0x00`; then the id from the
  connection object `mov eax,[esi+0x18]` is written MSB-first into `[esp+0x44..0x47]`
  (`shr ecx,0x18 … shr ecx,0x10 … shr ecx,8 …`, low byte to `[esp+0x47]`) → BE32.
  The whole thing is then taken as a 12-byte `std::string` (`mov dword [esp+8],0xc`,
  source ptr `[esp+0x3c]`, `std::string(const char*, 12, allocator)` at `0x080926ce`).
- `appled fcn.08110820` validates it: `memcmp(buf, "C4VC", …)` (`0x081108a4`, string
  `0x081a6d27 "C4VC"`); offset[4..5] read as `(buf[4]<<8)|buf[5]` and `cmp cx,1`
  (`0x08110955`) else "unsupported protocol version"; offset[6..7] read as
  `(buf[6]<<8)|buf[7]` and required `== 0` (`0x0811094f`); offset[8..11] assembled
  MSB-first in a 4-iteration `shl`/`or` loop (`0x0811096e`, counter 24→0) into the
  stream id, stored at `conn+0x6c`, and required non-zero (`test ecx,ecx; je …`).

### 1b. Data-stream packets (after the handshake) — 4-byte header + BE16 length

Once the handshake is accepted, every subsequent message uses a 4-byte header
whose last 2 bytes are a big-endian payload length. The stream reader
(`voice_apple fcn.08099470`, `transfer_one_packet`) reads `len = (buf[2]<<8)|buf[3]`
(`0x08099983: shl ecx,8; or ecx,eax`) and waits until `len + 4` bytes are
available (`0x080999de: add eax,4; cmp edi,eax; jb <need more>`). So:

```
offset  size   field
0       1      packet type
1       1      flags / format
2       2      payload length  (BE16)  = (total packet size) - 4
4       len    payload
```

`appled`'s `handlePacketRead` dispatches on `buf[0]` (`0x08111e18`: `movzx eax,
byte[esi]; cmp al,1 … cmp al,2 …`) and otherwise logs "invalid packet type: ".

---

## 2. Message / packet types

| type (`buf[0]`) | name | direction | framing |
|---|---|---|---|
| — | handshake request | client → appled | §1a, 12 bytes |
| — | handshake reply (accept/reject) | appled → client | §1a, 12 bytes |
| `0x01` | audio-data packet | client → appled | §1b (see §4) |
| `0x02` | finish packet | client → appled | §1b (see §6) |

appled → client status/ack packets during a transfer are short (3-byte
payloads; see §6); their exact field layout is only partially determined.

---

## 3. The `audio_info` payload — **JSON, but on the upstream hop, not on `:9200`**

Important correction to earlier notes: **appled does not receive a JSON
`audio_info` over c4vc.** `appled`'s c4vc path (`handleHeaderRead` /
`handlePacketRead`) contains no `Json::Value` decode and no `deserialize_audioinfo`;
it derives the audio format directly from the Opus packets
(`opus_packet_get_nb_channels`, Opus TOC sample rate — see §5). The only
`Json::Value` code in `appled` is unrelated HomeKit device provisioning
(`accsrv`/`bridge`).

The JSON `audio_info` belongs to the **upstream** protocol
(`voiced` → `voice_apple`, over the `VServiceDataStream` pipe, packets
`data_start` / `data` / `finish`). `voice_apple` decodes it there with
`c4::vservice::deserialize_audioinfo(const Json::Value&, audio_info&)`
(`fcn.08073ad0`) and then re-encodes the needed fields (`ms`, `db`) into each
`:9200` audio packet while forwarding the Opus bytes verbatim.

The JSON object is read with `Json::Value::operator[](const char*)` for each key
(no `isMember` guard, so missing keys take Json defaults). Keys and types,
exactly as dumped from the `lea [ebx-disp]` operands in `fcn.08073ad0`:

| JSON key | Json accessor | audio_info offset | C type | meaning |
|---|---|---|---|---|
| `"enc"`  | `asString` → enum map | `+0x00` (uint) | enum | encoding; mapped by `fcn.08073830` |
| `"db"`   | `asDouble` | `+0x04` (double, 8 bytes) | double | RMS level in dB |
| `"ms"`   | `asUInt`   | `+0x0c` (uint) | uint32 | packet time / duration in ms |
| `"rate"` | `asUInt`   | `+0x10` (uint) | uint32 | sample rate (Hz) |
| `"fmt"`  | `asString` → compare `"s16le"` | `+0x14` (uint) | enum | PCM sample format (0 = `s16le`) |
| `"ch"`   | `asUInt`   | `+0x18` (byte) | uint8  | channel count |

`"enc"` string → enum (`fcn.08073830`, case-insensitive compares against the
literals at `0x082b1d79 "pcm"`, `0x082b1d7d "opus"`, `0x082b1d82 "passthrough"`):

| string | enum value |
|---|---|
| (unmatched) | `0` = undetermined |
| `"passthrough"` | `1` |
| `"opus"` | `2` |
| `"pcm"` | `3` |

`"fmt"` is compared case-insensitively against `"s16le"` (`0x082b1d8e`); a match
sets sample-format `0` (`fcn.08073ad0 @ 0x08073c8d: xor eax,eax; mov [edi+0x14],eax`).

So a correct upstream `audio_info` for our use is, e.g.:
`{"enc":"opus","rate":16000,"ch":1,"ms":20,"db":<level>,"fmt":"s16le"}`.
A client that talks **only** to `appled:9200` does not send this JSON at all.

---

## 4. The audio-data packet (type `0x01`)

Built by `voice_apple fcn.08092bd0` (`queueAudioPacket`); parsed by
`appled fcn.08111df0` (`handlePacketRead`, `buf[0]==1` branch at `0x08111ff0`).

### Opus (the AppleVService path — `voice_apple` only ever sends Opus)

```
offset  size   field              source / encoding
0       1      type               = 0x01
1       1      format flag        = 0x00   (0 = opus)
2       2      payload length     BE16 = 12 + opus_len
4       4      ms (packet time)   BE32  = audio_info.ms
8       8      rms (dB)           IEEE-754 double, big-endian = audio_info.db
16      N      opus frame         N = opus_len bytes, verbatim
```

Total packet size = `16 + opus_len`.

Evidence (builder, `fcn.08092bd0`):
- First 4 bytes from local `[var_28h]`: dword zeroed then `byte[var_28h]=1`
  (`0x08092bfc`) → type = 1. `byte[var_29h]` (flag) stays 0 on the opus branch
  (it is set to 1 only on the pcm branch, `0x08092c29`). Bytes 2..3 =
  `BE16(opus_len + 12)`: `movzx eax, word[arg_6ch]; add eax,0xc`
  (`0x08092c87`), high byte → `[var_2ah]`, low byte → `[var_2bh]`; this 4-byte
  prefix is appended with length 4 (`mov dword [var_8h],4; std::string::append`).
- Opus branch `0x08092d30`: `eax=[audio_info+0xc]` (`ms`) written MSB-first to
  `[var_34h..0x37]` (BE32); the `db` double (`[audio_info+4]`,`[audio_info+8]`)
  is byte-reversed into `[var_38h..0x3f]` (big-endian double). These 12 bytes
  are appended with length 0xc (`0x08092cbe`). Then the raw Opus payload
  (`arg_68h`, `arg_6ch`) is appended (`0x08092cf5`).

Evidence (parser, `appled fcn.08111df0 @ 0x08111ff0`):
- `size-4` must be `> 11` (`sub edi,4; cmp edi,0xb; jbe <invalid payload>`),
  i.e. total ≥ 16.
- `byte[1]` must be 0 for the opus path (`movzx eax,byte[esi+1]; test al,al;
  jne <pcm/invalid-flags>`, `0x08112007`).
- Bytes[4..7] reassembled MSB-first (`0x08112071` loop) → passed on;
  bytes[8..15] copied/byte-reversed into a local double (`0x081120b0` loop);
  Opus pointer = `packet+16`, Opus length = `size-16` (`sub eax,0x10`,
  `0x081120cb`); handed to `0x0810c1f0` (the per-packet audio handler, which
  also recomputes RMS via `calcOpusPacketRmsDb`).

### PCM (type `0x01`, `byte[1]==1`) — documented for completeness; unused by voice_apple

`queueAudioPacket`'s pcm branch (`0x08092c26`, taken when `enc==3`) sets
`byte[1]=1` and the 12-byte sub-header to `BE32(ms)`, `BE32(rate)`, two zero
bytes, `ch` (byte), zero byte — i.e. `[ms:4][rate:4][00 00][ch][00]` — followed
by the raw PCM payload. `voice_apple` refuses non-Opus for the Apple path
("Unexpectedly received encoding other than opus", `fcn.08092f20`), so this
form is not emitted toward `appled` by `voice_apple`.

---

## 5. appled's accept / reject reply, and audio acceptance

### Handshake reply (12 bytes, §1a layout)

The client (`voice_apple fcn.08099d90`, state "awaiting handshake response")
reads a 12-byte reply and decides accept/reject **solely by whether the reply's
BE32 at offset 8 equals the stream id it sent**:

- length must be 12 (`cmp dword [edi-0xc], 0xc`, `0x08099e58`);
- `memcmp(reply, "C4VC")` else "unexpected signature" (`0x0809a130`, string `0x082b2290`);
- offset[6..7] BE16 checked; offset[4..5] BE16 `cmp ax,1` else "unsupported
  protocol version" (`0x0809a173`);
- offset[8..11] reassembled BE32 (`0x0809a3e0` loop) and `cmp dword [conn+0x18],
  edi` (`0x0809a3ff`) — **equal ⇒ accepted** (logs "c4vc handshake was accepted
  for c4vc stream <id>"), unequal ⇒ **rejected** (logs "c4vc handshake was
  rejected", sets state 4).

Therefore:
- **Acceptance on the wire** = the server echoes the client's 12-byte header
  unchanged (same marker, version 1, reserved 0, **same stream id**).
- **Rejection on the wire** = a 12-byte `C4VC`/version-1 header whose id field
  does **not** match (appled's reject builder `fcn.08114f90` emits
  `"C4VC" 00 01 00 00 00 00 00 <bool>` — id bytes zeroed, a status bool in
  byte 11 — which the client sees as a non-matching id).

The client **does** wait for and read this reply before it starts streaming
audio (the state machine advances from the handshake state to the transfer
state only on an accept).

### Per-packet audio acceptance (derived from Opus, not from the header)

In `handlePacketRead`, after decoding an audio packet appled checks the format
**from the Opus bytes**:
- sample rate ∈ { `0x1f40`=8000, `0x2ee0`=12000, `0x3e80`=16000, `0x5dc0`=24000,
  `0xbb80`=48000 } else "unsupported sample rate" (`0x0811273a`…`0x08112775`);
- channels must be `1` (mono) else "unsupported channels"
  (`0x08112820: cmp al,1`; cf. "channels! expected only mono!");
- encoding must be Opus ("incoming audio data format not constant: expecting
  opus"). 16 kHz mono Opus satisfies all checks.

---

## 6. The finish packet (type `0x02`)

Built by `voice_apple fcn.08093240` (`finishAudioPush(bool success, …)`):

```
offset  size  field       value
0       1     type        = 0x02
1       1     success     1 = success, 0 = abort
2       2     payload len BE16 = 0x0000   (no payload)
```

Total 4 bytes. Evidence: `[var_28h]` dword zeroed then `byte[var_28h]=2`
(`0x0809341c`), `byte[var_27h]=cl` where `cl` is the `success` argument
(`0x08093420`), appended with length 4 (`mov dword [var_4h],4`, `0x08093406`);
the "sending finish packet" log then prints `"success"`/`"failure"` from that
bool (`0x08093495: cmp byte [var_40h],0`).

Parsed by `appled fcn.08111df0`, `buf[0]==2` branch (`0x08111f60`):
`cmp byte[esi+1], 1` distinguishes success vs aborted; logs "incoming transfer
finish packet: " with `"success"`/`"aborted"` (strings `0x081a73c0`,
`0x0819a5c8`, `0x081a6d2c`). appled then forwards/acknowledges and the client's
`handleData` consumes a short status reply ("transfer was accepted
successfully" / "transfer failed with unsupported flag" / "transfer was aborted
by peer").

---

## 7. Full client sequence on the c4vc binary voice-server port

(This is the binary half only. It runs on the on-demand voice-server port `<P>`
from §0, **not** on 9200; reach `<P>` by first driving the `:9200` JSON control
plane as in §0.)

1. TCP connect to `appled` on the voice-server port `<P>`.
2. Send the 12-byte handshake: `43 34 56 43` `00 01` `00 00` `<id:BE32, non-zero>`
   (the id is the remote's `appled_stream_id`).
3. Read a 12-byte reply; **accepted iff** it starts with `C4VC`, version BE16 =
   1, reserved BE16 = 0, and its BE32 id == the id you sent.
4. For each 16 kHz-mono Opus frame, send a type-`0x01` audio packet:
   `01 00 <BE16 = 12+len> <BE32 ms> <BE64 double db> <opus bytes>`.
   (`ms`/`db` are advisory — appled recomputes RMS and reads rate/channels from
   the Opus TOC; mono only; rate ∈ {8000,12000,16000,24000,48000}.)
5. End with the finish packet: `02 01 00 00` (success) or `02 00 00 00` (abort).

---

## 8. `:9200` JSON method reference (`AJsonMessenger`)

Dispatcher: `fcn.080ad030` (parser) reads top-level keys **`method`** (string)
and **`args`** (object) only — **there is no top-level request `id`/`tag`**, so
replies are **not** correlated to requests; they are written back in order on
the same socket, each terminated by `std::endl` (`\n`) (writer uses
`operator<<`/`endl`). Commands are registered via `addCommand` (`0x080dc500`),
called for `bridge`, `provision`, `remote`, `status`, `activate`, `button`
(around `0x080cca30`); `ABridge` also registers `stop`, `changeprop`,
`factoryreset`, `ignore`.

Request envelope: `{"method":"<name>","args":{…}}\n`

Error reply (builder `fcn.080b9d30`): `{"error":{"code":<N>,"type":"<type>","message":"<msg>"}}`.
Error-code enum names (rodata `0x081974e0`–`0x08197506`): `0 Success`,
`1 General error`, `2 Invalid request`, `3 Unknown method`, `4 Aborted`,
`5 Invalid argument`, `6 Access denied` (the probe's `code:3` reply carried
`type:"Invalid request"`, `message:"Failed to parse request"` — the code is the
enum, the type/message are set by the failing handler). On success a handler
either returns no body or a result object (e.g. `status`'s `data`); the exact
success envelope key wrapper was not isolated (flag below).

### 8.1 `provision` — `ADaemon::cmdProvision` (`0x080c7010`)
Establishes the HomeKit pairing identity/token (HAP setup). Refused while the
bridge runs ("Cannot provision while bridge is running").
`args`:
| key | type | required | notes / error |
|---|---|---|---|
| `uuid` | string | yes | "Invalid uuid type", "Invalid uuid value" |
| `token` | string/data | yes | "Invalid token type", "Invalid token data", "Invalid token data format" |
| `force` | bool | no | "Invalid force type" |

Example: `{"method":"provision","args":{"uuid":"<accessory-uuid>","token":"<base64 token>","force":false}}`
Response: success/error envelope (no extra fields observed). "Failed to provision token".

### 8.2 `bridge` — `ADaemon::cmdBridge` (`0x080d1e30`) → `doStartBridge` (`0x080cd280`)
Starts the HAP bridge and its accessories from the provisioned data.
`args`:
| key | type | required | notes |
|---|---|---|---|
| `force` | bool | no | "Invalid force type"; if already up: "Instance already running" / "…: Stopping running bridge" then restart |

Example: `{"method":"bridge","args":{"force":false}}`  Response: success/error.

### 8.3 `remote` — `ADaemon::cmdRemote` (`0x080ccbd0`)
Adds/starts a remote accessory; on success calls `ABridge::startRemoteDevice`
which opens the c4vc voice-server port.
`args`:
| key | type | required | notes |
|---|---|---|---|
| `id` | uint | yes | the remote's id; read via `get`+`isUInt`; "Invalid id value"; "Remote already exists" |
| `data` | object | yes | applied via `fcn.080c7d80`; sub-keys below |

`data` sub-keys (all bool; `fcn.080c7d80`): **`active`**, **`available`**,
**`bridged`**, **`voice`**. (`voice:true` is the remote's voice-capability flag;
cf. the audio path's "Removing voice flag" when no Siri channel is active.)

Example: `{"method":"remote","args":{"id":1,"data":{"available":true,"bridged":true,"active":true,"voice":true}}}`
Response: success/error — **no port field** (see §8.7). "Failed to start remote".

### 8.4 `activate` — `ARemote::cmdActivate` (`0x080cf1c0`)
HomeKit **TargetControl**: makes the remote control a target (an Apple TV).
This opens the "Siri channel" precondition for audio.
`args`:
| key | type | required | notes |
|---|---|---|---|
| `identifier` | uint | yes | target id to control; "Invalid identifier value" |
| `timeout` | uint (ms) | yes/opt | "Invalid timeout value"; `timeout==0` ⇒ deactivate ("Deactivating remote ") |

Completion `handleActivateComplete` (`0x080c5850`): "Remote <id> successfully
activated #<N>", "failed to activate #", "Remote did not respond to request in
time", "Request was cancelled". Also "Bridge not running or remote not found".
Example: `{"method":"activate","args":{"identifier":1,"timeout":10000}}`  Response: success/error.

### 8.5 `button` — `ARemote::cmdButton` (`0x080cfe70`)
Injects a TargetControl button event to the controlled Apple TV.
`args`:
| key | type | required | notes |
|---|---|---|---|
| `button` | uint | yes | button id; "Invalid button value", "Invalid button value type" |
| `state` | string | yes | `"up"` / `"down"` / `"press"`; "Invalid state value", "Invalid state value type" |
| `timestamp` | uint | yes | "Invalid timestamp value" |
| `duration` | uint | for `"press"` | "Invalid duration value" / "…value type" |

Errors: "No active target" (must `activate` first), "Target does not have button".
The **Siri/voice** button is the button whose HAP type is the Siri button
(appled handles `siri-input-type` / `remote_siri:input-type`); pressing it
(`state:"down"`) opens the Siri voice channel, after which the c4vc voice server
accepts audio. The exact numeric `button` value is **accessory-specific** (set
when the remote was provisioned, validated against the target's button config:
"buttonConfig->buttonID > 0", "Invalid button type: %u") — **not a fixed
constant in this binary** (flag below).
Example (Siri down): `{"method":"button","args":{"button":<siriButtonId>,"state":"down","timestamp":<ms>}}`
Response: success/error.

### 8.6 `status` — `ADaemon::cmdStatus` (`0x080c6e50`)
`args`: none. Builds a `data` object: **`{"provisioned":<bool>,"bridged":<bool>}`**
(`operator[]("data")`, then `["provisioned"]`, `["bridged"]`). Example request:
`{"method":"status","args":{}}`. Expected reply carries that `data`.
*Why a bare `status` can look empty:* there is no request-id and the reply is a
single `\n`-terminated JSON line written back on the same socket — a raw client
must read that line. cmdStatus unconditionally builds `data`, so an empty result
most likely means the reply line was not read/framed by the probe (hypothesis;
not a server-side gate found in code).

### 8.7 How the voice-server port comes back — IT DOES NOT, over `:9200`
There is **no JSON key `port`** (or voiceport/etc.) anywhere in `appled`. The
dynamically-bound voice-server port appears **only** in the log line
`"Voice server is accepting cv4c connections on port <P>"` (`0x080cfd81`),
read back from the bound acceptor object (`movzx esi, word [[vs+0x20]+0xc]`,
`0x080cfd7d`) — i.e. it looks **dynamically/ephemerally bound**, not a
compile-time constant. `voice_apple` learns it as `appled_port` from voiced's
`start` command (`voice_target_config = "<stream_id>@<ip>:<port>"`, parsed in
`AppleVService::handleStart`; strings "appled_port:", "appled_stream_id:"). How
voiced/Director learns `<P>` from appled is **not visible in these two
binaries** (hypothesis: a direct voiced↔appled channel or Director orchestration
reading the log/an internal API; the examples use `:9201`). For our own
injection, read `<P>` from appled's log/stdout (or `lsof`/`netstat` once a
remote is started), since it is not returned over `:9200`.

### 8.8 HomeKit pairing precondition
`remote`/`activate`/`button` act through the bridge's HAP accessories against the
**already-paired** Apple TV: `activate` is a TargetControl selection and `button`
raises events over the HomeKit DataStream; appled owns the pairing (standard
HAP pair-setup/pair-verify, setup code). So this works on a controller whose
`appled` is **already HomeKit-paired to that room's Apple TV** (e.g. a garage
EA-1 paired to that Apple TV). `provision` sets up appled's own accessory
identity/token; it does **not** by itself establish the controller↔AppleTV
pairing — that is the normal HomeKit pairing, which must already exist. (Inference
from the HAP symbols/flow, not a single explicit check.)

---

## 9. What is not fully determined (hypotheses + evidence)

- **`ms`/`db` per-packet semantics.** The builder copies `audio_info.ms` into
  bytes[4..7] and `audio_info.db` into bytes[8..15] of every audio packet
  (`fcn.08092bd0 @ 0x08092d30`). appled passes bytes[4..7] to its audio handler
  `0x0810c1f0` and reads bytes[8..15] as a double, but then recomputes RMS with
  `calcOpusPacketRmsDb`, so the transmitted `db` appears advisory. Best
  hypothesis: byte[4..7] = packet duration in ms (key is literally `"ms"`;
  appled elsewhere defaults to "20ms packet time"), byte[8..15] = RMS dB.
  Not proven to be used as anything load-bearing by appled.

- **The appled → client acknowledgement packets.** The client state machine
  (`fcn.08099d90`) branches on received-message lengths of `3` as well as `12`
  and reads a leading byte compared to `1`/`2`, implying short
  (~3-byte) status/ack frames distinct from the data-stream framing of §1b.
  Their exact byte layout (and packet "type" value) was not fully traced; they
  are server→client only and not required to drive a push. Evidence:
  `0x08099ed8`/`0x08099fa8` (`cmp dword [edi-0xc], 3`), `0x08099ee9`/`0x08099fb7`
  (`cmp dl/al, 1`/`2`).

- **Exact `provision`/`bridge`/`remote.data` arg schemas and the port report.**
  `provision` feeds the HAP setup token; `bridge` carries the device/accessory
  list; `remote.data` carries the remote's config. Their full sub-key schemas
  were not exhaustively dumped (they flow into `c4::accsrv`/`c4::bridge`
  HomeKit provisioning). The voice-server port is assigned at bind time and
  logged ("accepting cv4c connections on port <P>"); it is reported to voiced
  out of band (the `voice_target_config` `id@ip:port`, examples show `:9201`).
  The precise JSON field that returns `<P>` to the caller (likely in the
  `status`/`remote` response `data`) was not pinned down — best hypothesis:
  read it from the `status` reply or appled's log; the `voiced` `start` command
  then passes it to `voice_apple` as `appled_port`.

- **Optional appended payload in the handshake builder.** `fcn.08092650`, after
  the 12-byte header, conditionally appends a `std::string` member of the
  connection object (`[conn+0x24]→[+0x98]`, guarded by its length at `[+0xa0]`
  at `0x080926e1`). For the AppleVService→appled path this is empty: appled
  reads the handshake with `transfer_exactly(12)` and has no JSON/audio_info
  decode on c4vc, so a non-empty append would desynchronize the stream. Best
  hypothesis: this is dead/base-class code for a different service variant; the
  on-wire handshake to `appled` is the fixed 12 bytes.
