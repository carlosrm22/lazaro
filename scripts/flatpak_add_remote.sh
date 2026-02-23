#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Uso: $0 <repo-url>"
  echo "Ejemplo: $0 https://carlosrm22.github.io/lazaro/"
  exit 1
fi

REPO_URL="$1"

flatpak remote-add --if-not-exists lazaro "$REPO_URL"
flatpak install -y lazaro io.lazaro.Lazaro

echo "Remote agregado e instalación completada."
