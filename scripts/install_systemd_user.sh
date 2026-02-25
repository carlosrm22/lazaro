#!/usr/bin/env bash
set -euo pipefail

UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
UNIT_FILE="$UNIT_DIR/lazaro.service"
mkdir -p "$UNIT_DIR"

cat > "$UNIT_FILE" <<'UNIT'
[Unit]
Description=Lázaro break reminder daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=lazaro
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
UNIT

systemctl --user daemon-reload
systemctl --user enable --now lazaro.service

echo "Installed and enabled: $UNIT_FILE"
