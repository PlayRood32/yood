use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Component, Path, PathBuf},
};

use chrono::Utc;
use directories::ProjectDirs;

use crate::{
    error::{AppError, AppResult},
    models::{Settings, SettingsPatch, DEFAULT_FILTER_LIST_URLS},
};

const CURRENT_SCHEMA: u32 = 2;

pub fn config_path() -> AppResult<PathBuf> {
    ProjectDirs::from("com", "PlayRood", "Yood")
        .map(|dirs| dirs.config_dir().join("settings.json"))
        .ok_or_else(|| {
            AppError::Configuration(
                "could not determine the platform configuration directory".into(),
            )
        })
}

pub fn load() -> AppResult<Settings> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    if fs::symlink_metadata(&path)?.file_type().is_symlink() {
        return Err(AppError::UnsafePath(
            "settings file is a symbolic link".into(),
        ));
    }
    let raw = fs::read_to_string(&path)?;
    let mut settings: Settings = serde_json::from_str(&raw)?;
    migrate(&mut settings)?;
    validate(&settings)?;
    Ok(settings)
}

pub fn save(settings: &Settings) -> AppResult<()> {
    validate(settings)?;
    let path = config_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Configuration("invalid settings path".into()))?;
    fs::create_dir_all(parent)?;
    if path.exists() && fs::symlink_metadata(&path)?.file_type().is_symlink() {
        return Err(AppError::UnsafePath(
            "settings file is a symbolic link".into(),
        ));
    }
    let temporary = parent.join(format!("settings.json.{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| {
            AppError::Storage(format!("cannot create atomic settings file: {error}"))
        })?;
    let bytes = serde_json::to_vec_pretty(settings)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, &path)
        .map_err(|error| AppError::Storage(format!("cannot activate settings: {error}")))
}

pub fn apply_patch(settings: &mut Settings, patch: SettingsPatch) -> AppResult<()> {
    if let Some(value) = patch.appearance {
        settings.appearance = value;
    }
    if let Some(value) = patch.tracking_protection {
        settings.tracking_protection = value;
    }
    if let Some(value) = patch.aggressive_tracking_protection {
        settings.aggressive_tracking_protection = value;
    }
    if let Some(value) = patch.max_concurrent_downloads {
        settings.max_concurrent_downloads = value;
    }
    if let Some(value) = patch.download_directory {
        settings.download_directory = value;
    }
    if let Some(value) = patch.temp_directory {
        settings.temp_directory = value;
    }
    if let Some(value) = patch.cleanup_temp_files {
        settings.cleanup_temp_files = value;
    }
    if let Some(value) = patch.overwrite_existing {
        settings.overwrite_existing = value;
    }
    if let Some(value) = patch.embed_thumbnail {
        settings.embed_thumbnail = value;
    }
    if let Some(value) = patch.download_subtitles {
        settings.download_subtitles = value;
    }
    if let Some(value) = patch.subtitle_language {
        settings.subtitle_language = value;
    }
    if let Some(value) = patch.log_level {
        settings.log_level = value;
    }
    if let Some(value) = patch.shortcut_download {
        settings.shortcut_download = value;
    }
    validate(settings)?;
    save(settings)
}

fn migrate(settings: &mut Settings) -> AppResult<()> {
    if settings.schema_version == 0 {
        settings.schema_version = CURRENT_SCHEMA;
    }
    if settings.schema_version > CURRENT_SCHEMA {
        return Err(AppError::Configuration(format!(
            "settings schema {} is newer than this application",
            settings.schema_version
        )));
    }
    for default_url in DEFAULT_FILTER_LIST_URLS {
        if !settings
            .filter_list_urls
            .iter()
            .any(|url| url == default_url)
        {
            settings.filter_list_urls.push((*default_url).into());
        }
    }
    settings.schema_version = CURRENT_SCHEMA;
    Ok(())
}

pub fn validate(settings: &Settings) -> AppResult<()> {
    if !(1..=4).contains(&settings.max_concurrent_downloads) {
        return Err(AppError::InvalidInput(
            "concurrent downloads must be between 1 and 4".into(),
        ));
    }
    validate_directory(&settings.download_directory, "download")?;
    validate_directory(&settings.temp_directory, "temporary")?;
    if settings.download_directory == settings.temp_directory {
        return Err(AppError::InvalidInput(
            "download and temporary folders must be different".into(),
        ));
    }
    if settings.subtitle_language.len() > 32
        || settings.subtitle_language.chars().any(|c| c.is_control())
    {
        return Err(AppError::InvalidInput("invalid subtitle language".into()));
    }
    if !["ERROR", "WARN", "INFO", "DEBUG", "TRACE"].contains(&settings.log_level.as_str()) {
        return Err(AppError::InvalidInput("invalid log level".into()));
    }
    let shortcut = settings.shortcut_download.chars().collect::<Vec<_>>();
    if shortcut.len() != 1 || !shortcut[0].is_ascii_alphanumeric() {
        return Err(AppError::InvalidInput(
            "download shortcut must be a single letter or digit".into(),
        ));
    }
    Ok(())
}

fn validate_directory(path: &Path, label: &str) -> AppResult<()> {
    if !path.is_absolute() {
        return Err(AppError::InvalidInput(format!(
            "{label} folder must be an absolute path"
        )));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::UnsafePath(format!(
            "{label} folder contains a parent traversal"
        )));
    }
    if path.parent().is_none() || path == Path::new("/") || path.file_name().is_none() {
        return Err(AppError::InvalidInput(format!("invalid {label} folder")));
    }
    Ok(())
}

pub fn mark_filter_success(
    settings: &mut Settings,
    etags: impl IntoIterator<Item = (String, String)>,
) -> AppResult<()> {
    settings.filter_last_success = Some(Utc::now());
    settings.filter_etags.extend(etags);
    save(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_valid() {
        let settings = Settings::default();
        assert_eq!(settings.schema_version, CURRENT_SCHEMA);
        assert_eq!(settings.log_level, "INFO");
        assert_eq!(settings.shortcut_download, "d");
        assert!(validate(&settings).is_ok());
    }

    #[test]
    fn migrates_legacy_schema_and_backfills_filter_lists() {
        let mut settings = Settings::default();
        settings.schema_version = 0;
        settings.filter_list_urls = vec!["https://example.test/custom.txt".into()];
        migrate(&mut settings).unwrap();
        assert_eq!(settings.schema_version, CURRENT_SCHEMA);
        assert!(settings
            .filter_list_urls
            .contains(&"https://easylist.to/easylist/easylist.txt".into()));
        assert!(settings
            .filter_list_urls
            .contains(&"https://example.test/custom.txt".into()));
        assert!(validate(&settings).is_ok());
    }

    #[test]
    fn rejects_newer_schema() {
        let mut settings = Settings::default();
        settings.schema_version = CURRENT_SCHEMA + 1;
        assert!(migrate(&mut settings).is_err());
    }

    #[test]
    fn rejects_invalid_settings() {
        let mut settings = Settings::default();
        settings.max_concurrent_downloads = 0;
        assert!(validate(&settings).is_err());
        settings = Settings::default();
        settings.max_concurrent_downloads = 5;
        assert!(validate(&settings).is_err());
        settings = Settings::default();
        settings.download_directory = settings.temp_directory.clone();
        assert!(validate(&settings).is_err());
        settings = Settings::default();
        settings.log_level = "VERBOSE".into();
        assert!(validate(&settings).is_err());
        settings = Settings::default();
        settings.shortcut_download = "dd".into();
        assert!(validate(&settings).is_err());
        settings = Settings::default();
        settings.shortcut_download = " ".into();
        assert!(validate(&settings).is_err());
    }

    #[test]
    fn apply_patch_updates_fields() {
        let mut settings = Settings::default();
        let patch = SettingsPatch {
            log_level: Some("DEBUG".into()),
            shortcut_download: Some("x".into()),
            tracking_protection: Some(true),
            ..Default::default()
        };
        apply_patch(&mut settings, patch).unwrap();
        assert_eq!(settings.log_level, "DEBUG");
        assert_eq!(settings.shortcut_download, "x");
        assert!(settings.tracking_protection);
    }
}
