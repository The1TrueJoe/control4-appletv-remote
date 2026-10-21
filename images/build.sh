#!/usr/bin/env bash
# Bake the Siri Remote → Control4 bridge Raspberry Pi image with pi-gen.
#
# One arm64 Raspberry Pi OS Lite image that boots on Pi 3 / 4 / 5.
#
#   images/build.sh --bin-dir <dir-with-siri-c4d[,siri-events]-aarch64-binaries>
#
# Heavy prerequisites (pi-gen): a Debian/Ubuntu host (or its Docker flow),
# root/privileged, qemu-user-static, ~10 GB free, and the usual pi-gen deps.
# In CI we use pi-gen's Docker build (./build-docker.sh). See images/README.md.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR=""
PIGEN_REF="${PIGEN_REF:-arm64}"            # pi-gen branch/tag
IMG_NAME="${IMG_NAME:-siri-c4d}"
WORK="${WORK:-$HERE/.pi-gen}"

while [ $# -gt 0 ]; do
	case "$1" in
		--bin-dir) BIN_DIR="$2"; shift ;;
		--img-name) IMG_NAME="$2"; shift ;;
		-h|--help) sed -n '2,12p' "$0" | sed 's/^# \?//'; exit 0 ;;
		*) echo "unknown option: $1" >&2; exit 1 ;;
	esac
	shift
done

[ -n "$BIN_DIR" ] || { echo "pass --bin-dir with the aarch64 binaries" >&2; exit 1; }

# Stage the binaries where the pi-gen stage installs them from.
dest="$HERE/stage-siri/00-install-siri/files/bin"
mkdir -p "$dest"
for b in siri-c4d siri-events; do
	for cand in "$BIN_DIR/$b-aarch64" "$BIN_DIR/$b"; do
		if [ -f "$cand" ]; then install -m755 "$cand" "$dest/$b"; break; fi
	done
done
[ -f "$dest/siri-c4d" ] || { echo "siri-c4d binary not found in $BIN_DIR" >&2; exit 1; }

# Fetch pi-gen and drop in our config + custom stage.
if [ ! -d "$WORK" ]; then
	git clone --depth 1 --branch "$PIGEN_REF" https://github.com/RPi-Distro/pi-gen "$WORK"
fi
cp "$HERE/config" "$WORK/config"
rm -rf "$WORK/stage-siri"
cp -r "$HERE/stage-siri" "$WORK/stage-siri"

export IMG_NAME
cd "$WORK"
# Docker flow is the most reproducible; falls back to native ./build.sh.
if command -v docker >/dev/null 2>&1; then
	PRESERVE_CONTAINER=0 CONTINUE=1 ./build-docker.sh
else
	sudo ./build.sh
fi

echo "Image(s) in $WORK/deploy/"
