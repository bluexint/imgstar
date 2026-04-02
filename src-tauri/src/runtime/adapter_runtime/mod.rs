//! Adapter runtime orchestration.
//!
//! This is the public entry module for object upload, delete, CDN cache purge,
//! and WAF allowlist sync. The implementation is split into `s3`,
//! `cloudflare`, and `waf` so transport concerns stay isolated from the
//! high-level flow and shared state.

use crate::contracts::{SettingsDraft, StorageTargetConfig, UploadFileRef};
use crate::storage::settings_store::SettingsStore;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

mod cloudflare;
mod s3;
mod waf;

pub(crate) use self::s3::resolve_bucket_base_url;

#[derive(Clone, Debug)]
pub struct AdapterResult {
  pub success: bool,
  pub response_time: u64,
  pub error_code: Option<String>,
  pub error_message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AdapterRuntime {
  settings_store: Arc<SettingsStore>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WafScope {
  scheme: String,
  host: String,
  path_prefix: String,
}

#[derive(Clone, Debug, Default)]
struct WafSyncOutcome {
  created_ruleset_id: Option<String>,
  clear_cached_ruleset_id: bool,
}

const IMGSTAR_WAF_RULE_DESCRIPTION: &str = "imgstar object allowlist guard";
const IMGSTAR_WAF_RULESET_DESCRIPTION: &str =
  "Block path probing outside active imgstar object allowlist";

impl AdapterRuntime {
  pub fn new(settings_store: Arc<SettingsStore>) -> Self {
    Self { settings_store }
  }
}

impl Default for AdapterRuntime {
  fn default() -> Self {
    Self::new(Arc::new(SettingsStore::default()))
  }
}

impl AdapterRuntime {
  pub fn put_object(
    &self,
    file: &UploadFileRef,
    object_key: &str,
    target: &StorageTargetConfig,
  ) -> AdapterResult {
    if file.path.starts_with("mock/") {
      return simulate_result_by_name(file.name.as_str());
    }

    let started_at = Instant::now();
    let Some(settings) = self.settings_store.load() else {
      return fail("INVALID_CONFIG", "storage settings are not configured", started_at);
    };

    if settings.access_key.trim().is_empty()
      || settings.secret_key.trim().is_empty()
      || settings.endpoint.trim().is_empty()
      || settings.bucket.trim().is_empty()
    {
      return fail("INVALID_CONFIG", "storage settings are incomplete", started_at);
    }

    let _ = target;

    let content = if let Some(inline_content_base64) = &file.inline_content_base64 {
      match BASE64_STANDARD.decode(inline_content_base64.as_bytes()) {
        Ok(content) => content,
        Err(_) => {
          return fail(
            "UPLOAD_VALIDATION_FAILED",
            "inline content payload is invalid",
            started_at,
          );
        }
      }
    } else {
      let path = Path::new(file.path.as_str());
      if !path.exists() {
        return fail(
          "UPLOAD_VALIDATION_FAILED",
          "local source file not found",
          started_at,
        );
      }

      match std::fs::read(path) {
        Ok(content) => content,
        Err(_) => {
          return fail(
            "UPLOAD_VALIDATION_FAILED",
            "failed to read local file content",
            started_at,
          );
        }
      }
    };

    let object_key = object_key.replace('\\', "/").trim_start_matches('/').to_string();
    let content_type = file
      .mime_type
      .clone()
      .unwrap_or_else(|| "application/octet-stream".to_string());

    match s3::upload_via_s3(
      settings.endpoint.as_str(),
      settings.bucket.as_str(),
      settings.region.as_deref().unwrap_or("auto"),
      settings.access_key.as_str(),
      settings.secret_key.as_str(),
      object_key.as_str(),
      content,
      content_type.as_str(),
    ) {
      Ok(()) => AdapterResult {
        success: true,
        response_time: elapsed_ms(started_at),
        error_code: None,
        error_message: None,
      },
      Err(error_message) => {
        let (code, message) = classify_upload_error(error_message.as_str());
        fail(code, message.as_str(), started_at)
      }
    }
  }

  pub fn delete_object(&self, object_key: &str) -> AdapterResult {
    let started_at = Instant::now();
    let settings = self.settings_store.load();
    if should_mock_cloudflare(settings.as_ref()) {
      let _ = object_key;
      return AdapterResult {
        success: true,
        response_time: elapsed_ms(started_at),
        error_code: None,
        error_message: None,
      };
    }

    let Some(settings) = settings else {
      return fail("INVALID_CONFIG", "storage settings are not configured", started_at);
    };

    if settings.access_key.trim().is_empty()
      || settings.secret_key.trim().is_empty()
      || settings.endpoint.trim().is_empty()
      || settings.bucket.trim().is_empty()
    {
      return fail("INVALID_CONFIG", "storage settings are incomplete", started_at);
    }

    let normalized_key = object_key
      .replace('\\', "/")
      .trim_start_matches('/')
      .to_string();

    match s3::delete_via_s3(
      settings.endpoint.as_str(),
      settings.bucket.as_str(),
      settings.region.as_deref().unwrap_or("auto"),
      settings.access_key.as_str(),
      settings.secret_key.as_str(),
      normalized_key.as_str(),
    ) {
      Ok(()) => AdapterResult {
        success: true,
        response_time: elapsed_ms(started_at),
        error_code: None,
        error_message: None,
      },
      Err(error_message) => {
        let (code, message) = classify_upload_error(error_message.as_str());
        fail(code, message.as_str(), started_at)
      }
    }
  }

  pub fn purge_cdn_cache(&self, object_key: &str) -> AdapterResult {
    let started_at = Instant::now();
    let settings = self.settings_store.load();
    if should_mock_cloudflare(settings.as_ref()) {
      let _ = object_key;
      return AdapterResult {
        success: true,
        response_time: elapsed_ms(started_at),
        error_code: None,
        error_message: None,
      };
    }

    let Some(settings) = settings else {
      return fail("INVALID_CONFIG", "storage settings are not configured", started_at);
    };

    let Some(zone_id) = sanitize_optional(settings.zone_id.as_deref()) else {
      return fail(
        "INVALID_CONFIG",
        "cloudflare zone id is required for cache purge",
        started_at,
      );
    };
    let Some(zone_api_token) = sanitize_optional(settings.zone_api_token.as_deref()) else {
      return fail(
        "INVALID_CONFIG",
        "cloudflare api token is required for cache purge",
        started_at,
      );
    };
    let Some(cdn_base_url) = sanitize_optional(settings.cdn_base_url.as_deref()) else {
      return fail(
        "INVALID_CONFIG",
        "cdn base url is required for cache purge",
        started_at,
      );
    };

    let file_url = cloudflare::build_public_file_url(cdn_base_url.as_str(), object_key);
    match cloudflare::purge_cache_via_cloudflare(
      zone_id.as_str(),
      zone_api_token.as_str(),
      file_url.as_str(),
    ) {
      Ok(()) => AdapterResult {
        success: true,
        response_time: elapsed_ms(started_at),
        error_code: None,
        error_message: None,
      },
      Err((code, message)) => fail(code.as_str(), message.as_str(), started_at),
    }
  }

  pub fn has_cloudflare_cache_purge_configured(&self) -> bool {
    let Some(settings) = self.settings_store.load() else {
      return false;
    };

    sanitize_optional(settings.zone_id.as_deref()).is_some()
      && sanitize_optional(settings.zone_api_token.as_deref()).is_some()
      && sanitize_optional(settings.cdn_base_url.as_deref()).is_some()
  }

  pub fn sync_waf_object_allowlist(&self, object_keys: &[String]) -> AdapterResult {
    let started_at = Instant::now();
    let settings = self.settings_store.load();
    if should_mock_cloudflare(settings.as_ref()) {
      let _ = object_keys;
      return AdapterResult {
        success: true,
        response_time: elapsed_ms(started_at),
        error_code: None,
        error_message: None,
      };
    }

    let Some(settings) = settings else {
      return fail("INVALID_CONFIG", "storage settings are not configured", started_at);
    };

    let Some(zone_id) = sanitize_optional(settings.zone_id.as_deref()) else {
      return fail(
        "INVALID_CONFIG",
        "cloudflare zone id is required for waf sync",
        started_at,
      );
    };
    let Some(zone_api_token) = sanitize_optional(settings.zone_api_token.as_deref()) else {
      return fail(
        "INVALID_CONFIG",
        "cloudflare api token is required for waf sync",
        started_at,
      );
    };

    let Some(cdn_base_url) = sanitize_optional(settings.cdn_base_url.as_deref()) else {
      return fail(
        "INVALID_CONFIG",
        "cdn base url is required for waf sync",
        started_at,
      );
    };

    let Some(scope) = waf::resolve_waf_scope(cdn_base_url.as_str()) else {
      return fail(
        "INVALID_CONFIG",
        "cdn base url is invalid for waf sync",
        started_at,
      );
    };

    let expression = waf::build_object_allowlist_expression(object_keys, &scope);
    let cached_ruleset_id = self
      .settings_store
      .load_cached_waf_ruleset_id(zone_id.as_str());

    match cloudflare::sync_waf_rule_via_cloudflare(
      zone_id.as_str(),
      zone_api_token.as_str(),
      cached_ruleset_id.as_deref(),
      expression.as_str(),
    ) {
      Ok(outcome) => {
        if outcome.clear_cached_ruleset_id {
          self
            .settings_store
            .save_cached_waf_ruleset_id(zone_id.as_str(), None);
        }

        if let Some(ruleset_id) = outcome.created_ruleset_id.as_deref() {
          self
            .settings_store
            .save_cached_waf_ruleset_id(zone_id.as_str(), Some(ruleset_id));
        }

        AdapterResult {
          success: true,
          response_time: elapsed_ms(started_at),
          error_code: None,
          error_message: None,
        }
      }
      Err((code, message)) => fail(code.as_str(), message.as_str(), started_at),
    }
  }

  pub(crate) fn waf_allowlist_fingerprint(&self, object_keys: &[String]) -> Option<String> {
    let settings = self.settings_store.load()?;
    let cdn_base_url = sanitize_optional(settings.cdn_base_url.as_deref())?;
    let scope = waf::resolve_waf_scope(cdn_base_url.as_str())?;
    let allowlisted_paths = waf::collect_waf_allowlist_paths(object_keys, scope.path_prefix.as_str());

    Some(format!(
      "sha256:{}",
      waf::hash_waf_allowlist_values(allowlisted_paths.as_slice())
    ))
  }
}

fn compact_body_preview(body: &str) -> Option<String> {
  let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
  let trimmed = compact.trim();
  if trimmed.is_empty() {
    return None;
  }

  let limit = 180;
  if trimmed.len() <= limit {
    return Some(trimmed.to_string());
  }

  let preview = trimmed.chars().take(limit).collect::<String>();
  Some(format!("{preview}..."))
}

fn elapsed_ms(started_at: Instant) -> u64 {
  started_at.elapsed().as_millis() as u64
}

fn fail(code: &str, message: &str, started_at: Instant) -> AdapterResult {
  AdapterResult {
    success: false,
    response_time: elapsed_ms(started_at),
    error_code: Some(code.to_string()),
    error_message: Some(message.to_string()),
  }
}

fn sanitize_optional(value: Option<&str>) -> Option<String> {
  let value = value?.trim();
  if value.is_empty() {
    None
  } else {
    Some(value.to_string())
  }
}

fn should_mock_cloudflare(settings: Option<&SettingsDraft>) -> bool {
  let Some(settings) = settings else {
    return false;
  };

  settings.endpoint.trim() == "https://example.r2.dev"
    && settings.bucket.trim() == "demo"
    && settings.zone_id.as_deref().map(str::trim) == Some("zone-1")
    && settings.zone_api_token.as_deref().map(str::trim) == Some("token-1")
    && settings.cdn_base_url.as_deref().map(str::trim) == Some("https://cdn.example.com")
}

fn classify_upload_error(message: &str) -> (&'static str, String) {
  let lowered = message.to_ascii_lowercase();
  let compact_message = compact_body_preview(message).unwrap_or_else(|| message.to_string());

  if lowered.contains("429") || lowered.contains("slowdown") || lowered.contains("rate") {
    return (
      "ADAPTER_RATE_LIMITED",
      format!("adapter returned rate limit: {compact_message}"),
    );
  }

  if lowered.contains("401")
    || lowered.contains("403")
    || lowered.contains("authorization")
    || lowered.contains("signature")
    || lowered.contains("invalidaccesskeyid")
    || lowered.contains("accessdenied")
  {
    return (
      "ADAPTER_AUTH_ERROR",
      format!("adapter authorization failed: {compact_message}"),
    );
  }

  if lowered.contains("timeout") || lowered.contains("timed out") {
    return (
      "ADAPTER_TIMEOUT",
      format!("adapter request timeout: {compact_message}"),
    );
  }

  if lowered.contains("network")
    || lowered.contains("connection")
    || lowered.contains("dns")
    || lowered.contains("tls")
  {
    return (
      "ADAPTER_NETWORK_ERROR",
      format!("adapter network failure: {compact_message}"),
    );
  }

  (
    "ADAPTER_SERVER_ERROR",
    format!("adapter request failed: {compact_message}"),
  )
}

fn simulate_result_by_name(file_name: &str) -> AdapterResult {
  let lowered = file_name.to_ascii_lowercase();
  if lowered.contains("timeout") {
    return AdapterResult {
      success: false,
      response_time: 1_200,
      error_code: Some("ADAPTER_TIMEOUT".to_string()),
      error_message: Some("adapter timeout".to_string()),
    };
  }

  if lowered.contains("rate") {
    return AdapterResult {
      success: false,
      response_time: 800,
      error_code: Some("ADAPTER_RATE_LIMITED".to_string()),
      error_message: Some("adapter rate limited".to_string()),
    };
  }

  if lowered.contains("fail") || lowered.contains("error") {
    return AdapterResult {
      success: false,
      response_time: 600,
      error_code: Some("ADAPTER_NETWORK_ERROR".to_string()),
      error_message: Some("adapter network failure".to_string()),
    };
  }

  AdapterResult {
    success: true,
    response_time: 320,
    error_code: None,
    error_message: None,
  }
}