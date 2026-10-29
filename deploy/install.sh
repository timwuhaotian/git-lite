#!/bin/sh
set -eu

BIN_SRC_SERVER="target/release/gl-server"
BIN_SRC_CLIENT="target/release/gl"
BIN_DST="/usr/local/bin"
DATA_DIR="${GL_DATA:-/var/lib/git-lite}"
SERVICE_SRC="deploy/gl-server.service"
SERVICE_DST="/etc/systemd/system/gl-server.service"

cargo build --release

install -m 0755 "$BIN_SRC_SERVER" "$BIN_DST/gl-server"
if [ -f "$BIN_SRC_CLIENT" ]; then
  install -m 0755 "$BIN_SRC_CLIENT" "$BIN_DST/gl"
fi

mkdir -p "$DATA_DIR"
if id git-lite >/dev/null 2>&1; then
  chown git-lite:git-lite "$DATA_DIR"
fi

if [ -f "$SERVICE_SRC" ]; then
  install -m 0644 "$SERVICE_SRC" "$SERVICE_DST"
  systemctl daemon-reload
  systemctl enable --now gl-server
fi

echo "git-lite installed. API: http://localhost:${PORT:-7331}/api/health"
