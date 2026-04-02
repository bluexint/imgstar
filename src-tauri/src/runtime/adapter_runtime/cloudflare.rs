//! Cloudflare API transport helpers.
//!
//! This module owns cache purge and ruleset synchronization requests. The WAF
//! module builds expressions and allowlist fingerprints, while this module
//! sends them to Cloudflare and classifies API/transport failures.

use super::{
  compact_body_preview,
  IMGSTAR_WAF_RULE_DESCRIPTION,
  IMGSTAR_WAF_RULESET_DESCRIPTION,
  WafSyncOutcome,
};
use reqwest::{blocking::Client as BlockingHttpClient, Method};
use serde_json::{json, Value};
use std::time::Duration;

pub(super) fn build_public_file_url(cdn_base_url: &str, object_key: &str) -> String {
  let base = cdn_base_url.trim().trim_end_matches('/');
  let key = object_key
    .replace('\\', "/")
    .trim_start_matches('/')
    .to_string();
  format!("{base}/{key}")
}

pub(super) fn purge_cache_via_cloudflare(
  zone_id: &str,
  api_token: &str,
  file_url: &str,
) -> Result<(), (String, String)> {
  let endpoint = format!(
    "https://api.cloudflare.com/client/v4/zones/{zone_id}/purge_cache"
  );
  let payload = json!({ "files": [file_url] });
  let _ = request_cloudflare(
    Method::POST,
    endpoint.as_str(),
    api_token,
    Some(payload),
    "CACHE_PURGE_FAILED",
    "cache purge",
  )?;
  Ok(())
}

pub(super) fn sync_waf_rule_via_cloudflare(
  zone_id: &str,
  api_token: &str,
  current_ruleset_id: Option<&str>,
  expression: &str,
) -> Result<WafSyncOutcome, (String, String)> {
  let rulesets_endpoint = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/rulesets");
  let rule = json!({
    "description": IMGSTAR_WAF_RULE_DESCRIPTION,
    "expression": expression,
    "action": "block",
    "enabled": true
  });

  let create_payload = json!({
    "name": "imgstar-sequence-guard",
    "description": IMGSTAR_WAF_RULESET_DESCRIPTION,
    "kind": "custom",
    "phase": "http_request_firewall_custom",
    "rules": [rule.clone()]
  });

  let update_payload = json!({
    "description": IMGSTAR_WAF_RULESET_DESCRIPTION,
    "rules": [rule.clone()]
  });

  let mut clear_cached_ruleset_id = false;

  if let Some(ruleset_id) = current_ruleset_id {
    let update_endpoint = format!(
      "https://api.cloudflare.com/client/v4/zones/{zone_id}/rulesets/{ruleset_id}"
    );
    match request_cloudflare(
      Method::PUT,
      update_endpoint.as_str(),
      api_token,
      Some(update_payload),
      "WAF_RULE_SYNC_FAILED",
      "update waf sequence rule",
    ) {
      Ok(_) => return Ok(WafSyncOutcome::default()),
      Err((code, _message)) if code == "INVALID_CONFIG" => {
        clear_cached_ruleset_id = true;
      }
      Err(error) => return Err(error),
    }
  }

  let created_ruleset = match request_cloudflare(
    Method::POST,
    rulesets_endpoint.as_str(),
    api_token,
    Some(create_payload),
    "WAF_RULE_SYNC_FAILED",
    "create waf sequence rule",
  ) {
    Ok(response) => response,
    Err((code, message)) if is_custom_ruleset_limit_error(code.as_str(), message.as_str()) => {
      sync_waf_rule_via_entrypoint(zone_id, api_token, rule)?;
      return Ok(WafSyncOutcome {
        created_ruleset_id: None,
        clear_cached_ruleset_id: true,
      });
    }
    Err(error) => return Err(error),
  };

  let new_ruleset_id = created_ruleset
    .get("result")
    .and_then(|result| result.get("id"))
    .and_then(Value::as_str)
    .map(ToString::to_string)
    .ok_or_else(|| {
      (
        "WAF_RULE_SYNC_FAILED".to_string(),
        "create waf sequence rule failed: cloudflare response missing ruleset id".to_string(),
      )
    })?;

  Ok(WafSyncOutcome {
    created_ruleset_id: Some(new_ruleset_id),
    clear_cached_ruleset_id,
  })
}

fn sync_waf_rule_via_entrypoint(
  zone_id: &str,
  api_token: &str,
  rule: Value,
) -> Result<(), (String, String)> {
  let endpoint = format!(
    "https://api.cloudflare.com/client/v4/zones/{zone_id}/rulesets/phases/http_request_firewall_custom/entrypoint"
  );

  let existing_rules = match request_cloudflare(
    Method::GET,
    endpoint.as_str(),
    api_token,
    None,
    "WAF_RULE_SYNC_FAILED",
    "load waf phase entrypoint",
  ) {
    Ok(response) => response
      .get("result")
      .and_then(|result| result.get("rules"))
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default(),
    Err((code, _message)) if code == "INVALID_CONFIG" => Vec::new(),
    Err(error) => return Err(error),
  };

  let mut merged_rules = existing_rules
    .into_iter()
    .filter(|existing| !is_imgstar_managed_waf_rule(existing))
    .collect::<Vec<_>>();
  merged_rules.push(rule);

  let payload = json!({
    "description": IMGSTAR_WAF_RULESET_DESCRIPTION,
    "rules": merged_rules
  });

  let _ = request_cloudflare(
    Method::PUT,
    endpoint.as_str(),
    api_token,
    Some(payload),
    "WAF_RULE_SYNC_FAILED",
    "update waf phase entrypoint",
  )?;

  Ok(())
}

fn is_imgstar_managed_waf_rule(rule: &Value) -> bool {
  rule
    .get("description")
    .and_then(Value::as_str)
    .map(|description| description == IMGSTAR_WAF_RULE_DESCRIPTION)
    .unwrap_or(false)
}

fn is_custom_ruleset_limit_error(code: &str, message: &str) -> bool {
  if code != "INVALID_CONFIG" {
    return false;
  }

  let lowered = message.to_ascii_lowercase();
  lowered.contains("exceeded maximum number of custom rulesets")
    && lowered.contains("http_request_firewall_custom")
}

fn request_cloudflare(
  method: Method,
  endpoint: &str,
  api_token: &str,
  payload: Option<Value>,
  default_error_code: &str,
  operation: &str,
) -> Result<Value, (String, String)> {
  let api_token = normalize_cloudflare_api_token(api_token);
  let client = BlockingHttpClient::builder()
    .timeout(Duration::from_secs(10))
    .build()
    .map_err(|_| {
      (
        default_error_code.to_string(),
        format!("{operation} failed: cloudflare client initialization failed"),
      )
    })?;

  let mut request = client
    .request(method, endpoint)
    .header("Authorization", format!("Bearer {api_token}"))
    .header("Accept", "application/json");

  if payload.is_some() {
    request = request.header("Content-Type", "application/json");
  }

  if let Some(payload) = payload {
    request = request.body(payload.to_string());
  }

  let response = request
    .send()
    .map_err(|error| classify_cloudflare_transport_error(&error, default_error_code, operation))?;

  let status = response.status();
  let raw_body = response.text().unwrap_or_default();
  if !status.is_success() {
    return Err(classify_cloudflare_http_error(
      status.as_u16(),
      raw_body.as_str(),
      default_error_code,
      operation,
    ));
  }

  if raw_body.trim().is_empty() {
    return Ok(Value::Null);
  }

  let parsed = serde_json::from_str::<Value>(raw_body.as_str()).map_err(|_| {
    (
      default_error_code.to_string(),
      format!("{operation} failed: cloudflare returned non-json response"),
    )
  })?;

  if let Some(false) = parsed.get("success").and_then(Value::as_bool) {
    let message = extract_cloudflare_error_message(parsed.get("errors"));
    return Err((
      default_error_code.to_string(),
      format!("{operation} failed: {message}"),
    ));
  }

  Ok(parsed)
}

fn normalize_cloudflare_api_token(value: &str) -> String {
  let token = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');

  if token.len() >= 7 && token[..7].eq_ignore_ascii_case("Bearer ") {
    token[7..].trim_start().to_string()
  } else {
    token.to_string()
  }
}

fn classify_cloudflare_transport_error(
  error: &reqwest::Error,
  default_error_code: &str,
  operation: &str,
) -> (String, String) {
  if error.is_timeout() {
    return (
      "ADAPTER_TIMEOUT".to_string(),
      format!("{operation} failed: cloudflare request timeout"),
    );
  }

  if error.is_connect() || error.is_request() {
    return (
      "ADAPTER_NETWORK_ERROR".to_string(),
      format!("{operation} failed: cloudflare network error"),
    );
  }

  (
    default_error_code.to_string(),
    format!("{operation} failed: cloudflare request error"),
  )
}

fn classify_cloudflare_http_error(
  status: u16,
  body: &str,
  default_error_code: &str,
  operation: &str,
) -> (String, String) {
  let parsed = serde_json::from_str::<Value>(body).ok();
  let detail = if let Some(parsed) = parsed.as_ref() {
    extract_cloudflare_error_message(parsed.get("errors"))
  } else {
    compact_body_preview(body).unwrap_or_else(|| "cloudflare api error".to_string())
  };
  let message = format!("{operation} failed: http {status}, {detail}");
  let lowered_detail = detail.to_ascii_lowercase();

  if status == 401
    || status == 403
    || (status == 400
      && (lowered_detail.contains("auth")
        || lowered_detail.contains("unauthorized")
        || lowered_detail.contains("invalid token")))
  {
    return ("ADAPTER_AUTH_ERROR".to_string(), message);
  }

  if status == 429 {
    return ("ADAPTER_RATE_LIMITED".to_string(), message);
  }

  if status == 400 || status == 404 {
    return ("INVALID_CONFIG".to_string(), message);
  }

  (default_error_code.to_string(), message)
}

fn extract_cloudflare_error_message(errors_value: Option<&Value>) -> String {
  if let Some(errors) = errors_value.and_then(Value::as_array) {
    if let Some(first) = errors.first() {
      if let Some(message) = first.get("message").and_then(Value::as_str) {
        return compact_body_preview(message).unwrap_or_else(|| message.to_string());
      }
      if let Some(code) = first.get("code") {
        return format!("cloudflare error code {code}");
      }
    }
  }

  if let Some(raw_text) = errors_value.and_then(Value::as_str) {
    return compact_body_preview(raw_text).unwrap_or_else(|| "cloudflare api error".to_string());
  }

  "cloudflare api error".to_string()
}

#[cfg(test)]
mod tests {
  use super::{
    build_public_file_url,
    classify_cloudflare_http_error,
    is_custom_ruleset_limit_error,
    normalize_cloudflare_api_token,
  };

  #[test]
  fn builds_public_url_for_cache_purge() {
    let url = build_public_file_url(
      "https://cdn.example.com/",
      "/img/public/000000001.png",
    );
    assert_eq!(url, "https://cdn.example.com/img/public/000000001.png");
  }

  #[test]
  fn normalizes_cloudflare_api_token_with_bearer_prefix() {
    assert_eq!(
      normalize_cloudflare_api_token("  Bearer abc123  "),
      "abc123"
    );
  }

  #[test]
  fn classifies_cloudflare_authentication_failed_400_as_auth_error() {
    let body = r#"{"success":false,"errors":[{"message":"Authentication failed"}]}"#;
    let (code, message) = classify_cloudflare_http_error(
      400,
      body,
      "WAF_RULE_SYNC_FAILED",
      "sync waf ruleset",
    );

    assert_eq!(code, "ADAPTER_AUTH_ERROR");
    assert!(message.contains("Authentication failed"));
  }

  #[test]
  fn identifies_custom_ruleset_quota_error_for_waf_phase() {
    let code = "INVALID_CONFIG";
    let message =
      "create waf sequence rule failed: http 400, exceeded maximum number of custom rulesets for the phase http_request_firewall_custom, max allowed: 0";

    assert!(is_custom_ruleset_limit_error(code, message));
    assert!(!is_custom_ruleset_limit_error(
      code,
      "create waf sequence rule failed: http 400, missing zone id"
    ));
    assert!(!is_custom_ruleset_limit_error(
      "WAF_RULE_SYNC_FAILED",
      message
    ));
  }
}