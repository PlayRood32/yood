<div align="center">

# Yood

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-blue.svg)](https://tauri.app)
[![Linux](https://img.shields.io/badge/Linux-supported-green.svg)](#-linux)
[![Windows](https://img.shields.io/badge/Windows-supported-blue.svg)](#-windows)

Lightweight private YouTube desktop app.

Official YouTube site in Brave app mode, with Yood-owned downloads, queue, filtering, and settings.

[What It Does](#-what-it-does) • [Layout](#-project-layout) • [Linux](#-linux) • [Windows](#-windows) • [Usage](#-usage) • [Development](#-development)

</div>

Yood is a Rust/Tauri desktop YouTube application. The primary window is the official YouTube site; Yood-owned dialogs, the queue, filtering, settings, and the optional local player are provided by the Rust integration layer.

Current app version: `0.2.0`

---

## ✨ What It Does

### 📺 YouTube-first

- open the official YouTube site in Brave app mode (no browser UI, isolated profile)
- keep Brave Shields ON with aggressive mode for YouTube + Yood custom filters
- inject download buttons into watch / Shorts pages via the companion extension
- Alt+L quick-open bar and in-player ad-skip helper

### 📥 Downloads

- queue video / audio / songs via `yt-dlp` + `ffmpeg`
- live progress, speed, ETA in the download manager
- remove items and open the download folder from the UI
- `yood://download?url=...` deep links from the extension

### 🛡️ Ad blocking

- Brave Shields (self-updating lists) + Yood rules as Brave custom filters
- companion extension network rules (`declarativeNetRequest`) + cosmetic hiding
- Rust filter layer for WebView mode; stays off in Brave mode (less CPU/RAM)

### 🔗 Integration

- `yood://` deep-link scheme with single-instance handling
- per-user install on Linux (no root), NSIS installer on Windows
- `yood --self-test` smoke test without a WebView

---

## 🗂️ Project Layout

```text
├── 📁 linux
│   ├── 📁 scripts
│   │   ├── 🐚 fetch-brave.sh
│   │   ├── 🐚 install.sh
│   │   ├── 🐚 lock-devtools.sh
│   │   └── 🐚 prepare-binaries.sh
│   ├── 📁 src-tauri
│   │   ├── 📁 capabilities
│   │   │   └── ⚙️ default.json
│   │   ├── 📁 gstreamer
│   │   │   ├── 🔧 libgstautodetect.so
│   │   │   └── 🔧 libgstpipewire.so
│   │   ├── 📁 icons
│   │   │   ├── 🖼️ icon.ico
│   │   │   ├── 🖼️ icon.png
│   │   │   └── 🖼️ icon.svg
│   │   ├── 📁 src
│   │   │   ├── 🦀 browser.rs
│   │   │   ├── 🦀 commands.rs
│   │   │   ├── 🦀 desktop.rs
│   │   │   ├── 🦀 download.rs
│   │   │   ├── 🦀 error.rs
│   │   │   ├── 🦀 filtering.rs
│   │   │   ├── 🦀 lib.rs
│   │   │   ├── 🦀 logging.rs
│   │   │   ├── 🦀 main.rs
│   │   │   ├── 🦀 models.rs
│   │   │   ├── 🦀 process.rs
│   │   │   ├── 🦀 secure.rs
│   │   │   ├── 🦀 security.rs
│   │   │   ├── 🦀 settings.rs
│   │   │   └── 🦀 updates.rs
│   │   ├── 🦀 build.rs
│   │   ├── 📦 Cargo.lock
│   │   ├── 📦🦀 Cargo.toml
│   │   └── ⚙️ tauri.conf.json
│   ├── 📖 README.md
│   └── 🐚 run.sh
├── 📁 shared
│   ├── 📁 extension
│   │   └── 📁 yood
│   │       ├── 🟨 content.js
│   │       ├── 🌐 manifest.json
│   │       └── ⚙️ rules.json
│   ├── 📁 filters
│   │   └── 📝 default.txt
│   ├── 📁 frontend
│   │   ├── 🟦 bootstrap.ts
│   │   ├── 🌐 index.html
│   │   ├── 🟦 injection.ts
│   │   ├── 📦 package-lock.json
│   │   ├── 📦 package.json
│   │   ├── 🌐 player.html
│   │   ├── 🟦 player.ts
│   │   └── ⚙️🟦 tsconfig.json
│   └── 📖 README.md
├── 📁 windows
│   ├── 📁 scripts
│   │   └── 🪟 build-windows.ps1
│   ├── 📁 src-tauri
│   │   ├── 📁 capabilities
│   │   │   └── ⚙️ default.json
│   │   ├── 📁 icons
│   │   │   ├── 🖼️ icon.ico
│   │   │   ├── 🖼️ icon.png
│   │   │   └── 🖼️ icon.svg
│   │   ├── 📁 src
│   │   │   ├── 🦀 browser.rs
│   │   │   ├── 🦀 commands.rs
│   │   │   ├── 🦀 desktop.rs
│   │   │   ├── 🦀 download.rs
│   │   │   ├── 🦀 error.rs
│   │   │   ├── 🦀 filtering.rs
│   │   │   ├── 🦀 lib.rs
│   │   │   ├── 🦀 logging.rs
│   │   │   ├── 🦀 main.rs
│   │   │   ├── 🦀 models.rs
│   │   │   ├── 🦀 process.rs
│   │   │   ├── 🦀 secure.rs
│   │   │   ├── 🦀 security.rs
│   │   │   ├── 🦀 settings.rs
│   │   │   └── 🦀 updates.rs
│   │   ├── 🦀 build.rs
│   │   ├── 📦 Cargo.lock
│   │   ├── 📦🦀 Cargo.toml
│   │   └── ⚙️ tauri.conf.json
│   └── 📖 README.md
├── 📝 CHANGES.md
├── 📖 README.md
└── 📝 Yood_Technical_Specification.md
```

- `shared/` is the single source of truth for the extension, filters, and frontend. Both `linux/src-tauri` and `windows/src-tauri` reference it via `../../shared/...`.
- `linux/` is Linux-only: AppImage/deb targets, `run.sh`, shell scripts, WebKit/GStreamer bits. No Windows code.
- `windows/` is Windows-only: NSIS target, `build-windows.ps1`, `CREATE_NO_WINDOW`, WebView2. See [windows/](windows/) and [linux/](linux/).

---

## 🐧 Linux

Full guide: [linux/](linux/)

### ✅ Requirements

Runtime:

- Brave (bundled portable or system `brave` / `brave-browser` / `brave-bin`)
- WebKit/GStreamer runtime for the optional WebView mode

Build:

- Rust toolchain (`rust-version = 1.80`)
- Node.js (frontend `tsc` build)
- Tauri Linux deps (WebKitGTK, etc.)
- `curl`, `unzip`, `sha256sum` for Brave/binary staging

#### linux:

install the WebKit/GStreamer runtime used by YouTube before launching Yood:

<details>

<summary><b>Arch Linux</b></summary>

```bash
sudo pacman -S --needed webkit2gtk-4.1 gst-plugins-base gst-plugins-good gst-libav pipewire
```

</details>

<details>

<summary><b>Debian</b></summary>

```bash
sudo apt install webkit2gtk-4.1 gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-libav pipewire
```

</details>

<details>

<summary><b>Fedora</b></summary>

```bash
sudo dnf install webkit2gtk4.1 gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-libav pipewire
```

</details>

<details>

<summary><b>Gentoo</b></summary>

```bash
sudo emerge --ask net-libs/webkit-gtk media-libs/gst-plugins-base media-plugins/gst-plugins-good media-plugins/gst-plugins-libav media-video/pipewire
```

</details>

### 🛠️ Building from Source

```bash
# Clone repository
git clone <your-repo-url> yood
cd yood

# Stable engine (Brave in app mode + Shields)
./linux/scripts/fetch-brave.sh linux-x86_64
./linux/run.sh brave
```

Release bundle (stage real binaries first):

```bash
./linux/scripts/prepare-binaries.sh linux-x86_64
cargo tauri build --config linux/src-tauri/tauri.conf.json
```

If the system `yt-dlp` is only a Python launcher, the staging script downloads yt-dlp's official standalone Linux binary so the AppImage does not depend on a system Python installation.

Per-user install (no root) after building:

```bash
cargo tauri build --config linux/src-tauri/tauri.conf.json
./linux/scripts/install.sh linux/src-tauri/target/release/bundle/appimage/yood_*_amd64.AppImage
```

---

## 🪟 Windows

Full guide: [windows/](windows/)

From an elevated-not-required PowerShell prompt, from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\windows\scripts\build-windows.ps1
```

This checks prerequisites (Rust MSVC toolchain, Node.js, the Tauri CLI, WebView2, Visual Studio C++ Build Tools), downloads the pinned portable Brave build plus `yt-dlp.exe`/`ffmpeg.exe` into `windows\src-tauri\binaries\windows-x86_64\`, builds `shared/frontend`, and runs `cargo tauri build`. The installer lands at `windows\src-tauri\target\release\bundle\nsis\Yood_*_x64-setup.exe`.

Development:

```powershell
npm install --prefix shared/frontend
npm run build --prefix shared/frontend
cargo tauri dev --config windows/src-tauri/tauri.conf.json
```

---

## 📖 Usage

### ▶️ Stable engine (Brave app mode)

Linux:

```bash
./linux/run.sh brave
```

Debloat: app mode (no browser UI), isolated profile, BraveWallet/Translate/MediaRouter features off, Sync/crash-reporting/first-run off, Rewards/News/VPN never surfaced. Shields stays ON, and component updates are intentionally left on so its filter lists keep updating.

### 🔗 Deep links

- `yood://www.youtube.com/watch?v=ID` opens Yood and navigates there
- `yood://download?url=<https-youtube-url>` enqueues a download
- a second `yood <url>` focuses the running instance instead of opening a new one

### 🧪 Self-test

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

This checks the configuration schema, URL policy, filter initialization, mandatory built-in ad rules, and that the companion extension manifest references a valid `rules.json` that never touches content CDNs.

---

## 📁 Configuration Files

Linux:

- Brave profile: `~/.local/share/com.playrood.yood/data/brave-profile`
- materialized extension: `~/.local/share/com.playrood.yood/data/yood-extension`
- logs: `~/.local/share/com.playrood.yood/logs/yood.log`
- install dir: `~/.local/share/yood`, launcher: `~/.local/bin/yood`

Windows:

- Brave profile: `%APPDATA%\PlayRood\Yood\data\brave-profile`
- logs: `%APPDATA%\PlayRood\Yood\logs\yood.log`

Log lines never contain passwords, cookies, or tokens. Log level is configured in Settings; WARN/ERROR go to the log file, stderr echo is debug-builds only.

---

## 🧪 Troubleshooting

<details>
<summary><b>Download buttons / Alt+L bar / ad-skip not working</b></summary>

Chromium disables `--load-extension` extensions unless the profile has Developer Mode on. Yood seeds `extensions.ui.developer_mode = true` into its own isolated Brave profile on first creation.

If you ran an older build first, delete the profile once so it gets recreated:

- Linux: remove `~/.local/share/com.playrood.yood/data/brave-profile` and relaunch
- Windows: remove `%APPDATA%\PlayRood\Yood\data\brave-profile` and relaunch

</details>

<details>
<summary><b>YouTube ads showing on a fresh profile</b></summary>

- do not turn Shields off; Yood seeds aggressive mode for `youtube.com`, `youtu.be`, `youtube-nocookie.com`
- keep component updates on (Shields lists update through them)
- Yood's own rules are also seeded as Brave custom filters (`brave://adblock`)

</details>

<details>
<summary><b>No Brave found</b></summary>

- Linux: run `./linux/scripts/fetch-brave.sh linux-x86_64` or install `brave-bin`, or set `YOOD_BRAVE_DIR`
- Windows: run `windows\scripts\build-windows.ps1` (stages portable Brave) or install Brave normally

</details>

<details>
<summary><b>WebKit view crashes on video pages (Linux)</b></summary>

That is the known upstream WebKitGTK bug (proven by bisection). Use `./linux/run.sh brave` for stable watching. `YOOD_WEBVIEW=1` is debug-only.

</details>

---

## ⚠️ Known Limits

- embedded WebKit view segfaults on YouTube's full player pages; Brave app mode is the supported path
- hotspot-style DRM rentals are out of scope (no Widevine in portable Brave builds; regular YouTube does not need it)
- `https://` links cannot open Yood without claiming all web traffic; use the `yood://` scheme
- release bundles must stage real `yt-dlp`/`ffmpeg` binaries before packaging

---

## 🔧 Development

Recommended checks (run from `linux/` or `windows/` as appropriate, frontend lives in `shared/frontend`):

```bash
cargo fmt
cargo check --manifest-path linux/src-tauri/Cargo.toml
cargo test --manifest-path linux/src-tauri/Cargo.toml
npm run build --prefix shared/frontend
```

---

## 📄 License

MIT. See `linux/src-tauri/Cargo.toml` (`license = "MIT"`).

---

## 📬 Support

- issues: open an issue in this repository
- spec: see [Yood_Technical_Specification.md](Yood_Technical_Specification.md)
- changelog: see [CHANGES.md](CHANGES.md)
