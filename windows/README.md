# Yood for Windows

Windows-only build. Shared code lives in [`../shared/`](../shared/). Root guide: [`../README.md`](../README.md).

## Build

From an elevated-not-required PowerShell prompt, from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\windows\scripts\build-windows.ps1
```

Result: `src-tauri\target\release\bundle\nsis\Yood_*_x64-setup.exe`

## Development

```powershell
npm install --prefix ..\shared\frontend
npm run build --prefix ..\shared\frontend
cargo tauri dev --config src-tauri\tauri.conf.json
```

## Notes

- `tauri.conf.json` targets `nsis` only, resources point at `../../shared/...`
- Rust `CREATE_NO_WINDOW` guards prevent console flashes (`reg.exe`, `rundll32`, `yt-dlp`, Brave child)
- `windows_subsystem = "windows"` is set for release GUI builds (no console window)
- Companion extension is loaded via `--load-extension`; Yood seeds Developer Mode in its own Brave profile so it stays enabled
- If buttons still don't show after upgrading from an old build, delete `%APPDATA%\PlayRood\Yood\data\brave-profile` once and relaunch

## Layout

- `src-tauri/` — Rust backend + `tauri.conf.json` (Windows-only)
- `scripts/build-windows.ps1` — automated prereq check + Brave/yt-dlp/ffmpeg staging + NSIS build
