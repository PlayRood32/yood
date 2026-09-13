use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
    time::timeout,
};

use crate::{
    error::{AppError, AppResult},
    models::BinaryStatus,
};

#[derive(Debug, Clone)]
pub struct BinaryResolver {
    resource_dir: Option<PathBuf>,
    app_data_dir: Option<PathBuf>,
}

impl BinaryResolver {
    pub fn new(resource_dir: Option<PathBuf>, app_data_dir: Option<PathBuf>) -> Self {
        Self {
            resource_dir,
            app_data_dir,
        }
    }

    pub fn resolve(&self, name: &str) -> AppResult<(PathBuf, bool)> {
        let filename = name.to_string();
        let mut candidates = Vec::new();
        if let Some(data) = &self.app_data_dir {
            candidates.push((data.join("tools").join(&filename), true));
            candidates.push((
                data.join("tools").join(platform_dir()).join(&filename),
                true,
            ));
        }
        if let Some(resource) = &self.resource_dir {
            candidates.push((resource.join("binaries").join(&filename), true));
            candidates.push((
                resource
                    .join("binaries")
                    .join(platform_dir())
                    .join(&filename),
                true,
            ));
        }
        // A system copy is a development fallback. Release packaging stages both
        // tools in the first candidates above, so end users do not need a PATH install.
        if let Ok(path) = which::which(name) {
            candidates.push((path, false));
        }
        for (path, bundled) in candidates {
            if is_executable_file(&path) {
                return Ok((path, bundled));
            }
        }
        Err(AppError::NotFound(format!(
            "{name} is not bundled or available on PATH"
        )))
    }

    pub fn writable_path(&self, name: &str) -> AppResult<PathBuf> {
        let data = self.app_data_dir.as_ref().ok_or_else(|| {
            AppError::Storage("platform app-data directory is unavailable".into())
        })?;
        let directory = data.join("tools");
        std::fs::create_dir_all(&directory)?;
        Ok(directory.join(name.to_string()))
    }

    pub async fn status(&self, name: &str) -> BinaryStatus {
        match self.resolve(name) {
            Ok((path, bundled)) => {
                let version = binary_version(&path, name).await.ok();
                BinaryStatus {
                    name: name.into(),
                    path: Some(path),
                    version,
                    available: true,
                    bundled,
                }
            }
            Err(_) => BinaryStatus {
                name: name.into(),
                path: None,
                version: None,
                available: false,
                bundled: false,
            },
        }
    }
}

fn platform_dir() -> &'static str {
    "linux-x86_64"
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.file_type().is_symlink() {
        return false;
    }
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
}

pub async fn binary_version(path: &Path, name: &str) -> AppResult<String> {
    let output = timeout(
        Duration::from_secs(10),
        Command::new(path)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| AppError::Timeout)?
    .map_err(|e| AppError::Process(format!("{name}: {e}")))?;
    if !output.status.success() {
        return Err(AppError::Process(format!(
            "{name} returned {}",
            output.status
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        return Err(AppError::Process(format!("{name} returned no version")));
    }
    Ok(text.chars().take(128).collect())
}

pub fn command(path: &Path, args: &[String]) -> AppResult<Command> {
    if !is_executable_file(path) {
        return Err(AppError::NotFound(path.display().to_string()));
    }
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

pub async fn capture(
    mut command: Command,
    timeout_duration: Duration,
) -> AppResult<(i32, String, String)> {
    let mut child = command
        .spawn()
        .map_err(|error| AppError::Process(error.to_string()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Process("stdout was not piped".into()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Process("stderr was not piped".into()))?;
    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes).await;
        bytes
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes).await;
        bytes
    });
    let status = match timeout(timeout_duration, child.wait()).await {
        Ok(result) => result.map_err(|error| AppError::Process(error.to_string()))?,
        Err(_) => {
            let _ = child.kill().await;
            return Err(AppError::Timeout);
        }
    };
    let out = stdout_task
        .await
        .map_err(|e| AppError::Process(e.to_string()))?;
    let err = stderr_task
        .await
        .map_err(|e| AppError::Process(e.to_string()))?;
    Ok((
        status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    ))
}

pub async fn terminate(child: &mut Child) {
    let _ = child.start_kill();
    let _ = timeout(Duration::from_secs(3), child.wait()).await;
}
