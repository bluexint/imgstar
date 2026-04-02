//! WAF allowlist and expression builder.
//!
//! `AdapterRuntime` delegates WAF scope parsing, allowlist filtering, and
//! fingerprint generation here. The resulting expression is consumed by the
//! Cloudflare transport module when syncing zone rules.

use super::WafScope;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[cfg(test)]
use std::collections::BTreeMap;

pub(super) fn resolve_waf_scope(cdn_base_url: &str) -> Option<WafScope> {
  let parsed = reqwest::Url::parse(cdn_base_url.trim()).ok()?;
  let scheme = parsed.scheme().to_ascii_lowercase();
  let host = parsed.host_str()?.to_ascii_lowercase();
  let path_prefix = normalize_waf_path_prefix(parsed.path());

  Some(WafScope {
    scheme,
    host,
    path_prefix,
  })
}

fn normalize_waf_path_prefix(value: &str) -> String {
  let cleaned = value
    .trim()
    .replace('\\', "/")
    .trim_end_matches('/')
    .to_string();

  if cleaned.is_empty() || cleaned == "/" {
    return String::new();
  }

  if cleaned.starts_with('/') {
    cleaned
  } else {
    format!("/{cleaned}")
  }
}

fn build_waf_guarded_path_prefix(path_prefix: &str) -> String {
  if path_prefix.is_empty() {
    "/img/public/".to_string()
  } else {
    format!("{path_prefix}/img/public/")
  }
}

pub(super) fn build_object_allowlist_expression(object_keys: &[String], scope: &WafScope) -> String {
  let host_condition = format!("http.host eq {}", quote_waf_string(scope.host.as_str()));
  let allowlisted_paths = collect_waf_allowlist_paths(object_keys, scope.path_prefix.as_str());

  if allowlisted_paths.is_empty() {
    return host_condition;
  }

  format!(
    "{host_condition} and not (raw.http.request.uri.path in {})",
    build_waf_set_literal(allowlisted_paths.as_slice())
  )
}

pub(super) fn collect_waf_allowlist_paths(object_keys: &[String], path_prefix: &str) -> Vec<String> {
  let mut unique_uris = BTreeSet::new();
  let guarded_prefix = build_waf_guarded_path_prefix(path_prefix);

  for object_key in object_keys {
    if let Some(normalized) = normalize_waf_object_key(object_key.as_str(), path_prefix) {
      if is_strict_allowlist_path(normalized.as_str(), guarded_prefix.as_str()) {
        unique_uris.insert(normalized);
      }
    }
  }

  unique_uris.into_iter().collect::<Vec<_>>()
}

fn build_waf_set_literal(values: &[String]) -> String {
  let quoted_values = values
    .iter()
    .map(|value| quote_waf_string(value.as_str()))
    .collect::<Vec<_>>();

  format!("{{{}}}", quoted_values.join(" "))
}

fn contains_waf_bypass_tokens(value: &str) -> bool {
  if value.chars().any(|ch| {
    ch.is_ascii_control()
      || ch.is_whitespace()
      || !ch.is_ascii()
      || matches!(ch, '^' | '$' | ';' | '%' | '?' | '#' | '=')
  }) {
    return true;
  }

  let normalized = value.replace('\\', "/");
  normalized.contains("//")
    || normalized.contains("/./")
    || normalized.contains("/../")
    || normalized.starts_with("./")
    || normalized.starts_with("../")
    || normalized.ends_with("/.")
    || normalized.ends_with("/..")
    || normalized == "."
    || normalized == ".."
}

fn is_strict_allowlist_path(path: &str, guarded_prefix: &str) -> bool {
  if !path.starts_with(guarded_prefix) {
    return false;
  }

  let file_part = &path[guarded_prefix.len()..];
  if file_part.is_empty() || file_part.contains('/') || contains_waf_bypass_tokens(file_part) {
    return false;
  }

  let Some((number, suffix)) = file_part.rsplit_once('.') else {
    return false;
  };

  if number.is_empty() || number.len() > 20 || !number.chars().all(|ch| ch.is_ascii_digit()) {
    return false;
  }

  if suffix.is_empty()
    || suffix.len() > 16
    || !suffix.chars().all(|ch| ch.is_ascii_alphanumeric())
  {
    return false;
  }

  true
}

pub(super) fn hash_waf_allowlist_values(values: &[String]) -> String {
  let mut ordered_values = values.to_vec();
  ordered_values.sort();
  ordered_values.dedup();

  let mut hasher = Sha256::new();
  for value in ordered_values {
    hasher.update(value.as_bytes());
    hasher.update([0]);
  }

  format!("{:x}", hasher.finalize())
}

fn normalize_waf_object_key(value: &str, path_prefix: &str) -> Option<String> {
  let trimmed = value.trim();
  if trimmed.is_empty() || contains_waf_bypass_tokens(trimmed) {
    return None;
  }

  let cleaned = trimmed
    .replace('\\', "/")
    .trim_start_matches('/')
    .to_string();

  if cleaned.is_empty() || contains_waf_bypass_tokens(cleaned.as_str()) {
    None
  } else {
    let normalized_path = format!("/{cleaned}");
    if path_prefix.is_empty() {
      Some(normalized_path)
    } else {
      Some(format!("{path_prefix}{normalized_path}"))
    }
  }
}

fn quote_waf_string(value: &str) -> String {
  serde_json::to_string(value).unwrap_or_else(|_| {
    let escaped = value
      .replace('\\', "\\\\")
      .replace('"', "\\\"");
    format!("\"{escaped}\"")
  })
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct WafObjectGroup {
  prefix: String,
  suffix: String,
  numbers: Vec<String>,
}

#[cfg(test)]
fn escape_waf_regex(value: &str) -> String {
  value
    .chars()
    .fold(String::new(), |mut output, ch| {
      if matches!(
        ch,
        '.' | '+' | '*' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '[' | ']' | '|' | '\\'
      ) {
        output.push('\\');
      }
      output.push(ch);
      output
    })
}

#[cfg(test)]
fn build_digit_range(start: u32, end: u32) -> String {
  if start == end {
    start.to_string()
  } else {
    format!("[{start}-{end}]")
  }
}

#[cfg(test)]
fn parse_waf_object_key_parts(value: &str, path_prefix: &str) -> Option<(String, String, String)> {
  let normalized = normalize_waf_object_key(value, path_prefix)?;
  let last_slash = normalized.rfind('/')?;
  let last_dot = normalized.rfind('.')?;

  if last_dot <= last_slash || last_dot + 1 >= normalized.len() {
    return None;
  }

  let number = &normalized[last_slash + 1..last_dot];
  if !number.chars().all(|ch| ch.is_ascii_digit()) {
    return None;
  }

  let prefix = normalized[..last_slash + 1].to_string();
  let suffix = normalized[last_dot + 1..].to_string();
  Some((prefix, number.to_string(), suffix))
}

#[cfg(test)]
fn build_numeric_range_regex(start: &str, end: &str) -> String {
  if start == end {
    return start.to_string();
  }

  let start_bytes = start.as_bytes();
  let end_bytes = end.as_bytes();
  let mut prefix_len = 0;
  while prefix_len < start_bytes.len() && start_bytes[prefix_len] == end_bytes[prefix_len] {
    prefix_len += 1;
  }

  let prefix = &start[..prefix_len];
  let start_digit = (start_bytes[prefix_len] - b'0') as u32;
  let end_digit = (end_bytes[prefix_len] - b'0') as u32;
  let remaining_len = start_bytes.len() - prefix_len - 1;

  if remaining_len == 0 {
    return format!("{prefix}{}", build_digit_range(start_digit, end_digit));
  }

  let mut parts = vec![build_numeric_range_regex(
    start,
    &format!("{prefix}{}{}", start_bytes[prefix_len] as char, "9".repeat(remaining_len)),
  )];

  if start_digit + 1 <= end_digit.saturating_sub(1) {
    parts.push(format!(
      "{prefix}{}[0-9]{{{remaining_len}}}",
      build_digit_range(start_digit + 1, end_digit - 1)
    ));
  }

  parts.push(build_numeric_range_regex(
    &format!("{prefix}{}{}", end_bytes[prefix_len] as char, "0".repeat(remaining_len)),
    end,
  ));

  parts.sort();
  parts.dedup();
  if parts.len() == 1 {
    parts.remove(0)
  } else {
    format!("(?:{})", parts.join("|"))
  }
}

#[cfg(test)]
fn collect_waf_object_fragments(object_keys: &[String], path_prefix: &str) -> Vec<String> {
  let mut grouped: BTreeMap<String, WafObjectGroup> = BTreeMap::new();

  for value in object_keys {
    let Some((prefix, number, suffix)) = parse_waf_object_key_parts(value.as_str(), path_prefix) else {
      continue;
    };

    let group_key = format!("{prefix}\0{suffix}\0{}", number.len());
    let entry = grouped.entry(group_key).or_insert_with(|| WafObjectGroup {
      prefix,
      suffix,
      numbers: Vec::new(),
    });
    entry.numbers.push(number);
  }

  let mut fragments = Vec::new();

  for group in grouped.values() {
    let mut ordered_numbers = group.numbers.clone();
    ordered_numbers.sort();
    ordered_numbers.dedup();

    if ordered_numbers.is_empty() {
      continue;
    }

    let mut run_start = ordered_numbers[0].clone();
    let mut previous = ordered_numbers[0].clone();

    for current in ordered_numbers.iter().skip(1) {
      let previous_value = previous
        .parse::<u128>()
        .expect("waf numbers should fit into u128");
      let current_value = current
        .parse::<u128>()
        .expect("waf numbers should fit into u128");

      if current_value == previous_value + 1 {
        previous = current.clone();
        continue;
      }

      fragments.push(format!(
        "{}{}\\.{}",
        escape_waf_regex(group.prefix.as_str()),
        build_numeric_range_regex(run_start.as_str(), previous.as_str()),
        escape_waf_regex(group.suffix.as_str())
      ));
      run_start = current.clone();
      previous = current.clone();
    }

    fragments.push(format!(
      "{}{}\\.{}",
      escape_waf_regex(group.prefix.as_str()),
      build_numeric_range_regex(run_start.as_str(), previous.as_str()),
      escape_waf_regex(group.suffix.as_str())
    ));
  }

  fragments.sort();
  fragments.dedup();
  fragments
}

#[cfg(test)]
fn build_waf_object_pattern(object_keys: &[String], path_prefix: &str) -> String {
  let fragments = collect_waf_object_fragments(object_keys, path_prefix);
  if fragments.is_empty() {
    return String::new();
  }

  if fragments.len() == 1 {
    return format!("^{}$", fragments[0]);
  }

  format!("^(?:{})$", fragments.join("|"))
}

#[cfg(test)]
mod tests {
  use super::{
    build_object_allowlist_expression,
    build_waf_object_pattern,
    hash_waf_allowlist_values,
    resolve_waf_scope,
  };

  #[test]
  fn blocks_all_public_paths_when_allowlist_is_empty() {
    let scope = resolve_waf_scope("https://cdn.example.com").expect("valid scope");
    let expression = build_object_allowlist_expression(&[], &scope);
    assert_eq!(expression, "http.host eq \"cdn.example.com\"");
  }

  #[test]
  fn builds_waf_object_allowlist_expression_with_exact_path_set() {
    let scope = resolve_waf_scope("https://cdn.example.com/assets/").expect("valid scope");
    let object_keys = [
      "img/public/000000002.webp".to_string(),
      "/img/public/000000001.png".to_string(),
      "img\\public\\000000003.jpg".to_string(),
    ];
    let expression = build_object_allowlist_expression(&object_keys, &scope);

    assert!(expression.contains("http.host eq \"cdn.example.com\""));
    assert_eq!(
      expression,
      "http.host eq \"cdn.example.com\" and not (raw.http.request.uri.path in {\"/assets/img/public/000000001.png\" \"/assets/img/public/000000002.webp\" \"/assets/img/public/000000003.jpg\"})"
    );
  }

  #[test]
  fn drops_non_strict_allowlist_entries_with_meta_chars() {
    let scope = resolve_waf_scope("https://cdn.example.com").expect("valid scope");
    let object_keys = [
      "img/public/000000001.png".to_string(),
      "img/public/000000002.^png".to_string(),
      "img/public/000000003.p$ng".to_string(),
      "img/public/000000006.p ng".to_string(),
      "img/public/000000007.p\u{00B7}ng".to_string(),
      "img/public/000000008.p\u{2026}ng".to_string(),
      "img/public/000000009.png=1".to_string(),
      "img/public/00A000004.png".to_string(),
      "img/public/000000005..png".to_string(),
    ];

    let expression = build_object_allowlist_expression(&object_keys, &scope);

    assert!(expression.contains("raw.http.request.uri.path in {"));
    assert!(expression.contains("\"/img/public/000000001.png\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/000000002.^png\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/000000003.p$ng\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/000000006.p ng\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/000000007.p\u{00B7}ng\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/000000008.p\u{2026}ng\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/000000009.png=1\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/00A000004.png\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/000000005..png\""));
  }

  #[test]
  fn drops_allowlist_entries_with_path_bypass_markers() {
    let scope = resolve_waf_scope("https://cdn.example.com").expect("valid scope");
    let object_keys = [
      "img/public/000000006.png".to_string(),
      "/img/public/../img/public/000000006.png".to_string(),
      "/img//img/public/000000006.png".to_string(),
      "/img/public/000000006.png;a=1".to_string(),
      "/img/public/;x=1/000000006.png".to_string(),
      "img/public/%2e%2e/etc/passwd".to_string(),
    ];

    let expression = build_object_allowlist_expression(&object_keys, &scope);

    assert!(expression.contains("raw.http.request.uri.path in {"));
    assert!(expression.contains("\"/img/public/000000006.png\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/../img/public/000000006.png\""));
    assert!(!expression.contains("\"https://cdn.example.com/img//img/public/000000006.png\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/000000006.png;a=1\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/;x=1/000000006.png\""));
    assert!(!expression.contains("\"https://cdn.example.com/img/public/%2e%2e/etc/passwd\""));
  }

  #[test]
  fn blocks_all_when_allowlist_contains_only_non_strict_entries() {
    let scope = resolve_waf_scope("https://cdn.example.com").expect("valid scope");
    let object_keys = [
      "img/public/000000001.^png".to_string(),
      "img/public/000000002.p$ng".to_string(),
      "img/public/00A000003.png".to_string(),
    ];

    let expression = build_object_allowlist_expression(&object_keys, &scope);

    assert_eq!(expression, "http.host eq \"cdn.example.com\"");
    assert!(!expression.contains("not (http.request.full_uri in {"));
  }

  #[test]
  fn builds_waf_object_pattern_with_compressed_ranges() {
    let pattern = build_waf_object_pattern(&[
      "img/public/000000001.png".to_string(),
      "img/public/000000002.png".to_string(),
      "img/public/000000003.png".to_string(),
      "img/public/000000005.png".to_string(),
    ], "");

    assert_eq!(
      pattern,
      "^(?:/img/public/000000005\\.png|/img/public/00000000[1-3]\\.png)$"
    );
  }

  #[test]
  fn builds_waf_object_pattern_with_unique_sorted_paths() {
    let pattern = build_waf_object_pattern(&[
      "img/public/000000002.webp".to_string(),
      "/img/public/000000001.png".to_string(),
      "img/public/000000002.webp".to_string(),
    ], "/assets");

    assert_eq!(
      pattern,
      "^(?:/assets/img/public/000000001\\.png|/assets/img/public/000000002\\.webp)$"
    );
  }

  #[test]
  fn resolves_waf_scope_with_host_and_path_prefix() {
    let scope = resolve_waf_scope("https://cdn.example.com/media/v1/").expect("valid scope");
    assert_eq!(scope.scheme, "https");
    assert_eq!(scope.host, "cdn.example.com");
    assert_eq!(scope.path_prefix, "/media/v1");
  }

  #[test]
  fn fingerprints_allowlist_values_in_stable_order() {
    let a = hash_waf_allowlist_values(&[
      "https://cdn.example.com/img/public/000000001.png".to_string(),
      "https://cdn.example.com/img/public/000000002.webp".to_string(),
    ]);
    let b = hash_waf_allowlist_values(&[
      "https://cdn.example.com/img/public/000000002.webp".to_string(),
      "https://cdn.example.com/img/public/000000001.png".to_string(),
    ]);

    assert_eq!(a, b);
    assert_eq!(a.len(), 64);
  }
}