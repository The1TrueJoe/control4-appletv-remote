#!/usr/bin/env bash
# Install the Siri Remote tools.
#
#   siri-events   stream remote events as NDJSON (glue it to anything)
#   siri-c4d      ready-made Control4 bridge with a web UI
#
# Idempotent — safe to re-run to upgrade.
#
#   sudo ./install.sh --binary-dir ./dist       # prebuilt
#   sudo ./install.sh --build                   # from source
#   sudo ./install.sh --build --no-daemon       # just siri-events
set -euo pipefail

PREFIX=${PREFIX:-/usr/local}
CONFIG_DIR=/etc/siri-c4d
STATE_DIR=/var/lib/siri-c4d
UNIT=/etc/systemd/system/siri-c4d.service
BINDIR=""
BUILD=0
DAEMON=1
PORT=8752

say()  { printf '\033[1m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[33m==> %s\033[0m\n' "$*"; }
die()  { printf '\033[31m==> %s\033[0m\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --binary-dir) BINDIR="$2"; shift ;;
    --build)      BUILD=1 ;;
    --no-daemon)  DAEMON=0 ;;
    --port)       PORT="$2"; shift ;;
    -h|--help)    sed -n '2,14p' "$0" | sed 's/^# \?//'; exit 0 ;;
    *) die "unknown option: $1" ;;
  esac
  shift
done

[ "$(id -u)" -eq 0 ] || die "run as root (sudo $0)"

say "Installing runtime dependencies"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
  bluez libopus0 libspeex1 libdbus-1-3 ca-certificates >/dev/null

here=$(cd "$(dirname "$0")/.." && pwd)

if [ -n "$BINDIR" ]; then
  say "Installing prebuilt binaries from $BINDIR"
  for b in siri-events siri-c4d; do
    src="$BINDIR/$b-aarch64"
    [ -f "$src" ] || src="$BINDIR/$b"
    [ -f "$src" ] || die "missing $b in $BINDIR"
    install -Dm755 "$src" "$PREFIX/bin/$b"
  done
elif [ "$BUILD" = 1 ]; then
  say "Building from source"
  warn "On a Pi 3 this is slow and wants ~1GB of swap; cross-building is faster."
  apt-get install -y -qq --no-install-recommends \
    build-essential pkg-config libdbus-1-dev libopus-dev libspeex-dev curl >/dev/null
  if ! command -v cargo >/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
  fi
  ( cd "$here" && cargo build --release )
  install -Dm755 "$here/target/release/siri-c4d"    "$PREFIX/bin/siri-c4d"
  # siri-events lives in the siri-remote repo; build the same tag siri-c4d pins.
  cargo install --locked --root "$PREFIX" \
    --git https://github.com/The1TrueJoe/siri-remote --tag v0.2.0 siri-events
elif [ -d "$here/dist" ]; then
  say "Installing bundled binaries"
  for b in siri-events siri-c4d; do
    src="$here/dist/$b-aarch64"
    [ -f "$src" ] || die "missing $b in $here/dist"
    install -Dm755 "$src" "$PREFIX/bin/$b"
  done
else
  die "nothing to install: pass --binary-dir <dir> or --build"
fi

say "Installed: $(command -v siri-events), $(command -v siri-c4d)"

if [ "$DAEMON" = 0 ]; then
  cat <<DONE

  siri-events is ready. Next:

    siri-events scan
    siri-events pair <address>
    siri-events listen <address> | your-glue-script

DONE
  exit 0
fi

install -d -m 0755 "$CONFIG_DIR" "$STATE_DIR" "$STATE_DIR/audio"

if [ ! -f "$CONFIG_DIR/config.toml" ]; then
  say "Writing initial configuration"
  cat > "$CONFIG_DIR/config.toml" <<CFG
# siri-c4d — configure the rest from the web UI.

# Remotes are added by pairing from the web UI, which appends [[remotes]]
# tables here. This must stay above every table: a bare key after a table
# header would be parsed as belonging to that table.
remotes = []

[web]
bind = "0.0.0.0"
port = $PORT

[control4]
# Address of the Room Control Webhook root driver.
host = ""
port = 5080
password = ""
room = ""
timeout_ms = 2500

[voice]
# Holding Siri writes a 16 kHz mono WAV here. Transcription is deliberately
# out of scope: point your own recogniser at these files, or stream live PCM
# with: siri-events listen <addr> --pcm-fifo /tmp/siri.pcm
enabled = true
audio_dir = "$STATE_DIR/audio"
keep_recordings = true
CFG
  chmod 0640 "$CONFIG_DIR/config.toml"
fi

say "Installing systemd unit"
cat > "$UNIT" <<UNITEOF
[Unit]
Description=Siri Remote to Control4 bridge
Documentation=https://github.com/The1TrueJoe/control4-appletv-remote
After=bluetooth.service network-online.target
Wants=network-online.target
Requires=bluetooth.service

[Service]
Type=simple
ExecStart=$PREFIX/bin/siri-c4d --config $CONFIG_DIR/config.toml
Restart=always
RestartSec=5
# Raw L2CAP sockets and BlueZ pairing both need these.
AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN
CapabilityBoundingSet=CAP_NET_RAW CAP_NET_ADMIN
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ReadWritePaths=$CONFIG_DIR $STATE_DIR
StateDirectory=siri-c4d

[Install]
WantedBy=multi-user.target
UNITEOF

systemctl daemon-reload
systemctl enable --now siri-c4d.service >/dev/null 2>&1 || systemctl restart siri-c4d.service
sleep 2

if systemctl is-active --quiet siri-c4d.service; then
  ip=$(hostname -I 2>/dev/null | awk '{print $1}')
  say "Running. Open the web UI:"
  printf '\n    \033[1mhttp://%s:%s\033[0m\n\n' "${ip:-<pi-address>}" "$PORT"
  echo "  Next: set the Control4 host, then scan and pair a remote."
  echo "  Logs: journalctl -u siri-c4d -f"
else
  warn "Service failed to start. Recent logs:"
  journalctl -u siri-c4d -n 25 --no-pager || true
  exit 1
fi
