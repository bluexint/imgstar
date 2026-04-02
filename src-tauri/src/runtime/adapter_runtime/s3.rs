//! S3-compatible object transfer helpers.
//!
//! `AdapterRuntime` uses this module for presigned PUT/DELETE flows and bucket
//! base URL resolution. It keeps endpoint normalization and HTTP transfer logic
//! away from the higher-level Cloudflare and WAF orchestration code.

use super::compact_body_preview;
use aws_sdk_s3::presigning::PresigningConfig;
use reqwest::blocking::Client as BlockingHttpClient;
use std::time::Duration;

pub(crate) fn resolve_bucket_base_url(endpoint: &str, bucket: &str) -> String {
  let endpoint = endpoint.trim().trim_end_matches('/');
  let bucket = bucket.trim().trim_matches('/');

  if endpoint_has_bucket(endpoint, bucket) {
    endpoint.to_string()
  } else {
    format!("{endpoint}/{bucket}")
  }
}

fn resolve_s3_endpoint(endpoint: &str, bucket: &str) -> String {
  let normalized = endpoint.trim().trim_end_matches('/');
  if !endpoint_has_bucket(normalized, bucket) {
    return normalized.to_string();
  }

  let Ok(mut parsed) = reqwest::Url::parse(normalized) else {
    return normalized.to_string();
  };

  if let Some(host) = parsed.host_str().map(|value| value.to_string()) {
    let host_lower = host.to_ascii_lowercase();
    let bucket_prefix = format!("{}.", bucket.to_ascii_lowercase());
    if host_lower.starts_with(bucket_prefix.as_str()) {
      let new_host = host[bucket_prefix.len()..].to_string();
      let _ = parsed.set_host(Some(new_host.as_str()));
    }
  }

  let segments = parsed
    .path_segments()
    .map(|parts| parts.map(|segment| segment.to_string()).collect::<Vec<_>>())
    .unwrap_or_default();

  if let Some(first) = segments.first() {
    if first.eq_ignore_ascii_case(bucket.trim_matches('/')) {
      if let Ok(mut writer) = parsed.path_segments_mut() {
        writer.clear();
        for segment in segments.iter().skip(1) {
          writer.push(segment.as_str());
        }
      }
    }
  }

  parsed.as_str().trim_end_matches('/').to_string()
}

async fn build_r2_client(
  endpoint: &str,
  bucket: &str,
  access_key: &str,
  secret_key: &str,
) -> Result<aws_sdk_s3::Client, String> {
  let endpoint = resolve_s3_endpoint(endpoint, bucket);
  let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
    .endpoint_url(endpoint)
    .credentials_provider(aws_sdk_s3::config::Credentials::new(
      access_key,
      secret_key,
      None,
      None,
      "R2",
    ))
    .region("auto")
    .load()
    .await;

  Ok(aws_sdk_s3::Client::new(&config))
}

async fn presign_put_object_url(
  client: &aws_sdk_s3::Client,
  bucket: &str,
  object_key: &str,
) -> Result<String, String> {
  let presigning_config = PresigningConfig::expires_in(Duration::from_secs(300))
    .map_err(|error| format!("presigning configuration failed: {error}"))?;
  let request = client
    .put_object()
    .bucket(bucket)
    .key(object_key)
    .presigned(presigning_config)
    .await
    .map_err(|error| format!("presign upload request failed: {error}"))?;

  Ok(request.uri().to_string())
}

async fn presign_delete_object_url(
  client: &aws_sdk_s3::Client,
  bucket: &str,
  object_key: &str,
) -> Result<String, String> {
  let presigning_config = PresigningConfig::expires_in(Duration::from_secs(300))
    .map_err(|error| format!("presigning configuration failed: {error}"))?;
  let request = client
    .delete_object()
    .bucket(bucket)
    .key(object_key)
    .presigned(presigning_config)
    .await
    .map_err(|error| format!("presign delete request failed: {error}"))?;

  Ok(request.uri().to_string())
}

pub(super) fn upload_via_s3(
  endpoint: &str,
  bucket: &str,
  region: &str,
  access_key: &str,
  secret_key: &str,
  object_key: &str,
  content: Vec<u8>,
  content_type: &str,
) -> Result<(), String> {
  let _ = region;

  let endpoint = endpoint.to_string();
  let bucket = bucket.to_string();
  let access_key = access_key.to_string();
  let secret_key = secret_key.to_string();
  let object_key = object_key.to_string();

  let presigned_url = tauri::async_runtime::block_on(async move {
    let client = build_r2_client(
      endpoint.as_str(),
      bucket.as_str(),
      access_key.as_str(),
      secret_key.as_str(),
    )
    .await?;

    presign_put_object_url(&client, bucket.as_str(), object_key.as_str()).await
  })?;

  let client = BlockingHttpClient::builder()
    .timeout(Duration::from_secs(30))
    .build()
    .map_err(|error| format!("r2 client initialization failed: {error}"))?;

  let response = client
    .put(presigned_url)
    .header("Content-Type", content_type)
    .body(content)
    .send()
    .map_err(|error| error.to_string())?;

  let status = response.status();
  let raw_body = response.text().unwrap_or_default();
  if !status.is_success() {
    return Err(format!(
      "api error: {status}, {}",
      compact_body_preview(raw_body.as_str()).unwrap_or_else(|| "cloudflare api error".to_string())
    ));
  }

  Ok(())
}

pub(super) fn delete_via_s3(
  endpoint: &str,
  bucket: &str,
  region: &str,
  access_key: &str,
  secret_key: &str,
  object_key: &str,
) -> Result<(), String> {
  let _ = region;

  let endpoint = endpoint.to_string();
  let bucket = bucket.to_string();
  let access_key = access_key.to_string();
  let secret_key = secret_key.to_string();
  let object_key = object_key.to_string();

  let presigned_url = tauri::async_runtime::block_on(async move {
    let client = build_r2_client(
      endpoint.as_str(),
      bucket.as_str(),
      access_key.as_str(),
      secret_key.as_str(),
    )
    .await?;

    presign_delete_object_url(&client, bucket.as_str(), object_key.as_str()).await
  })?;

  let client = BlockingHttpClient::builder()
    .timeout(Duration::from_secs(30))
    .build()
    .map_err(|error| format!("r2 client initialization failed: {error}"))?;

  let response = client
    .delete(presigned_url)
    .send()
    .map_err(|error| error.to_string())?;

  let status = response.status();
  let raw_body = response.text().unwrap_or_default();
  if !status.is_success() {
    return Err(format!(
      "api error: {status}, {}",
      compact_body_preview(raw_body.as_str()).unwrap_or_else(|| "cloudflare api error".to_string())
    ));
  }

  Ok(())
}

fn endpoint_has_bucket(endpoint: &str, bucket: &str) -> bool {
  if bucket.is_empty() {
    return false;
  }

  let without_scheme = endpoint
    .trim_start_matches("https://")
    .trim_start_matches("http://");
  let mut parts = without_scheme.splitn(2, '/');
  let host = parts.next().unwrap_or("");
  let path = parts.next().unwrap_or("");

  let bucket_lc = bucket.to_ascii_lowercase();
  let host_lc = host.to_ascii_lowercase();

  if host_lc == bucket_lc || host_lc.starts_with(format!("{bucket_lc}." ).as_str()) {
    return true;
  }

  let first_path_segment = path
    .trim_start_matches('/')
    .split('/')
    .next()
    .unwrap_or("");
  first_path_segment.eq_ignore_ascii_case(bucket)
}

#[cfg(test)]
fn resolve_object_upload_url(endpoint: &str, bucket: &str, object_key: &str) -> String {
  let key = object_key
    .replace('\\', "/")
    .trim_start_matches('/')
    .to_string();
  let base = resolve_bucket_base_url(endpoint, bucket);
  format!("{base}/{key}")
}

#[cfg(test)]
mod tests {
  use super::{
    resolve_bucket_base_url,
    resolve_object_upload_url,
    resolve_s3_endpoint,
  };

  #[test]
  fn appends_bucket_for_account_endpoint() {
    let url = resolve_object_upload_url(
      "https://abc123.r2.cloudflarestorage.com",
      "imgstar",
      "img/public/000000001.png",
    );
    assert_eq!(
      url,
      "https://abc123.r2.cloudflarestorage.com/imgstar/img/public/000000001.png"
    );
  }

  #[test]
  fn avoids_bucket_duplication_when_bucket_is_in_host() {
    let url = resolve_object_upload_url(
      "https://imgstar.abc123.r2.cloudflarestorage.com",
      "imgstar",
      "img/public/000000001.png",
    );
    assert_eq!(
      url,
      "https://imgstar.abc123.r2.cloudflarestorage.com/img/public/000000001.png"
    );
  }

  #[test]
  fn avoids_bucket_duplication_when_bucket_is_in_path() {
    let base = resolve_bucket_base_url("https://example.com/imgstar", "imgstar");
    let url = resolve_object_upload_url(
      "https://example.com/imgstar",
      "imgstar",
      "img/public/000000001.png",
    );
    assert_eq!(base, "https://example.com/imgstar");
    assert_eq!(url, "https://example.com/imgstar/img/public/000000001.png");
  }

  #[test]
  fn resolves_s3_endpoint_from_virtual_hosted_bucket_endpoint() {
    let endpoint = resolve_s3_endpoint(
      "https://imgstar.abc123.r2.cloudflarestorage.com",
      "imgstar",
    );
    assert_eq!(endpoint, "https://abc123.r2.cloudflarestorage.com");
  }

  #[test]
  fn resolves_s3_endpoint_from_path_style_bucket_endpoint() {
    let endpoint = resolve_s3_endpoint("https://example.com/imgstar", "imgstar");
    assert_eq!(endpoint, "https://example.com");
  }
}