//! Brave-first runtime: the `yood` binary launches Brave itself in app mode
//! and then runs headless (downloads, deep links, single instance) with no
//! WebKit window at all. No extra dev command needed for end users.
//!
//! The legacy embedded WebKit view is kept behind `YOOD_WEBVIEW=1` for
//! debugging only: it segfaults on YouTube's full player pages on affected
//! systems (upstream WebKitGTK bug, proven by bisection).
//!
//! This is the only launch path now (the dev-only bash helper was removed).

use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::Duration,
};

use crate::{
    commands::RuntimeState, logging, models::DownloadStatus, startup_url,
};

static BRAVE_CHILD: Mutex<Option<Child>> = Mutex::new(None);

pub fn is_webview_mode() -> bool {
    std::env::var_os("YOOD_WEBVIEW").is_some()
}

#[cfg(windows)]
fn brave_exe_name() -> &'static str {
    "brave.exe"
}

#[cfg(not(windows))]
fn brave_exe_name() -> &'static str {
    "brave"
}

fn is_executable(path: &PathBuf) -> bool {
    #[cfg(windows)]
    {
        path.is_file()
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        path.is_file()
            && std::fs::metadata(path)
                .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    }
}

fn bundled_or_system_brave() -> Option<PathBuf> {
    // Every layout we ship, in priority order. Logged by the caller when
    // nothing matches, so a missing binary is diagnosable from yood.log.
    let mut tried: Vec<PathBuf> = Vec::new();
    let mut consider = |path: PathBuf| -> Option<PathBuf> {
        tried.push(path.clone());
        if is_executable(&path) {
            return Some(path);
        }
        None
    };
    if let Some(dir) = std::env::var_os("YOOD_BRAVE_DIR") {
        let dir = PathBuf::from(dir);
        if let Some(hit) = consider(dir.join(brave_exe_name())) {
            return Some(hit);
        }
    }
    // Installed layout: `yood` next to `brave/brave` (see scripts/install.sh).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Tauri-bundle layout: resources keep their repo-relative shape
            // (binaries/<platform>/brave) next to the executable; some
            // installers nest them under resources/ instead.
            for candidate in [
                dir.join("brave").join(brave_exe_name()),
                dir.join("resources").join("brave").join(brave_exe_name()),
                dir.join("binaries")
                    .join("windows-x86_64")
                    .join("brave")
                    .join(brave_exe_name()),
                dir.join("binaries")
                    .join("linux-x86_64")
                    .join("brave")
                    .join(brave_exe_name()),
                dir.join("resources")
                    .join("binaries")
                    .join("windows-x86_64")
                    .join("brave")
                    .join(brave_exe_name()),
                dir.join("resources")
                    .join("binaries")
                    .join("linux-x86_64")
                    .join("brave")
                    .join(brave_exe_name()),
            ] {
                if let Some(hit) = consider(candidate) {
                    return Some(hit);
                }
            }
        }
    }
    // Prefer the real binary over distro wrapper scripts (e.g. Arch's
    // /usr/bin/brave, which injects --password-store and user flag files).
    #[cfg(not(windows))]
    for candidate in [
        "/opt/brave-bin/brave",
        "/opt/brave.com/brave/brave",
    ] {
        let candidate = PathBuf::from(candidate);
        if let Some(hit) = consider(candidate) {
            return Some(hit);
        }
    }
    #[cfg(windows)]
    for candidate in [
        "C:\\Program Files\\BraveSoftware\\Brave-Browser\\Application\\brave.exe",
        "C:\\Program Files (x86)\\BraveSoftware\\Brave-Browser\\Application\\brave.exe",
    ] {
        let candidate = PathBuf::from(candidate);
        if let Some(hit) = consider(candidate) {
            return Some(hit);
        }
    }
    if let Some(hit) = ["brave", "brave-browser", "brave-bin"]
        .into_iter()
        .find_map(|name| which::which(name).ok())
    {
        return Some(hit);
    }
    logging::warn(format!(
        "no Brave binary found; searched: {}",
        tried
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    None
}

fn extension_dir() -> Option<PathBuf> {
    // Installed layout first: `yood` next to `extension/yood`.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Some installers nest bundled resources under resources/.
            for candidate in [
                dir.join("extension").join("yood"),
                dir.join("resources").join("extension").join("yood"),
            ] {
                if candidate.join("manifest.json").is_file() {
                    return Some(candidate);
                }
            }
            // Development layout: target/<profile>/yood -> repo/extension.
            // Walk up a few ancestors so `cargo run` / release binaries built
            // from the repo find the companion extension without any setup.
            let mut ancestor = dir.to_path_buf();
            for _ in 0..4 {
                let candidate = ancestor.join("extension").join("yood");
                if candidate.join("manifest.json").is_file() {
                    return Some(candidate);
                }
                if !ancestor.pop() {
                    break;
                }
            }
        }
    }
    // Last resort: the extension is embedded in this binary (always
    // version-locked to it). Materialize it under the app data dir so ANY
    // install layout — NSIS, portable, dev — loads the current version.
    materialize_embedded_extension()
}

/// The companion extension, compiled into the binary so it can never go
/// missing next to the executable (the cause of "download buttons
/// unavailable" on careless installs).
const EXTENSION_MANIFEST: &str = include_str!("../../../shared/extension/yood/manifest.json");
const EXTENSION_CONTENT_JS: &str = include_str!("../../../shared/extension/yood/content.js");
const EXTENSION_RULES_JSON: &str = include_str!("../../../shared/extension/yood/rules.json");

fn materialize_embedded_extension() -> Option<PathBuf> {
    let project = directories::ProjectDirs::from("com", "PlayRood", "Yood")?;
    let dir = project.data_dir().join("yood-extension");
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    // Always (over)write: guarantees the loaded extension matches this
    // binary after updates. Files are tiny; Brave reads them at startup.
    if std::fs::write(dir.join("manifest.json"), EXTENSION_MANIFEST).is_err() {
        return None;
    }
    if std::fs::write(dir.join("content.js"), EXTENSION_CONTENT_JS).is_err() {
        return None;
    }
    // declarativeNetRequest rule set referenced by manifest.json. Older
    // materialized copies predate it; writing unconditionally also heals
    // those installs.
    if std::fs::write(dir.join("rules.json"), EXTENSION_RULES_JSON).is_err() {
        return None;
    }
    Some(dir)
}

fn profile_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "PlayRood", "Yood")
        .map(|project| project.data_dir().join("brave-profile"))
}

/// Seed first-run preferences: no P3A (including its top infobar), no
/// Leo/AI buttons, and — critically — Developer Mode turned on so the
/// companion extension actually stays enabled. Only fills in missing keys —
/// never overrides an existing profile's choices. Keys were verified
/// against the shipped binary; unknown keys would be ignored by Brave, so
/// this can only help, never harm.
fn seed_profile_prefs(profile: &std::path::Path) {
    seed_prefs_file(
        &profile.join("Default").join("Preferences"),
        &[
            (&["brave", "p3a", "enabled"][..], serde_json::Value::Bool(false)),
            (
                &["brave", "p3a", "notice_acknowledged"][..],
                serde_json::Value::Bool(true),
            ),
            (
                &["brave", "ai_chat", "context_menu_enabled"][..],
                serde_json::Value::Bool(false),
            ),
            (
                &["brave", "ai_chat", "show_toolbar_button"][..],
                serde_json::Value::Bool(false),
            ),
            // Chromium (Brave included, current versions) disables any
            // extension loaded via `--load-extension` unless this profile
            // has Developer Mode switched on — command-line-loaded unpacked
            // extensions are otherwise shown as "disabled" in
            // brave://extensions with no error surfaced anywhere else.
            // Yood's companion extension (download buttons, Alt+L bar,
            // in-player ad-skip) is *only* ever loaded this way, so on a
            // brand-new profile it silently does nothing while Brave's own
            // built-in Shields ad-blocking keeps working fine (Shields isn't
            // an extension, so it isn't affected). A profile that happened
            // to have Developer Mode turned on previously (e.g. from earlier
            // manual testing on that machine) would mask this bug there
            // while a fresh profile on another machine or OS would not —
            // this is why it can look like "works on Linux, not on
            // Windows" even though nothing in Yood's own code differs by
            // platform. Seed it explicitly so this never depends on a
            // profile's prior history again.
            (
                &["extensions", "ui", "developer_mode"][..],
                serde_json::Value::Bool(true),
            ),
        ],
    );
    seed_prefs_file(
        &profile.join("Local State"),
        &[
            (&["brave", "p3a", "enabled"][..], serde_json::Value::Bool(false)),
            (
                &["brave", "p3a", "notice_acknowledged"][..],
                serde_json::Value::Bool(true),
            ),
            (
                &["brave", "ai_chat", "context_menu_enabled"][..],
                serde_json::Value::Bool(false),
            ),
            (
                &["brave", "ai_chat", "show_toolbar_button"][..],
                serde_json::Value::Bool(false),
            ),
        ],
    );
    // Shields aggressive for YouTube + Yood's own rules as Brave custom
    // filters. Without this a fresh profile runs Shields in Standard mode,
    // which deliberately leaves first-party YouTube ads (pre/mid-roll)
    // alone — the "ad blocker doesn't work" report. Both seeds only fill
    // in missing keys and never override existing choices.
    seed_shields_aggressive(profile);
    seed_custom_filters(profile);
}

/// Brave Shields aggressive mode for YouTube domains.
///
/// Standard (the Brave default) only blocks third-party ads/trackers;
/// YouTube video ads are first-party, so they sail through. Aggressive
/// applies filter lists to first-party subresources too and hides
/// first-party ad elements. Stored per-site under
/// `profile.content_settings.exceptions` in the profile's Preferences
/// file (keys verified against brave-shields-cli's mapping, which reads
/// and writes the same file): `braveShields=1` (on), `shieldsAds=2`,
/// `trackers=2`, `cosmeticFilteringV2={cosmeticFilteringV2:1}`.
/// Unknown keys would be ignored by Brave, so this can only help.
fn seed_shields_aggressive(profile: &std::path::Path) {
    const DOMAINS: &[&str] = &[
        "youtube.com",
        "www.youtube.com",
        "m.youtube.com",
        "music.youtube.com",
        "youtu.be",
        "youtube-nocookie.com",
    ];
    // Chromium `last_modified`: microseconds since 1601-01-01, as a string.
    let timestamp = {
        const UNIX_TO_CHROMIUM_MICROS: i128 = 11_644_473_600_000_000;
        let micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_micros() as i128)
            .unwrap_or(0);
        (micros + UNIX_TO_CHROMIUM_MICROS).to_string()
    };
    let path = profile.join("Default").join("Preferences");
    let mut data: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(serde_json::Value::Null);
    if !data.is_object() {
        data = serde_json::json!({});
    }
    let mut changed = !path.exists();
    {
        let mut cursor = data.as_object_mut().expect("root object");
        for segment in ["profile", "content_settings", "exceptions"] {
            let entry = cursor
                .entry(segment.to_string())
                .or_insert_with(|| serde_json::json!({}));
            if !entry.is_object() {
                *entry = serde_json::json!({});
            }
            cursor = entry.as_object_mut().expect("nested object");
        }
        for key in ["braveShields", "shieldsAds", "trackers", "cosmeticFilteringV2"] {
            let bucket = cursor
                .entry(key.to_string())
                .or_insert_with(|| serde_json::json!({}));
            if !bucket.is_object() {
                *bucket = serde_json::json!({});
            }
            let map = bucket.as_object_mut().expect("exceptions bucket");
            for domain in DOMAINS {
                let pattern = format!("{domain},*");
                if map.contains_key(&pattern) {
                    continue;
                }
                let value = if key == "braveShields" {
                    serde_json::json!({"last_modified": timestamp, "setting": 1})
                } else if key == "cosmeticFilteringV2" {
                    serde_json::json!({"last_modified": timestamp, "setting": {"cosmeticFilteringV2": 1}})
                } else {
                    serde_json::json!({"last_modified": timestamp, "setting": 2})
                };
                map.insert(pattern, value);
                changed = true;
            }
        }
    }
    if changed {
        if std::fs::create_dir_all(path.parent().expect("profile has parent")).is_ok() {
            let _ = std::fs::write(&path, serde_json::to_string(&data).unwrap_or_default());
        }
    }
}

/// Seed Yood's built-in network/cosmetic rules as Brave custom filters
/// (`brave.ad_block.custom_filters` in Local State, newline-separated —
/// the same store edited by `brave://adblock`). Brave's native adblock
/// engine then enforces them directly, independent of the companion
/// extension. Merges with any existing user filters: missing Yood lines
/// are appended, existing lines (user or previously seeded) are left
/// untouched and never removed.
fn seed_custom_filters(profile: &std::path::Path) {
    const DEFAULT_RULES: &str = include_str!("../../../shared/filters/default.txt");
    let wanted: Vec<&str> = DEFAULT_RULES
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('!') && !line.starts_with('['))
        .collect();
    if wanted.is_empty() {
        return;
    }
    let path = profile.join("Local State");
    let mut data: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(serde_json::Value::Null);
    if !data.is_object() {
        data = serde_json::json!({});
    }
    let mut changed = !path.exists();
    {
        let root = data.as_object_mut().expect("root object");
        let brave = root
            .entry("brave".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !brave.is_object() {
            *brave = serde_json::json!({});
        }
        let ad_block = brave
            .as_object_mut()
            .expect("brave object")
            .entry("ad_block".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !ad_block.is_object() {
            *ad_block = serde_json::json!({});
        }
        let slot = ad_block
            .as_object_mut()
            .expect("ad_block object")
            .entry("custom_filters".to_string())
            .or_insert_with(|| serde_json::Value::String(String::new()));
        let existing = slot.as_str().unwrap_or_default().to_string();
        let mut lines: Vec<String> = existing
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        for rule in &wanted {
            if !lines.iter().any(|line| line == rule) {
                lines.push(rule.to_string());
                changed = true;
            }
        }
        if changed {
            *slot = serde_json::Value::String(lines.join("\n") + "\n");
        }
    }
    if changed {
        if std::fs::create_dir_all(path.parent().expect("profile has parent")).is_ok() {
            let _ = std::fs::write(&path, serde_json::to_string(&data).unwrap_or_default());
        }
    }
}

/// Sets each `(nested.key.path, value)` pair only if that key is not
/// already present, leaving any existing user choice untouched. Missing
/// intermediate objects along the path are created as needed.
fn seed_prefs_file(path: &std::path::Path, entries: &[(&[&str], serde_json::Value)]) {
    let mut data: serde_json::Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(serde_json::Value::Null);
    if !data.is_object() {
        data = serde_json::json!({});
    }
    let mut changed = !path.exists();
    for (segments, value) in entries {
        let Some((key, parents)) = segments.split_last() else {
            continue;
        };
        let mut cursor = data.as_object_mut().expect("root object");
        for segment in parents {
            let entry = cursor
                .entry(segment.to_string())
                .or_insert_with(|| serde_json::json!({}));
            if !entry.is_object() {
                *entry = serde_json::json!({});
            }
            cursor = entry.as_object_mut().expect("nested object");
        }
        if !cursor.contains_key(*key) {
            cursor.insert(key.to_string(), value.clone());
            changed = true;
        }
    }
    if changed {
        if std::fs::create_dir_all(path.parent().expect("profile has parent")).is_ok() {
            let _ = std::fs::write(path, serde_json::to_string(&data).unwrap_or_default());
        }
    }
}

/// Windows-only: nothing to do here anymore.
///
/// History: Yood used to strip Brave menu entries (VPN, Talk) by writing
/// per-user (HKCU) enterprise policies via `reg.exe`. That approach had
/// three problems, all reported as bugs:
/// 1. Spawning `reg.exe` from a GUI app flashes a console window on every
///    launch (unless CREATE_NO_WINDOW is set, which it wasn't).
/// 2. When the write failed for any reason it logged
///    `WARN could not write Brave policy ...`, so users saw a terminal
///    pop up just to tell them a menu entry stayed.
/// 3. Registry policies are browser-wide: they leak out of Yood's isolated
///    profile and affect the user's regular Brave install too.
///
/// The same debloat is already achieved per-launch (and scoped to Yood
/// only) via `--disable-features=BraveWallet,BraveVPN,AIChat,...` in
/// `launch()` below, which needs no registry, no console window, and can
/// never fail with a WARN. So there is intentionally no registry write
/// here at all. Kept as an empty stub so call sites and `cfg` gates don't
/// need to change.
#[cfg(windows)]
fn apply_windows_policies() {}

fn has_active_downloads(downloads: &crate::download::DownloadManager) -> bool {    downloads.list().iter().any(|item| {
        matches!(
            item.status,
            DownloadStatus::Queued | DownloadStatus::Downloading | DownloadStatus::Paused
        )
    })
}

/// Launch bundled (or system) Brave in Yood app mode and supervise it.
/// Returns an error (aborting startup) when no Brave binary can be found.
pub fn launch(app: &tauri::AppHandle, runtime: &RuntimeState) -> Result<(), String> {
    let brave = bundled_or_system_brave().ok_or_else(|| {
        "no Brave browser found (set YOOD_BRAVE_DIR, stage one with scripts/fetch-brave.sh, or install brave-bin)".to_string()
    })?;
    let profile = profile_dir()
        .ok_or_else(|| "could not determine app directories".to_string())?;
    if std::fs::create_dir_all(&profile).is_err() {
        return Err("could not create Brave profile directory".to_string());
    }
    seed_profile_prefs(&profile);
    #[cfg(windows)]
    apply_windows_policies();
    let url = startup_url().unwrap_or_else(|| "https://www.youtube.com".into());
    let mut args: Vec<String> = vec![
        format!("--app={url}"),
        format!("--user-data-dir={}", profile.display()),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-sync".into(),
        "--disable-breakpad".into(),
        "--no-report-upload".into(),
        "--disable-domain-reliability".into(),
        "--disable-features=BraveWallet,BraveVPN,AIChat,Translate,DialMediaRouteProvider".into(),
        "--renderer-process-limit=4".into(),
        "--disable-notifications".into(),
        "--deny-permission-prompts".into(),
    ];
    #[cfg(target_os = "linux")]
    args.push("--class=yood".into());
    if let Some(extension) = extension_dir() {
        logging::info(format!(
            "Yood download buttons: companion extension at {}",
            extension.display()
        ));
        args.push(format!("--load-extension={}", extension.display()));
    } else {
        logging::warn(
            "companion extension not found next to the binary; download buttons unavailable",
        );
    }
    logging::info(format!(
        "Yood {} launching Brave {} ({})",
        env!("CARGO_PKG_VERSION"),
        brave.display(),
        std::env::consts::OS
    ));
    let mut cmd = Command::new(&brave);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Windows GUI apps: never flash a console window for the child.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW = 0x08000000
        cmd.creation_flags(0x08000000);
    }
    let child = cmd
        .spawn()
        .map_err(|error| format!("could not launch Brave ({}): {error}", brave.display()))?;
    *BRAVE_CHILD
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = Some(child);
    let handle = app.clone();
    let downloads = runtime.downloads.clone();
    std::thread::spawn(move || {
        // Poll the child: it stays owned by the global slot so the Exit
        // hook can take and kill it when Yood itself is asked to quit.
        loop {
            let exited = BRAVE_CHILD
                .lock()
                .map(|mut slot| match slot.as_mut() {
                    Some(child) => matches!(child.try_wait(), Ok(Some(_))),
                    // Taken by shutdown(): Brave is going away.
                    None => true,
                })
                .unwrap_or(true);
            if exited {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        *BRAVE_CHILD
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = None;
        // Brave closed: keep serving active downloads headless, then exit.
        logging::info("Brave exited; finishing active downloads before quitting");
        loop {
            if !has_active_downloads(&downloads) {
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        handle.exit(0);
    });
    Ok(())
}

/// Kill the supervised Brave process, if any (Yood is quitting).
pub fn shutdown() {
    let mut slot = BRAVE_CHILD.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some(mut child) = slot.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_profile(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("yood-browser-test-{tag}-{nanos}"))
    }

    #[test]
    fn seeds_aggressive_shields_without_overriding() {
        let profile = temp_profile("shields");
        seed_shields_aggressive(&profile);
        let text = std::fs::read_to_string(profile.join("Default").join("Preferences"))
            .expect("preferences seeded");
        let data: serde_json::Value = serde_json::from_str(&text).expect("valid prefs");
        let exceptions = data
            .pointer("/profile/content_settings/exceptions")
            .expect("exceptions seeded");
        // Aggressive = shieldsAds/trackers 2 + cosmetic inner value 1.
        let ads = exceptions
            .pointer("/shieldsAds")
            .and_then(|value| value.as_object())
            .expect("shieldsAds bucket");
        let entry = ads.get("youtube.com,*").expect("youtube pattern");
        assert_eq!(entry.get("setting").and_then(|v| v.as_i64()), Some(2));
        let cosmetic = exceptions
            .pointer("/cosmeticFilteringV2")
            .and_then(|value| value.as_object())
            .expect("cosmetic bucket");
        let centry = cosmetic.get("www.youtube.com,*").expect("www pattern");
        assert_eq!(
            centry
                .pointer("/setting/cosmeticFilteringV2")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
        // Existing user choice is never overridden.
        let prefs_path = profile.join("Default").join("Preferences");
        let mut current: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&prefs_path).expect("prefs"),
        )
        .expect("json");
        current["profile"]["content_settings"]["exceptions"]["shieldsAds"]["youtube.com,*"] =
            serde_json::json!({"last_modified": "1", "setting": 1});
        std::fs::write(&prefs_path, serde_json::to_string(&current).unwrap()).unwrap();
        seed_shields_aggressive(&profile);
        let again: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&prefs_path).expect("prefs"),
        )
        .expect("json");
        assert_eq!(
            again
                .pointer("/profile/content_settings/exceptions/shieldsAds")
                .and_then(|bucket| bucket.as_object())
                .and_then(|bucket| bucket.get("youtube.com,*"))
                .and_then(|entry| entry.get("setting"))
                .and_then(|value| value.as_i64()),
            Some(1),
            "user override must survive re-seeding"
        );
        let _ = std::fs::remove_dir_all(&profile);
    }

    #[test]
    fn merges_custom_filters_without_duplicates() {
        let profile = temp_profile("filters");
        seed_custom_filters(&profile);
        let first = std::fs::read_to_string(profile.join("Local State")).expect("local state");
        let data: serde_json::Value = serde_json::from_str(&first).expect("json");
        let filters = data
            .pointer("/brave/ad_block/custom_filters")
            .and_then(|value| value.as_str())
            .expect("custom filters seeded");
        assert!(filters.contains("||doubleclick.net^"));
        assert!(filters.contains("youtube.com##ytd-ad-slot-renderer"));
        let count = filters.lines().filter(|line| line.trim() == "||doubleclick.net^").count();
        assert_eq!(count, 1);
        // Re-seeding must not duplicate, and user lines must survive.
        let path = profile.join("Local State");
        let mut current: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("state")).expect("json");
        let slot = current
            .pointer_mut("/brave/ad_block/custom_filters")
            .expect("slot");
        *slot = serde_json::Value::String(format!("{}||user-example.test^\n", slot.as_str().unwrap_or("")));
        std::fs::write(&path, serde_json::to_string(&current).unwrap()).unwrap();
        seed_custom_filters(&profile);
        let again = std::fs::read_to_string(&path).expect("state");
        let data: serde_json::Value = serde_json::from_str(&again).expect("json");
        let merged = data
            .pointer("/brave/ad_block/custom_filters")
            .and_then(|value| value.as_str())
            .expect("merged");
        assert!(merged.contains("||user-example.test^"), "user filter survives");
        assert_eq!(
            merged.lines().filter(|line| line.trim() == "||doubleclick.net^").count(),
            1,
            "no duplicates after re-seed"
        );
        let _ = std::fs::remove_dir_all(&profile);
    }

    #[test]
    fn embedded_extension_ships_its_network_rules() {
        assert!(EXTENSION_MANIFEST.contains("rules.json"));
        let rules: serde_json::Value =
            serde_json::from_str(EXTENSION_RULES_JSON).expect("rules.json parses");
        assert!(rules.as_array().map(|rules| !rules.is_empty()).unwrap_or(false));
        let text = EXTENSION_RULES_JSON.to_ascii_lowercase();
        for cdn in ["googlevideo.com", "ytimg.com", "gstatic.com"] {
            assert!(!text.contains(cdn), "rules must never block {cdn}");
        }
    }
}
