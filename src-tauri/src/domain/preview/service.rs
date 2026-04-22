use crate::contracts::{PreviewResult, UploadFileRef};
use crate::storage::settings_store::SettingsStore;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const PREVIEW_REQUEST_TIMEOUT_SECS: u64 = 10;

#[derive(Clone, Debug)]
pub struct PreviewService {
  settings_store: Arc<SettingsStore>,
  http_client: Option<Client>,
}

impl PreviewService {
  pub fn new(settings_store: Arc<SettingsStore>) -> Self {
    let http_client = Client::builder()
      .timeout(Duration::from_secs(PREVIEW_REQUEST_TIMEOUT_SECS))
      .redirect(Policy::none())
      .build()
      .ok();

    Self {
      settings_store,
      http_client,
    }
  }

  pub fn get_preview(&self, file: UploadFileRef) -> Result<PreviewResult, String> {
    if file.size == 0 {
      return Err("UPLOAD_VALIDATION_FAILED".to_string());
    }

    let bytes = if let Some(inline_content_base64) = &file.inline_content_base64 {
      BASE64_STANDARD
        .decode(inline_content_base64.as_bytes())
        .map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())?
    } else if is_remote_path(file.path.as_str()) {
      self.read_remote_bytes(file.path.as_str())?
    } else {
      let path = Path::new(file.path.as_str());
      if !path.exists() {
        return Err("PREVIEW_SOURCE_NOT_FOUND".to_string());
      }

      std::fs::read(path).map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())?
    };
    let mut hasher = Sha256::new();
    hasher.update(bytes.as_slice());
    let hash = format!("{:x}", hasher.finalize());

    let mime_type = resolve_mime(file.mime_type.as_deref(), file.name.as_str());
    let image_data_url = if mime_type.starts_with("image/") {
      Some(format!(
        "data:{mime_type};base64,{}",
        BASE64_STANDARD.encode(bytes.as_slice())
      ))
    } else {
      None
    };

    Ok(PreviewResult {
      file_name: file.name.clone(),
      hash,
      hash_enabled: true,
      hash_algorithm: Some("sha256".to_string()),
      image_data_url,
      mime_type: Some(mime_type),
    })
  }

  fn read_remote_bytes(&self, raw_url: &str) -> Result<Vec<u8>, String> {
    let request_url = self.resolve_allowed_remote_url(raw_url)?;
    let client = self
      .http_client
      .as_ref()
      .ok_or_else(|| "PREVIEW_SOURCE_NOT_FOUND".to_string())?;

    let response = client
      .get(request_url)
      .send()
      .map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())?;

    if !response.status().is_success() {
      return Err("PREVIEW_SOURCE_NOT_FOUND".to_string());
    }

    response
      .bytes()
      .map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())
      .map(|bytes| bytes.to_vec())
  }

  fn resolve_allowed_remote_url(&self, raw_url: &str) -> Result<Url, String> {
    let request_url = Url::parse(raw_url).map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())?;
    let allowed_base_url = self
      .allowed_cdn_base_url()
      .ok_or_else(|| "PREVIEW_SOURCE_NOT_FOUND".to_string())?;

    if is_allowed_remote_url(&request_url, &allowed_base_url) {
      Ok(request_url)
    } else {
      Err("PREVIEW_SOURCE_NOT_FOUND".to_string())
    }
  }

  fn allowed_cdn_base_url(&self) -> Option<Url> {
    let settings = self.settings_store.load()?;
    let raw_base_url = settings.cdn_base_url?;
    let base_url = Url::parse(raw_base_url.trim()).ok()?;

    match base_url.scheme() {
      "http" | "https" => Some(base_url),
      _ => None,
    }
  }
}

impl Default for PreviewService {
  fn default() -> Self {
    Self::new(Arc::new(SettingsStore::default()))
  }
}

fn is_remote_path(path: &str) -> bool {
  path.starts_with("http://") || path.starts_with("https://")
}

fn is_allowed_remote_url(request_url: &Url, allowed_base_url: &Url) -> bool {
  has_same_origin(request_url, allowed_base_url)
    && has_path_prefix(request_url, allowed_base_url)
    && request_url.username().is_empty()
    && request_url.password().is_none()
}

fn has_same_origin(left: &Url, right: &Url) -> bool {
  left.scheme() == right.scheme()
    && left.host_str() == right.host_str()
    && left.port_or_known_default() == right.port_or_known_default()
}

fn has_path_prefix(candidate: &Url, base: &Url) -> bool {
  let candidate_segments = path_segments(candidate);
  let base_segments = path_segments(base);

  candidate_segments.len() >= base_segments.len()
    && candidate_segments
      .iter()
      .zip(base_segments.iter())
      .all(|(candidate_segment, base_segment)| candidate_segment == base_segment)
}

fn path_segments(url: &Url) -> Vec<&str> {
  url
    .path_segments()
    .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
    .unwrap_or_default()
}

fn resolve_mime(raw: Option<&str>, name: &str) -> String {
  if let Some(raw) = raw {
    if !raw.trim().is_empty() {
      return raw.to_string();
    }
  }

  let lowered = name.to_ascii_lowercase();
  if lowered.ends_with(".png") {
    return "image/png".to_string();
  }
  if lowered.ends_with(".jpg") || lowered.ends_with(".jpeg") {
    return "image/jpeg".to_string();
  }
  if lowered.ends_with(".webp") {
    return "image/webp".to_string();
  }
  if lowered.ends_with(".gif") {
    return "image/gif".to_string();
  }
  if lowered.ends_with(".bmp") {
    return "image/bmp".to_string();
  }
  if lowered.ends_with(".svg") {
    return "image/svg+xml".to_string();
  }

  "application/octet-stream".to_string()
}

#[cfg(test)]
mod tests {
  use super::{is_allowed_remote_url, PreviewService};
  use crate::contracts::{SettingsDraft, UploadFileRef};
  use crate::storage::settings_store::SettingsStore;
  use sha2::{Digest, Sha256};
  use std::io::{Read, Write};
  use std::net::TcpListener;
  use std::sync::Arc;

  #[allow(deprecated)]
  fn build_service(cdn_base_url: Option<String>) -> PreviewService {
    let settings_store = Arc::new(SettingsStore::default());
    if let Some(base_url) = cdn_base_url {
      settings_store.save(SettingsDraft {
        access_key: String::new(),
        secret_key: String::new(),
        endpoint: String::new(),
        bucket: String::new(),
        zone_id: None,
        zone_api_token: None,
        cdn_base_url: Some(base_url),
        region: None,
        key_pattern: None,
        digit_count: None,
        reuse_delay_ms: None,
        preview_hash_enabled: None,
        theme: None,
        language: None,
      });
    }

    PreviewService::new(settings_store)
  }

  fn spawn_test_server(body: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let address = listener.local_addr().expect("listener should have local addr");
    let base_url = format!("http://{address}");

    let handle = std::thread::spawn(move || {
      if let Ok((mut stream, _)) = listener.accept() {
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer);
        let headers = format!(
          "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n",
          body.len()
        );
        let _ = stream.write_all(headers.as_bytes());
        let _ = stream.write_all(body.as_slice());
      }
    });

    (base_url, handle)
  }

  #[test]
  fn accepts_remote_preview_under_configured_cdn_base_url() {
    let body = b"remote-preview-bytes".to_vec();
    let (base_url, handle) = spawn_test_server(body.clone());
    let service = build_service(Some(format!("{base_url}/cdn")));

    let result = service
      .get_preview(UploadFileRef {
        path: format!("{base_url}/cdn/test.png"),
        name: "test.png".to_string(),
        size: body.len() as u64,
        mime_type: Some("image/png".to_string()),
        inline_content_base64: None,
      })
      .expect("remote preview should succeed");

    let expected_hash = format!("{:x}", Sha256::digest(body.as_slice()));
    assert_eq!(result.hash, expected_hash);
    assert!(result.image_data_url.is_some());

    handle.join().expect("server thread should finish");
  }

  #[test]
  fn rejects_remote_preview_outside_configured_cdn_path_prefix() {
    let allowed_base = reqwest::Url::parse("https://cdn.example.com/assets")
      .expect("base url should parse");
    let request_url = reqwest::Url::parse("https://cdn.example.com/other/file.png")
      .expect("request url should parse");

    assert!(!is_allowed_remote_url(&request_url, &allowed_base));
  }
}
