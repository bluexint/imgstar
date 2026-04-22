use crate::contracts::{
  UploadEventLevel,
  UploadEventModule,
  UploadEventStatus,
  UploadFileStatus,
  UploadRecyclePayload,
  UploadRecycleResult,
};
use crate::domain::logging::center::{LogCenter, LogRecord};
use crate::domain::upload::support::{
  compact_stack,
  context_from_pairs,
  context_from_pairs_with_allowlist_hash,
  recycle_failed,
};
use crate::domain::upload::waf_sync::WafAllowlistSync;
use crate::runtime::adapter_runtime::AdapterRuntime;
use crate::storage::key_allocator::KeyAllocator;
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct UploadRecycleCoordinator {
  key_allocator: Arc<KeyAllocator>,
  adapter_runtime: AdapterRuntime,
  log_center: LogCenter,
  waf_allowlist_sync: WafAllowlistSync,
}

impl UploadRecycleCoordinator {
  pub fn new(
    key_allocator: Arc<KeyAllocator>,
    adapter_runtime: AdapterRuntime,
    log_center: LogCenter,
    waf_allowlist_sync: WafAllowlistSync,
  ) -> Self {
    Self {
      key_allocator,
      adapter_runtime,
      log_center,
      waf_allowlist_sync,
    }
  }

  pub fn recycle(&self, payload: UploadRecyclePayload) -> UploadRecycleResult {
    let trace_id = payload
      .trace_id
      .clone()
      .filter(|value| !value.trim().is_empty())
      .unwrap_or_else(|| self.log_center.new_trace_id());

    let number = payload.number.trim().to_string();
    let object_key = payload.object_key.trim().to_string();
    let file_name = payload.file_name.trim().to_string();

    if number.is_empty() || object_key.is_empty() || file_name.is_empty() {
      self.log_center.emit(
        LogRecord::new(
          trace_id.clone(),
          UploadEventModule::Upload,
          "upload:recycle_failed",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          0,
          context_from_pairs(vec![
            ("reason", json!("missing_payload_fields")),
            ("cleanupStatus", json!("not_started")),
          ]),
        )
        .with_error("UPLOAD_RECYCLE_FAILED", "recycle payload is invalid"),
      );

      return UploadRecycleResult {
        trace_id,
        status: UploadFileStatus::Failed,
        error: Some("UPLOAD_RECYCLE_FAILED".to_string()),
        cache_purged: false,
        waf_synced: false,
      };
    }

    self.log_center.emit(LogRecord::new(
      trace_id.clone(),
      UploadEventModule::Upload,
      "upload:recycle_started",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      0,
      context_from_pairs(vec![
        ("file", json!(file_name.clone())),
        ("number", json!(number.clone())),
        ("objectKey", json!(object_key.clone())),
      ]),
    ));

    if !self.key_allocator.mark_deleted(number.as_str()) {
      return recycle_failed(
        &self.log_center,
        trace_id,
        "KEY_NOT_ACTIVE",
        "number is not in active state",
        "not_started",
        false,
        false,
      );
    }

    let (waf_result, waf_allowlist_hash) = self.waf_allowlist_sync.sync_active_object_allowlist();
    if !waf_result.success {
      let rollback_ok = self.key_allocator.restore_active(number.as_str());
      let error_code = waf_result
        .error_code
        .unwrap_or_else(|| "WAF_RULE_SYNC_FAILED".to_string());
      let error_message = waf_result
        .error_message
        .unwrap_or_else(|| "waf object allowlist sync failed".to_string());

      self.log_center.emit(
        LogRecord::new(
          trace_id.clone(),
          UploadEventModule::Upload,
          "upload:waf_sync_error",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          waf_result.response_time,
          context_from_pairs_with_allowlist_hash(
            vec![
              ("file", json!(file_name.clone())),
              ("number", json!(number.clone())),
              ("objectKey", json!(object_key.clone())),
              ("rollbackDelete", json!(rollback_ok)),
            ],
            waf_allowlist_hash.as_ref(),
          ),
        )
        .with_error(error_code.clone(), error_message.clone())
        .with_stack(compact_stack("waf_sync", error_code.clone(), error_message.clone())),
      );

      return recycle_failed(
        &self.log_center,
        trace_id,
        error_code.as_str(),
        error_message.as_str(),
        if rollback_ok {
          "rollback_to_active"
        } else {
          "rollback_failed"
        },
        false,
        false,
      );
    }

    self.log_center.emit(LogRecord::new(
      trace_id.clone(),
      UploadEventModule::Upload,
      "upload:waf_synced",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      waf_result.response_time,
      context_from_pairs_with_allowlist_hash(
        vec![("number", json!(number.clone()))],
        waf_allowlist_hash.as_ref(),
      ),
    ));

    let delete_result = self.adapter_runtime.delete_object(object_key.as_str());
    if !delete_result.success {
      let error_code = delete_result
        .error_code
        .unwrap_or_else(|| "ADAPTER_SERVER_ERROR".to_string());
      let error_message = delete_result
        .error_message
        .unwrap_or_else(|| "cloud object delete failed".to_string());

      return recycle_failed(
        &self.log_center,
        trace_id,
        error_code.as_str(),
        error_message.as_str(),
        "cloud_delete_failed_waf_blocked",
        false,
        true,
      );
    }

    let cache_purge_configured = self.adapter_runtime.has_cloudflare_cache_purge_configured();
    let mut cache_purged = false;
    if cache_purge_configured {
      let purge_result = self.adapter_runtime.purge_cdn_cache(object_key.as_str());
      if !purge_result.success {
        let error_code = purge_result
          .error_code
          .unwrap_or_else(|| "CACHE_PURGE_FAILED".to_string());
        let error_message = purge_result
          .error_message
          .unwrap_or_else(|| "cache purge failed".to_string());

        return recycle_failed(
          &self.log_center,
          trace_id,
          error_code.as_str(),
          error_message.as_str(),
          "cloud_deleted_cache_pending",
          false,
          true,
        );
      }

      cache_purged = true;
      self.log_center.emit(LogRecord::new(
        trace_id.clone(),
        UploadEventModule::Upload,
        "upload:cache_purged",
        UploadEventLevel::Info,
        UploadEventStatus::Success,
        purge_result.response_time,
        context_from_pairs(vec![("number", json!(number.clone()))]),
      ));
    }

    let mark_cooling_ok = self.key_allocator.mark_cooling(number.as_str());
    if !mark_cooling_ok {
      return recycle_failed(
        &self.log_center,
        trace_id,
        "KEY_REUSE_CONFLICT",
        "number recycle cooling transition failed",
        "state_transition_failed",
        true,
        true,
      );
    }

    if !self.key_allocator.mark_free_immediately(number.as_str()) {
      return recycle_failed(
        &self.log_center,
        trace_id,
        "KEY_REUSE_CONFLICT",
        "number recycle release transition failed",
        "state_transition_failed",
        true,
        true,
      );
    }

    self.log_center.emit(LogRecord::new(
      trace_id.clone(),
      UploadEventModule::Upload,
      "upload:recycle_success",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      0,
      context_from_pairs(vec![
        ("number", json!(number)),
        ("cleanupStatus", json!("recycled_to_free")),
      ]),
    ));

    UploadRecycleResult {
      trace_id,
      status: UploadFileStatus::Success,
      error: None,
      cache_purged,
      waf_synced: true,
    }
  }
}