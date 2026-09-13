use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use chrono::Utc;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
    sync::{mpsc, Notify},
    time::sleep,
};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    logging,
    models::{
        DownloadItem, DownloadMode, DownloadRequest, DownloadStatus, InspectResult, Settings,
        VideoEntry,
    },
    process::{self, BinaryResolver},
    security::{ensure_safe_output_dir, sanitize_filename, validate_youtube_url},
};

const MAX_HISTORY: usize = 1000;

#[derive(Clone)]
pub struct DownloadManager {
    items: Arc<Mutex<Vec<DownloadItem>>>,
    controls: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    active: Arc<AtomicUsize>,
    scheduler_started: Arc<AtomicBool>,
    notify: Arc<Notify>,
    settings: Arc<Mutex<Settings>>,
    resolver: BinaryResolver,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    history_path: PathBuf,
}

enum RunOutcome {
    Completed(PathBuf),
    Paused,
    Cancelled,
    Failed(AppError),
}

type Progress = (f32, Option<String>, Option<String>, Option<u64>);

impl DownloadManager {
    pub fn new(settings: Arc<Mutex<Settings>>, resolver: BinaryResolver) -> AppResult<Self> {
        let history_path = directories::ProjectDirs::from("com", "PlayRood", "Yood")
            .map(|dirs| dirs.data_dir().join("downloads.json"))
            .ok_or_else(|| AppError::Configuration("could not determine data directory".into()))?;
        let items = load_history(&history_path)?;
        Ok(Self {
            items: Arc::new(Mutex::new(items)),
            controls: Arc::new(Mutex::new(HashMap::new())),
            active: Arc::new(AtomicUsize::new(0)),
            scheduler_started: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
            settings,
            resolver,
            app_handle: Arc::new(Mutex::new(None)),
            history_path,
        })
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        if let Ok(mut app) = self.app_handle.lock() {
            *app = Some(handle);
        }
    }

    pub fn list(&self) -> Vec<DownloadItem> {
        let mut result = self
            .items
            .lock()
            .map(|items| items.clone())
            .unwrap_or_default();
        result.sort_by_key(|item| std::cmp::Reverse(item.created_at));
        result
    }

    pub fn enqueue(&self, mut request: DownloadRequest) -> AppResult<DownloadItem> {
        let url = validate_youtube_url(&request.url)?;
        request.url = url.to_string();
        validate_request(&request)?;
        let id = Uuid::new_v4().to_string();
        let item = DownloadItem {
            id: id.clone(),
            request,
            title: "Queued download".into(),
            channel: String::new(),
            thumbnail: None,
            output_path: None,
            status: DownloadStatus::Queued,
            progress: 0.0,
            speed: None,
            eta: None,
            total_bytes: None,
            downloaded_bytes: None,
            error: None,
            created_at: Utc::now(),
            completed_at: None,
        };
        self.items
            .lock()
            .map_err(|_| AppError::Internal("download state lock poisoned".into()))?
            .push(item.clone());
        self.trim_and_save()?;
        // Let an already-open download manager pick up the new queue entry
        // immediately instead of only appearing once it starts downloading.
        self.emit_item(&item);
        self.notify.notify_one();
        self.ensure_scheduler();
        Ok(item)
    }

    pub fn pause(&self, id: &str) -> AppResult<()> {
        let item = self.item(id)?;
        if !matches!(
            item.status,
            DownloadStatus::Downloading | DownloadStatus::Queued
        ) {
            return Err(AppError::InvalidInput(
                "download cannot be paused in its current state".into(),
            ));
        }
        if let Some(control) = self
            .controls
            .lock()
            .map_err(|_| AppError::Internal("download control lock poisoned".into()))?
            .get(id)
        {
            control.store(true, Ordering::SeqCst);
        }
        self.set_status(id, DownloadStatus::Paused, None)?;
        self.notify.notify_one();
        Ok(())
    }

    pub fn resume(&self, id: &str) -> AppResult<()> {
        let item = self.item(id)?;
        if !matches!(
            item.status,
            DownloadStatus::Paused | DownloadStatus::Interrupted
        ) {
            return Err(AppError::InvalidInput(
                "download is not paused or interrupted".into(),
            ));
        }
        self.set_status(id, DownloadStatus::Queued, None)?;
        self.notify.notify_one();
        self.ensure_scheduler();
        Ok(())
    }

    pub fn cancel(&self, id: &str) -> AppResult<()> {
        let item = self.item(id)?;
        if matches!(
            item.status,
            DownloadStatus::Completed | DownloadStatus::Cancelled
        ) {
            return Err(AppError::InvalidInput(
                "download is already finished".into(),
            ));
        }
        if let Some(control) = self
            .controls
            .lock()
            .map_err(|_| AppError::Internal("download control lock poisoned".into()))?
            .get(id)
        {
            control.store(true, Ordering::SeqCst);
        }
        self.set_status(id, DownloadStatus::Cancelled, None)?;
        self.notify.notify_one();
        Ok(())
    }

    pub fn retry(&self, id: &str) -> AppResult<()> {
        let item = self.item(id)?;
        if !matches!(
            item.status,
            DownloadStatus::Failed | DownloadStatus::Cancelled
        ) {
            return Err(AppError::InvalidInput(
                "only failed or cancelled downloads can be retried".into(),
            ));
        }
        self.set_status(id, DownloadStatus::Queued, None)?;
        self.notify.notify_one();
        self.ensure_scheduler();
        Ok(())
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let mut items = self
            .items
            .lock()
            .map_err(|_| AppError::Internal("download state lock poisoned".into()))?;
        let index = items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(id.into()))?;
        if matches!(
            items[index].status,
            DownloadStatus::Downloading | DownloadStatus::Queued | DownloadStatus::Paused
        ) {
            return Err(AppError::InvalidInput(
                "active downloads must be cancelled first".into(),
            ));
        }
        items.remove(index);
        drop(items);
        self.trim_and_save()
    }

    pub async fn shutdown_async(&self) {
        if let Ok(controls) = self.controls.lock() {
            for control in controls.values() {
                control.store(true, Ordering::SeqCst);
            }
        }
        for _ in 0..24 {
            if self.active.load(Ordering::SeqCst) == 0 {
                break;
            }
            sleep(Duration::from_millis(250)).await;
        }
        let _ = self.save();
    }

    pub async fn inspect(&self, raw_url: &str) -> AppResult<InspectResult> {
        let url = validate_youtube_url(raw_url)?;
        let (yt_dlp, _) = self.resolver.resolve("yt-dlp")?;
        let args = vec![
            "--ignore-config".into(),
            "--no-warnings".into(),
            "--dump-single-json".into(),
            "--skip-download".into(),
            "--flat-playlist".into(),
            url.to_string(),
        ];
        let command = process::command(&yt_dlp, &args)?;
        let (code, stdout, stderr) = process::capture(command, Duration::from_secs(45)).await?;
        if code != 0 {
            return Err(AppError::Process(safe_diagnostic(&stderr)));
        }
        if stdout.len() > 16 * 1024 * 1024 {
            return Err(AppError::Process("yt-dlp metadata was too large".into()));
        }
        parse_inspect(&stdout)
    }

    pub async fn resolve_player_source(&self, raw_url: &str) -> AppResult<String> {
        let url = validate_youtube_url(raw_url)?;
        let (yt_dlp, _) = self.resolver.resolve("yt-dlp")?;
        // A progressive stream lets the lightweight HTML media element play
        // immediately without opening a second heavyweight YouTube WebView.
        // yt-dlp performs URL extraction; Yood never handles account
        // credentials or constructs a shell command.
        let args = vec![
            "--ignore-config".into(),
            "--no-warnings".into(),
            "--no-playlist".into(),
            "--format".into(),
            "best[ext=mp4][vcodec!=none][acodec!=none]/best[vcodec!=none][acodec!=none]".into(),
            "--get-url".into(),
            url.to_string(),
        ];
        let command = process::command(&yt_dlp, &args)?;
        let (code, stdout, stderr) = process::capture(command, Duration::from_secs(45)).await?;
        if code != 0 {
            return Err(AppError::Process(safe_diagnostic(&stderr)));
        }
        parse_player_source(&stdout)
    }

    fn ensure_scheduler(&self) {
        if self.scheduler_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            manager.scheduler_loop().await;
        });
    }

    async fn scheduler_loop(self) {
        loop {
            let limit = self
                .settings
                .lock()
                .map(|settings| settings.max_concurrent_downloads as usize)
                .unwrap_or(1)
                .clamp(1, 4);
            while self.active.load(Ordering::SeqCst) < limit {
                let Some(id) = self.next_queued() else {
                    break;
                };
                self.active.fetch_add(1, Ordering::SeqCst);
                let manager = self.clone();
                tauri::async_runtime::spawn(async move {
                    manager.run(id).await;
                });
            }
            let has_queued = self
                .items
                .lock()
                .map(|items| {
                    items
                        .iter()
                        .any(|item| matches!(item.status, DownloadStatus::Queued))
                })
                .unwrap_or(false);
            if !has_queued && self.active.load(Ordering::SeqCst) == 0 {
                self.scheduler_started.store(false, Ordering::SeqCst);
                let still_queued = self
                    .items
                    .lock()
                    .map(|items| {
                        items
                            .iter()
                            .any(|item| matches!(item.status, DownloadStatus::Queued))
                    })
                    .unwrap_or(false);
                if still_queued && !self.scheduler_started.swap(true, Ordering::SeqCst) {
                    continue;
                }
                break;
            }
            self.notify.notified().await;
        }
    }

    fn next_queued(&self) -> Option<String> {
        let mut items = self.items.lock().ok()?;
        let item = items
            .iter_mut()
            .find(|item| matches!(item.status, DownloadStatus::Queued))?;
        item.status = DownloadStatus::Downloading;
        let result = item.id.clone();
        drop(items);
        let _ = self.save();
        self.emit(result.as_str());
        Some(result)
    }

    async fn run(&self, id: String) {
        let control = Arc::new(AtomicBool::new(false));
        if let Ok(mut controls) = self.controls.lock() {
            controls.insert(id.clone(), control.clone());
        }
        let result = self.perform(&id, control).await;
        match result {
            RunOutcome::Completed(path) => {
                logging::info(format!("download {id} completed: {}", path.display()));
                let _ = self.finish(&id, DownloadStatus::Completed, None, Some(path));
            }
            RunOutcome::Paused => {
                logging::info(format!("download {id} paused"));
                let _ = self.finish(&id, DownloadStatus::Paused, None, None);
            }
            RunOutcome::Cancelled => {
                logging::info(format!("download {id} cancelled"));
                let _ = self.finish(&id, DownloadStatus::Cancelled, None, None);
            }
            RunOutcome::Failed(error) => {
                logging::error(format!("download {id} failed: {error}"));
                let _ = self.finish(&id, DownloadStatus::Failed, Some(error.to_string()), None);
            }
        }
        if let Ok(mut controls) = self.controls.lock() {
            controls.remove(&id);
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.notify.notify_one();
    }

    async fn perform(&self, id: &str, control: Arc<AtomicBool>) -> RunOutcome {
        let item = match self.item(id) {
            Ok(item) => item,
            Err(error) => return RunOutcome::Failed(error),
        };
        if control.load(Ordering::SeqCst) {
            return RunOutcome::Cancelled;
        }
        let metadata = match self.inspect(&item.request.url).await {
            Ok(value) => value,
            Err(error) => return RunOutcome::Failed(error),
        };
        let title = metadata
            .title
            .clone()
            .unwrap_or_else(|| "YouTube download".into());
        let channel = metadata.channel.clone().unwrap_or_default();
        logging::info(format!("downloading {title}"));
        let _ = self.update_metadata(id, &metadata);
        let settings = match self.settings.lock() {
            Ok(settings) => settings.clone(),
            Err(_) => {
                return RunOutcome::Failed(AppError::Internal("settings lock poisoned".into()))
            }
        };
        let destination = settings.download_directory.clone();
        let temporary_root = settings.temp_directory.join(id);
        if let Err(error) = ensure_safe_output_dir(&destination)
            .and_then(|_| ensure_safe_output_dir(&settings.temp_directory))
            .and_then(|_| ensure_safe_output_dir(&temporary_root))
        {
            return RunOutcome::Failed(error);
        }
        if let Some(size) = metadata_size(&metadata) {
            let needed = size.saturating_add(size / 20);
            if let Some(free) = insufficient_disk_space(&destination, needed) {
                return RunOutcome::Failed(AppError::Storage(format!(
                    "not enough free disk space: need about {}, only {} free on {}",
                    format_bytes(size),
                    format_bytes(free),
                    destination.display()
                )));
            }
        }
        let (yt_dlp, _) = match self.resolver.resolve("yt-dlp") {
            Ok(value) => value,
            Err(error) => return RunOutcome::Failed(error),
        };
        let mut args = match build_download_args(&item.request, &temporary_root, &settings) {
            Ok(args) => args,
            Err(error) => return RunOutcome::Failed(error),
        };
        if let Ok((ffmpeg, _)) = self.resolver.resolve("ffmpeg") {
            args.extend([
                "--ffmpeg-location".into(),
                ffmpeg.to_string_lossy().into_owned(),
            ]);
        }
        let command = match process::command(&yt_dlp, &args) {
            Ok(command) => command,
            Err(error) => return RunOutcome::Failed(error),
        };
        let process_result = match run_download_process(
            command,
            control.clone(),
            self.clone(),
            id.to_string(),
        )
        .await
        {
            Ok(result) => result,
            Err(error) => return RunOutcome::Failed(error),
        };
        match process_result {
            ProcessResult::Paused => RunOutcome::Paused,
            ProcessResult::Cancelled => RunOutcome::Cancelled,
            ProcessResult::Failed(error) => RunOutcome::Failed(error),
            ProcessResult::Completed => {
                let source = match locate_media(&temporary_root) {
                    Some(path) => path,
                    None => {
                        return RunOutcome::Failed(AppError::Process(
                            "yt-dlp completed without producing a media file".into(),
                        ))
                    }
                };
                let extension = source.extension().and_then(|x| x.to_str()).unwrap_or("bin");
                let stem =
                    sanitize_filename(&format!("{} - {}", channel, title), "yood-download", 180);
                let target =
                    unique_target(&destination, &stem, extension, settings.overwrite_existing);
                if let Err(error) = move_media(&source, &target) {
                    return RunOutcome::Failed(error);
                }
                if settings.cleanup_temp_files {
                    let _ = fs::remove_dir_all(&temporary_root);
                }
                RunOutcome::Completed(target)
            }
        }
    }

    fn finish(
        &self,
        id: &str,
        status: DownloadStatus,
        error: Option<String>,
        output: Option<PathBuf>,
    ) -> AppResult<()> {
        let mut items = self
            .items
            .lock()
            .map_err(|_| AppError::Internal("download state lock poisoned".into()))?;
        let item = items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(id.into()))?;
        item.status = status;
        item.error = error;
        item.output_path = output;
        if matches!(item.status, DownloadStatus::Completed) {
            item.progress = 100.0;
            item.completed_at = Some(Utc::now());
        }
        item.speed = None;
        item.eta = None;
        let snapshot = item.clone();
        drop(items);
        self.save()?;
        self.emit_item(&snapshot);
        Ok(())
    }

    fn update_metadata(&self, id: &str, metadata: &InspectResult) -> AppResult<()> {
        let mut items = self
            .items
            .lock()
            .map_err(|_| AppError::Internal("download state lock poisoned".into()))?;
        if let Some(item) = items.iter_mut().find(|item| item.id == id) {
            item.title = metadata
                .title
                .clone()
                .unwrap_or_else(|| "YouTube download".into());
            item.channel = metadata.channel.clone().unwrap_or_default();
            item.thumbnail = metadata.thumbnail.clone();
        }
        let snapshot = items.iter().find(|item| item.id == id).cloned();
        drop(items);
        self.save()?;
        if let Some(item) = snapshot {
            self.emit_item(&item);
        }
        Ok(())
    }

    fn set_status(&self, id: &str, status: DownloadStatus, error: Option<String>) -> AppResult<()> {
        let mut items = self
            .items
            .lock()
            .map_err(|_| AppError::Internal("download state lock poisoned".into()))?;
        let item = items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(id.into()))?;
        item.status = status;
        item.error = error;
        let snapshot = item.clone();
        drop(items);
        self.save()?;
        self.emit_item(&snapshot);
        Ok(())
    }

    fn update_progress(
        &self,
        id: &str,
        progress: f32,
        speed: Option<String>,
        eta: Option<String>,
        bytes: Option<u64>,
    ) {
        if let Ok(mut items) = self.items.lock() {
            if let Some(item) = items.iter_mut().find(|item| item.id == id) {
                item.progress = progress.clamp(0.0, 100.0);
                item.speed = speed;
                item.eta = eta;
                item.downloaded_bytes = bytes;
                let snapshot = item.clone();
                drop(items);
                logging::trace(format!("download {id} progress {progress:.1}%"));
                self.emit_item(&snapshot);
            }
        }
    }

    fn item(&self, id: &str) -> AppResult<DownloadItem> {
        self.items
            .lock()
            .map_err(|_| AppError::Internal("download state lock poisoned".into()))?
            .iter()
            .find(|item| item.id == id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(id.into()))
    }
    fn trim_and_save(&self) -> AppResult<()> {
        if let Ok(mut items) = self.items.lock() {
            if items.len() > MAX_HISTORY {
                items.sort_by_key(|item| std::cmp::Reverse(item.created_at));
                items.truncate(MAX_HISTORY);
            }
        }
        self.save()
    }
    fn save(&self) -> AppResult<()> {
        let items = self.list();
        if let Some(parent) = self.history_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self
            .history_path
            .with_extension(format!("tmp.{}", std::process::id()));
        let data = serde_json::to_vec(&items)?;
        fs::write(&temporary, data)?;
        fs::rename(temporary, &self.history_path)?;
        Ok(())
    }
    fn emit(&self, id: &str) {
        if let Ok(item) = self.item(id) {
            self.emit_item(&item);
        }
    }
    fn emit_item(&self, item: &DownloadItem) {
        if let Ok(handle) = self.app_handle.lock() {
            if let Some(handle) = handle.as_ref() {
                let _ = handle.emit("download-updated", item);
            }
        }
    }
}

fn load_history(path: &Path) -> AppResult<Vec<DownloadItem>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(AppError::UnsafePath(
            "download history is a symbolic link".into(),
        ));
    }
    let mut items: Vec<DownloadItem> = serde_json::from_slice(&fs::read(path)?)?;
    for item in &mut items {
        if matches!(item.status, DownloadStatus::Downloading) {
            item.status = DownloadStatus::Interrupted;
            item.error = Some("Yood closed while this download was active".into());
        }
    }
    items.truncate(MAX_HISTORY);
    Ok(items)
}

fn validate_request(request: &DownloadRequest) -> AppResult<()> {
    if request.quality.len() > 32 || request.container != "mp4" && request.container != "mkv" {
        return Err(AppError::InvalidInput(
            "unsupported download quality or container".into(),
        ));
    }
    if let Some(selector) = &request.format_selector {
        if selector.len() > 256 || selector.chars().any(|c| c.is_control()) {
            return Err(AppError::InvalidInput(
                "invalid yt-dlp format selector".into(),
            ));
        }
    }
    if let Some(context) = &request.context {
        if context.len() > 32 || context.chars().any(|c| c.is_control()) {
            return Err(AppError::InvalidInput("invalid download context".into()));
        }
    }
    if let Some(language) = &request.subtitle_language {
        if !is_valid_subtitle_language(language) {
            return Err(AppError::InvalidInput("invalid subtitle language".into()));
        }
    }
    Ok(())
}

fn is_valid_subtitle_language(value: &str) -> bool {
    // BCP47-ish tags as yt-dlp expects them: "en", "he", "pt-BR", "zh-Hans".
    !value.is_empty()
        && value.len() <= 16
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn build_download_args(
    request: &DownloadRequest,
    temp: &Path,
    settings: &Settings,
) -> AppResult<Vec<String>> {
    let mut args = vec!["--ignore-config".into(), "--no-playlist".into(), "--newline".into(), "--progress".into(), "--progress-template".into(), "download:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress.downloaded_bytes)s".into(), "--retries".into(), "3".into(), "--fragment-retries".into(), "3".into(), "--socket-timeout".into(), "30".into(), "--continue".into(), "--windows-filenames".into(), "--paths".into(), temp.to_string_lossy().into_owned(), "--output".into(), "yood-source.%(ext)s".into()];
    match request.mode {
        DownloadMode::Video => {
            let selector =
                request
                    .format_selector
                    .clone()
                    .unwrap_or_else(|| match request.quality.as_str() {
                        "1080" => "bestvideo*[height<=1080]+bestaudio/best[height<=1080]".into(),
                        "720" => "bestvideo*[height<=720]+bestaudio/best[height<=720]".into(),
                        "480" => "bestvideo*[height<=480]+bestaudio/best[height<=480]".into(),
                        "worst" => "worstvideo+worstaudio/worst".into(),
                        _ => "bestvideo*+bestaudio/best".into(),
                    });
            args.extend([
                "-f".into(),
                selector,
                "--merge-output-format".into(),
                request.container.clone(),
            ]);
            if request.subtitles || settings.download_subtitles {
                // Per-download language wins; otherwise the configured
                // default. Auto-generated subtitles are included too, so the
                // chosen language is available even when the uploader only
                // provided auto captions (YouTube auto-translates).
                let language = request
                    .subtitle_language
                    .clone()
                    .filter(|value| is_valid_subtitle_language(value))
                    .unwrap_or_else(|| settings.subtitle_language.clone());
                args.extend([
                    "--write-subs".into(),
                    "--write-auto-subs".into(),
                    "--sub-langs".into(),
                    language,
                    "--embed-subs".into(),
                ]);
            }
        }
        DownloadMode::Audio | DownloadMode::Songs => {
            args.extend([
                "-f".into(),
                "bestaudio/best".into(),
                "-x".into(),
                "--audio-format".into(),
                "mp3".into(),
                "--audio-quality".into(),
                request.audio_quality.clone().unwrap_or_else(|| "0".into()),
            ]);
            if request.embed_thumbnail
                || settings.embed_thumbnail
                || matches!(request.mode, DownloadMode::Songs)
            {
                args.extend(["--embed-thumbnail".into(), "--add-metadata".into()]);
            }
        }
    }
    args.push(request.url.clone());
    Ok(args)
}

#[derive(Debug)]
enum ProcessResult {
    Completed,
    Paused,
    Cancelled,
    Failed(AppError),
}

async fn run_download_process(
    mut command: tokio::process::Command,
    control: Arc<AtomicBool>,
    manager: DownloadManager,
    id: String,
) -> AppResult<ProcessResult> {
    let mut child: Child = command
        .spawn()
        .map_err(|error| AppError::Process(error.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Process("yt-dlp stdout unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Process("yt-dlp stderr unavailable".into()))?;
    let (sender, mut receiver) = mpsc::unbounded_channel::<String>();
    let stdout_task = tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let stderr_task = tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut text = String::new();
        let _ = tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut text).await;
        text
    });
    let mut last_error = String::new();
    let status = loop {
        tokio::select! {
            line = receiver.recv() => if let Some(line) = line { if let Some((progress, speed, eta, bytes)) = parse_progress(&line) { manager.update_progress(&id, progress, speed, eta, bytes); } else if !line.is_empty() { last_error = line; } },
            result = child.wait() => break result.map_err(|error| AppError::Process(error.to_string()))?,
            _ = sleep(Duration::from_millis(250)) => {
                if control.load(Ordering::SeqCst) { process::terminate(&mut child).await; let _ = stdout_task.await; let _ = stderr_task.await; let item = manager.item(&id).ok(); return Ok(if item.is_some_and(|item| matches!(item.status, DownloadStatus::Paused)) { ProcessResult::Paused } else { ProcessResult::Cancelled }); }
            }
        }
    };
    let _ = stdout_task.await;
    let stderr_text = stderr_task
        .await
        .map_err(|error| AppError::Process(error.to_string()))?;
    if status.success() {
        Ok(ProcessResult::Completed)
    } else {
        if last_error.is_empty() {
            last_error = stderr_text;
        }
        if is_disk_full_error(&last_error) {
            return Ok(ProcessResult::Failed(AppError::Storage(
                "the destination drive ran out of disk space during the download".into(),
            )));
        }
        Ok(ProcessResult::Failed(AppError::Process(safe_diagnostic(
            &last_error,
        ))))
    }
}

fn parse_progress(line: &str) -> Option<Progress> {
    let values = line
        .strip_prefix("download:")?
        .split('|')
        .collect::<Vec<_>>();
    if values.len() < 4 {
        return None;
    }
    let progress = values[0].trim().trim_end_matches('%').parse::<f32>().ok()?;
    let speed = normalise_progress_text(values[1]);
    let eta = normalise_progress_text(values[2]);
    let bytes = values[3].trim().parse::<u64>().ok();
    Some((progress, speed, eta, bytes))
}

fn normalise_progress_text(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == "N/A" {
        None
    } else {
        Some(value.chars().take(32).collect())
    }
}

fn parse_player_source(stdout: &str) -> AppResult<String> {
    let source = stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("https://"))
        .ok_or_else(|| AppError::Process("yt-dlp returned no playable media URL".into()))?;
    if source.len() > 16 * 1024 || source.chars().any(char::is_control) {
        return Err(AppError::Process("yt-dlp returned an invalid media URL".into()));
    }
    let parsed = url::Url::parse(source)
        .map_err(|_| AppError::Process("yt-dlp returned an invalid media URL".into()))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(AppError::Process("yt-dlp returned an unsafe media URL".into()));
    }
    Ok(parsed.into())
}

fn parse_inspect(stdout: &str) -> AppResult<InspectResult> {
    let value: Value = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| AppError::Process("yt-dlp returned invalid metadata".into()))?;
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string);
    let channel = value
        .get("uploader")
        .or_else(|| value.get("channel"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let duration = value
        .get("duration_string")
        .or_else(|| value.get("duration"))
        .map(|value| value.to_string())
        .filter(|value| value != "null");
    let thumbnail = value
        .get("thumbnail")
        .and_then(Value::as_str)
        .map(str::to_string);
    let size_bytes = value
        .get("filesize")
        .and_then(Value::as_u64)
        .or_else(|| value.get("filesize_approx").and_then(Value::as_u64));
    let mut entries = Vec::new();
    if let Some(array) = value.get("entries").and_then(Value::as_array) {
        for entry in array.iter().take(500).filter_map(parse_entry) {
            entries.push(entry);
        }
    } else if let Some(entry) = parse_entry(&value) {
        entries.push(entry);
    }
    Ok(InspectResult {
        title,
        channel,
        duration,
        thumbnail,
        size_bytes,
        entries,
    })
}

fn parse_entry(value: &Value) -> Option<VideoEntry> {
    let id = value.get("id")?.as_str()?.to_string();
    if id.is_empty() {
        return None;
    }
    let url = value
        .get("webpage_url")
        .or_else(|| value.get("url"))
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("http"))
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={id}"));
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled video")
        .chars()
        .take(512)
        .collect();
    let channel = value
        .get("uploader")
        .or_else(|| value.get("channel"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(256)
        .collect();
    let duration = value
        .get("duration_string")
        .or_else(|| value.get("duration"))
        .map(|value| value.to_string())
        .filter(|value| value != "null");
    let thumbnail = value
        .get("thumbnail")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(VideoEntry {
        id,
        url,
        title,
        channel,
        duration,
        thumbnail,
    })
}

fn metadata_size(value: &InspectResult) -> Option<u64> {
    value.size_bytes
}

fn insufficient_disk_space(destination: &Path, needed_bytes: u64) -> Option<u64> {
    let free = fs2::available_space(destination).ok()?;
    (free < needed_bytes).then_some(free)
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn is_disk_full_error(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "no space left on device",
        "disk quota exceeded",
        "insufficient space",
        "out of disk space",
        "not enough space",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn locate_media(directory: &Path) -> Option<PathBuf> {
    let mut candidates = fs::read_dir(directory)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).ok()?;
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || path.extension().is_some_and(|ext| {
                    matches!(
                        ext.to_str(),
                        Some("part" | "ytdl" | "vtt" | "jpg" | "jpeg" | "png" | "webp" | "json")
                    )
                })
            {
                return None;
            }
            Some((meta.len(), path))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(size, _)| *size);
    candidates.pop().map(|(_, path)| path)
}

fn unique_target(directory: &Path, stem: &str, extension: &str, overwrite: bool) -> PathBuf {
    let base = directory.join(format!("{stem}.{extension}"));
    if overwrite || !base.exists() {
        return base;
    }
    for number in 1..10000 {
        let target = directory.join(format!("{stem} ({number}).{extension}"));
        if !target.exists() {
            return target;
        }
    }
    directory.join(format!("{stem}-{}.{extension}", Uuid::new_v4()))
}

fn move_media(source: &Path, target: &Path) -> AppResult<()> {
    if target.exists() && fs::symlink_metadata(target)?.file_type().is_symlink() {
        return Err(AppError::UnsafePath("target is a symbolic link".into()));
    }
    fs::rename(source, target)
        .or_else(|_| {
            fs::copy(source, target)
                .map(|_| ())
                .and_then(|_| fs::remove_file(source))
        })
        .map_err(|error| AppError::Storage(format!("cannot move completed download: {error}")))
}

fn safe_diagnostic(value: &str) -> String {
    value
        .split_whitespace()
        .map(|part| {
            if part.starts_with("http://") || part.starts_with("https://") {
                "[URL redacted]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(1000)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_direct_arguments_without_shell_interpretation() {
        let settings = Settings::default();
        let request = DownloadRequest {
            url: "https://www.youtube.com/watch?v=abc".into(),
            ..Default::default()
        };
        let args = build_download_args(&request, Path::new("/tmp/yood"), &settings).unwrap();
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--output", "yood-source.%(ext)s"]));
        assert!(args.contains(&request.url));
    }

    #[test]
    fn parses_progress() {
        let progress = parse_progress("download:42.5%|1.2MiB/s|00:03|1234").unwrap();
        assert_eq!(progress.0, 42.5);
        assert_eq!(progress.3, Some(1234));
    }

    #[test]
    fn accepts_only_https_player_sources() {
        assert!(parse_player_source("https://rr1---sn.example.test/media.mp4\n").is_ok());
        assert!(parse_player_source("http://example.test/media.mp4\n").is_err());
        assert!(parse_player_source("not a URL\n").is_err());
    }

    #[test]
    fn picks_unique_targets_when_not_overwriting() {
        let dir = std::env::temp_dir().join(format!("yood-target-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = unique_target(&dir, "song", "mp3", false);
        assert_eq!(first, dir.join("song.mp3"));
        std::fs::write(&first, b"x").unwrap();
        let second = unique_target(&dir, "song", "mp3", false);
        assert_eq!(second, dir.join("song (1).mp3"));
        std::fs::write(&second, b"x").unwrap();
        let overwrite = unique_target(&dir, "song", "mp3", true);
        assert_eq!(overwrite, dir.join("song.mp3"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_filesize_from_inspect_json() {
        let json = r#"{"title":"T","uploader":"U","filesize":123456789,"filesize_approx":999}
"#;
        let result = parse_inspect(json).unwrap();
        assert_eq!(result.size_bytes, Some(123456789));
        let json_approx = r#"{"title":"T","filesize_approx":42}
"#;
        assert_eq!(parse_inspect(json_approx).unwrap().size_bytes, Some(42));
        let json_none = r#"{"title":"T"}
"#;
        assert_eq!(parse_inspect(json_none).unwrap().size_bytes, None);
    }

    #[test]
    fn audio_mode_builds_mp3_args_and_songs_embed_metadata() {
        let settings = Settings::default();
        let audio = DownloadRequest {
            url: "https://www.youtube.com/watch?v=abc".into(),
            mode: DownloadMode::Audio,
            ..Default::default()
        };
        let args = build_download_args(&audio, Path::new("/tmp/yood"), &settings).unwrap();
        assert!(args.windows(2).any(|p| p == ["--audio-format", "mp3"]));
        assert!(args.windows(2).any(|p| p == ["-f", "bestaudio/best"]));
        assert!(!args.iter().any(|a| a == "--embed-thumbnail"));
        let songs = DownloadRequest {
            url: "https://www.youtube.com/watch?v=abc".into(),
            mode: DownloadMode::Songs,
            ..Default::default()
        };
        let args = build_download_args(&songs, Path::new("/tmp/yood"), &settings).unwrap();
        assert!(args.iter().any(|a| a == "--embed-thumbnail"));
        assert!(args.iter().any(|a| a == "--add-metadata"));
    }

    #[test]
    fn detects_disk_full_errors() {
        assert!(is_disk_full_error("ERROR: No space left on device"));
        assert!(is_disk_full_error("disk quota exceeded while writing"));
        assert!(is_disk_full_error("insufficient space available"));
        assert!(!is_disk_full_error("network error occurred"));
    }

    #[test]
    fn formats_human_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KiB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    #[test]
    fn validates_requests() {
        let good = DownloadRequest {
            url: "https://www.youtube.com/watch?v=abc".into(),
            container: "mkv".into(),
            ..Default::default()
        };
        assert!(validate_request(&good).is_ok());
        let bad_container = DownloadRequest {
            url: "https://www.youtube.com/watch?v=abc".into(),
            container: "avi".into(),
            ..Default::default()
        };
        assert!(validate_request(&bad_container).is_err());
        let bad_selector = DownloadRequest {
            url: "https://www.youtube.com/watch?v=abc".into(),
            format_selector: Some("rm -rf /".into()),
            ..Default::default()
        };
        assert!(validate_request(&bad_selector).is_ok());
        let control_selector = DownloadRequest {
            url: "https://www.youtube.com/watch?v=abc".into(),
            format_selector: Some("foo\nbar".into()),
            ..Default::default()
        };
        assert!(validate_request(&control_selector).is_err());
    }
}
