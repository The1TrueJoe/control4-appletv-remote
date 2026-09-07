# Glue examples

`siri-events` reports what the remote did. These decide what it means.

| Example | What it shows |
|---|---|
| `python/minimal.py` | The smallest useful consumer — react to presses and swipes. |
| `python/control4_glue.py` | A complete Control4 bridge in ~100 lines, including hold-to-ramp volume. |
| `python/voice_stream.py` | Consuming live microphone PCM while the Siri button is held. |
| `shell/mqtt_glue.sh` | Glue needs no particular language: `jq` plus a CLI publisher. |

```bash
siri-events listen DC:2B:2A:E0:18:71 | python/minimal.py

siri-events listen DC:2B:2A:E0:18:71 \
  | python/control4_glue.py --host 10.0.0.20 --room "Living Room" --dry-run
```

Pass `--dry-run` to the Control4 example to watch the mapping without sending
anything.

## Writing your own

Read stdin, parse each line as JSON, switch on `event`. The events that matter
are `button` (with `state` of `down`/`up`), `swipe`, `battery`, `voice_start`
and `voice_end`.

Two things worth copying from `control4_glue.py`:

- **Use both edges for hold-style actions.** Volume ramps by sending
  `START_VOL_UP` on press and `STOP_VOL_UP` on release, rather than a stream of
  discrete steps.
- **Ignore what you don't map.** New event kinds get added; a `switch` that
  falls through harmlessly keeps your glue working across upgrades.
