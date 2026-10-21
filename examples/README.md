# Glue examples

`siri-events` reports what the remote did. These decide what it means.

| Example | What it shows |
|---|---|
| `python/control4_glue.py` | A complete Control4 bridge in ~100 lines, including hold-to-ramp volume. |

```bash
siri-events listen DC:2B:2A:E0:18:71 \
  | python/control4_glue.py --host 10.0.0.20 --room "Living Room" --dry-run
```

Pass `--dry-run` to watch the mapping without sending anything.

More examples (a minimal consumer, live microphone PCM, MQTT from shell) and
the event reference live in
[siri-remote](https://github.com/The1TrueJoe/siri-remote/tree/main/examples).

Two things worth copying from `control4_glue.py`:

- **Use both edges for hold-style actions.** Volume ramps by sending
  `START_VOL_UP` on press and `STOP_VOL_UP` on release, rather than a stream of
  discrete steps.
- **Ignore what you don't map.** New event kinds get added; a `switch` that
  falls through harmlessly keeps your glue working across upgrades.
