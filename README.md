# Lazaro

Lazaro is a Linux-first break reminder app designed to be strict when you want it and flexible when you do not.

## Current status

This repository contains:
- `lazaro-core`: timer engine, profile model, and weekly analytics.
- `apps/desktop/src-tauri`: desktop shell scaffold for Tauri commands and startup management.
- setup scripts for XDG autostart and systemd --user.
- initial Flatpak manifest and CI workflow.

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
