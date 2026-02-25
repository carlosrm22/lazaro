#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Uso: $0 <repo-url>"
  echo "Ejemplo: $0 https://carlosrm22.github.io/lazaro/"
  exit 1
fi

REPO_URL="$1"
KEY_URL="${REPO_URL%/}/lazaro-flatpak-public.asc"
KEY_FILE="$(mktemp)"
trap 'rm -f "$KEY_FILE"' EXIT

curl -fsSL "$KEY_URL" -o "$KEY_FILE"

flatpak remote-add --user --if-not-exists --gpg-import="$KEY_FILE" lazaro "$REPO_URL"
flatpak install --user -y lazaro io.lazaro.Lazaro

echo "Remote firmado agregado e instalación completada."
