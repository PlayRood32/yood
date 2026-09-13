use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use adblock::{
    lists::{FilterSet, ParseOptions},
    request::Request,
    Engine,
};
use chrono::{Duration as ChronoDuration, Utc};
use futures_util::future::join_all;
use reqwest::{header, Client, StatusCode};

use crate::{
    error::AppResult,
    models::{FilterScriptRules, FilterStatus, Settings},
    settings,
};

const MAX_FILTER_LIST_BYTES: usize = 8 * 1024 * 1024;
const MAX_COMBINED_FILTER_BYTES: usize = 32 * 1024 * 1024;
const MAX_FILTER_LINES: usize = 1_000_000;

#[derive(Clone)]
struct CompiledRules {
    engine: Arc<Engine>,
    rule_count: usize,
}

#[derive(Clone)]
pub struct FilterManager {
    cache_path: PathBuf,
    source_cache_dir: PathBuf,
    rules: Arc<RwLock<Arc<CompiledRules>>>,
}

impl FilterManager {
    pub fn new(cache_dir: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(&cache_dir)?;
        let cache_path = cache_dir.join("filters.cache");
        let source_cache_dir = cache_dir.join("sources");
        fs::create_dir_all(&source_cache_dir)?;
        let cached = read_cache(&cache_path)?;
        let compiled = compile_rules(include_str!("../../../shared/filters/default.txt"), &cached);
        Ok(Self {
            cache_path,
            source_cache_dir,
            rules: Arc::new(RwLock::new(Arc::new(compiled))),
        })
    }

    pub fn status(&self, settings: &Settings) -> FilterStatus {
        let rule_count = self
            .rules
            .read()
            .map(|rules| rules.rule_count)
            .unwrap_or_default();
        FilterStatus {
            enabled: true,
            tracking_enabled: settings.tracking_protection,
            rule_count,
            last_success: settings.filter_last_success,
            cache_path: Some(self.cache_path.clone()),
        }
    }

    pub fn script_rules(
        &self,
        page_url: &str,
        classes: &[String],
        ids: &[String],
    ) -> FilterScriptRules {
        let Ok(rules) = self.rules.read() else {
            return FilterScriptRules {
                blocked_hosts: Vec::new(),
                cosmetic_selectors: Vec::new(),
            };
        };
        let resources = rules.engine.url_cosmetic_resources(page_url);
        let mut selectors = resources.hide_selectors;
        if !resources.generichide {
            selectors.extend(rules.engine.hidden_class_id_selectors(
                classes,
                ids,
                &resources.exceptions,
            ));
        }
        let mut cosmetic_selectors = selectors
            .into_iter()
            .filter(|selector| is_safe_selector(selector))
            .take(10_000)
            .collect::<Vec<_>>();
        cosmetic_selectors.sort_unstable();
        cosmetic_selectors.dedup();
        FilterScriptRules {
            // Network decisions are made by the compiled engine. The field is
            // retained for compatibility with older frontend bundles.
            blocked_hosts: Vec::new(),
            cosmetic_selectors,
        }
    }

    pub fn matches_blocked(&self, url: &str, _tracking_protection: bool) -> bool {
        self.check_network_request(url, "https://www.youtube.com/", "other")
    }

    pub fn check_network_request(&self, url: &str, source_url: &str, request_type: &str) -> bool {
        if url.len() > 16 * 1024 || source_url.len() > 16 * 1024 {
            return false;
        }
        // Hard safety net: never let a filter-list rule block YouTube's own
        // video/audio streaming CDN. A false positive here doesn't just hide
        // an ad, it silently breaks playback, and there is no legitimate
        // reason for an ad/tracker list to need to match it for a
        // YouTube-only browser.
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                if host == "googlevideo.com" || host.ends_with(".googlevideo.com") {
                    return false;
                }
            }
        }
        let Ok(request) = Request::new(url, source_url, request_type, "GET") else {
            return false;
        };
        self.rules
            .read()
            .ok()
            .map(|rules| rules.engine.check_network_request(&request).should_block())
            .unwrap_or(false)
    }

    pub async fn update_if_due(&self, settings: &mut Settings, force: bool) -> AppResult<bool> {
        let due = force
            || settings
                .filter_last_success
                .map(|last| Utc::now() - last > ChronoDuration::hours(24))
                .unwrap_or(true);
        if !due {
            return Ok(false);
        }
        self.update(settings).await
    }

    async fn update(&self, settings: &mut Settings) -> AppResult<bool> {
        let client = Client::builder()
            .user_agent("Yood/0.1 filter updater")
            .timeout(Duration::from_secs(15))
            .build()?;
        let sources = settings
            .filter_list_urls
            .iter()
            .filter(|source| source.len() <= 2048 && source.starts_with("https://"))
            .cloned()
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Ok(false);
        }
        // Fetch every list concurrently. Each source is independent: a fresh
        // download replaces its on-disk copy, a failure falls back to that
        // copy. One slow/blocked host must never discard the other lists
        // (previously a single timeout threw away the whole update and forced
        // a full retry on every launch, leaving only the tiny built-in list
        // active in the meantime — except the stale aggregate below).
        let fetches = sources.iter().map(|source| {
            let client = client.clone();
            let source = source.clone();
            let etag = settings.filter_etags.get(&source).cloned();
            let path = source_cache_path(&self.source_cache_dir, &source);
            async move { fetch_one_source(&client, &source, etag, &path).await }
        });
        let mut etags = Vec::new();
        let mut bodies = Vec::with_capacity(sources.len());
        for result in join_all(fetches).await {
            if let Some(etag) = result.etag {
                etags.push(etag);
            }
            if let Some(body) = result.body {
                bodies.push(body);
            }
        }
        if bodies.is_empty() {
            // Nothing usable at all — keep the currently compiled engine.
            return Ok(false);
        }
        let mut aggregate = String::new();
        for body in bodies {
            if aggregate.len().saturating_add(body.len() + 1) > MAX_COMBINED_FILTER_BYTES {
                return Ok(false);
            }
            aggregate.push_str(&body);
            aggregate.push('\n');
        }
        if aggregate.len() > MAX_COMBINED_FILTER_BYTES {
            return Ok(false);
        }
        if aggregate != read_cache(&self.cache_path)? {
            write_atomic(&self.cache_path, aggregate.as_bytes())?;
            self.replace_compiled(&aggregate);
        }
        settings::mark_filter_success(settings, etags)?;
        Ok(true)
    }

    fn replace_compiled(&self, cached: &str) {
        let compiled = compile_rules(include_str!("../../../shared/filters/default.txt"), cached);
        if let Ok(mut rules) = self.rules.write() {
            *rules = Arc::new(compiled);
        }
    }
}

struct SourceFetch {
    body: Option<String>,
    etag: Option<(String, String)>,
}

/// Downloads one filter list. A fresh, valid download replaces the on-disk
/// copy; anything else (304, HTTP error, timeout, invalid body) falls back
/// to that copy so a single bad host never poisons the other lists.
async fn fetch_one_source(
    client: &Client,
    source: &str,
    etag: Option<String>,
    path: &PathBuf,
) -> SourceFetch {
    let cached = read_bounded(path, MAX_FILTER_LIST_BYTES)
        .ok()
        .flatten();
    let mut request = client.get(source);
    if cached.is_some() {
        if let Some(etag) = etag.as_deref() {
            request = request.header(header::IF_NONE_MATCH, etag);
        }
    }
    match request.send().await {
        Ok(response) if response.status() == StatusCode::NOT_MODIFIED => {}
        Ok(response) if response.status().is_success() => {
            let fresh_etag = response
                .headers()
                .get(header::ETAG)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            if let Ok(bytes) = response.bytes().await {
                if !bytes.is_empty() && bytes.len() <= MAX_FILTER_LIST_BYTES {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        if has_usable_rule(&text) {
                            // Best effort: even if persisting fails, the
                            // in-memory text is still used for this session.
                            let _ = write_atomic(path, text.as_bytes());
                            return SourceFetch {
                                etag: fresh_etag.map(|value| (source.to_string(), value)),
                                body: Some(text),
                            };
                        }
                    }
                }
            }
        }
        Ok(_) | Err(_) => {}
    }
    SourceFetch {
        body: cached,
        etag: None,
    }
}

fn compile_rules(default_rules: &str, cached_rules: &str) -> CompiledRules {
    let mut filter_set = FilterSet::new(false);
    filter_set.add_filter_list(default_rules.to_string(), ParseOptions::default());
    if !cached_rules.is_empty() {
        filter_set.add_filter_list(cached_rules.to_string(), ParseOptions::default());
    }
    let rule_count = default_rules
        .lines()
        .chain(cached_rules.lines())
        .take(MAX_FILTER_LINES)
        .filter(|line| is_usable_rule(line))
        .count();
    CompiledRules {
        engine: Arc::new(Engine::new_with_filter_set(filter_set)),
        rule_count,
    }
}

fn read_cache(path: &PathBuf) -> AppResult<String> {
    Ok(read_bounded(path, MAX_COMBINED_FILTER_BYTES)?.unwrap_or_default())
}

fn read_bounded(path: &PathBuf, max_bytes: usize) -> AppResult<Option<String>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes as u64
    {
        return Ok(None);
    }
    Ok(fs::read_to_string(path).ok())
}

fn source_cache_path(source_cache_dir: &Path, source: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    source_cache_dir.join(format!("{:016x}.txt", hasher.finish()))
}

fn write_atomic(path: &PathBuf, bytes: &[u8]) -> AppResult<()> {
    let temporary = path.with_extension(format!("tmp.{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)?;
    Ok(())
}

fn has_usable_rule(text: &str) -> bool {
    text.lines().take(MAX_FILTER_LINES).any(is_usable_rule)
}

fn is_usable_rule(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !line.starts_with('!') && !line.starts_with('[')
}

fn is_safe_selector(selector: &str) -> bool {
    !selector.is_empty()
        && selector.len() <= 2048
        && !selector.chars().any(|character| character.is_control())
        && !selector.contains(['{', '}', ';'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn manager() -> FilterManager {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        FilterManager::new(std::env::temp_dir().join(format!("yood-filter-test-{suffix}")))
            .expect("filter manager")
    }

    #[test]
    fn uses_brave_engine_network_syntax_and_exceptions() {
        let filter = manager();
        assert!(filter.matches_blocked("https://doubleclick.net/ad.js", false));
        let compiled = compile_rules(
            "||ads.example.test^$script\n@@ads.example.test/allowed.js",
            "",
        );
        assert!(compiled
            .engine
            .check_network_request(
                &Request::new(
                    "https://ads.example.test/ad.js",
                    "https://www.youtube.com/watch?v=test",
                    "script",
                    "GET"
                )
                .expect("request")
            )
            .should_block());
        assert!(!compiled
            .engine
            .check_network_request(
                &Request::new(
                    "https://ads.example.test/allowed.js",
                    "https://www.youtube.com/watch?v=test",
                    "script",
                    "GET"
                )
                .expect("request")
            )
            .should_block());
    }

    #[test]
    fn exposes_hostname_cosmetic_rules_for_the_active_page() {
        let filter = manager();
        let rules = filter.script_rules("https://www.youtube.com/watch?v=test", &[], &[]);
        assert!(rules
            .cosmetic_selectors
            .iter()
            .any(|selector| selector.contains("ytp-ad")));
        assert!(rules
            .cosmetic_selectors
            .iter()
            .any(|selector| selector.contains("promoted")));
    }

    #[test]
    fn builtin_rules_block_known_ad_hosts_but_never_content_cdn() {
        let filter = manager();
        for url in [
            "https://googleads.g.doubleclick.net/pagead/id",
            "https://tpc.googlesyndication.com/sodar/sodar2.js",
            "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js",
            "https://static.doubleclick.net/instream/ad_status.js",
            "https://jnn-pa.googleapis.com/$rpc/google.internal.waa.v1.Waa/GenerateChallenge",
            "https://www.youtube.com/ptracking?video_id=test",
        ] {
            assert!(filter.matches_blocked(url, false), "should block {url}");
        }
        // The content CDN and normal pages must never match: a false
        // positive there breaks playback instead of just hiding an ad.
        for url in [
            "https://rr1---sn-abc.googlevideo.com/videoplayback?expire=1&mn=sn-abc",
            "https://www.youtube.com/watch?v=test",
            "https://i.ytimg.com/vi/test/hqdefault.jpg",
        ] {
            assert!(!filter.matches_blocked(url, false), "must not block {url}");
        }
    }
}
