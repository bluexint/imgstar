use crate::contracts::{ConnectionPingResult, SaveSettingsResult, SettingsDraft, SettingsSnapshot};
use crate::runtime::adapter_runtime::resolve_bucket_base_url;
use crate::storage::settings_store::SettingsStore;
use chrono::{SecondsFormat, Utc};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFAULT_REGION: &str = "auto";
const DEFAULT_DIGIT_COUNT: u32 = 9;
const DEFAULT_REUSE_DELAY_MS: u64 = 900_000;
const DEFAULT_THEME: &str = "system";
const DEFAULT_LANGUAGE: &str = "zh-CN";
const DEFAULT_KEY_PATTERN_SUFFIXES: [&str; 7] = ["bmp", "gif", "jpeg", "jpg", "png", "svg", "webp"];

#[derive(Clone)]
pub struct SettingsService {
  store: Arc<SettingsStore>,
}

impl SettingsService {
  pub fn new(store: Arc<SettingsStore>) -> Self {
    Self { store }
  }

  pub fn save(&self, payload: SettingsDraft) -> Result<SaveSettingsResult, String> {
    let normalized = Self::normalize_for_save(payload);

    if !Self::is_configured(&normalized)
      || normalized.access_key.trim().is_empty()
      || normalized.secret_key.trim().is_empty()
      || normalized.endpoint.trim().is_empty()
      || normalized.bucket.trim().is_empty()
      || (!normalized.endpoint.starts_with("http://")
        && !normalized.endpoint.starts_with("https://"))
    {
      return Err("INVALID_CONFIG".to_string());
    }

    if let Some(digit_count) = normalized.digit_count {
      if !(1..=20).contains(&digit_count) {
        return Err("INVALID_CONFIG".to_string());
      }
    }

    if let Some(reuse_delay_ms) = normalized.reuse_delay_ms {
      if reuse_delay_ms < DEFAULT_REUSE_DELAY_MS {
        return Err("INVALID_CONFIG".to_string());
      }
    }

    let zone_id = normalized.zone_id.as_deref().unwrap_or_default();
    let zone_api_token = normalized.zone_api_token.as_deref().unwrap_or_default();
    let cdn_base_url = normalized.cdn_base_url.as_deref().unwrap_or_default();

    let has_zone_id = !zone_id.is_empty();
    let has_zone_api_token = !zone_api_token.is_empty();
    let has_cdn_base_url = !cdn_base_url.is_empty();

    if has_zone_id || has_zone_api_token || has_cdn_base_url {
      if !has_zone_id || !has_zone_api_token || !has_cdn_base_url {
        return Err("INVALID_CONFIG".to_string());
      }

      if !cdn_base_url.starts_with("http://") && !cdn_base_url.starts_with("https://") {
        return Err("INVALID_CONFIG".to_string());
      }
    }

    self.store.save(normalized);

    Ok(SaveSettingsResult {
      saved_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    })
  }

  pub fn get(&self) -> SettingsDraft {
    self
      .store
      .load()
      .map(Self::normalize_for_read)
      .unwrap_or_else(Self::default_draft)
  }

  pub fn snapshot(&self) -> SettingsSnapshot {
    let draft = self.get();
    let configured = Self::is_configured(&draft);

    SettingsSnapshot { draft, configured }
  }

  pub fn reset_app(&self) -> SettingsSnapshot {
    self.store.clear();
    self.snapshot()
  }

  pub async fn ping(&self) -> Result<ConnectionPingResult, String> {
    let Some(settings) = self.store.load() else {
      return Err("INVALID_CONFIG".to_string());
    };

    if settings.access_key.trim().is_empty()
      || settings.secret_key.trim().is_empty()
      || settings.endpoint.trim().is_empty()
      || settings.bucket.trim().is_empty()
    {
      return Err("INVALID_CONFIG".to_string());
    }

    let probe_url = resolve_bucket_base_url(
      settings.endpoint.as_str(),
      settings.bucket.as_str(),
    );

    let client = reqwest::Client::builder()
      .timeout(Duration::from_secs(5))
      .build()
      .map_err(|_| "INTERNAL_ERROR".to_string())?;

    let started_at = Instant::now();
    match client
      .head(probe_url)
      .header("x-imgstar-ping", "1")
      .send()
      .await
    {
      Ok(_) => Ok(ConnectionPingResult {
        latency_ms: started_at.elapsed().as_millis() as u64,
        checked_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
      }),
      Err(error) => {
        if error.is_timeout() {
          return Err("ADAPTER_TIMEOUT".to_string());
        }

        if error.is_connect() || error.is_request() {
          return Err("ADAPTER_NETWORK_ERROR".to_string());
        }

        Err("ADAPTER_SERVER_ERROR".to_string())
      }
    }
  }

  fn default_draft() -> SettingsDraft {
    SettingsDraft {
      access_key: String::new(),
      secret_key: String::new(),
      endpoint: String::new(),
      bucket: String::new(),
      zone_id: None,
      zone_api_token: None,
      cdn_base_url: None,
      region: Some(DEFAULT_REGION.to_string()),
      key_pattern: Some(Self::default_key_pattern(DEFAULT_DIGIT_COUNT)),
      digit_count: Some(DEFAULT_DIGIT_COUNT),
      reuse_delay_ms: Some(DEFAULT_REUSE_DELAY_MS),
      preview_hash_enabled: Some(true),
      theme: Some(DEFAULT_THEME.to_string()),
      language: Some(DEFAULT_LANGUAGE.to_string()),
    }
  }

  fn normalize_for_read(payload: SettingsDraft) -> SettingsDraft {
    let digit_count = payload
      .digit_count
      .map(|value| value.clamp(1, 20))
      .or(Some(DEFAULT_DIGIT_COUNT));

    SettingsDraft {
      access_key: payload.access_key.trim().to_string(),
      secret_key: payload.secret_key.trim().to_string(),
      endpoint: payload.endpoint.trim().to_string(),
      bucket: payload.bucket.trim().to_string(),
      zone_id: Self::normalize_optional_text(payload.zone_id),
      zone_api_token: Self::normalize_cloudflare_api_token(payload.zone_api_token),
      cdn_base_url: Self::normalize_optional_text(payload.cdn_base_url),
      region: Self::normalize_optional_text(payload.region)
        .or_else(|| Some(DEFAULT_REGION.to_string())),
      key_pattern: Self::normalize_optional_text(payload.key_pattern)
        .or_else(|| Some(Self::default_key_pattern(digit_count.unwrap_or(DEFAULT_DIGIT_COUNT)))),
      digit_count,
      reuse_delay_ms: payload
        .reuse_delay_ms
        .map(|value| value.max(DEFAULT_REUSE_DELAY_MS))
        .or(Some(DEFAULT_REUSE_DELAY_MS)),
      preview_hash_enabled: Some(payload.preview_hash_enabled.unwrap_or(true)),
      theme: Self::normalize_theme(payload.theme),
      language: Self::normalize_language(payload.language),
    }
  }

  fn normalize_for_save(payload: SettingsDraft) -> SettingsDraft {
    let digit_count = payload.digit_count.or(Some(DEFAULT_DIGIT_COUNT));

    SettingsDraft {
      access_key: payload.access_key.trim().to_string(),
      secret_key: payload.secret_key.trim().to_string(),
      endpoint: payload.endpoint.trim().to_string(),
      bucket: payload.bucket.trim().to_string(),
      zone_id: Self::normalize_optional_text(payload.zone_id),
      zone_api_token: Self::normalize_cloudflare_api_token(payload.zone_api_token),
      cdn_base_url: Self::normalize_optional_text(payload.cdn_base_url),
      region: Self::normalize_optional_text(payload.region)
        .or_else(|| Some(DEFAULT_REGION.to_string())),
      key_pattern: Self::normalize_optional_text(payload.key_pattern)
        .or_else(|| Some(Self::default_key_pattern(digit_count.unwrap_or(DEFAULT_DIGIT_COUNT)))),
      digit_count,
      reuse_delay_ms: payload.reuse_delay_ms.or(Some(DEFAULT_REUSE_DELAY_MS)),
      preview_hash_enabled: Some(payload.preview_hash_enabled.unwrap_or(true)),
      theme: Self::normalize_theme(payload.theme),
      language: Self::normalize_language(payload.language),
    }
  }

  fn is_configured(settings: &SettingsDraft) -> bool {
    if settings.access_key.trim().is_empty()
      || settings.secret_key.trim().is_empty()
      || settings.endpoint.trim().is_empty()
      || settings.bucket.trim().is_empty()
    {
      return false;
    }

    settings.endpoint.starts_with("http://") || settings.endpoint.starts_with("https://")
  }

  fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
      .as_deref()
      .map(str::trim)
      .filter(|text| !text.is_empty())
      .map(ToString::to_string)
  }

  fn normalize_cloudflare_api_token(value: Option<String>) -> Option<String> {
    let token = Self::normalize_optional_text(value)?;
    let token = token.trim_matches(|ch| ch == '"' || ch == '\'');

    let token = if token.len() >= 7 && token[..7].eq_ignore_ascii_case("Bearer ") {
      token[7..].trim_start()
    } else {
      token
    };

    if token.is_empty() {
      None
    } else {
      Some(token.to_string())
    }
  }

  fn normalize_theme(value: Option<String>) -> Option<String> {
    match value.as_deref().map(str::trim) {
      Some("light") => Some("light".to_string()),
      Some("dark") => Some("dark".to_string()),
      Some("system") => Some("system".to_string()),
      _ => Some(DEFAULT_THEME.to_string()),
    }
  }

  fn normalize_language(value: Option<String>) -> Option<String> {
    match value.as_deref().map(str::trim) {
      Some("zh-CN") => Some("zh-CN".to_string()),
      Some("en") => Some("en".to_string()),
      _ => Some(DEFAULT_LANGUAGE.to_string()),
    }
  }

  fn default_key_pattern(digit_count: u32) -> String {
    format!(
      "^/img/public/[0-9]{{{digit_count}}}\\.(?:{})$",
      DEFAULT_KEY_PATTERN_SUFFIXES.join("|")
    )
  }
}
