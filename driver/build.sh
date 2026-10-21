#!/usr/bin/env bash
# Package the Control4 driver into a .c4z (a zip with driver.xml at the root).
set -euo pipefail
cd "$(dirname "$0")"

OUT="apple_tv_remote_bridge.c4z"
rm -f "$OUT"

# Rasterize the icon from SVG if a converter is available (optional).
if [ ! -f icons/device_lg.png ]; then
  if command -v rsvg-convert >/dev/null 2>&1; then
    rsvg-convert -w 32  -h 32  icons/remote.svg -o icons/device_sm.png || true
    rsvg-convert -w 128 -h 128 icons/remote.svg -o icons/device_lg.png || true
  elif command -v sips >/dev/null 2>&1 && [ -f icons/remote.png ]; then
    sips -z 32 32   icons/remote.png --out icons/device_sm.png >/dev/null || true
    sips -z 128 128 icons/remote.png --out icons/device_lg.png >/dev/null || true
  fi
fi

zip -r "$OUT" driver.xml *.lua icons README.md -x "*.DS_Store" >/dev/null
echo "built $OUT"
unzip -l "$OUT"
