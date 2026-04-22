use crate::contracts::SettingsDraft;

const DEFAULT_REGION: &str = "auto";
const DEFAULT_DIGIT_COUNT: u32 = 9;
const DEFAULT_REUSE_DELAY_MS: u64 = 900_000;
const DEFAULT_THEME: &str = "system";
const DEFAULT_LANGUAGE: &str = "zh-CN";
const DEFAULT_KEY_PATTERN_SUFFIXES: [&str; 7] = ["bmp", "gif", "jpeg", "jpg", "png", "svg", "webp"];

#[derive(Clone, Default)]
pub struct SettingsNormalizer;

impl SettingsNormalizer {
  #[allow(deprecated)]
  pub fn default_draft(&self) -> SettingsDraft {
    SettingsDraft {
      access_key: String::new(),
      secret_key: String::new(),
      endpoint: String::new(),
      bucket: String::new(),
      zone_id: None,
      zone_api_token: None,
      cdn_base_url: None,
      region: Some(DEFAULT_REGION.to_string()),
      key_pattern: Some(self.default_key_pattern(DEFAULT_DIGIT_COUNT)),
      digit_count: Some(DEFAULT_DIGIT_COUNT),
      reuse_delay_ms: Some(DEFAULT_REUSE_DELAY_MS),
      preview_hash_enabled: Some(true),
      theme: Some(DEFAULT_THEME.to_string()),
      language: Some(DEFAULT_LANGUAGE.to_string()),
    }
  }

  #[allow(deprecated)]
  pub fn normalize_for_read(&self, payload: SettingsDraft) -> SettingsDraft {
    let digit_count = payload
      .digit_count
      .map(|value| value.clamp(1, 20))
      .or(Some(DEFAULT_DIGIT_COUNT));

    SettingsDraft {
      access_key: payload.access_key.trim().to_string(),
      secret_key: payload.secret_key.trim().to_string(),
      endpoint: payload.endpoint.trim().to_string(),
      bucket: payload.bucket.trim().to_string(),
      zone_id: normalize_optional_text(payload.zone_id),
      zone_api_token: normalize_cloudflare_api_token(payload.zone_api_token),
      cdn_base_url: normalize_optional_text(payload.cdn_base_url),
      region: normalize_optional_text(payload.region)
        .or_else(|| Some(DEFAULT_REGION.to_string())),
      key_pattern: normalize_optional_text(payload.key_pattern)
        .or_else(|| Some(self.default_key_pattern(digit_count.unwrap_or(DEFAULT_DIGIT_COUNT)))),
      digit_count,
      reuse_delay_ms: payload
        .reuse_delay_ms
        .map(|value| value.max(DEFAULT_REUSE_DELAY_MS))
        .or(Some(DEFAULT_REUSE_DELAY_MS)),
      preview_hash_enabled: Some(payload.preview_hash_enabled.unwrap_or(true)),
      theme: normalize_theme(payload.theme),
      language: normalize_language(payload.language),
    }
  }

  #[allow(deprecated)]
  pub fn normalize_for_save(&self, payload: SettingsDraft) -> SettingsDraft {
    let digit_count = payload.digit_count.or(Some(DEFAULT_DIGIT_COUNT));

    SettingsDraft {
      access_key: payload.access_key.trim().to_string(),
      secret_key: payload.secret_key.trim().to_string(),
      endpoint: payload.endpoint.trim().to_string(),
      bucket: payload.bucket.trim().to_string(),
      zone_id: normalize_optional_text(payload.zone_id),
      zone_api_token: normalize_cloudflare_api_token(payload.zone_api_token),
      cdn_base_url: normalize_optional_text(payload.cdn_base_url),
      region: normalize_optional_text(payload.region)
        .or_else(|| Some(DEFAULT_REGION.to_string())),
      key_pattern: normalize_optional_text(payload.key_pattern)
        .or_else(|| Some(self.default_key_pattern(digit_count.unwrap_or(DEFAULT_DIGIT_COUNT)))),
      digit_count,
      reuse_delay_ms: payload.reuse_delay_ms.or(Some(DEFAULT_REUSE_DELAY_MS)),
      preview_hash_enabled: Some(payload.preview_hash_enabled.unwrap_or(true)),
      theme: normalize_theme(payload.theme),
      language: normalize_language(payload.language),
    }
  }

  fn default_key_pattern(&self, digit_count: u32) -> String {
    format!(
      "^/img/public/[0-9]{{{digit_count}}}\\.(?:{})$",
      DEFAULT_KEY_PATTERN_SUFFIXES.join("|")
    )
  }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
  value
    .as_deref()
    .map(str::trim)
    .filter(|text| !text.is_empty())
    .map(ToString::to_string)
}

fn normalize_cloudflare_api_token(value: Option<String>) -> Option<String> {
  let token = normalize_optional_text(value)?;
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

#[cfg(test)]
mod tests {
  use super::SettingsNormalizer;
  use crate::contracts::SettingsDraft;

  #[test]
  #[allow(deprecated)]
  fn normalizes_optional_fields_for_save() {
    let normalizer = SettingsNormalizer;
    let normalized = normalizer.normalize_for_save(SettingsDraft {
      access_key: " ak ".to_string(),
      secret_key: " sk ".to_string(),
      endpoint: " https://example.r2.dev ".to_string(),
      bucket: " demo ".to_string(),
      zone_id: Some(" zone-1 ".to_string()),
      zone_api_token: Some(" Bearer token-1 ".to_string()),
      cdn_base_url: Some(" https://cdn.example.com ".to_string()),
      region: None,
      key_pattern: None,
      digit_count: None,
      reuse_delay_ms: None,
      preview_hash_enabled: None,
      theme: Some("invalid".to_string()),
      language: Some("unknown".to_string()),
    });

    assert_eq!(normalized.access_key, "ak");
    assert_eq!(normalized.secret_key, "sk");
    assert_eq!(normalized.zone_api_token.as_deref(), Some("token-1"));
    assert_eq!(normalized.region.as_deref(), Some("auto"));
    assert_eq!(normalized.theme.as_deref(), Some("system"));
    assert_eq!(normalized.language.as_deref(), Some("zh-CN"));
  }
}