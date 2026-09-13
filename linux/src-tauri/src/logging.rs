use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU8, Ordering},
        Mutex, OnceLock,
    },
};

use chrono::Utc;
use regex::Regex;

const ERROR: u8 = 1;
const WARN: u8 = 2;
const INFO: u8 = 3;
const DEBUG: u8 = 4;
const TRACE: u8 = 5;

static LEVEL: AtomicU8 = AtomicU8::new(INFO);
static FILE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();

fn url_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r#"https?://[^\s"'<>]+"#).expect("url regex"))
}

fn header_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)(cookie|authorization|set-cookie|proxy-authorization):[^\r\n,;]*")
            .expect("header regex")
    })
}

const SENSITIVE_QUERY_KEYS: &[&str] = &[
    "token",
    "key",
    "sig",
    "cp",
    "oauth",
    "auth",
    "cookie",
    "pass",
    "password",
    "session",
    "sapisid",
    "hsid",
    "ssid",
    "id_token",
    "access_token",
    "refresh_token",
    "code",
    "credential",
    "secret",
];

/// Never allow credentials into a log line. URLs keep their visible parts but
/// sensitive query values and credential-style header values are replaced.
pub fn redact_sensitive(input: &str) -> String {
    let mut result = header_regex()
        .replace_all(input, |captures: &regex::Captures<'_>| {
            format!("{}: [redacted]", &captures[1])
        })
        .into_owned();
    result = url_regex()
        .replace_all(&result, |captures: &regex::Captures<'_>| {
            redact_url(&captures[0])
        })
        .into_owned();
    result
}

fn redact_url(raw: &str) -> String {
    let Ok(mut url) = url::Url::parse(raw) else {
        return raw.to_string();
    };
    let query = match url.query() {
        Some(query) if !query.is_empty() => query.to_string(),
        _ => return raw.to_string(),
    };
    let mut changed = false;
    let mut rebuilt = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        let key = key.into_owned();
        if SENSITIVE_QUERY_KEYS
            .iter()
            .any(|sensitive| key.eq_ignore_ascii_case(sensitive))
            && !value.is_empty()
        {
            rebuilt.append_pair(&key, "[redacted]");
            changed = true;
        } else {
            rebuilt.append_pair(&key, &value);
        }
    }
    if changed {
        url.set_query(Some(&rebuilt.finish()));
        url.to_string()
    } else {
        raw.to_string()
    }
}

pub fn parse_level(level: &str) -> u8 {
    match level.trim().to_ascii_uppercase().as_str() {
        "ERROR" => ERROR,
        "WARN" => WARN,
        "DEBUG" => DEBUG,
        "TRACE" => TRACE,
        _ => INFO,
    }
}

pub fn set_level(level: &str) {
    LEVEL.store(parse_level(level), Ordering::SeqCst);
}

/// Opens the local log file. Logging is best-effort: failure to open the log
/// file must never stop the application from starting.
pub fn init(level: &str) {
    set_level(level);
    let _ = FILE.get_or_init(|| {
        let path = log_path();
        let file = path.as_ref().and_then(|path| {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()
        });
        Mutex::new(file)
    });
}

fn log_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "PlayRood", "Yood")
        .map(|dirs| dirs.data_dir().join("logs").join("yood.log"))
}

fn log(level: u8, message: &str) {
    if level > LEVEL.load(Ordering::SeqCst) {
        return;
    }
    let level_name = match level {
        ERROR => "ERROR",
        WARN => "WARN",
        DEBUG => "DEBUG",
        TRACE => "TRACE",
        _ => "INFO",
    };
    let line = format!(
        "{} {} {}\n",
        Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
        level_name,
        redact_sensitive(message)
    );
    if let Some(file) = FILE.get() {
        if let Ok(mut guard) = file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
    // Never pop up terminal output in a shipped GUI build: WARN/ERROR go to
    // the log file above. Echo to stderr only in debug builds (cargo run /
    // `cargo tauri dev`), where a console is already attached and the extra
    // visibility helps development.
    #[cfg(debug_assertions)]
    if level <= WARN {
        eprintln!("{} {}", level_name, redact_sensitive(message));
    }
}

pub fn error(message: impl AsRef<str>) {
    log(ERROR, message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    log(WARN, message.as_ref());
}

pub fn info(message: impl AsRef<str>) {
    log(INFO, message.as_ref());
}

pub fn debug(message: impl AsRef<str>) {
    log(DEBUG, message.as_ref());
}

pub fn trace(message: impl AsRef<str>) {
    log(TRACE, message.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_levels_with_info_default() {
        assert_eq!(parse_level("ERROR"), 1);
        assert_eq!(parse_level("WARN"), 2);
        assert_eq!(parse_level("INFO"), 3);
        assert_eq!(parse_level("DEBUG"), 4);
        assert_eq!(parse_level("TRACE"), 5);
        assert_eq!(parse_level("garbage"), 3);
        assert_eq!(parse_level("debug"), 4);
    }

    #[test]
    fn redacts_sensitive_url_queries_and_headers() {
        let redacted = redact_sensitive(
            "downloading https://www.youtube.com/watch?v=abc&token=SECRET&cp=42 plus https://example.test/x?q=hello",
        );
        assert!(!redacted.contains("SECRET"));
        assert!(redacted.contains("token="));
        assert!(redacted.contains("redacted"));
        assert!(redacted.contains("q=hello"));
        let headers = redact_sensitive("Cookie: SID=leak; header Authorization: Bearer leak2");
        assert!(!headers.contains("leak"));
        assert!(headers.contains("Cookie: [redacted]"));
        assert!(headers.contains("Authorization: [redacted]"));
    }

    #[test]
    fn leaves_plain_messages_unchanged() {
        let message = "download completed in 12s";
        assert_eq!(redact_sensitive(message), message);
    }
}