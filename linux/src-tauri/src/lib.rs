mod browser;
mod commands;
mod desktop;
mod download;
mod error;
mod filtering;
mod logging;
mod models;
mod process;
mod secure;
mod security;
mod settings;
mod updates;

use commands::RuntimeState;
use security::is_supported_navigation;
use tauri::{webview::Color, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use url::Url;

const INITIALIZATION: &str = concat!(
    include_str!("../../../shared/frontend/dist/bootstrap.js"),
    "\n",
    include_str!("../../../shared/frontend/dist/injection.js")
);

pub fn run() {
    let startup_settings = settings::load().unwrap_or_default();
    logging::init(&startup_settings.log_level);
    if std::env::args()
        .skip(1)
        .any(|argument| argument == "--self-test")
    {
        if let Err(error) = self_test() {
            logging::error(format!("self-test failed: {error}"));
            eprintln!("Yood self-test failed: {error}");
            std::process::exit(1);
        }
        println!("Yood self-test OK");
        return;
    }
    if std::env::args()
        .skip(1)
        .any(|argument| argument == "--quit")
    {
        // A second `yood --quit` is forwarded to the running backend by the
        // single-instance plugin, whose callback exits it; a first instance
        // with --quit simply has nothing to do.
        return;
    }
    // Brave-first: the embedded WebKit view stays only behind YOOD_WEBVIEW=1
    // (it segfaults on YouTube player pages on affected systems).
    let webview = browser::is_webview_mode();
    if webview {
        #[cfg(target_os = "linux")]
        apply_webkit_rendering_workarounds();
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().skip(1).any(|argument| argument == "--quit") {
                app.exit(0);
                return;
            }
            // A second `yood <url>` invocation (or the OS re-launching Yood
            // because it is already the registered handler) lands here
            // instead of spawning a competing process/WebView.
            focus_and_navigate(app, argv.into_iter().skip(1));
        }))
        .plugin(tauri_plugin_deep_link::init())
        .setup(move |app| {
            {
                // Best-effort: makes the yood:// scheme work even when Yood
                // was launched from an AppImage that a user has not run
                // through a desktop-integration tool. Failure here (for
                // example xdg-mime being unavailable) must not stop startup.
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    focus_and_navigate(&handle, event.urls().into_iter().map(|url| url.to_string()));
                });
            }
            if webview {
                configure_linux_gstreamer(app);
            }
            let runtime = RuntimeState::new(app)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            logging::set_level(
                &runtime
                    .settings
                    .lock()
                    .map(|value| value.log_level.clone())
                    .unwrap_or_default(),
            );
            logging::info(format!(
                "Yood {} starting ({})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS
            ));
            let filters = runtime.filters.clone();
            let settings = runtime.settings.clone();
            runtime.downloads.set_app_handle(app.handle().clone());
            app.manage(runtime.clone());
            // Cold start with yood://download?url=... (companion extension):
            // enqueue before the window even appears.
            for argument in std::env::args().skip(1) {
                enqueue_download_url(app.handle(), &argument);
            }
            // In browser mode Brave Shields (seeded aggressive for YouTube,
            // plus Yood's rules as Brave custom filters) together with the
            // companion extension (network rules, cosmetic hiding,
            // player-response stripping, in-player skip) owns ad blocking,
            // and Shields' lists update themselves — so the Rust filter
            // updater stays off. Less CPU/RAM, no duplicate work.
            if webview {
            let update_filters = filters.clone();
            tauri::async_runtime::spawn(async move {
                let mut candidate = match settings.lock() {
                    Ok(value) => value.clone(),
                    Err(_) => return,
                };
                match update_filters
                    .update_if_due(&mut candidate, false)
                    .await
                {
                    Ok(true) => logging::info("filter lists refreshed"),
                    Ok(false) => logging::debug("filter lists are current; no refresh needed"),
                    Err(error) => logging::warn(format!("filter list refresh failed: {error}")),
                }
                if let Ok(mut current) = settings.lock() {
                    *current = candidate;
                }
            });
            }
            if webview {
            let destination = startup_url().unwrap_or_else(|| "https://www.youtube.com".into());
            let desktop = desktop::detect();
            // Diagnostic knob, not a feature: YOOD_NO_HOOKS=1 runs pure
            // YouTube with no Yood scripts and no native request filter.
            // Used to bisect UI-process crashes (hooks vs. WebKit itself).
            let no_hooks = std::env::var_os("YOOD_NO_HOOKS").is_some();
            if no_hooks {
                logging::warn("YOOD_NO_HOOKS=1: running without ad blocking or integration scripts");
            }
            let mut builder = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(Url::parse(&destination).expect("valid startup URL")),
            )
            .title("Yood")
            .inner_size(1280.0, 800.0)
            .min_inner_size(800.0, 500.0)
            .decorations(!desktop.prefers_borderless_window())
            // Minimal browser: DevTools / web inspector are always off,
            // even in debug builds. Release builds already lack the
            // `devtools` Cargo feature, this also locks debug builds.
            .devtools(false)
            // The external YouTube document is white until WebKit has parsed
            // its first stylesheet. Keep the native surface dark so a dark
            // desktop never receives a full-window white flash on startup.
            .background_color(Color(15, 15, 15, 255));
            // Diagnostic override: YOOD_UA_EMPTY=1 sends WebKit's stock user
            // agent (YouTube then serves its conservative fallback layout),
            // YOOD_UA=<string> sends a custom one. Used to bisect whether a
            // served code path crashes this WebKitGTK build.
            //
            // WebKitGTK's default user agent carries no recognizable browser
            // name/version token (just "AppleWebKit/605.1.15 (KHTML, like
            // Gecko)"), which YouTube's frontend can't confidently classify
            // and falls back to a more conservative layout for - notably
            // fewer grid columns on the home/subscriptions feed than a
            // Chromium browser (Chrome, Brave, Edge) gets on the same
            // window width. Presenting as a recent desktop Chrome build
            // gives Yood the same modern, full-width layout Brave shows,
            // since Brave itself reports as Chrome under the hood.
            let user_agent: Option<String> =
                if std::env::var_os("YOOD_UA_EMPTY").is_some() {
                    None
                } else if let Ok(custom) = std::env::var("YOOD_UA") {
                    Some(custom)
                } else {
                    Some("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".to_string())
                };
            if let Some(user_agent) = user_agent {
                builder = builder.user_agent(&user_agent);
            }
            let builder = if no_hooks {
                builder
            } else {
                builder.initialization_script(INITIALIZATION)
            };
            let window = builder
            .on_navigation(is_supported_navigation)
            .build()?;
            if !no_hooks {
                install_native_filter(&window, filters)?;
            }
            let _ = window.set_focus();
            } else {
                browser::launch(app.handle(), &runtime)
                    .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::list_downloads,
            commands::enqueue_download,
            commands::pause_download,
            commands::resume_download,
            commands::cancel_download,
            commands::retry_download,
            commands::remove_download,
            commands::inspect_url,
            commands::get_app_info,
            commands::get_filter_status,
            commands::get_filter_script_rules,
            commands::check_filter_request,
            commands::tracking_protection_enabled,
            commands::get_system_appearance,
            commands::get_desktop_environment,
            commands::update_filter_lists,
            commands::update_ytdlp,
            commands::get_account_hints,
            commands::set_account_hints,
            commands::clear_account_hints,
            commands::open_download,
            commands::open_download_folder,
            commands::open_external,
            commands::open_native_player,
            commands::open_native_player_url,
            commands::take_player_source,
            commands::resolve_media_path,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Yood")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                browser::shutdown();
                if let Some(state) = app.try_state::<RuntimeState>() {
                    tauri::async_runtime::block_on(state.downloads.shutdown_async());
                }
            }
        });
}

#[cfg(target_os = "linux")]
fn apply_webkit_rendering_workarounds() {
    // Must run before GTK/WebKitGTK initialize anything - that happens as
    // soon as the Tauri builder registers its plugins, not just when the
    // WebView itself is constructed. Setting these later (e.g. inside the
    // `.setup()` closure) can be too late on some setups.
    //
    // WebKitGTK's DMABUF renderer can leave the page blank, freeze it, or -
    // per Tauri's own Linux graphics guidance - cause it to die outright on
    // resize with no useful error, on a range of AMD/Intel/NVIDIA and
    // Wayland/X11 combinations (tauri-apps/tauri#9394). Yood disables it by
    // default.
    //
    // Accelerated compositing itself stays ON by default: turning it off as
    // well forces full software rendering and makes YouTube dramatically
    // slower (and heavier on CPU) on machines whose GPU path is otherwise
    // fine. If the page is still unstable, opt into full software rendering
    // with YOOD_SOFTWARE_RENDERING=1, which also sets
    // WEBKIT_DISABLE_COMPOSITING_MODE=1.
    //
    // Both are skipped if the user has already set them, so this stays
    // fully overridable: YOOD_FORCE_GPU_RENDERER=1 turns hardware
    // compositing back on for machines that don't hit these bugs, and
    // anyone who wants a *different* combination can still export
    // WEBKIT_DISABLE_DMABUF_RENDERER / WEBKIT_DISABLE_COMPOSITING_MODE
    // themselves before launching Yood.
    if std::env::var_os("YOOD_FORCE_GPU_RENDERER").is_none() {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if std::env::var_os("YOOD_SOFTWARE_RENDERING").is_some()
            && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none()
        {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }
}

#[cfg(target_os = "linux")]
fn configure_linux_gstreamer(app: &tauri::App) {
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|path| path.join("gstreamer"));
    let development = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("gstreamer");
    let Some(plugin_dir) = [bundled, Some(development)]
        .into_iter()
        .flatten()
        .find(|path| path.join("libgstautodetect.so").is_file())
    else {
        return;
    };
    let existing = std::env::var_os("GST_PLUGIN_PATH");
    let value = match existing {
        Some(existing) => {
            let mut paths = std::env::split_paths(&existing).collect::<Vec<_>>();
            paths.insert(0, plugin_dir);
            std::env::join_paths(paths).ok()
        }
        None => Some(plugin_dir.into_os_string()),
    };
    if let Some(value) = value {
        // This must happen before the WebView is constructed; WebKit initializes
        // GStreamer lazily when the first media-capable page is created.
        std::env::set_var("GST_PLUGIN_PATH", value);
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_gstreamer(_app: &tauri::App) {}

#[cfg(target_os = "linux")]
fn install_native_filter(
    window: &WebviewWindow,
    filters: filtering::FilterManager,
) -> tauri::Result<()> {
    window.with_webview(move |platform| {
        use webkit2gtk::glib::Cast;
        use webkit2gtk::{
            NavigationPolicyDecisionExt, PolicyDecisionExt, PolicyDecisionType, URIRequestExt,
            WebViewExt,
        };

        let webview = platform.inner();
        // Block at the NavigationAction stage (before the request is sent),
        // matching how GNOME's Epiphany performs request-level blocking.
        // Matched ad subframes are redirected to about:blank and the
        // navigation completes normally. Cancelling them with
        // `decision.ignore()` instead is what drives WebKit's
        // `WebLoaderStrategy::internallyFailedLoadTimerFired` retry loop -
        // the network process keeps retrying loads cancelled in flight -
        // and that path has been observed to destabilise the UI process on
        // watch pages, so it is deliberately avoided here.
        //
        // NavigationAction only sees document navigations. Main-frame
        // subresources (notably ad scripts such as Google IMA, which load
        // via <script> on watch pages) are handled at the Response stage
        // below with the same redirect-to-blank pattern.
        webview.connect_decide_policy(move |webview, decision, decision_type| {
            if decision_type == PolicyDecisionType::Response {
                return filter_response(webview, decision, &filters);
            }
            if decision_type != PolicyDecisionType::NavigationAction {
                return false;
            }
            let Some(navigation_decision) =
                decision.downcast_ref::<webkit2gtk::NavigationPolicyDecision>()
            else {
                return false;
            };
            let mut action = match navigation_decision.navigation_action() {
                Some(action) => action,
                None => return false,
            };
            // A main-frame navigation has no frame name; those are always
            // allowed (Yood is a YouTube-only window). Only subframe
            // navigations - typically ad iframes - are candidates.
            if action.frame_name().is_none() {
                return false;
            }
            let Some(request) = action.request() else {
                return false;
            };
            let Some(uri) = request.uri() else {
                return false;
            };
            let source = webview
                .uri()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "https://www.youtube.com/".into());
            if filters.check_network_request(uri.as_str(), &source, "sub_frame") {
                logging::debug(format!(
                    "blocked ad subframe navigation: {} (source {})",
                    uri.as_str(),
                    source
                ));
                // Redirect to about:blank and let the navigation complete
                // instead of cancelling it with `decision.ignore()`.
                // Cancelling subframe loads in flight drives WebKit's
                // `internallyFailedLoadTimerFired` retry loop and has been
                // observed to destabilise the UI process on watch pages;
                // a completed blank navigation is the safe equivalent.
                request.set_uri("about:blank");
                decision.use_();
                true
            } else {
                false
            }
        });
    })
}

#[cfg(target_os = "linux")]
fn filter_response(
    webview: &webkit2gtk::WebView,
    decision: &webkit2gtk::PolicyDecision,
    filters: &filtering::FilterManager,
) -> bool {
    use webkit2gtk::glib::Cast;
    use webkit2gtk::{
        PolicyDecisionExt, ResponsePolicyDecisionExt, URIRequestExt, URIResponseExt, WebViewExt,
    };
    let Some(response_decision) =
        decision.downcast_ref::<webkit2gtk::ResponsePolicyDecision>()
    else {
        return false;
    };
    // Never touch the main document itself: top-level navigation policy is
    // enforced separately by `is_supported_navigation`.
    if response_decision.is_main_frame_main_resource() {
        return false;
    }
    let Some(request) = response_decision.request() else {
        return false;
    };
    let Some(uri) = request.uri() else {
        return false;
    };
    let uri = uri.to_string();
    // about:blank and data: URLs are our own redirects / inline content.
    if uri.starts_with("about:") || uri.starts_with("data:") {
        return false;
    }
    let request_type = response_decision
        .response()
        .and_then(|response| response.mime_type())
        .map(|mime| mime.to_string())
        .map(|mime| {
            if mime.contains("javascript") || mime.contains("ecmascript") {
                "script"
            } else if mime.starts_with("image/") {
                "image"
            } else if mime.contains("json") {
                "xmlhttprequest"
            } else {
                "other"
            }
        })
        .unwrap_or("other");
    let source = webview
        .uri()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "https://www.youtube.com/".into());
    if filters.check_network_request(&uri, &source, request_type) {
        logging::debug(format!(
            "blocked ad subresource ({request_type}): {uri} (source {source})"
        ));
        // Same pattern as subframe navigations: redirect to a blank document
        // and let the load complete instead of cancelling it in flight.
        request.set_uri("about:blank");
        decision.use_();
        true
    } else {
        false
    }
}

#[cfg(not(target_os = "linux"))]
fn install_native_filter(
    _window: &WebviewWindow,
    _filters: filtering::FilterManager,
) -> tauri::Result<()> {
    Ok(())
}

fn self_test() -> error::AppResult<()> {
    let settings = settings::load()?;
    settings::validate(&settings)?;
    let _ = security::validate_youtube_url("https://www.youtube.com/watch?v=self-test")?;
    let filters =
        filtering::FilterManager::new(std::env::temp_dir().join("yood-self-test-filters"))?;
    if !filters.status(&settings).enabled {
        return Err(error::AppError::Internal(
            "mandatory filter layer is disabled".into(),
        ));
    }
    if !filters.matches_blocked("https://doubleclick.net/test", false) {
        return Err(error::AppError::Internal(
            "built-in ad rules are not active".into(),
        ));
    }
    // Brave-mode ad stack must ship coherent: the embedded manifest has to
    // reference the rules file, the rules must parse, and neither layer may
    // ever touch content CDNs (a false positive there breaks playback).
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../shared/extension/yood/manifest.json"))
            .map_err(|_| {
                error::AppError::Internal("companion extension manifest is invalid".into())
            })?;
    let resources = manifest
        .pointer("/declarative_net_request/rule_resources")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            error::AppError::Internal("companion extension is missing its network rules".into())
        })?;
    if !resources
        .iter()
        .any(|entry| entry.get("path").and_then(|p| p.as_str()) == Some("rules.json"))
    {
        return Err(error::AppError::Internal(
            "companion extension manifest does not reference rules.json".into(),
        ));
    }
    let rules: serde_json::Value =
        serde_json::from_str(include_str!("../../../shared/extension/yood/rules.json")).map_err(
            |_| {
                error::AppError::Internal("companion extension network rules are invalid".into())
            },
        )?;
    let rules_text = rules.to_string().to_ascii_lowercase();
    for cdn in ["googlevideo.com", "ytimg.com", "gstatic.com"] {
        if rules_text.contains(cdn) {
            return Err(error::AppError::Internal(
                "companion extension rules must never block the content CDN".into(),
            ));
        }
    }
    Ok(())
}

/// Accepts either a real https:// YouTube URL (passed by an OS file/URL
/// association the user configured manually, or a `yood <url>` CLI
/// invocation) or Yood's own `yood://` custom scheme, which the deep-link
/// plugin can register with the OS without needing to claim all `https://`
/// traffic. `yood://www.youtube.com/watch?v=ID` unwraps to
/// `https://www.youtube.com/watch?v=ID`.
fn youtube_url_from_str(argument: &str) -> Option<String> {
    let url = Url::parse(argument).ok()?;
    let normalised = if url.scheme() == "yood" {
        let rest = argument.strip_prefix("yood://")?;
        Url::parse(&format!("https://{rest}")).ok()?
    } else {
        url
    };
    let host = normalised.host_str()?;
    if is_supported_navigation(&normalised) && (host.contains("youtube") || host == "youtu.be") {
        Some(normalised.to_string())
    } else {
        None
    }
}

fn startup_url() -> Option<String> {
    std::env::args()
        .skip(1)
        .find_map(|argument| youtube_url_from_str(&argument))
}

/// `yood://download?url=<https-youtube-url>&mode=&quality=&container=...`
/// — sent by the Brave companion extension's download dialog. Returns the
/// validated video URL plus download options. Uses the same host allow-list
/// as navigation, plus strict option allow-lists, so a crafted link can
/// never smuggle an arbitrary URL or yt-dlp argument into the backend
/// (DownloadManager::enqueue re-validates everything again anyway).
struct DownloadLink {
    url: String,
    mode: crate::models::DownloadMode,
    quality: String,
    container: String,
    format_selector: Option<String>,
    subtitles: bool,
    subtitle_language: Option<String>,
    embed_thumbnail: bool,
}

fn download_url_from_str(argument: &str) -> Option<DownloadLink> {
    let url = Url::parse(argument).ok()?;
    if url.scheme() != "yood" || url.host_str() != Some("download") {
        return None;
    }
    let params: std::collections::HashMap<String, String> =
        url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    let target = params.get("url")?;
    if target.len() > 4096 {
        return None;
    }
    let parsed = Url::parse(target).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?;
    if !(is_supported_navigation(&parsed) && (host.contains("youtube") || host == "youtu.be")) {
        return None;
    }
    let mode = match params.get("mode").map(String::as_str).unwrap_or("video") {
        "video" => crate::models::DownloadMode::Video,
        "audio" => crate::models::DownloadMode::Audio,
        "songs" => crate::models::DownloadMode::Songs,
        _ => return None,
    };
    let quality = params.get("quality").map(String::as_str).unwrap_or("best");
    if !matches!(quality, "best" | "1080" | "720" | "480" | "worst") {
        return None;
    }
    let container = params.get("container").map(String::as_str).unwrap_or("mp4");
    if !matches!(container, "mp4" | "mkv") {
        return None;
    }
    let format_selector = match params.get("format_selector") {
        Some(selector) if !selector.is_empty() => {
            if selector.len() > 256 || selector.chars().any(|c| c.is_control()) {
                return None;
            }
            Some(selector.clone())
        }
        _ => None,
    };
    Some(DownloadLink {
        url: parsed.to_string(),
        mode,
        quality: quality.to_string(),
        container: container.to_string(),
        format_selector,
        subtitles: params.get("subtitles").map(String::as_str) == Some("1"),
        subtitle_language: match params.get("subtitle_lang") {
            Some(language) if !language.is_empty() => {
                if language.len() > 16
                    || !language
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-')
                {
                    return None;
                }
                Some(language.clone())
            }
            _ => None,
        },
        embed_thumbnail: params.get("embed_thumbnail").map(String::as_str) == Some("1"),
    })
}

fn enqueue_download_url(app: &tauri::AppHandle, argument: &str) {
    let Some(link) = download_url_from_str(argument) else {
        return;
    };
    let Some(state) = app.try_state::<commands::RuntimeState>() else {
        return;
    };
    let request = crate::models::DownloadRequest {
        url: link.url,
        mode: link.mode,
        quality: link.quality,
        container: link.container,
        format_selector: link.format_selector,
        audio_quality: None,
        subtitles: link.subtitles,
        subtitle_language: link.subtitle_language,
        embed_thumbnail: link.embed_thumbnail,
        context: Some("extension".into()),
    };
    match state.downloads.enqueue(request) {
        Ok(item) => logging::info(format!("queued download from extension: {}", item.id)),
        Err(error) => logging::warn(format!("extension download rejected: {error}")),
    }
}

/// Focuses the main window and, if the re-invocation carried a YouTube or
/// `yood://` URL, navigates it there. Used both when the OS starts a second
/// process because Yood is already running (single-instance) and when the
/// deep-link plugin reports a URL directly on macOS/mobile-style platforms.
fn focus_and_navigate(app: &tauri::AppHandle, urls: impl IntoIterator<Item = String>) {
    let urls: Vec<String> = urls.into_iter().collect();
    for raw in &urls {
        enqueue_download_url(app, raw);
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    if let Some(destination) = urls.into_iter().find_map(|url| youtube_url_from_str(&url)) {
        if let Ok(parsed) = Url::parse(&destination) {
            let _ = window.eval(&format!(
                "window.location.replace({});",
                serde_json::to_string(&parsed.to_string()).unwrap_or_default()
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_extension_download_links() {
        let good = download_url_from_str(
            "yood://download?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc123",
        )
        .expect("valid download link");
        assert_eq!(good.url, "https://www.youtube.com/watch?v=abc123");
        assert_eq!(good.quality, "best");
        assert_eq!(good.container, "mp4");
        assert!(!good.subtitles);
        let full = download_url_from_str(
            "yood://download?url=https%3A%2F%2Fyoutu.be%2Fabc12312345&mode=audio&quality=720&container=mkv&format_selector=best&subtitles=1&embed_thumbnail=1",
        )
        .expect("full download link");
        assert!(matches!(full.mode, crate::models::DownloadMode::Audio));
        assert_eq!(full.quality, "720");
        assert_eq!(full.container, "mkv");
        assert_eq!(full.format_selector.as_deref(), Some("best"));
        assert!(full.subtitles && full.embed_thumbnail);
        let sub = download_url_from_str(
            "yood://download?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc123&subtitles=1&subtitle_lang=he",
        )
        .expect("subtitle download link");
        assert!(sub.subtitles);
        assert_eq!(sub.subtitle_language.as_deref(), Some("he"));
        assert!(download_url_from_str(
            "yood://download?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc123&subtitle_lang=xx!!"
        )
        .is_none());
        // Wrong scheme / host / missing param / non-YouTube target rejected.
        assert!(download_url_from_str("https://www.youtube.com/watch?v=abc123").is_none());
        assert!(download_url_from_str("yood://www.youtube.com/watch?v=abc123").is_none());
        assert!(download_url_from_str("yood://download").is_none());
        assert!(download_url_from_str(
            "yood://download?url=https%3A%2F%2Fexample.test%2Fevil"
        )
        .is_none());
        assert!(download_url_from_str(
            "yood://download?url=javascript%3Aalert(1)"
        )
        .is_none());
        // Unknown options rejected rather than smuggled through.
        assert!(download_url_from_str(
            "yood://download?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc123&mode=evil"
        )
        .is_none());
        assert!(download_url_from_str(
            "yood://download?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc123&container=exe"
        )
        .is_none());
        assert!(download_url_from_str(
            "yood://download?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc123&quality=8k"
        )
        .is_none());
    }
}
