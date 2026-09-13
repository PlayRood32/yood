use std::{fs, path::Path, time::Duration};

use futures_util::StreamExt;
use reqwest::Client;

use crate::{
    error::{AppError, AppResult},
    process::{self, BinaryResolver},
};

const MAX_YTDLP_BYTES: u64 = 20 * 1024 * 1024;

pub async fn update_ytdlp(resolver: &BinaryResolver) -> AppResult<String> {
    let destination = resolver.writable_path("yt-dlp")?;
    let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
    let client = Client::builder()
        .user_agent("Yood/0.1 yt-dlp updater")
        .timeout(Duration::from_secs(90))
        .build()?;
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(AppError::Network(format!(
            "yt-dlp update returned {}",
            response.status()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_YTDLP_BYTES)
    {
        return Err(AppError::Network(
            "yt-dlp update is unexpectedly large".into(),
        ));
    }
    let temporary = destination.with_extension(format!("download.{}", std::process::id()));
    let mut file = tokio::fs::File::create(&temporary).await?;
    let mut stream = response.bytes_stream();
    let mut total = 0_u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total = total.saturating_add(chunk.len() as u64);
        if total > MAX_YTDLP_BYTES {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(AppError::Network(
                "yt-dlp update exceeded the size limit".into(),
            ));
        }
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    drop(file);
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&temporary)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&temporary, permissions)?;
    }
    validate_staged_binary(&temporary)?;
    if process::binary_version(&temporary, "yt-dlp").await.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(AppError::Process(
            "downloaded yt-dlp failed version validation".into(),
        ));
    }
    let previous = destination.with_extension("previous");
    if destination.exists() {
        let _ = fs::remove_file(&previous);
        fs::rename(&destination, &previous)?;
    }
    if let Err(error) = fs::rename(&temporary, &destination) {
        if previous.exists() {
            let _ = fs::rename(&previous, &destination);
        }
        return Err(AppError::Storage(error.to_string()));
    }
    process::binary_version(&destination, "yt-dlp").await
}

pub fn validate_staged_binary(path: &Path) -> AppResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AppError::UnsafePath(
            "staged binary is not a regular file".into(),
        ));
    }
    Ok(())
}
