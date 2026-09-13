use std::{
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tauri::{webview::Color, AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use url::Url;

use crate::{
    desktop,
    download::DownloadManager,
    error::{AppError, AppResult},
    filtering::FilterManager,
    models::{
        AppInfo, DownloadItem, DownloadRequest, FilterScriptRules, FilterStatus, InspectResult,
        Settings, SettingsPatch,
    },
    process::BinaryResolver,
    secure,
    security::validate_user_path,
    settings, updates,
};

#[derive(Clone)]
pub struct RuntimeState {
    pub settings: Arc<Mutex<Settings>>,
    pub downloads: DownloadManager,
    pub filters: FilterManager,
    pub resolver: BinaryResolver,
    pub pending_player: Arc<Mutex<Option<PlayerPayload>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerPayload {
    pub source: String,
    pub title: String,
}

impl RuntimeState {
    pub fn new(app: &tauri::App) -> AppResult<Self> {
        let settings = Arc::new(Mutex::new(settings::load()?));
        let project = directories::ProjectDirs::from("com", "PlayRood", "Yood")
            .ok_or_else(|| AppError::Configuration("could not determine app directories".into()))?;
        let resolver = BinaryResolver::new(
            app.path().resource_dir().ok(),
            app.path().app_data_dir().ok(),
        );
        let filters = FilterManager::new(project.cache_dir().join("filters"))?;
        let downloads = DownloadManager::new(settings.clone(), resolver.clone())?;
        downloads.set_app_handle(app.handle().clone());
        Ok(Self {
            settings,
            downloads,
            filters,
            resolver,
            pending_player: Arc::new(Mutex::new(None)),
        })
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, RuntimeState>) -> AppResult<Settings> {
    state
        .settings
        .lock()
        .map(|settings| settings.clone())
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, RuntimeState>,
    patch: SettingsPatch,
) -> AppResult<Settings> {
    let mut current = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))?;
    settings::apply_patch(&mut current, patch)?;
    crate::logging::set_level(&current.log_level);
    Ok(current.clone())
}

#[tauri::command]
pub fn list_downloads(state: State<'_, RuntimeState>) -> Vec<DownloadItem> {
    state.downloads.list()
}

#[tauri::command]
pub fn enqueue_download(
    state: State<'_, RuntimeState>,
    request: DownloadRequest,
) -> AppResult<DownloadItem> {
    state.downloads.enqueue(request)
}

#[tauri::command]
pub fn pause_download(state: State<'_, RuntimeState>, id: String) -> AppResult<()> {
    state.downloads.pause(&id)
}

#[tauri::command]
pub fn resume_download(state: State<'_, RuntimeState>, id: String) -> AppResult<()> {
    state.downloads.resume(&id)
}

#[tauri::command]
pub fn cancel_download(state: State<'_, RuntimeState>, id: String) -> AppResult<()> {
    state.downloads.cancel(&id)
}

#[tauri::command]
pub fn retry_download(state: State<'_, RuntimeState>, id: String) -> AppResult<()> {
    state.downloads.retry(&id)
}

#[tauri::command]
pub fn remove_download(state: State<'_, RuntimeState>, id: String) -> AppResult<()> {
    state.downloads.remove(&id)
}

#[tauri::command]
pub async fn inspect_url(state: State<'_, RuntimeState>, url: String) -> AppResult<InspectResult> {
    state.downloads.inspect(&url).await
}

#[tauri::command]
pub async fn get_app_info(state: State<'_, RuntimeState>) -> AppResult<AppInfo> {
    let filter = state
        .settings
        .lock()
        .map(|settings| state.filters.status(&settings))
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))?;
    let ytdlp = state.resolver.status("yt-dlp").await;
    let ffmpeg = state.resolver.status("ffmpeg").await;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        binaries: vec![ytdlp, ffmpeg],
        filter,
    })
}

#[tauri::command]
pub fn get_filter_status(state: State<'_, RuntimeState>) -> AppResult<FilterStatus> {
    state
        .settings
        .lock()
        .map(|settings| state.filters.status(&settings))
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))
}

#[tauri::command]
pub async fn get_filter_script_rules(
    state: State<'_, RuntimeState>,
    page_url: String,
    classes: Vec<String>,
    ids: Vec<String>,
) -> AppResult<FilterScriptRules> {
    // Async so the (potentially large) cosmetic matching over the full
    // EasyList-based engine never stalls the WebView's main thread on every
    // YouTube navigation. The frontend already awaits this as a promise.
    let filters = state.filters.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let parsed = Url::parse(&page_url)
            .map_err(|_| AppError::InvalidInput("invalid filter page URL".into()))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || page_url.len() > 16 * 1024
        {
            return Err(AppError::InvalidInput("invalid filter page URL".into()));
        }
        let classes = classes
            .into_iter()
            .filter(|value| is_safe_dom_token(value))
            .take(2_500)
            .collect::<Vec<_>>();
        let ids = ids
            .into_iter()
            .filter(|value| is_safe_dom_token(value))
            .take(2_500)
            .collect::<Vec<_>>();
        Ok(filters.script_rules(&page_url, &classes, &ids))
    })
    .await
    .map_err(|_| AppError::Internal("filter worker failed".into()))?
}

#[tauri::command]
pub async fn check_filter_request(
    state: State<'_, RuntimeState>,
    url: String,
    source_url: String,
    request_type: String,
) -> AppResult<bool> {
    // Async for the same reason: the page can fire these on almost every
    // subresource, and they must never queue behind (or stall) UI work.
    check_filter_request_sync(&state, &url, &source_url, &request_type)
}

fn check_filter_request_sync(
    state: &State<'_, RuntimeState>,
    url: &str,
    source_url: &str,
    request_type: &str,
) -> AppResult<bool> {
    if url.len() > 16 * 1024
        || source_url.len() > 16 * 1024
        || request_type.len() > 32
        || url.chars().any(|character| character.is_control())
        || source_url.chars().any(|character| character.is_control())
    {
        return Ok(false);
    }
    let parsed = match Url::parse(&url) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https" | "ws" | "wss") => parsed,
        _ => return Ok(false),
    };
    if parsed.host_str().is_none() {
        return Ok(false);
    }
    Ok(state
        .filters
        .check_network_request(&url, &source_url, &request_type))
}

fn is_safe_dom_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .chars()
            .all(|character| !character.is_control() && !matches!(character, '{' | '}' | ';'))
}

#[tauri::command]
pub fn tracking_protection_enabled(state: State<'_, RuntimeState>) -> AppResult<bool> {
    state
        .settings
        .lock()
        .map(|settings| settings.tracking_protection)
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))
}

#[tauri::command]
pub async fn get_system_appearance() -> String {
    // Desktop-theme lookup may invoke gsettings/reg.exe.  Keep that blocking
    // OS work off Tauri's UI/event thread so a theme transition never stalls
    // the WebView.
    tauri::async_runtime::spawn_blocking(detect_system_appearance)
        .await
        .unwrap_or_else(|_| "system".into())
}

#[tauri::command]
pub fn get_desktop_environment() -> String {
    desktop::detect().identifier().into()
}

fn detect_system_appearance() -> String {
    #[cfg(target_os = "linux")]
    {
        let executable = if std::path::Path::new("/usr/bin/gsettings").is_file() {
            "/usr/bin/gsettings"
        } else {
            "gsettings"
        };
        for key in ["color-scheme", "gtk-theme"] {
            let output = Command::new(executable)
                .args(["get", "org.gnome.desktop.interface", key])
                .output();
            if let Ok(output) = output {
                let value = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
                if value.contains("dark") || value.contains("prefer-dark") {
                    return "dark".into();
                }
                if value.contains("light") || value.contains("prefer-light") {
                    return "light".into();
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("reg.exe");
        cmd.args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ]);
        // Theme lookup must never flash a console window.
        // CREATE_NO_WINDOW = 0x08000000
        cmd.creation_flags(0x08000000);
        if let Ok(output) = cmd.output() {
            let value = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
            if value.contains("0x0") {
                return "dark".into();
            }
            if value.contains("0x1") {
                return "light".into();
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
        {
            if String::from_utf8_lossy(&output.stdout)
                .to_ascii_lowercase()
                .contains("dark")
            {
                return "dark".into();
            }
        }
    }
    // Let the WebView's prefers-color-scheme result decide when the native
    // desktop integration is unavailable or inconclusive.
    "system".into()
}

#[tauri::command]
pub async fn update_filter_lists(state: State<'_, RuntimeState>) -> AppResult<FilterStatus> {
    let mut candidate = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))?
        .clone();
    let updated = state.filters.update_if_due(&mut candidate, true).await?;
    *state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))? = candidate.clone();
    if updated {
        crate::logging::info("filter lists updated manually");
    }
    Ok(state.filters.status(&candidate))
}

#[tauri::command]
pub async fn update_ytdlp(state: State<'_, RuntimeState>) -> AppResult<String> {
    let version = updates::update_ytdlp(&state.resolver).await?;
    if let Ok(mut settings) = state.settings.lock() {
        settings.ytdlp_last_update = Some(chrono::Utc::now());
        let _ = settings::save(&settings);
    }
    crate::logging::info(format!("yt-dlp updated to {version}"));
    Ok(version)
}

#[tauri::command]
pub async fn get_account_hints() -> AppResult<Option<String>> {
    // Keyring access talks to the Secret Service over D-Bus and can block;
    // keep it off the WebView thread like the other filter commands.
    tauri::async_runtime::spawn_blocking(secure::get_account_hints)
        .await
        .map_err(|_| AppError::Internal("secure storage worker failed".into()))?
}

#[tauri::command]
pub async fn set_account_hints(value: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || secure::set_account_hints(&value))
        .await
        .map_err(|_| AppError::Internal("secure storage worker failed".into()))?
}

#[tauri::command]
pub async fn clear_account_hints() -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(secure::clear_account_hints)
        .await
        .map_err(|_| AppError::Internal("secure storage worker failed".into()))?
}

#[tauri::command]
pub fn open_download(app: AppHandle, state: State<'_, RuntimeState>, id: String) -> AppResult<()> {
    let item = state
        .downloads
        .list()
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound(id.clone()))?;
    let path = item
        .output_path
        .ok_or_else(|| AppError::NotFound("download has no output file".into()))?;
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))?
        .clone();
    let roots: [&std::path::Path; 2] = [
        settings.download_directory.as_path(),
        settings.temp_directory.as_path(),
    ];
    let path = validate_user_path(&path, &roots)?;
    open_path(&path, false)?;
    let _ = app;
    Ok(())
}

#[tauri::command]
pub fn open_download_folder(state: State<'_, RuntimeState>, id: String) -> AppResult<()> {
    let item = state
        .downloads
        .list()
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound(id.clone()))?;
    let path = item
        .output_path
        .ok_or_else(|| AppError::NotFound("download has no output file".into()))?;
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))?
        .clone();
    let roots: [&std::path::Path; 2] = [
        settings.download_directory.as_path(),
        settings.temp_directory.as_path(),
    ];
    let path = validate_user_path(&path, &roots)?;
    open_path(
        path.parent()
            .ok_or_else(|| AppError::InvalidInput("download has no parent folder".into()))?,
        false,
    )
}

#[tauri::command]
pub fn open_external(url: String) -> AppResult<()> {
    let parsed =
        url::Url::parse(&url).map_err(|_| AppError::InvalidInput("invalid external URL".into()))?;
    if parsed.scheme() != "https" {
        return Err(AppError::UnsupportedUrl(
            "only HTTPS external links are allowed".into(),
        ));
    }
    open_path(PathBuf::from(parsed.as_str()).as_path(), true)
}

#[tauri::command]
pub fn open_native_player(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    id: String,
) -> AppResult<()> {
    let item = state
        .downloads
        .list()
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound(id.clone()))?;
    let path = item.output_path.ok_or_else(|| {
        AppError::NotFound("native player requires a completed local download".into())
    })?;
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))?
        .clone();
    let roots: [&std::path::Path; 2] = [
        settings.download_directory.as_path(),
        settings.temp_directory.as_path(),
    ];
    let path = validate_user_path(&path, &roots)?;
    let source = Url::from_file_path(path)
        .map(|url| url.to_string())
        .map_err(|_| AppError::InvalidInput("could not convert media path".into()))?;
    let payload = PlayerPayload {
        source,
        title: player_title(&item.title),
    };
    open_player_window(&app, &state, payload)
}

#[tauri::command]
pub async fn open_native_player_url(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    url: String,
    title: String,
) -> AppResult<()> {
    if title.len() > 2048 || title.chars().any(char::is_control) {
        return Err(AppError::InvalidInput("invalid player title".into()));
    }
    let source = state.downloads.resolve_player_source(&url).await?;
    let payload = PlayerPayload {
        source,
        title: player_title(&title),
    };
    open_player_window(&app, &state, payload)
}

fn open_player_window(
    app: &AppHandle,
    state: &RuntimeState,
    payload: PlayerPayload,
) -> AppResult<()> {
    *state
        .pending_player
        .lock()
        .map_err(|_| AppError::Internal("player state lock poisoned".into()))? =
        Some(payload.clone());
    if let Some(window) = app.get_webview_window("native-player") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("player-load", &payload);
        return Ok(());
    }
    WebviewWindowBuilder::new(app, "native-player", WebviewUrl::App("player.html".into()))
        .title("Yood Native Player")
        .inner_size(960.0, 600.0)
        .min_inner_size(480.0, 320.0)
        .decorations(!desktop::detect().prefers_borderless_window())
        .background_color(Color(15, 15, 15, 255))
        // Same minimal-browser policy as the main window: no DevTools.
        .devtools(false)
        .build()
        .map_err(|error| AppError::Internal(format!("could not create native player: {error}")))?;
    Ok(())
}

fn player_title(title: &str) -> String {
    let title = title
        .chars()
        .filter(|character| !character.is_control())
        .take(512)
        .collect::<String>();
    if title.trim().is_empty() {
        "Yood Native Player".into()
    } else {
        title
    }
}

#[tauri::command]
pub fn take_player_source(state: State<'_, RuntimeState>) -> AppResult<Option<PlayerPayload>> {
    state
        .pending_player
        .lock()
        .map(|mut payload| payload.take())
        .map_err(|_| AppError::Internal("player state lock poisoned".into()))
}

#[tauri::command]
pub fn resolve_media_path(state: State<'_, RuntimeState>, path: PathBuf) -> AppResult<String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock poisoned".into()))?
        .clone();
    let roots: [&std::path::Path; 2] = [
        settings.download_directory.as_path(),
        settings.temp_directory.as_path(),
    ];
    let safe = validate_user_path(&path, &roots)?;
    url::Url::from_file_path(safe)
        .map(|url| url.to_string())
        .map_err(|_| AppError::InvalidInput("could not convert media path".into()))
}

fn open_path(path: &std::path::Path, external_url: bool) -> AppResult<()> {
    if external_url {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let mut cmd = Command::new("rundll32.exe");
            cmd.args([
                "url.dll,FileProtocolHandler",
                path.to_string_lossy().as_ref(),
            ]);
            // CREATE_NO_WINDOW = 0x08000000
            cmd.creation_flags(0x08000000);
            cmd.spawn()
                .map_err(|error| AppError::Process(error.to_string()))?;
        }
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|error| AppError::Process(error.to_string()))?;
        }
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("explorer.exe");
        cmd.arg(path);
        // CREATE_NO_WINDOW = 0x08000000
        cmd.creation_flags(0x08000000);
        cmd.spawn()
            .map_err(|error| AppError::Process(error.to_string()))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| AppError::Process(error.to_string()))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|error| AppError::Process(error.to_string()))?;
    }
    Ok(())
}
