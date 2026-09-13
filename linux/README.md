# Yood for Linux

Linux-only build. Shared code lives in [`../shared/`](../shared/). Root guide: [`../README.md`](../README.md).

## Quick start

```bash
npm install --prefix ../shared/frontend
npm run build --prefix ../shared/frontend
./run.sh brave
```

## Brave engine

```bash
../linux/scripts/fetch-brave.sh linux-x86_64
./run.sh brave
```

Uses the bundled portable Brave first (`src-tauri/binaries/linux-x86_64/brave/brave`), then `/opt/brave-bin/brave`, `/opt/brave.com/brave/brave`, then `brave` / `brave-browser` / `brave-bin` on `PATH`.

## Build

```bash
./scripts/prepare-binaries.sh linux-x86_64
cargo tauri build --config src-tauri/tauri.conf.json
```

Targets: `appimage`, `deb`. Resources include `gstreamer/` plus `../../shared/filters` and `../../shared/extension`.

## Install (per-user, no root)

```bash
./scripts/install.sh src-tauri/target/release/bundle/appimage/yood_*_amd64.AppImage
```

## Self-test

```bash
./run.sh self-test
```

## Layout

- `src-tauri/` — Rust backend + `tauri.conf.json` (Linux-only)
- `scripts/` — `fetch-brave.sh`, `prepare-binaries.sh`, `install.sh`, `lock-devtools.sh`
- `run.sh` — Linux-only helper (no Windows commands)
