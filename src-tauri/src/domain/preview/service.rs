use crate::contracts::{PreviewResult, UploadFileRef};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct PreviewService;

impl PreviewService {
  pub fn get_preview(&self, file: UploadFileRef) -> Result<PreviewResult, String> {
    if file.size == 0 {
      return Err("UPLOAD_VALIDATION_FAILED".to_string());
    }

    let bytes = if let Some(inline_content_base64) = &file.inline_content_base64 {
      BASE64_STANDARD
        .decode(inline_content_base64.as_bytes())
        .map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())?
    } else if file.path.starts_with("http://") || file.path.starts_with("https://") {
      let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())?;

      let response = client
        .get(file.path.as_str())
        .send()
        .map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())?;

      if !response.status().is_success() {
        return Err("PREVIEW_SOURCE_NOT_FOUND".to_string());
      }

      response
        .bytes()
        .map_err(|_| "PREVIEW_SOURCE_NOT_FOUND".to_string())?
        .to_vec()
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
