# Lázaro

Lázaro is a Linux-first break reminder app designed to be strict when you want it and flexible when you do not.

## Current status

This repository contains:
- `lazaro-core`: timer engine, profile model, and weekly analytics.
- `apps/desktop/src-tauri`: desktop shell scaffold for Tauri commands and startup management.
- setup scripts for XDG autostart and systemd --user.
- initial Flatpak manifest and CI workflow.
- Flatpak release pipeline with GitHub Releases + GitHub Pages repo for updates.

## Features planned in V1

- Micro break, rest break, and daily limit timers.
- Modes: soft, medium, strict.
- Notifications: desktop + overlay + sound.
- Profiles and weekly analytics dashboard.
- Autostart support on Linux via XDG and systemd user services.

## Local development

### Requirements

- Rust + Cargo
- Node.js + npm
- Tauri dependencies for your distro

### Core tests

```bash
cargo test -p lazaro-core
```

### Desktop app (after installing Tauri runtime deps)

```bash
cd apps/desktop
npm install
npm run tauri dev
```

## Autostart scripts

Install XDG autostart entry:

```bash
./scripts/install_autostart.sh
```

Install systemd user service:

```bash
./scripts/install_systemd_user.sh
```

## GitHub

Create and push private repo once authenticated:

```bash
gh auth login
gh repo create lazaro --private --source . --remote origin --push
```

## Flatpak packaging

### Build local Flatpak bundle + repo

Requirements:
- `flatpak`, `flatpak-builder`
- `flatpak-cargo-generator` (`pip install --user flatpak-cargo-generator`)
- runtimes Flatpak:
  - `org.gnome.Platform//48`
  - `org.gnome.Sdk//48`
  - `org.freedesktop.Sdk.Extension.rust-stable//24.08`

Install runtimes:

```bash
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y flathub org.gnome.Platform//48 org.gnome.Sdk//48 org.freedesktop.Sdk.Extension.rust-stable//24.08
```

Build:

```bash
./scripts/build_flatpak.sh
```

Install generated bundle:

```bash
flatpak install --user dist/io.lazaro.Lazaro-local-*.flatpak
flatpak run io.lazaro.Lazaro
```

### Release automation

Workflow: `.github/workflows/flatpak-release.yml`

Trigger release build:
- push a tag `v*` (for example `v0.1.1`)
- or run `workflow_dispatch` manually

On tag:
- builds Flatpak bundle (`.flatpak`)
- uploads bundle as workflow artifact
- attaches bundle to GitHub Release
- signs Flatpak repo summary with GPG
- publishes Flatpak repo + public key to GitHub Pages (`https://<owner>.github.io/<repo>/`)

Required GitHub Secrets for signing:
- `FLATPAK_GPG_PRIVATE_KEY` (ASCII armored private key) or
- `FLATPAK_GPG_PRIVATE_KEY_BASE64` (same key in base64)

### Dev preview automation

Workflow: `.github/workflows/flatpak-dev-preview.yml`

Trigger preview build:
- every push to `dev`
- or `workflow_dispatch`

On `dev`:
- builds `io.lazaro.Lazaro-dev-latest.flatpak`
- uploads artifact in GitHub Actions
- updates prerelease `dev-latest` in GitHub Releases

Install latest dev preview:

```bash
curl -L -o /tmp/lazaro-dev.flatpak https://github.com/carlosrm22/lazaro/releases/download/dev-latest/io.lazaro.Lazaro-dev-latest.flatpak
flatpak install --user /tmp/lazaro-dev.flatpak
flatpak run io.lazaro.Lazaro
```

## Updates strategy

For automatic updates on user machines, use the published Flatpak repo:

```bash
curl -L -o /tmp/lazaro-flatpak-public.asc https://carlosrm22.github.io/lazaro/lazaro-flatpak-public.asc
flatpak remote-add --user --if-not-exists --gpg-import=/tmp/lazaro-flatpak-public.asc lazaro https://carlosrm22.github.io/lazaro/
flatpak install --user lazaro io.lazaro.Lazaro
flatpak update --user io.lazaro.Lazaro
```

Helper script:

```bash
./scripts/flatpak_add_remote.sh https://carlosrm22.github.io/lazaro/
```
