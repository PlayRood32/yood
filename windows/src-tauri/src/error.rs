use std::io;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("unsupported URL: {0}")]
    UnsupportedUrl(String),
    #[error("path is not allowed: {0}")]
    UnsafePath(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("external process failed: {0}")]
    Process(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("operation timed out")]
    Timeout,
    #[error("internal error: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        match value.kind() {
            io::ErrorKind::PermissionDenied => Self::PermissionDenied(value.to_string()),
            io::ErrorKind::NotFound => Self::NotFound(value.to_string()),
            _ => Self::Storage(value.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Configuration(value.to_string())
    }
}

impl From<url::ParseError> for AppError {
    fn from(value: url::ParseError) -> Self {
        Self::InvalidInput(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() {
            Self::Timeout
        } else {
            Self::Network(value.to_string())
        }
    }
}

impl From<keyring::Error> for AppError {
    fn from(value: keyring::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_map_to_typed_variants() {
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(matches!(AppError::from(denied), AppError::PermissionDenied(_)));
        let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert!(matches!(AppError::from(missing), AppError::NotFound(_)));
        let other = std::io::Error::new(std::io::ErrorKind::Other, "boom");
        assert!(matches!(AppError::from(other), AppError::Storage(_)));
    }

    #[test]
    fn serde_and_url_errors_convert() {
        let json = serde_json::from_str::<serde_json::Value>("{oops") as Result<_, serde_json::Error>;
        assert!(matches!(
            AppError::from(json.unwrap_err()),
            AppError::Configuration(_)
        ));
        let url = "not a url".parse::<url::Url>() as Result<_, url::ParseError>;
        assert!(matches!(
            AppError::from(url.unwrap_err()),
            AppError::InvalidInput(_)
        ));
    }

    #[test]
    fn error_strings_are_user_facing() {
        assert!(AppError::Storage("disk full".into()).to_string().contains("disk full"));
        assert!(AppError::Timeout.to_string().contains("timed out"));
    }
}
