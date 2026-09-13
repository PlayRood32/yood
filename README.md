# Yood

Lightweight private YouTube desktop app.

Yood opens the official YouTube site in Brave app mode, with built-in
downloads, queue, filtering and settings. The main window is YouTube itself.
Download dialogs, the download manager, filtering, settings and the optional
local player are provided by the Rust layer.

Repository: https://github.com/PlayRood32/yood

Current version: `1.0.0`

## Download

For a direct download without cloning, go to Releases:

https://github.com/PlayRood32/yood/releases

Latest release: https://github.com/PlayRood32/yood/releases/tag/v1.0.0

Contents:

- Linux: AppImage / deb built from `linux/src-tauri`
- Windows: NSIS installer (`Yood_*_x64-setup.exe`) built from `windows/src-tauri`

If there are no assets under the tag yet, build from source as described below.

## What it does

YouTube-first:

- opens the official YouTube site in Brave app mode (no browser UI, isolated profile)
- keeps Brave Shields on with aggressive mode for YouTube plus Yood custom filters
- injects download buttons into watch / Shorts pages via the companion extension
- Alt+L quick-open bar and an in-player ad-skip helper

Downloads:

- queue video / audio / songs via `yt-dlp` + `ffmpeg`
- live progress, speed and ETA in the download manager
- remove items and open the download folder from the UI
- `yood://download?url=...` deep links from the extension

Ad blocking:

- Brave Shields plus Yood rules as Brave custom filters
- companion extension network rules (`declarativeNetRequest`) and cosmetic hiding
- Rust filter layer for WebView mode, off in Brave mode

Integration:

- `yood://` deep-link scheme with single-instance handling
- per-user install on Linux (no root), NSIS installer on Windows
- `yood --self-test` smoke test without a WebView

## Project layout

```text
linux/
  scripts/
    fetch-brave.sh
    install.sh
    lock-devtools.sh
    prepare-binaries.sh
  src-tauri/
    capabilities/default.json
    gstreamer/
    icons/
    src/
    build.rs
    Cargo.toml
    tauri.conf.json
  README.md
  run.sh
shared/
  extension/yood/
    content.js
    manifest.json
    rules.json
  filters/default.txt
  frontend/
    bootstrap.ts
    index.html
    injection.ts
    player.html
    player.ts
    tsconfig.json
  README.md
windows/
  scripts/build-windows.ps1
  src-tauri/
    capabilities/default.json
    icons/
    src/
    build.rs
    Cargo.toml
    tauri.conf.json
  README.md
CHANGES.md
README.md
Yood_Technical_Specification.md
```

- `shared/` holds the extension, filters and frontend. Both `linux/src-tauri`
  and `windows/src-tauri` point at it with `../../shared/...`.
- `linux/` is Linux-only: AppImage/deb targets, `run.sh`, shell scripts.
- `windows/` is Windows-only: NSIS target, `build-windows.ps1`, WebView2.
  See [windows/](windows/) and [linux/](linux/).

## Linux

Full guide: [linux/](linux/)

Requirements:

Runtime:

- Brave (bundled portable, or system `brave` / `brave-browser` / `brave-bin`)
- WebKit/GStreamer runtime for the optional WebView mode

Build:

- Rust toolchain (`rust-version = 1.80`)
- Node.js (frontend `tsc` build)
- Tauri Linux dependencies (WebKitGTK and the rest)
- `curl`, `unzip`, `sha256sum` for Brave/binary staging

Install the WebKit/GStreamer runtime before launching Yood:

Arch Linux:

```bash
sudo pacman -S --needed webkit2gtk-4.1 gst-plugins-base gst-plugins-good gst-libav pipewire
```

Debian:

```bash
sudo apt install webkit2gtk-4.1 gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-libav pipewire
```

Fedora:

```bash
sudo dnf install webkit2gtk4.1 gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-libav pipewire
```

Gentoo:

```bash
sudo emerge --ask net-libs/webkit-gtk media-libs/gst-plugins-base media-plugins/gst-plugins-good media-plugins/gst-plugins-libav media-video/pipewire
```

### Building from source

```bash
git clone https://github.com/PlayRood32/yood yood
cd yood

./linux/scripts/fetch-brave.sh linux-x86_64
./linux/run.sh brave
```

Release bundle (stage real binaries first):

```bash
./linux/scripts/prepare-binaries.sh linux-x86_64
cargo tauri build --config linux/src-tauri/tauri.conf.json
```

If the system `yt-dlp` is only a Python launcher, the staging script fetches
the official standalone Linux binary so the AppImage does not depend on
a system Python install.

Per-user install (no root) after building:

```bash
cargo tauri build --config linux/src-tauri/tauri.conf.json
./linux/scripts/install.sh linux/src-tauri/target/release/bundle/appimage/yood_*_amd64.AppImage
```

## Windows

Full guide: [windows/](windows/)

From a normal PowerShell prompt, from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\windows\scripts\build-windows.ps1
```

This checks prerequisites (Rust MSVC toolchain, Node.js, Tauri CLI, WebView2,
Visual Studio C++ Build Tools), downloads the pinned portable Brave build
plus `yt-dlp.exe` / `ffmpeg.exe` into
`windows\src-tauri\binaries\windows-x86_64\`, builds `shared/frontend`,
and runs `cargo tauri build`. The installer lands at
`windows\src-tauri\target\release\bundle\nsis\Yood_*_x64-setup.exe`.

Development:

```powershell
npm install --prefix shared/frontend
npm run build --prefix shared/frontend
cargo tauri dev --config windows/src-tauri/tauri.conf.json
```

## Usage

Stable engine (Brave app mode), Linux:

```bash
./linux/run.sh brave
```

Brave runs in app mode (no browser UI) with an isolated profile.
Wallet/Translate/MediaRouter, Sync, crash reporting and first-run are off.
Shields stays on, component updates stay on so filter lists keep updating.

Deep links:

- `yood://www.youtube.com/watch?v=ID` opens Yood and navigates there
- `yood://download?url=<youtube-url>` enqueues a download
- a second `yood <url>` focuses the running instance instead of opening a new one

Self-test:

```bash
# Linux
./linux/run.sh self-test
# or directly
linux/src-tauri/target/release/yood --self-test
```

```powershell
# Windows
windows\src-tauri\target\release\yood.exe --self-test
```

This checks the config schema, URL policy, filter init, built-in ad rules,
and that the companion extension manifest points at a valid `rules.json`.

## Config files

Linux:

- Brave profile: `~/.local/share/com.playrood.yood/data/brave-profile`
- materialized extension: `~/.local/share/com.playrood.yood/data/yood-extension`
- logs: `~/.local/share/com.playrood.yood/logs/yood.log`
- install dir: `~/.local/share/yood`, launcher: `~/.local/bin/yood`

Windows:

- Brave profile: `%APPDATA%\PlayRood\Yood\data\brave-profile`
- logs: `%APPDATA%\PlayRood\Yood\logs\yood.log`

Logs never contain passwords, cookies or tokens. Log level is set in
Settings. WARN/ERROR go to the log file, stderr echo is debug-builds only.

## Troubleshooting

Download buttons / Alt+L bar / ad-skip not working:

Chromium ignores `--load-extension` extensions unless Developer Mode is on
in that profile. Yood seeds `extensions.ui.developer_mode = true` into its
own isolated Brave profile on first creation. If you ran an older build
first, delete the profile once so it gets recreated:

- Linux: remove `~/.local/share/com.playrood.yood/data/brave-profile` and relaunch
- Windows: remove `%APPDATA%\PlayRood\Yood\data\brave-profile` and relaunch

YouTube ads on a fresh profile:

- leave Shields on. Yood seeds aggressive mode for `youtube.com`,
  `youtu.be`, `youtube-nocookie.com`
- leave component updates on (Shields lists update through them)
- Yood rules are also seeded as Brave custom filters (`brave://adblock`)

No Brave found:

- Linux: run `./linux/scripts/fetch-brave.sh linux-x86_64`, or install
  `brave-bin`, or set `YOOD_BRAVE_DIR`
- Windows: run `windows\scripts\build-windows.ps1` (stages portable Brave),
  or install Brave normally

WebKit view crashes on video pages (Linux):

Upstream WebKitGTK issue. Use `./linux/run.sh brave` for normal watching.
`YOOD_WEBVIEW=1` is debug-only.

## Known limits

- the embedded WebKit view segfaults on YouTube player pages on some
  systems. Brave app mode is the supported path.
- DRM rentals are out of scope (no Widevine in portable Brave builds,
  regular YouTube does not need it).
- `https://` links cannot open Yood without claiming all web traffic.
  Use the `yood://` scheme.
- release bundles must stage real `yt-dlp` / `ffmpeg` binaries before packaging.

## Development

From `linux/` or `windows/` as needed, frontend lives in `shared/frontend`:

```bash
cargo fmt
cargo check --manifest-path linux/src-tauri/Cargo.toml
cargo test --manifest-path linux/src-tauri/Cargo.toml
npm run build --prefix shared/frontend
```

## License

MIT. See `LICENSE` and `linux/src-tauri/Cargo.toml`.

## Support

- repo: https://github.com/PlayRood32/yood
- issues: https://github.com/PlayRood32/yood/issues
- spec: see [Yood_Technical_Specification.md](Yood_Technical_Specification.md)
- changelog: see [CHANGES.md](CHANGES.md)
