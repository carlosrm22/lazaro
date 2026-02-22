#!/usr/bin/env bash
set -euo pipefail

AUTOSTART_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"
mkdir -p "$AUTOSTART_DIR"

cat > "$AUTOSTART_DIR/io.lazaro.Lazaro.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Version=1.0
Name=Lazaro
Comment=Personalized break reminder
Exec=lazaro
Terminal=false
Categories=Utility;
X-GNOME-Autostart-enabled=true
DESKTOP

echo "Installed: $AUTOSTART_DIR/io.lazaro.Lazaro.desktop"
