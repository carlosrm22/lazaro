#!/usr/bin/env bash
set -euo pipefail

UNIT_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/lazaro.service"

systemctl --user disable --now lazaro.service 2>/dev/null || true
if [[ -f "$UNIT_FILE" ]]; then
  rm -f "$UNIT_FILE"
fi
systemctl --user daemon-reload

echo "Removed service lazaro.service"
