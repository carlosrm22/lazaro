#!/usr/bin/env bash
set -euo pipefail

AUTOSTART_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/io.lazaro.Lazaro.desktop"
if [[ -f "$AUTOSTART_FILE" ]]; then
  rm -f "$AUTOSTART_FILE"
  echo "Removed: $AUTOSTART_FILE"
else
  echo "Not found: $AUTOSTART_FILE"
fi
