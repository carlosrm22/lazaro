#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${FLATPAK_GPG_PRIVATE_KEY:-}" && -z "${FLATPAK_GPG_PRIVATE_KEY_BASE64:-}" ]]; then
  echo "Missing FLATPAK_GPG_PRIVATE_KEY or FLATPAK_GPG_PRIVATE_KEY_BASE64"
  exit 1
fi

GNUPGHOME="$(mktemp -d)"
export GNUPGHOME
chmod 700 "$GNUPGHOME"
trap 'rm -rf "$GNUPGHOME"' EXIT

KEY_FILE="$GNUPGHOME/lazaro-signing.key"
if [[ -n "${FLATPAK_GPG_PRIVATE_KEY_BASE64:-}" ]]; then
  printf '%s' "$FLATPAK_GPG_PRIVATE_KEY_BASE64" | base64 --decode > "$KEY_FILE"
else
  printf '%s' "$FLATPAK_GPG_PRIVATE_KEY" > "$KEY_FILE"
fi

gpg --batch --homedir "$GNUPGHOME" --import "$KEY_FILE"

KEY_ID="$(gpg --batch --homedir "$GNUPGHOME" --list-secret-keys --with-colons | awk -F: '/^sec:/ {print $5; exit}')"
if [[ -z "$KEY_ID" ]]; then
  echo "No secret key imported for Flatpak signing"
  exit 1
fi

mkdir -p dist
flatpak-builder --force-clean --repo=dist/repo dist/build packaging/flatpak/io.lazaro.Lazaro.yml
# Sign exported app commit(s) so clients with gpg-verify=true can install/update.
flatpak build-sign dist/repo io.lazaro.Lazaro stable --gpg-homedir="$GNUPGHOME" --gpg-sign="$KEY_ID"
flatpak build-update-repo dist/repo --generate-static-deltas --prune --gpg-homedir="$GNUPGHOME" --gpg-sign="$KEY_ID"
gpg --batch --homedir "$GNUPGHOME" --armor --export "$KEY_ID" > dist/repo/lazaro-flatpak-public.asc

BUNDLE_NAME="io.lazaro.Lazaro-${GITHUB_REF_NAME:-dev}-${GITHUB_SHA::7}.flatpak"
flatpak build-bundle dist/repo "dist/${BUNDLE_NAME}" io.lazaro.Lazaro stable --gpg-homedir="$GNUPGHOME" --gpg-sign="$KEY_ID"

if [[ -n "${GITHUB_ENV:-}" ]]; then
  echo "BUNDLE_NAME=${BUNDLE_NAME}" >> "$GITHUB_ENV"
fi

echo "Generated signed bundle: dist/${BUNDLE_NAME}"
