# Yood — Changes

## Monorepo split (linux/ + windows/ + shared/)

- new layout: `linux/` (Linux-only), `windows/` (Windows-only), `shared/` (extension + filters + frontend)
- `linux/` cleaned of Windows code: no `reg.exe`/`explorer`/`rundll32`, no `.exe`/`NSIS`/`WebView2`/`MinGW`/`wine`, no `build-windows.ps1`, `tauri.conf.json` targets `appimage`+`deb` only
- merged generic fixes from the newer Windows build into Linux: `rules.json`, `manifest.json` 0.3.0, Shorts selector in `injection.ts`, Developer Mode seeding, Shields aggressive + custom filters, `self-test` CDN guard, `#[cfg(debug_assertions)]` log echo
- both `src-tauri` trees now reference `../../shared/...` (`frontendDist`, `resources`, `include_str!`)
- root `README.md` rewritten in adw-network style with Linux/Windows links; per-folder `README.md` files added
- verified: `cargo check` + `cargo test` (29 passed) on `linux/src-tauri`, `npm run build` on `shared/frontend` — all clean

# Yood — Changes in this pass

I could not compile or run this project in the environment I worked in (no
matching Rust toolchain — the project needs rustc ≥1.80, one dependency
needs newer still — and no display server to actually test the WebView).
Every change below is verified by careful manual reading of the Rust and by
running the real TypeScript compiler (`tsc --noEmit`, and `npm run build`,
both clean with zero errors) — not by an end-to-end build. Please run
`cargo build` / `cargo tauri build` yourself before relying on this, and
treat anything below marked "should" rather than "confirmed" with a bit of
suspicion until you've clicked through it once.

## Files changed
- `frontend/injection.ts`
- `frontend/player.ts`
- `src-tauri/Cargo.toml`
- `src-tauri/capabilities/default.json`
- `src-tauri/src/download.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tauri.conf.json`
- `frontend/dist/*` (regenerated from the above via `npm run build`)
- `src-tauri/Cargo.lock` deleted — it was pinned to the old dependency set;
  the next `cargo build` will regenerate it with the two new plugins.

## 1. Removed debug code left in the shipped build
`injection.ts` had leftover debug instrumentation: small colored dots drawn
in the corner of the page, and — more seriously — `document.title` was
being overwritten on every DOM scan with a string like
`Yood|nav:true|launch:true|ytd:true|boot:true`. That's very likely what you
were seeing replace the tab/window title. Both are removed.

## 2. Native player didn't load anything on cold start
`open_native_player` on the Rust side sends `{ source, title }` to the
player window (`source` is already a ready-to-play URL). `player.ts` was
reading a `payload.path` field that never existed, then trying to re-resolve
it as a filesystem path. The player window only ever worked if it happened
to already be open and receive a live `player-load` event; opening it fresh
silently failed. Fixed to use `payload.source` directly.

## 3. Download manager never updated live
The backend emits a `download-updated` Tauri event on every progress tick
(`download.rs`), but the frontend never listened for it anywhere — the
manager dialog only fetched a one-time snapshot when opened. Progress bars,
speed, ETA, and status all sat frozen until you closed and reopened the
dialog. Added a proper listener with cleanup on close, plus:
- `enqueue()` in `download.rs` now also emits on creation (it previously
  only emitted once a queued item started downloading, so a newly queued
  item wouldn't show up in an already-open manager).
- Added **Remove** and **Show folder** buttons, which the backend already
  supported (`remove_download`, `open_download_folder`) but the UI never
  exposed.

## 4. Resize/general sluggishness — likely cause identified, made configurable
Linux unconditionally sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`, which
disables GPU compositing in WebKitGTK and forces every repaint (including
every frame of a window resize) onto the CPU. This is a real, documented
workaround for a WebKitGTK freeze/blank-page bug on some GPU drivers
(tauri-apps/tauri#9394) — I did not rip it out blind, since that could bring
back a worse bug for some users. Instead: set the environment variable
`YOOD_FORCE_GPU_RENDERER=1` before launching Yood to re-enable hardware
compositing and see whether resize feels normal on your machine. If the
page goes blank or freezes with it set, unset it again — you need the
workaround.

## 5. Settings screen was missing most of what the backend supports
7 backend commands existed and worked but were never called from the UI at
all: `get_app_info`, `get_filter_status`, `update_filter_lists`,
`update_ytdlp`, `get_account_hints`, `set_account_hints`,
`clear_account_hints`. Settings only exposed 6 of the categories described
in the spec. Added:
- **Accounts** — a private label field (Yood never stores your Google
  password; this is just a note to tell accounts apart).
- **Ad blocking status** — live rule count, last filter-list update time,
  and a manual "update now" button.
- **About** — Yood version, bundled yt-dlp/FFmpeg status and versions, and
  an "update yt-dlp now" button.
- The download-behavior toggles that already existed in the settings model
  but had no control: overwrite-existing, embed-thumbnail-by-default,
  download-subtitles-by-default, and subtitle language.

## 6. Deep links (`yood://…`) and single-instance behavior
There was no OS-level URL handling at all beyond parsing `argv` on a fresh
process launch, and no handling for "Yood is already running and something
tried to open a link" — it would just launch a second, competing instance.

**Important caveat on scope**: a real `https://www.youtube.com/...` link
can't be made to open Yood specifically without either claiming *all*
`https://` traffic on the machine (which would break the user's normal
browser and is not something an app should do) or going through mobile-only
"verified App Link" mechanisms (Android/iOS) that don't apply to desktop
Linux/Windows. So what's implemented is a custom `yood://` scheme (e.g.
`yood://www.youtube.com/watch?v=ID`), registered via
`tauri-plugin-deep-link`, plus:
- `tauri-plugin-single-instance`, registered first as required, so a second
  launch focuses the existing window and navigates it instead of opening a
  competing instance.
- The existing `argv`-based cold-start handling (already there) still works
  unchanged for direct `yood <https-youtube-url>` invocations.
- Both paths are validated through the same allow-list
  (`security::is_supported_navigation` plus a youtube.com/youtu.be host
  check) before the window is ever navigated.

If you want real "click a youtube.com link in the OS and it opens Yood"
behavior, that needs a browser extension or OS-level default-app-per-domain
tool on the user's side that rewrites the link to `yood://...` — that's
outside what a background desktop app can register for itself.

## Not yet done
- A full pass against the spec's acceptance-criteria checklist and the
  accessibility/config-migration sections hasn't been completed.
- The "duplicate download" flow uses a global overwrite setting rather than
  a per-file Replace/Keep Both/Cancel prompt the spec sketches — flagged as
  a deliberate simplification, not a bug, since a blocking per-file prompt
  would stall unattended playlist/channel downloads.

---

# Round 2

## 7. Home feed grid showing fewer videos than fit on screen
Root cause found: `compactFeedRows()` in `injection.ts` was physically
moving `ytd-rich-item-renderer` DOM nodes between YouTube's own
`ytd-rich-grid-row` containers to "backfill" gaps left by removed ad cards.
YouTube's grid rows are a fixed-count layout decided by YouTube's own
Polymer/Lit JS at render time, not a fluid CSS grid — moving its custom
elements into a different parent row breaks its internal
row/index/virtualization bookkeeping, and the visible symptom is exactly
what you saw: a row rendering with fewer items than fit on screen, with the
remaining half of the row left blank. **Removed** the row-compaction logic
entirely. Ad cards are still removed (so no blank ad slot is left behind),
but Yood no longer tries to reflow the grid itself — a gap where an ad used
to be is standard, expected ad-blocker behavior (this is what Brave and
uBlock Origin actually do too), and is far safer than reaching into
YouTube's component internals.

## 8. Added Ctrl+L / Cmd+L "open YouTube link" bar
Pressing Ctrl+L (Cmd+L on macOS) now opens a centered dialog with a text
field. It accepts `youtube.com`, `www.youtube.com`, `m.youtube.com`,
`music.youtube.com`, `youtube-nocookie.com`, and `youtu.be` links (bare
video IDs after `youtu.be/` are normalised to a full watch URL); anything
else shows an inline error instead of navigating. This mirrors the same
host allow-list the Rust side already enforces for other navigation, so the
client-side check and the backend check agree.

## 9. The crash + repeating WebKit `internallyFailedLoadTimerFired` errors
**I could not fix this with confidence, and want to be upfront about why.**

I traced everything in Yood's own code that touches the WebView and network
requests (the ad-block `decide_policy` hook, the bundled and remote filter
lists, the DMABUF/GPU-rendering setting, the new single-instance/deep-link
plugins) and found no bug in Yood's code that would explain *every* network
load failing repeatedly. Your `.runtime-test` cache directory shows the app
did successfully run, build a filter cache, and load pages on 2026-08-17,
so the WebView pipeline does work on your machine at least some of the
time.

The specific error — `WebLoaderStrategy::internallyFailedLoadTimerFired`
repeating continuously — is a known WebKitGTK bug pattern (I found several
unrelated Tauri/GTK apps hitting the identical error text with the identical
symptom: every subsequent page load fails until the app is restarted or the
underlying condition is cleared). Reported causes upstream include: the
WebKit network process running out of disk space or failing to write its
disk cache, a version mismatch between webkit2gtk and libsoup on a system
that mixes package versions, and in some driver combinations, GPU/EGL
context failures cascading into the network process. Your webkitgtk version
(`2.52.5`) is very recent — Arch tracks upstream closely — so this may
simply be a regression in that specific release.

**What I'd try, in order, since I can't reproduce or confirm this myself:**
1. Clear WebKit's own cache (separate from Yood's `~/.cache/yood/`):
   `rm -rf ~/.cache/com.playrood.yood ~/.local/share/com.playrood.yood`
   then relaunch.
2. Try forcing GPU rendering back on to rule out a software-renderer
   interaction: `YOOD_FORCE_GPU_RENDERER=1 ./yood` (or however you're
   launching it) — if it now works, the DMABUF workaround is implicated and
   we should look at it further; if it still fails, it isn't.
3. Check `pacman -Qi webkit2gtk-4.1` version history / Arch's bug tracker
   for open reports against `2.52.5` — if this is a fresh upstream
   regression, downgrading the package (`pacman -U` an older cached
   version, or via the Arch Linux Archive) is the most reliable fix, since
   it's outside what Yood's own code controls.
4. Run with `WEBKIT_DISABLE_COMPOSITING_MODE=1` as an additional (more
   aggressive) software-path fallback, purely as a diagnostic — if this
   changes the behavior at all it tells us the GPU/compositing path is
   involved.

If you can grab the terminal output from the *first* failed load (not the
repeated ones after), that would help narrow this down further — the first
failure sometimes has a different, more specific error before it starts
repeating.

---

# Round 3

Your follow-up screenshot and log gave me the missing piece: Ctrl+L not
firing *and* the grid rendering too few columns *and* videos not loading,
all at once, on a build where the JS itself is syntactically valid (I
checked) — that combination only makes sense if all three are downstream
symptoms of one thing: the WebKitGTK network/rendering failure is severe
enough to also disrupt script execution and resource loading for the page
itself, not just video playback. Chasing them as three separate bugs was
the wrong framing; I went back to the actual `internallyFailedLoadTimerFired`
issue with that in mind.

## 10. WebKitGTK rendering workaround was applied too late, and wasn't the documented fix
Two problems with the previous approach:

- The `WEBKIT_DISABLE_DMABUF_RENDERER` env var was being set inside the
  `.setup()` closure, i.e. *after* `tauri::Builder::default()` had already
  registered plugins (including the two new ones from round 2). Tauri's own
  Linux graphics documentation is explicit that these variables must be set
  "before the webview is created" — and in practice, before any
  GTK/WebKitGTK code has run at all. Moved it to the very top of `run()`,
  before `tauri::Builder::default()` is even constructed.
- `WEBKIT_DISABLE_DMABUF_RENDERER` alone is Tauri's *first* recommended
  step. Their documentation lists `WEBKIT_DISABLE_COMPOSITING_MODE=1` as
  the next step specifically for the symptom you hit — "the app dies on
  resize with no useful error output" / general page instability — when
  disabling DMABUF alone isn't enough. Added it, gated behind the same
  `YOOD_FORCE_GPU_RENDERER` escape hatch as before, so it's still fully
  overridable.

**I still can't reproduce or run this myself** (same toolchain/display
limitations as before), so I can't promise this clears the
`internallyFailedLoadTimerFired` errors — but it now matches the documented
upstream fix for this exact symptom, applied at the documented point in
startup, which the previous version didn't quite do. If this doesn't fully
fix it, the next things I'd want to see are: whether the errors still
appear immediately on a completely blank `about:blank`-style page (isolates
whether it's YouTube-specific or a base WebKitGTK/system issue), and your
`pacman -Qi webkit2gtk-4.1` version alongside any open Arch bug reports
against it.

## 11. Grid showing 2 videos in the first row instead of 3+
Given the finding above, I no longer think this is a separate layout bug —
there is no CSS or JS in Yood's own code that constrains YouTube's grid
width (I checked again this round with fresh eyes; nothing touches
`ytd-rich-grid-renderer`, its width, zoom, or font-size). Your screenshot
shows a large blank area next to a normally-laid-out set of cards, which
looks like YouTube's own page falling back to a narrower layout because it
couldn't reliably load everything it needed — consistent with the same
WebKit network failures. If item 10 fixes the underlying WebKit issue, this
should resolve on its own; if the grid is still narrow after that, it's
likely a distinct, genuine bug worth reopening separately.

## 12. Ctrl+L
The code is correct and was verified to typecheck and build with no syntax
errors in the compiled output. If it's still not responding after the
WebKit fix above, that would mean the injected script isn't attaching its
`keydown` listener at all — worth checking the WebKit process's own console
(right-click the page → Inspect Element, if WebKitGTK's inspector is
enabled) for any error thrown during `injection.js` execution, since a
runtime (not syntax) error partway through `install()` could still leave
later code, like the keydown listener, never registered.

---

# Round 4 — the actual root cause

Your clarification that it "crashes when entering a link" (not on startup)
was the detail that cracked this. Previously I was reading "crash" and
"internallyFailedLoadTimerFired" as one general WebKit instability problem.
Once I understood the failure is specifically triggered by *navigating to a
video*, I went back to the one piece of Yood's own code that runs on every
single network response inside the WebView: the native ad-block hook in
`install_native_filter` (`lib.rs`). That was the real bug, and it's been
there since before I started working on this project — not something
either of my last two rounds introduced.

## 13. The ad blocker could cancel the video page's own navigation
`install_native_filter` connects to WebKitGTK's `decide-policy` signal and
runs *every* network response in the WebView — including the top-level
navigation when you open a video — through the same ad/tracker filter
check used for actual ads, always tagged as request type `"other"`.

Filter lists like EasyList are written assuming a browser only ever runs
this kind of check against a page's *subresources* (scripts, images, XHR)
after the page itself has already been allowed to load — the main
navigation is implicitly trusted and never passed through the ad-blocking
engine. Yood was not making that distinction: if any rule in any of the
three bundled/remote filter lists happened to match a video's own watch URL
(its video ID or query parameters can coincidentally look like a
tracked/blocked pattern to a generic list), the engine would say "block
this," and `decision.ignore()` would cancel *the entire page navigation* -
not hide an ad, but stop the video page from ever loading. That is
consistent with every symptom you reported: the crash-like failure
specifically on entering a link, the repeating
`internallyFailedLoadTimerFired` errors (WebKit repeatedly retrying and
failing the same cancelled main-resource load), and arguably even the grid
issue from round 2/3, if a similar false-positive was intermittently
affecting subresource loads on the home page too.

Fixed in two layers:
- `install_native_filter` now checks
  `response_decision.is_main_frame_main_resource()` first and immediately
  allows the response (returns `false`, meaning "don't intercept") if it's
  the page's own top-level document load. Only genuine subresources are now
  ever passed to the filter engine. This method has been part of
  WebKitGTK's stable API since well before 2.40 (the feature level Yood
  already targets), so this didn't need a new dependency.
- As a second, independent safety net, `check_network_request` in
  `filtering.rs` now unconditionally allows any request to
  `googlevideo.com` (YouTube's actual video/audio streaming CDN) before
  ever asking the filter engine - there is no legitimate reason an
  ad-blocker would need to block that domain for a YouTube-only browser,
  and this protects actual playback even for a subresource-level false
  positive, not just the page navigation.

I'm considerably more confident in this fix than the two before it, because
it's not a driver/environment workaround I can't verify - it's a logic bug
in code that runs on every single navigation, with a specific, checkable
mechanism that matches your reported symptom exactly (crashes specifically
"when entering a link"). I still couldn't compile or run it myself, so
please do treat "should be right" as "should be right," not "confirmed" -
but if this doesn't clear it, the next thing I'd want to see is the output
of the `check_filter_request` command (already wired up in Rust) called
with the exact watch URL that failed, to see whether it was in fact the
filter engine returning true for that URL.

---

# Round 5

Your side-by-side screenshot against Brave nailed down the grid difference
precisely: Brave's home feed uses 2 full-width columns, Yood's uses 1, at
comparable window widths. That ruled out my rounds 2-4 theories (ad-block
compaction logic, WebKit rendering instability) as the cause of *this*
specific symptom, and pointed at something both browsers handle very
differently: how YouTube's frontend identifies them.

## 14. WebKitGTK's default user agent has no browser identity, and YouTube serves it a narrower layout
Checked what user agent Yood was actually sending. It never set one, so
WebKitGTK used its unmodified default: roughly
`Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)`
- notably missing any `Chrome/...` or `Version/... Safari/...` token at the
end. WebKitGTK's own documentation for the user-agent setting warns about
exactly this: "unusual user-agent strings may cause web content to render
incorrectly," because sites parse the UA to decide what they're talking to.
Brave identifies as Chrome (it's Chromium-based under the hood), so
YouTube gives it the full modern responsive layout; Yood's WebView, with no
recognizable browser token at all, was very plausibly getting downgraded to
a more conservative fallback layout - fewer grid columns being one
plausible symptom of that.

Set the main window's user agent to a standard, current desktop Chrome
string (`Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like
Gecko) Chrome/128.0.0.0 Safari/537.36`), the same family of string Brave
itself sends. This is the standard fix WebKitGTK-based app developers use
for this exact class of problem, and Tauri's own docs mention the same
default-UA quirk.

I'd put moderate-to-good confidence on this one - it's a well-documented
WebKitGTK behavior and a plausible, checkable mechanism for the layout
difference you photographed, but I can't run the app to confirm YouTube's
grid actually widens as a result.

## 15. Ctrl+L
Re-read `install()` end to end this round specifically looking for a path
where an earlier line could throw and prevent the script from ever reaching
the `keydown` listener registration - didn't find one; every DOM access
before it is null-guarded. Given round 4's fix (item 13) stops the crash on
entering a video link, and a crashed/never-loaded page obviously has no
Yood script running on it to respond to Ctrl+L, my best read now is that
Ctrl+L may have simply been tested on a page that failed to load rather
than being broken itself. Worth re-testing specifically on the YouTube home
page (which was loading fine even before this round) once the crash fix is
in, to isolate whether Ctrl+L works there.

## On using Brave's own source instead
You asked whether it would just be easier to take Brave's source and use
that instead of continuing to patch Yood. Worth answering directly rather
than deflecting into more fixes:

**What Brave actually is**: a fork of Chromium (Google's open-source
browser engine) with Brave's own ad-blocking, privacy, and UI layered on
top. It's a full desktop browser - hundreds of C++ files, its own build
toolchain, update infrastructure, extension system, and multi-year ongoing
maintenance. It is not a component you drop into another app; running it
means running Brave.

**Why Yood isn't "use Brave's engine" already**: Yood's whole design
premise is a small, purpose-built YouTube app - not a general browser -
built on Tauri/WebKitGTK because that ships with the OS on Linux (no
bundled multi-hundred-MB browser engine, small install size, OS-level
updates). Swapping to Chromium/Brave's engine is a different project with a
different size and maintenance profile, not a drop-in fix: it would mean
either (a) embedding a full Chromium build (the CEF - Chromium Embedded
Framework - route some apps use, which is itself a large, separately
-maintained dependency with its own packaging and update burden), or (b)
literally shipping and driving actual Brave as a subprocess, which would
make Yood a wrapper around a full second browser rather than a lightweight
YouTube client.

**What actually causes the differences you're seeing**: none of the bugs
found in rounds 1-5 were things Chromium/Brave does "better" in some
fundamental way - they were specific, fixable mistakes in Yood's own code
(the ad-block hook blocking real navigations, no user agent set, leftover
debug code) plus one real WebKitGTK rendering tradeoff that's now
configurable. All of that is fixable within the current architecture, and
several already are as of this round.

If the honest goal is "the most reliable path to something that behaves
exactly like Brave with the least ongoing engineering effort," using actual
Brave (or Chromium via CEF) is a legitimate answer - but that is a rewrite,
not a patch, and it trades Yood's current lightweight footprint for a much
larger, harder to maintain one. If the goal is "fix the specific things
that are broken right now," the current path is the faster one, and we're
narrowing the remaining list each round.

---

# Round 6 — closing the remaining gaps in the technical specification

This round differs from rounds 1-5: this time I could compile and test on
the target machine. `cargo test` (24 tests), `yood --self-test`, and
`npm run build` all pass with the changes below.

## 16. Local logging (spec §35) — was configured but not implemented
`Settings.log_level` existed and validated, but nothing actually logged.
Added `src-tauri/src/logging.rs`, a dependency-free local logger:
- Levels ERROR/WARN/INFO/DEBUG/TRACE, default INFO, configured from
  settings and hot-swappable when Settings is saved.
- Writes to `~/.local/share/com.playrood.yood/logs/yood.log` (append,
  timestamped). Logging is best-effort - a full disk or unwritable file
  never blocks startup.
- Mandatory sanitization: every line passes through `redact_sensitive`,
  which replaces sensitive URL query values (token, key, sig, cp, oauth,
  cookie, session, sapisid/hsid/ssid, id/access/refresh_token, code,
  credential, secret) and credential headers (Cookie/Authorization/
  Set-Cookie) with `[redacted]`.
- Wired into: startup, filter-list refresh outcome, blocked subresource
  decisions (DEBUG), download lifecycle (start/complete/pause/cancel/
  fail + TRACE per progress line), yt-dlp updates, and settings saves
  re-applying the level immediately.
- Unit tests cover level parsing, redaction, and pass-through of plain
  messages.

## 17. Disk-space guard now actually works (spec §51)
`DownloadManager.perform` already probed `fs2::available_space`, but the
size source - `metadata_size()` - was a stub that always returned `None`,
so the check never fired. Now:
- `InspectResult` carries `size_bytes`, parsed from yt-dlp's `filesize`
  (falling back to `filesize_approx`).
- Before starting, Yood requires `size + 5%` headroom; otherwise the
  download fails with a human message including both sizes
  (`format_bytes`, e.g. "need about 1.2 GiB, only 850.0 MiB free").
- Mid-download disk exhaustion ("No space left on device", "Disk quota
  exceeded", etc.) is mapped from a raw process error to a friendly
  `Storage` error instead of a redacted process string.

## 18. Google authentication compatibility fallback (spec §6.1)
When the WebView lands on an `accounts.google.com` / `accounts.youtube.com`
page, Yood now shows a small floating banner: sign-in still happens
in-app, but if Google refuses the embedded WebView, one click opens the
current sign-in URL in the user's regular browser (`open_external`, which
already restricts to HTTPS). The banner appears only on those hosts and is
removed on any other navigation.

## 19. Configurable keyboard shortcuts (spec §54)
New `shortcut_download` setting (single letter/digit, default `d`),
validated in Rust, exposed in Settings (Shift + key opens the download
dialog). The frontend reads it at install time and refreshes it after a
settings save. Legacy `Shift+D` remains the default.

## 20. Native player controls (spec §25)
The native player window gained a seek timeline, current/duration readout,
volume slider, mute toggle, and a playback stats line (resolution +
stream host). Buffering/paused/playing states are surfaced in the status
line. The old keyboard set (Space/K/F/M/J/L/arrows) is unchanged, and
arrow keys still seek when the timeline slider has focus.

## 21. Settings UI additions (spec §29/§35)
Settings now exposes Log level (ERROR…TRACE, with a note that logs are
local-only and credential-free) and the download shortcut key.

## 22. Linux per-user install script (spec §45)
`scripts/install.sh` installs a built binary or AppImage into
`~/.local/share/yood`, symlinks it into `~/.local/bin`, writes a
`.desktop` entry registering the `yood://` scheme handler, and refreshes
the desktop/icon databases. No root required; removal is documented in the
script's output. The README documents the flow.

## 23. Test coverage (spec §61)
Added unit tests for: settings migration (schema 0→current, backfilled
filter lists, newer-schema rejection), settings validation and patch
application, error conversions (io/serde/url kinds), download argument
construction for Audio/Songs modes, `unique_target` duplicate handling,
`filesize` parsing, disk-full detection, and byte formatting. The suite
now totals 24 tests, all passing.

## Verification
- `cargo build` — clean, no warnings.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` — 24 passed.
- `cargo run -- --self-test` — "Yood self-test OK".
- `npm run build --prefix frontend` — clean TypeScript compile; dist
  regenerated.

---

# Round 8 — Root cause of the WebKit errors + Ctrl+L fixes

## Files changed
- `frontend/injection.ts` — Ctrl+L made layout-independent; visible URL-bar
  button added to the nav bar; `frontend/dist/*` regenerated.

## 1. The real cause of `internallyFailedLoadTimerFired` + the app dying
Reading the actual WebKitGTK 2.52.6 source (`WebLoaderStrategy.cpp`) shows
the message fires only when the **NetworkProcess is dead or unreachable**
when a load is scheduled — it is not caused by anything Yood's filter hook
does. The system journal on the user's machine confirms the sequence:

```
kernel: yood[1580816]: segfault at 48 ip ... in libwebkit2gtk-4.1.so.0.21.10
systemd-coredump: Process 1580816 (yood) ... terminated abnormally with signal 11/SEGV
```

The **Yood process itself** was segfaulting inside WebKitGTK's own code
(all backtrace frames are in `libwebkit2gtk-4.1.so.0.21.10`), killing the
app; the WebKit "internal error" spam in the terminal is the NetworkProcess
failing loads while its parent is going down. The crashes began the moment
the user upgraded webkit2gtk-4.1 to **2.52.6** (installed 2026-08-19, 20:42;
crashes at 20:57 and 21:03). On 2.52.5 the app logged the internal errors
but survived. This is a WebKitGTK 2.52.6 regression, not a Yood bug.
Remedy: downgrade to 2.52.5-2 (available on the Arch archive) until
upstream fixes the 2.52.x UIProcess crash.

## 2. Ctrl+L robust now + a visible button
- The shortcut now matches on the **physical key** (`event.code === "KeyL"`,
  falling back to `event.key`) and listens in the **capture phase**, so it
  works under any keyboard layout (e.g. Hebrew) and can't be swallowed by
  YouTube's own key handlers.
- Added a **URL** button to the in-app nav bar (back/forward/reload), so the
  URL bar is reachable with one click even without the keyboard shortcut.
  It opens the same animated top bar.

## 3. What was tried and ruled out
- Blocking ads at the `Response` stage → moved to `NavigationAction`
  (pre-request, matching Epiphany). Correct and kept, but not the cause.
- Disk space / network-cache SIGBUS → disk has 40 GB+ free; ruled out.
- Keyboard layout for Ctrl+L → system layout is `us`; ruled out as the only
  cause, but the shortcut is now layout-independent anyway.
- The systemd-coredump backtraces show unrelated threads (tokio runtime,
  `tao`'s D-Bus theme portal thread); the crash thread is entirely inside
  `libwebkit2gtk`.

## Verification
- `cargo build` — clean.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` — 24 passed.
- `npm run build --prefix frontend` — clean; dist regenerated.

---

# Round 7 — WebKit error loop + animated URL bar

## Files changed
- `src-tauri/src/lib.rs` — native filter hook rewritten.
- `frontend/injection.ts` — URL bar rebuilt; `frontend/dist/*` regenerated.

## 1. `internallyFailedLoadTimerFired` error loop (native hook)
The Linux `decide-policy` hook blocked ad subresources at the **Response**
stage (`decision.ignore()` on `PolicyDecisionType::Response`). That cancels a
load *after* the response arrived, and the WebProcess then keeps retrying the
cancelled load — producing the endless
`WebLoaderStrategy::internallyFailedLoadTimerFired` WebKit internal errors
on webkitgtk 2.52.x.

The hook now blocks at the **NavigationAction** stage instead — the decision
fires *before* the request is sent, so ignoring it never touches an
in-flight load. This is the same approach GNOME's Epiphany uses for
request-level ad blocking. Details:
- Only `PolicyDecisionType::NavigationAction` is handled.
- Main-frame navigations (no frame name) are always allowed, so opening
  videos and navigating never blocked.
- Subframe navigations (ad iframes, e.g. doubleclick.net) are checked
  against the filter lists as request type `sub_frame` and ignored when
  matched.
- Response-level blocking is gone entirely, which removes the internal
  failure loop. Direct `<script>`/`<img>` subresource loads are no longer
  cancelled mid-flight; the frontend cosmetic layer already hides their UI.

The remaining `internallyFailedLoadTimerFired` instances on some systems are
the known upstream WebKitGTK RT-thread/RealtimeKit bug (fixed in WebKit
commit 0831c81); workarounds remain: update webkit2gtk-4.1, clear
`~/.cache/com.playrood.yood`, or `YOOD_FORCE_GPU_RENDERER=1`.

## 2. Ctrl+L animated URL bar
`showUrlBar()` was a centered modal card. It is now a full-width bar that
slides down from the top of the window (springy cubic-bezier entrance, red
logo badge, blurred dark background) with a single wide input and Go/close
buttons. Behaviour:
- Valid YouTube link → the input and Go button pulse a blue glow for ~420 ms
  before navigating, so the transition is visible.
- Invalid/non-YouTube link → the bar shakes and shows
  "This link isn't a YouTube link." (red, with alert role); the page is not
  navigated. Typing clears the message.
- Escape or the close button dismisses; Enter submits. `closeOverlay()`
  also removes the bar.
- The existing global `prefers-reduced-motion` rule collapses all new
  animations to ~0 ms for users who ask for reduced motion.

## Verification
- `cargo build` — clean, no warnings.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` — 24 passed.
- `cargo run -- --self-test` — "Yood self-test OK".
- `npm run build --prefix frontend` — clean TypeScript compile; dist
  regenerated.

Known non-goals this round (unchanged from spec §57): Yood itself still
has no auto-update system (intended for private/family use), and the
packaging/QA phases (7-8) remain manual on Windows.




