# Shared

Single source of truth for both platforms.

- `extension/yood/` — companion extension (`manifest.json`, `content.js` with Shorts support, `rules.json` for `declarativeNetRequest`)
- `filters/default.txt` — built-in ad rules, also seeded as Brave custom filters
- `frontend/` — `bootstrap.ts`, `injection.ts`, `player.ts`, built `dist/`

Referenced from:

- `../linux/src-tauri/tauri.conf.json` → `../../shared/frontend/dist`, `../../shared/filters`, `../../shared/extension`
- `../windows/src-tauri/tauri.conf.json` → same
- Rust `include_str!("../../../shared/...")` in `browser.rs`, `lib.rs`, `filtering.rs`

Build:

```bash
npm install --prefix frontend
npm run build --prefix frontend
```
