use crate::contracts::SettingsDraft;

#[derive(Clone, Default)]
pub struct SettingsValidator;

impl SettingsValidator {
  pub fn validate_save(&self, settings: &SettingsDraft) -> Result<(), String> {
    if !self.is_configured(settings)
      || settings.access_key.trim().is_empty()
      || settings.secret_key.trim().is_empty()
      || settings.endpoint.trim().is_empty()
      || settings.bucket.trim().is_empty()
      || (!settings.endpoint.starts_with("http://")
        && !settings.endpoint.starts_with("https://"))
    {
      return Err("INVALID_CONFIG".to_string());
    }

    if let Some(digit_count) = settings.digit_count {
      if !(1..=20).contains(&digit_count) {
        return Err("INVALID_CONFIG".to_string());
      }
    }

    let zone_id = settings.zone_id.as_deref().unwrap_or_default();
    let zone_api_token = settings.zone_api_token.as_deref().unwrap_or_default();
    let cdn_base_url = settings.cdn_base_url.as_deref().unwrap_or_default();

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

    Ok(())
  }

  pub fn validate_ping(&self, settings: &SettingsDraft) -> Result<(), String> {
    if settings.access_key.trim().is_empty()
      || settings.secret_key.trim().is_empty()
      || settings.endpoint.trim().is_empty()
      || settings.bucket.trim().is_empty()
    {
      return Err("INVALID_CONFIG".to_string());
    }

    Ok(())
  }

  pub fn is_configured(&self, settings: &SettingsDraft) -> bool {
    if settings.access_key.trim().is_empty()
      || settings.secret_key.trim().is_empty()
      || settings.endpoint.trim().is_empty()
      || settings.bucket.trim().is_empty()
    {
      return false;
    }

    settings.endpoint.starts_with("http://") || settings.endpoint.starts_with("https://")
  }
}

#[cfg(test)]
mod tests {
  use super::SettingsValidator;
  use crate::contracts::SettingsDraft;

  #[allow(deprecated)]
  fn configured_settings() -> SettingsDraft {
    SettingsDraft {
      access_key: "ak".to_string(),
      secret_key: "sk".to_string(),
      endpoint: "https://example.r2.dev".to_string(),
      bucket: "demo".to_string(),
      zone_id: None,
      zone_api_token: None,
      cdn_base_url: None,
      region: None,
      key_pattern: None,
      digit_count: Some(9),
      reuse_delay_ms: None,
      preview_hash_enabled: None,
      theme: None,
      language: None,
    }
  }

  #[test]
  #[allow(deprecated)]
  fn rejects_partial_cloudflare_configuration() {
    let validator = SettingsValidator;
    let mut settings = configured_settings();
    settings.zone_id = Some("zone-1".to_string());

    assert_eq!(validator.validate_save(&settings), Err("INVALID_CONFIG".to_string()));
  }

  #[test]
  #[allow(deprecated)]
  fn rejects_out_of_range_digit_count() {
    let validator = SettingsValidator;
    let mut settings = configured_settings();
    settings.digit_count = Some(21);

    assert_eq!(validator.validate_save(&settings), Err("INVALID_CONFIG".to_string()));
  }
}