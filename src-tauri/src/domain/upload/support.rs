use crate::contracts::{
  UploadEventLevel,
  UploadEventModule,
  UploadEventStatus,
  UploadFileResult,
  UploadFileStatus,
  UploadRecycleResult,
  UploadStartResult,
  UploadStartStatus,
};
use crate::domain::logging::center::{LogCenter, LogRecord};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct ProcessedUploadFile {
  pub(crate) number: String,
  pub(crate) object_key: String,
}

pub(crate) fn failed_result(
  trace_id: String,
  error_code: &str,
  file_results: Vec<UploadFileResult>,
) -> UploadStartResult {
  UploadStartResult {
    trace_id,
    status: UploadStartStatus::Failed,
    error: Some(error_code.to_string()),
    files: Some(file_results),
  }
}

pub(crate) fn context_from_pairs(entries: Vec<(&str, Value)>) -> HashMap<String, Value> {
  let mut context = HashMap::new();
  for (key, value) in entries {
    context.insert(key.to_string(), value);
  }
  context
}

pub(crate) fn context_from_pairs_with_allowlist_hash(
  mut entries: Vec<(&str, Value)>,
  allowlist_hash: Option<&String>,
) -> HashMap<String, Value> {
  if let Some(hash) = allowlist_hash {
    entries.push(("allowlistHash", json!(hash)));
  }

  context_from_pairs(entries)
}

pub(crate) fn compact_stack(
  stage: &str,
  error_code: impl Into<String>,
  details: impl Into<String>,
) -> String {
  format!(
    "upload::{} > {} > {}",
    stage,
    error_code.into(),
    details.into()
  )
}

pub(crate) fn recycle_failed(
  log_center: &LogCenter,
  trace_id: String,
  error_code: &str,
  error_message: &str,
  cleanup_status: &str,
  cache_purged: bool,
  waf_synced: bool,
) -> UploadRecycleResult {
  log_center.emit(
    LogRecord::new(
      trace_id.clone(),
      UploadEventModule::Upload,
      "upload:recycle_failed",
      UploadEventLevel::Error,
      UploadEventStatus::Failed,
      0,
      context_from_pairs(vec![("cleanupStatus", json!(cleanup_status))]),
    )
    .with_error(error_code, error_message)
    .with_stack(compact_stack("recycle", error_code, error_message)),
  );

  UploadRecycleResult {
    trace_id,
    status: UploadFileStatus::Failed,
    error: Some(error_code.to_string()),
    cache_purged,
    waf_synced,
  }
}