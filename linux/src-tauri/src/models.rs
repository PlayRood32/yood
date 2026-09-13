use std::{collections::BTreeMap, path::PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const DEFAULT_FILTER_LIST_URLS: &[&str] = &[
    "https://easylist.to/easylist/easylist.txt",
    "https://easylist.to/easylist/easyprivacy.txt",
    "https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/filters.txt",
    "https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/privacy.txt",
    "https://raw.githubusercontent.com/brave/adblock-lists/master/brave-lists/brave-specific.txt",
    "https://raw.githubusercontent.com/brave/adblock-lists/master/brave-lists/brave-firstparty.txt",
    "https://raw.githubusercontent.com/brave/adblock-lists/master/brave-unbreak.txt",
    "https://raw.githubusercontent.com/easylist/EasyListHebrew/master/EasyListHebrew.txt",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub appearance: Appearance,
    pub tracking_protection: bool,
    pub aggressive_tracking_protection: bool,
    pub max_concurrent_downloads: u8,
    pub download_directory: PathBuf,
    pub temp_directory: PathBuf,
    pub cleanup_temp_files: bool,
    pub overwrite_existing: bool,
    pub embed_thumbnail: bool,
    pub download_subtitles: bool,
    pub subtitle_language: String,
    pub shortcut_download: String,
    pub filter_list_urls: Vec<String>,
    pub filter_last_success: Option<DateTime<Utc>>,
    pub filter_etags: BTreeMap<String, String>,
    pub ytdlp_last_update: Option<DateTime<Utc>>,
    pub log_level: String,
}

impl Default for Settings {
    fn default() -> Self {
        let project = directories::ProjectDirs::from("com", "PlayRood", "Yood");
        let download_directory = project
            .as_ref()
            .map(|p| p.data_dir().join("downloads"))
            .unwrap_or_else(|| PathBuf::from("downloads"));
        let temp_directory = project
            .as_ref()
            .map(|p| p.cache_dir().join("tmp"))
            .unwrap_or_else(|| PathBuf::from(".yood-tmp"));
        Self {
            schema_version: 2,
            appearance: Appearance::System,
            tracking_protection: false,
            aggressive_tracking_protection: false,
            max_concurrent_downloads: 2,
            download_directory,
            temp_directory,
            cleanup_temp_files: true,
            overwrite_existing: false,
            embed_thumbnail: false,
            download_subtitles: false,
            subtitle_language: "en".into(),
            shortcut_download: "d".into(),
            filter_list_urls: DEFAULT_FILTER_LIST_URLS
                .iter()
                .map(|url| (*url).into())
                .collect(),
            filter_last_success: None,
            filter_etags: BTreeMap::new(),
            ytdlp_last_update: None,
            log_level: "INFO".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SettingsPatch {
    pub appearance: Option<Appearance>,
    pub tracking_protection: Option<bool>,
    pub aggressive_tracking_protection: Option<bool>,
    pub max_concurrent_downloads: Option<u8>,
    pub download_directory: Option<PathBuf>,
    pub temp_directory: Option<PathBuf>,
    pub cleanup_temp_files: Option<bool>,
    pub overwrite_existing: Option<bool>,
    pub embed_thumbnail: Option<bool>,
    pub download_subtitles: Option<bool>,
    pub subtitle_language: Option<String>,
    pub log_level: Option<String>,
    pub shortcut_download: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadMode {
    #[default]
    Video,
    Audio,
    Songs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadRequest {
    pub url: String,
    pub mode: DownloadMode,
    pub quality: String,
    pub container: String,
    pub format_selector: Option<String>,
    pub audio_quality: Option<String>,
    pub subtitles: bool,
    pub subtitle_language: Option<String>,
    pub embed_thumbnail: bool,
    pub context: Option<String>,
}

impl Default for DownloadRequest {
    fn default() -> Self {
        Self {
            url: String::new(),
            mode: DownloadMode::Video,
            quality: "best".into(),
            container: "mp4".into(),
            format_selector: None,
            audio_quality: None,
            subtitles: false,
            subtitle_language: None,
            embed_thumbnail: false,
            context: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Interrupted,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub request: DownloadRequest,
    pub title: String,
    pub channel: String,
    pub thumbnail: Option<String>,
    pub output_path: Option<PathBuf>,
    pub status: DownloadStatus,
    pub progress: f32,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: Option<u64>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoEntry {
    pub id: String,
    pub url: String,
    pub title: String,
    pub channel: String,
    pub duration: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectResult {
    pub title: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<String>,
    pub thumbnail: Option<String>,
    pub size_bytes: Option<u64>,
    pub entries: Vec<VideoEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryStatus {
    pub name: String,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub available: bool,
    pub bundled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterStatus {
    pub enabled: bool,
    pub tracking_enabled: bool,
    pub rule_count: usize,
    pub last_success: Option<DateTime<Utc>>,
    pub cache_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterScriptRules {
    pub blocked_hosts: Vec<String>,
    pub cosmetic_selectors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    pub binaries: Vec<BinaryStatus>,
    pub filter: FilterStatus,
}
