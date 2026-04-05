use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub type ErrorCode = String;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HookStage {
  PreKey,
  PostKey,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UploadStartStatus {
  Queued,
  Running,
  Success,
  Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UploadTaskStatus {
  Draft,
  Queued,
  Running,
  Success,
  Failed,
  Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadTaskSnapshot {
  pub id: String,
  pub file: UploadFileRef,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trace_id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub number: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub object_key: Option<String>,
  pub progress: u32,
  pub status: UploadTaskStatus,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<ErrorCode>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub started_at: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub completed_at: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub speed_bps: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadQueueSnapshot {
  pub tasks: Vec<UploadTaskSnapshot>,
  pub thumbnails: HashMap<String, String>,
  pub target_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UploadEventModule {
  Upload,
  Plugin,
  Storage,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UploadEventLevel {
  Info,
  Warn,
  Error,
  Debug,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UploadEventStatus {
  Success,
  Failed,
  Skipped,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadFileRef {
  pub path: String,
  pub name: String,
  pub size: u64,
  pub mime_type: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub inline_content_base64: Option<String>,
}

impl UploadFileRef {
  pub fn extension(&self) -> &str {
    self
      .name
      .rsplit_once('.')
      .map(|(_, suffix)| suffix)
      .filter(|suffix| !suffix.is_empty())
      .unwrap_or("bin")
  }

  pub fn looks_like_image(&self) -> bool {
    if let Some(mime) = &self.mime_type {
      return mime.starts_with("image/");
    }

    let lowered = self.name.to_ascii_lowercase();
    lowered.ends_with(".png")
      || lowered.ends_with(".jpg")
      || lowered.ends_with(".jpeg")
      || lowered.ends_with(".webp")
      || lowered.ends_with(".gif")
      || lowered.ends_with(".bmp")
      || lowered.ends_with(".svg")
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageTargetConfig {
  pub id: String,
  pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfig {
  pub id: String,
  pub enabled: bool,
  pub hook_type: String,
  pub stage: HookStage,
  pub priority: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadStartPayload {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trace_id: Option<String>,
  pub files: Vec<UploadFileRef>,
  pub target: StorageTargetConfig,
  pub plugin_chain: Vec<PluginConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UploadFileStatus {
  Success,
  Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadFileResult {
  pub index: usize,
  pub file_name: String,
  pub status: UploadFileStatus,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub number: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub object_key: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<ErrorCode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadStartResult {
  pub trace_id: String,
  pub status: UploadStartStatus,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<ErrorCode>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub files: Option<Vec<UploadFileResult>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadRecyclePayload {
  pub number: String,
  pub object_key: String,
  pub file_name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trace_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadRecycleResult {
  pub trace_id: String,
  pub status: UploadFileStatus,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<ErrorCode>,
  pub cache_purged: bool,
  pub waf_synced: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
  pub file_name: String,
  pub hash: String,
  pub hash_enabled: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hash_algorithm: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub image_data_url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mime_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDraft {
  pub access_key: String,
  pub secret_key: String,
  pub endpoint: String,
  pub bucket: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub zone_id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub zone_api_token: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cdn_base_url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub region: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub key_pattern: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub digit_count: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[deprecated = "兼容字段，已废弃。回收链路在删除与缓存清除请求完成后立即释放编号，不再等待冷却延迟"]
  pub reuse_delay_ms: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub preview_hash_enabled: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub theme: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub language: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsResult {
  pub saved_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
  pub draft: SettingsDraft,
  pub configured: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionPingResult {
  pub latency_ms: u64,
  pub checked_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginVerificationResult {
  pub verified: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub signature_algorithm: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub signer: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub signer_source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadEvent {
  pub trace_id: String,
  pub timestamp: String,
  pub module: UploadEventModule,
  pub event_name: String,
  pub level: UploadEventLevel,
  pub status: UploadEventStatus,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error_code: Option<ErrorCode>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error_message: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stack: Option<String>,
  pub duration: u64,
  pub context: HashMap<String, Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadEventFilter {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub module: Option<UploadEventModule>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub level: Option<UploadEventLevel>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trace_id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error_code: Option<ErrorCode>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub start_at: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub end_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KvReadonlySnapshot {
  pub digit_count: u32,
  pub objects: Vec<KvReadonlyObjectEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KvReadonlyObjectEntry {
  pub number: String,
  pub object_key: String,
}
