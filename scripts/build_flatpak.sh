#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT_DIR/packaging/flatpak/io.lazaro.Lazaro.yml"
DIST_DIR="$ROOT_DIR/dist"
BUILD_DIR="$DIST_DIR/build"
REPO_DIR="$DIST_DIR/repo"

if ! command -v flatpak-builder >/dev/null 2>&1; then
  echo "flatpak-builder no está instalado."
  echo "Instala: sudo dnf install flatpak-builder  (o equivalente en tu distro)."
  exit 1
fi

if ! command -v flatpak-cargo-generator >/dev/null 2>&1; then
  echo "flatpak-cargo-generator no está instalado."
  echo "Instala: pip install --user flatpak-cargo-generator"
  exit 1
fi

if ! flatpak info --user org.gnome.Sdk//48 >/dev/null 2>&1 || ! flatpak info --user org.gnome.Platform//48 >/dev/null 2>&1; then
  echo "Faltan runtimes Flatpak requeridos."
  echo "Ejecuta:"
  echo "  flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo"
  echo "  flatpak install --user -y flathub org.gnome.Platform//48 org.gnome.Sdk//48 org.freedesktop.Sdk.Extension.rust-stable//24.08"
  exit 1
fi

mkdir -p "$DIST_DIR"

echo "Generando packaging/flatpak/cargo-sources.json ..."
(
  cd "$ROOT_DIR"
  flatpak-cargo-generator Cargo.lock -o packaging/flatpak/cargo-sources.json
)

echo "Compilando Flatpak ..."
flatpak-builder --force-clean --repo="$REPO_DIR" "$BUILD_DIR" "$MANIFEST"
flatpak build-update-repo "$REPO_DIR" --generate-static-deltas --prune

BUNDLE_NAME="io.lazaro.Lazaro-local-$(date +%Y%m%d-%H%M%S).flatpak"
flatpak build-bundle "$REPO_DIR" "$DIST_DIR/$BUNDLE_NAME" io.lazaro.Lazaro stable

echo
echo "Listo:"
echo "  Bundle: $DIST_DIR/$BUNDLE_NAME"
echo "  Repo:   $REPO_DIR"
echo
echo "Instalar bundle:"
echo "  flatpak install --user $DIST_DIR/$BUNDLE_NAME"
