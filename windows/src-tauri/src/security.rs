use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use url::Url;

use crate::error::{AppError, AppResult};

const YOUTUBE_HOSTS: &[&str] = &[
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
    "youtu.be",
];

pub fn validate_youtube_url(raw: &str) -> AppResult<Url> {
    if raw.len() > 4096 || raw.chars().any(|c| c.is_control()) {
        return Err(AppError::InvalidInput("invalid URL".into()));
    }
    let url = Url::parse(raw).map_err(|_| AppError::InvalidInput("invalid URL".into()))?;
    if url.scheme() != "https" {
        return Err(AppError::UnsupportedUrl(
            "only HTTPS YouTube URLs are supported".into(),
        ));
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !YOUTUBE_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
    {
        return Err(AppError::UnsupportedUrl(
            "only YouTube URLs are supported".into(),
        ));
    }
    if host == "youtu.be" && url.path().len() < 2 {
        return Err(AppError::InvalidInput(
            "short YouTube URL has no video id".into(),
        ));
    }
    Ok(url)
}

pub fn is_supported_navigation(url: &Url) -> bool {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let allowed = [
        "youtube.com",
        "www.youtube.com",
        "m.youtube.com",
        "music.youtube.com",
        "youtu.be",
        "google.com",
        "www.google.com",
        "accounts.google.com",
        "accounts.youtube.com",
        "gstatic.com",
        "googleusercontent.com",
        "ytimg.com",
        "googlevideo.com",
        "youtube-nocookie.com",
    ];
    allowed
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}

pub fn sanitize_filename(input: &str, fallback: &str, max_bytes: usize) -> String {
    let mut result = String::with_capacity(input.len().min(max_bytes));
    for ch in input.chars() {
        let invalid =
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            result.push('_');
        } else {
            result.push(ch);
        }
    }
    let trimmed = result.trim().trim_end_matches(['.', ' ']).to_string();
    let mut result = if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed
    };
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3",
    ];
    if reserved
        .iter()
        .any(|name| result.eq_ignore_ascii_case(name))
    {
        result.insert(0, '_');
    }
    while result.len() > max_bytes {
        result.pop();
    }
    result.trim_end_matches(['.', ' ']).to_string()
}

pub fn validate_user_path(path: &Path, allowed_roots: &[&Path]) -> AppResult<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::UnsafePath(path.display().to_string()));
    }
    let candidate = if path.exists() {
        fs::canonicalize(path)?
    } else {
        let parent = path
            .parent()
            .ok_or_else(|| AppError::UnsafePath(path.display().to_string()))?;
        fs::canonicalize(parent)?.join(
            path.file_name()
                .ok_or_else(|| AppError::UnsafePath(path.display().to_string()))?,
        )
    };
    if !allowed_roots.iter().any(|root| candidate.starts_with(root)) {
        return Err(AppError::UnsafePath(format!(
            "{} is outside the allowed folders",
            path.display()
        )));
    }
    Ok(candidate)
}

pub fn ensure_safe_output_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(AppError::UnsafePath(format!(
            "{} is not a safe directory",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_windows_names_and_traversal() {
        assert_eq!(
            sanitize_filename("../CON: demo?.mp4", "fallback", 100),
            ".._CON_ demo_.mp4"
        );
        assert_eq!(sanitize_filename("...", "fallback", 100), "fallback");
        assert!(validate_youtube_url("https://www.youtube.com/watch?v=abc").is_ok());
        assert!(validate_youtube_url("https://example.test/watch?v=abc").is_err());
        assert!(validate_youtube_url("javascript:alert(1)").is_err());
    }
}
