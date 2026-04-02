use crate::contracts::{
  HookStage,
  KvReadonlyObjectEntry,
  StorageTargetConfig,
  UploadFileResult,
  UploadFileStatus,
  UploadFileRef,
  UploadEventLevel,
  UploadEventFilter,
  UploadEventModule,
  UploadEventStatus,
  UploadRecyclePayload,
  UploadRecycleResult,
  UploadStartPayload,
  UploadStartResult,
  UploadStartStatus,
};
use crate::domain::logging::center::{LogCenter, LogRecord};
use crate::runtime::adapter_runtime::AdapterResult;
use crate::runtime::adapter_runtime::AdapterRuntime;
use crate::runtime::plugin_runtime::PluginRuntime;
use crate::storage::key_allocator::KeyAllocator;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CANCELLED_TRACE_TTL_MS: u64 = 3_600_000;
const MAX_CANCELLED_TRACE_ENTRIES: usize = 4_096;

#[derive(Clone, Debug)]
struct ProcessedUploadFile {
  number: String,
  object_key: String,
}

#[derive(Clone)]
pub struct UploadOrchestrator {
  key_allocator: Arc<KeyAllocator>,
  adapter_runtime: AdapterRuntime,
  plugin_runtime: PluginRuntime,
  log_center: LogCenter,
  cancelled_traces: Arc<Mutex<HashMap<String, u64>>>,
}

impl UploadOrchestrator {
  pub fn new(
    key_allocator: Arc<KeyAllocator>,
    adapter_runtime: AdapterRuntime,
    plugin_runtime: PluginRuntime,
    log_center: LogCenter,
  ) -> Self {
    Self {
      key_allocator,
      adapter_runtime,
      plugin_runtime,
      log_center,
      cancelled_traces: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  pub fn start(&self, payload: UploadStartPayload) -> UploadStartResult {
    let trace_id = payload
      .trace_id
      .clone()
      .filter(|value| !value.trim().is_empty())
      .unwrap_or_else(|| self.log_center.new_trace_id());
    self.clear_cancelled(trace_id.as_str());

    let mut file_results: Vec<UploadFileResult> = Vec::new();

    if payload.files.is_empty() {
      self.log_center.emit(
        LogRecord::new(
          trace_id.clone(),
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          0,
          context_from_pairs(vec![("reason", json!("empty_files"))]),
        )
        .with_error("UPLOAD_VALIDATION_FAILED", "empty file list"),
      );
      self.clear_cancelled(trace_id.as_str());
      return failed_result(trace_id, "UPLOAD_VALIDATION_FAILED", file_results);
    }

    let estimated_size: u64 = payload.files.iter().map(|file| file.size).sum();

    self.log_center.emit(LogRecord::new(
      trace_id.clone(),
      UploadEventModule::Upload,
      "upload:task_created",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      0,
      context_from_pairs(vec![
        ("fileCount", json!(payload.files.len())),
        ("estimatedSize", json!(estimated_size)),
        ("pluginCount", json!(payload.plugin_chain.len())),
      ]),
    ));

    for (index, file) in payload.files.iter().enumerate() {
      if self.is_cancelled(trace_id.as_str()) {
        self.log_center.emit(
          LogRecord::new(
            trace_id.clone(),
            UploadEventModule::Upload,
            "upload:task_failed",
            UploadEventLevel::Warn,
            UploadEventStatus::Failed,
            0,
            context_from_pairs(vec![("cleanupStatus", json!("cancelled"))]),
          )
          .with_error("UPLOAD_CANCELLED", "upload cancelled by user"),
        );
        self.clear_cancelled(trace_id.as_str());
        return failed_result(trace_id, "UPLOAD_CANCELLED", file_results);
      }

      match self.process_single_file(trace_id.as_str(), file, &payload) {
        Ok(processed) => {
          file_results.push(UploadFileResult {
            index,
            file_name: file.name.clone(),
            status: UploadFileStatus::Success,
            number: Some(processed.number),
            object_key: Some(processed.object_key),
            error: None,
          });
        }
        Err(error_code) => {
          file_results.push(UploadFileResult {
            index,
            file_name: file.name.clone(),
            status: UploadFileStatus::Failed,
            number: None,
            object_key: None,
            error: Some(error_code.clone()),
          });
          self.clear_cancelled(trace_id.as_str());
          return failed_result(trace_id, error_code.as_str(), file_results);
        }
      }
    }

    self.log_center.emit(LogRecord::new(
      trace_id.clone(),
      UploadEventModule::Upload,
      "upload:task_success",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      0,
      context_from_pairs(vec![("fileCount", json!(payload.files.len()))]),
    ));

    self.clear_cancelled(trace_id.as_str());
    UploadStartResult {
      trace_id,
      status: UploadStartStatus::Success,
      error: None,
      files: Some(file_results),
    }
  }

  pub fn cancel(&self, trace_id: String) {
    let normalized = trace_id.trim().to_string();
    if normalized.is_empty() {
      return;
    }

    self.mark_cancelled(normalized.as_str());

    self.log_center.emit(LogRecord::new(
      normalized,
      UploadEventModule::Upload,
      "upload:task_cancelled",
      UploadEventLevel::Warn,
      UploadEventStatus::Success,
      0,
      context_from_pairs(vec![("reason", json!("user_cancelled"))]),
    ));
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
      return self.recycle_failed(
        trace_id,
        "KEY_NOT_ACTIVE",
        "number is not in active state",
        "not_started",
        false,
        false,
      );
    }

    let (waf_result, waf_allowlist_hash) = self.sync_active_object_allowlist();
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
        .with_stack(compact_stack(
          "waf_sync",
          error_code.clone(),
          error_message.clone(),
        )),
      );

      return self.recycle_failed(
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

      return self.recycle_failed(
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

        return self.recycle_failed(
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
      return self.recycle_failed(
        trace_id,
        "KEY_REUSE_CONFLICT",
        "number recycle cooling transition failed",
        "state_transition_failed",
        true,
        true,
      );
    }

    if !self.key_allocator.mark_free_immediately(number.as_str()) {
      return self.recycle_failed(
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

  fn process_single_file(
    &self,
    trace_id: &str,
    file: &UploadFileRef,
    payload: &UploadStartPayload,
  ) -> Result<ProcessedUploadFile, String> {
    if self.is_cancelled(trace_id) {
      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Warn,
          UploadEventStatus::Failed,
          0,
          context_from_pairs(vec![("cleanupStatus", json!("cancelled"))]),
        )
        .with_error("UPLOAD_CANCELLED", "upload cancelled by user"),
      );
      return Err("UPLOAD_CANCELLED".to_string());
    }

    if file.size == 0 || !file.looks_like_image() {
      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          1,
          context_from_pairs(vec![("cleanupStatus", json!("not_started"))]),
        )
        .with_error("UPLOAD_VALIDATION_FAILED", "file failed backend validation")
        .with_stack(compact_stack(
          "validation",
          "UPLOAD_VALIDATION_FAILED",
          "file failed backend validation",
        )),
      );
      return Err("UPLOAD_VALIDATION_FAILED".to_string());
    }

    self.emit_hook_boundary(trace_id, HookStage::PreKey, true, payload.plugin_chain.len());
    if let Err(error_code) = self
      .plugin_runtime
      .execute_stage(HookStage::PreKey, &payload.plugin_chain)
    {
      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:hook_error",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          2,
          context_from_pairs(vec![
            ("stage", json!("pre_key")),
            ("file", json!(file.name.clone())),
          ]),
        )
        .with_error(error_code.clone(), "pre_key hook failed")
        .with_stack(compact_stack("pre_key_hook", error_code.clone(), "hook failed")),
      );

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          2,
          context_from_pairs(vec![("cleanupStatus", json!("not_started"))]),
        )
        .with_error(error_code.clone(), "upload stopped at pre_key")
        .with_stack(compact_stack("pre_key_hook", error_code.clone(), "task stopped")),
      );

      return Err(error_code);
    }
    self.emit_hook_boundary(trace_id, HookStage::PreKey, false, payload.plugin_chain.len());

    if self.is_cancelled(trace_id) {
      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Warn,
          UploadEventStatus::Failed,
          2,
          context_from_pairs(vec![("cleanupStatus", json!("cancelled"))]),
        )
        .with_error("UPLOAD_CANCELLED", "upload cancelled by user"),
      );
      return Err("UPLOAD_CANCELLED".to_string());
    }

    let allocation = match self.key_allocator.allocate(file.name.as_str()) {
      Ok(allocation) => allocation,
      Err(error_code) => {
        self.log_center.emit(
          LogRecord::new(
            trace_id,
            UploadEventModule::Upload,
            "upload:task_failed",
            UploadEventLevel::Error,
            UploadEventStatus::Failed,
            2,
            context_from_pairs(vec![("cleanupStatus", json!("not_started"))]),
          )
          .with_error(error_code.clone(), "key allocation failed")
          .with_stack(compact_stack("key_allocation", error_code.clone(), "allocator failed")),
        );

        return Err(error_code);
      }
    };

    self.log_center.emit(LogRecord::new(
      trace_id,
      UploadEventModule::Upload,
      "upload:key_allocated",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      3,
      context_from_pairs(vec![
        ("stage", json!("reserved")),
        ("file", json!(file.name.clone())),
        ("number", json!(allocation.number.clone())),
        ("objectKey", json!(allocation.object_key.clone())),
      ]),
    ));

    if self.is_cancelled(trace_id) {
      let rollback_ok = self.key_allocator.release_reserved(allocation.number.as_str());

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Warn,
          UploadEventStatus::Failed,
          3,
          context_from_pairs(vec![
            (
              "cleanupStatus",
              json!(if rollback_ok { "rolled_back" } else { "rollback_failed" }),
            ),
            ("number", json!(allocation.number.clone())),
          ]),
        )
        .with_error("UPLOAD_CANCELLED", "upload cancelled after key allocation"),
      );

      return Err("UPLOAD_CANCELLED".to_string());
    }

    self.emit_hook_boundary(trace_id, HookStage::PostKey, true, payload.plugin_chain.len());
    if let Err(error_code) = self
      .plugin_runtime
      .execute_stage(HookStage::PostKey, &payload.plugin_chain)
    {
      let rollback_ok = self.key_allocator.release_reserved(allocation.number.as_str());

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:hook_error",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          4,
          context_from_pairs(vec![
            ("stage", json!("post_key")),
            ("file", json!(file.name.clone())),
            ("objectKey", json!(allocation.object_key.clone())),
          ]),
        )
        .with_error(error_code.clone(), "post_key hook failed")
        .with_stack(compact_stack("post_key_hook", error_code.clone(), "hook failed")),
      );

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          4,
          context_from_pairs(vec![(
            "cleanupStatus",
            json!(if rollback_ok { "rolled_back" } else { "rollback_failed" }),
          )]),
        )
        .with_error(error_code.clone(), "upload stopped at post_key")
        .with_stack(compact_stack("post_key_hook", error_code.clone(), "task stopped")),
      );

      return Err(error_code);
    }
    self.emit_hook_boundary(trace_id, HookStage::PostKey, false, payload.plugin_chain.len());

    if self.is_cancelled(trace_id) {
      let rollback_ok = self.key_allocator.release_reserved(allocation.number.as_str());

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Warn,
          UploadEventStatus::Failed,
          4,
          context_from_pairs(vec![
            (
              "cleanupStatus",
              json!(if rollback_ok { "rolled_back" } else { "rollback_failed" }),
            ),
            ("number", json!(allocation.number.clone())),
          ]),
        )
        .with_error("UPLOAD_CANCELLED", "upload cancelled after post_key stage"),
      );

      return Err("UPLOAD_CANCELLED".to_string());
    }

    self.log_center.emit(LogRecord::new(
      trace_id,
      UploadEventModule::Upload,
      "upload:adapter_start",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      5,
      context_from_pairs(vec![
        ("adapterName", json!(payload.target.label.clone())),
        ("file", json!(file.name.clone())),
        ("target", json!(payload.target.id.clone())),
        ("objectKey", json!(allocation.object_key.clone())),
      ]),
    ));

    let (adapter_result, retry_count) =
      self.put_object_with_retry(trace_id, file, allocation.object_key.as_str(), &payload.target);

    if !adapter_result.success {
      let rollback_ok = self.key_allocator.release_reserved(allocation.number.as_str());
      let error_code = adapter_result
        .error_code
        .unwrap_or_else(|| "ADAPTER_SERVER_ERROR".to_string());
      let error_message = adapter_result
        .error_message
        .unwrap_or_else(|| "adapter failed".to_string());

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:adapter_error",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          adapter_result.response_time,
          context_from_pairs(vec![
            ("adapterName", json!(payload.target.label.clone())),
            ("file", json!(file.name.clone())),
            ("target", json!(payload.target.id.clone())),
            ("objectKey", json!(allocation.object_key.clone())),
            ("retryCount", json!(retry_count)),
          ]),
        )
        .with_error(error_code.clone(), error_message.clone())
        .with_stack(compact_stack(
          "adapter",
          error_code.clone(),
          error_message.clone(),
        )),
      );

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          adapter_result.response_time,
          context_from_pairs(vec![(
            "cleanupStatus",
            json!(if rollback_ok { "rolled_back" } else { "rollback_failed" }),
          )]),
        )
        .with_error(error_code.clone(), "upload failed in adapter stage")
        .with_stack(compact_stack("adapter", error_code.clone(), error_message.clone())),
      );

      return Err(error_code);
    }

    let _ = self.key_allocator.activate(allocation.number.as_str());

    if self.is_cancelled(trace_id) {
      let rollback_delete_ok = self
        .adapter_runtime
        .delete_object(allocation.object_key.as_str())
        .success;
      if rollback_delete_ok {
        let _ = self.key_allocator.release_active(allocation.number.as_str());
        let _ = self.sync_active_object_allowlist();
      }

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Warn,
          UploadEventStatus::Failed,
          0,
          context_from_pairs(vec![
            (
              "cleanupStatus",
              json!(if rollback_delete_ok {
                "rolled_back"
              } else {
                "rollback_failed"
              }),
            ),
            ("number", json!(allocation.number.clone())),
          ]),
        )
        .with_error("UPLOAD_CANCELLED", "upload cancelled after cloud write"),
      );

      return Err("UPLOAD_CANCELLED".to_string());
    }

    let (waf_result, waf_allowlist_hash) = self.sync_active_object_allowlist();
    if !waf_result.success {
      let rollback_delete_ok = self
        .adapter_runtime
        .delete_object(allocation.object_key.as_str())
        .success;
      if rollback_delete_ok {
        let _ = self.key_allocator.release_active(allocation.number.as_str());
      }

      let error_code = waf_result
        .error_code
        .unwrap_or_else(|| "WAF_RULE_SYNC_FAILED".to_string());
      let error_message = waf_result
        .error_message
        .unwrap_or_else(|| "waf object allowlist sync failed".to_string());

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:waf_sync_error",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          waf_result.response_time,
          context_from_pairs_with_allowlist_hash(
            vec![
              ("file", json!(file.name.clone())),
              ("number", json!(allocation.number.clone())),
              ("objectKey", json!(allocation.object_key.clone())),
              ("rollbackDelete", json!(rollback_delete_ok)),
            ],
            waf_allowlist_hash.as_ref(),
          ),
        )
        .with_error(error_code.clone(), error_message.clone())
        .with_stack(compact_stack(
          "waf_sync",
          error_code.clone(),
          error_message.clone(),
        )),
      );

      self.log_center.emit(
        LogRecord::new(
          trace_id,
          UploadEventModule::Upload,
          "upload:task_failed",
          UploadEventLevel::Error,
          UploadEventStatus::Failed,
          waf_result.response_time,
          context_from_pairs(vec![("cleanupStatus", json!("waf_sync_failed"))]),
        )
        .with_error(error_code.clone(), "upload stopped at waf sync stage")
        .with_stack(compact_stack("waf_sync", error_code.clone(), error_message.clone())),
      );

      return Err(error_code);
    }

    self.log_center.emit(LogRecord::new(
      trace_id,
      UploadEventModule::Upload,
      "upload:waf_synced",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      waf_result.response_time,
      context_from_pairs_with_allowlist_hash(
        vec![
          ("file", json!(file.name.clone())),
          ("number", json!(allocation.number.clone())),
          ("objectKey", json!(allocation.object_key.clone())),
        ],
        waf_allowlist_hash.as_ref(),
      ),
    ));

    self.log_center.emit(LogRecord::new(
      trace_id,
      UploadEventModule::Upload,
      "upload:adapter_success",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      adapter_result.response_time,
      context_from_pairs(vec![
        ("adapterName", json!(payload.target.label.clone())),
        ("file", json!(file.name.clone())),
        ("target", json!(payload.target.id.clone())),
        ("objectKey", json!(allocation.object_key.clone())),
        ("retryCount", json!(retry_count)),
      ]),
    ));

    Ok(ProcessedUploadFile {
      number: allocation.number,
      object_key: allocation.object_key,
    })
  }

  pub fn collect_active_object_entries(&self) -> Vec<KvReadonlyObjectEntry> {
    let active_numbers = self.key_allocator.active_numbers();
    let mut entries = Vec::with_capacity(active_numbers.len());

    for number in active_numbers {
      if let Some(object_key) = self.key_allocator.object_key_for_number(number.as_str()) {
        entries.push(KvReadonlyObjectEntry { number, object_key });
        continue;
      }

      if let Some(object_key) = self.find_object_key_from_logs(number.as_str()) {
        entries.push(KvReadonlyObjectEntry { number, object_key });
      }
    }

    entries
  }

  fn sync_active_object_allowlist(&self) -> (AdapterResult, Option<String>) {
    let object_keys = self
      .collect_active_object_entries()
      .into_iter()
      .map(|entry| entry.object_key)
      .collect::<Vec<_>>();

    let allowlist_hash = self
      .adapter_runtime
      .waf_allowlist_fingerprint(object_keys.as_slice());
    let result = self
      .adapter_runtime
      .sync_waf_object_allowlist(object_keys.as_slice());

    (result, allowlist_hash)
  }

  fn find_object_key_from_logs(&self, number: &str) -> Option<String> {
    self
      .log_center
      .list(UploadEventFilter::default())
      .into_iter()
      .find_map(|event| {
        if event.module != UploadEventModule::Upload {
          return None;
        }

        if event.event_name != "upload:adapter_success"
          && event.event_name != "upload:key_allocated"
        {
          return None;
        }

        if event.context.get("number").and_then(Value::as_str) != Some(number) {
          return None;
        }

        event
          .context
          .get("objectKey")
          .and_then(Value::as_str)
          .map(|value| value.to_string())
      })
  }

  fn put_object_with_retry(
    &self,
    trace_id: &str,
    file: &UploadFileRef,
    object_key: &str,
    target: &StorageTargetConfig,
  ) -> (AdapterResult, u32) {
    let max_attempts: u32 = 3;
    let mut backoff_ms: u64 = 300;
    let mut attempt: u32 = 0;

    loop {
      if self.is_cancelled(trace_id) {
        return (
          AdapterResult {
            success: false,
            response_time: 0,
            error_code: Some("UPLOAD_CANCELLED".to_string()),
            error_message: Some("upload cancelled by user".to_string()),
          },
          attempt,
        );
      }

      let result = self.adapter_runtime.put_object(file, object_key, target);
      let should_retry = matches!(
        result.error_code.as_deref(),
        Some("ADAPTER_NETWORK_ERROR" | "ADAPTER_TIMEOUT" | "ADAPTER_RATE_LIMITED")
      );

      if result.success || !should_retry || attempt + 1 >= max_attempts {
        return (result, attempt);
      }

      thread::sleep(Duration::from_millis(backoff_ms));
      backoff_ms = (backoff_ms * 2).min(30_000);
      attempt += 1;
    }
  }

  fn mark_cancelled(&self, trace_id: &str) {
    let Ok(mut registry) = self.cancelled_traces.lock() else {
      return;
    };

    Self::cleanup_cancelled_registry(&mut registry);
    registry.insert(trace_id.to_string(), Self::timestamp_ms());
  }

  fn clear_cancelled(&self, trace_id: &str) {
    let Ok(mut registry) = self.cancelled_traces.lock() else {
      return;
    };

    Self::cleanup_cancelled_registry(&mut registry);

    registry.remove(trace_id);
  }

  fn is_cancelled(&self, trace_id: &str) -> bool {
    let Ok(mut registry) = self.cancelled_traces.lock() else {
      return false;
    };

    Self::cleanup_cancelled_registry(&mut registry);

    registry.contains_key(trace_id)
  }

  fn cleanup_cancelled_registry(registry: &mut HashMap<String, u64>) {
    let now_ms = Self::timestamp_ms();
    registry.retain(|_, marked_at| now_ms.saturating_sub(*marked_at) <= CANCELLED_TRACE_TTL_MS);

    if registry.len() <= MAX_CANCELLED_TRACE_ENTRIES {
      return;
    }

    let mut ordered = registry
      .iter()
      .map(|(trace_id, marked_at)| (trace_id.clone(), *marked_at))
      .collect::<Vec<_>>();
    ordered.sort_by_key(|(_, marked_at)| *marked_at);

    let overflow = registry.len().saturating_sub(MAX_CANCELLED_TRACE_ENTRIES);
    for (trace_id, _) in ordered.into_iter().take(overflow) {
      registry.remove(trace_id.as_str());
    }
  }

  fn timestamp_ms() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|duration| duration.as_millis() as u64)
      .unwrap_or(0)
  }

  fn recycle_failed(
    &self,
    trace_id: String,
    error_code: &str,
    error_message: &str,
    cleanup_status: &str,
    cache_purged: bool,
    waf_synced: bool,
  ) -> UploadRecycleResult {
    self.log_center.emit(
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

  fn emit_hook_boundary(
    &self,
    trace_id: &str,
    stage: HookStage,
    is_before: bool,
    plugin_count: usize,
  ) {
    let event_name = if is_before {
      "upload:hook_before_process"
    } else {
      "upload:hook_after_process"
    };

    self.log_center.emit(LogRecord::new(
      trace_id,
      UploadEventModule::Upload,
      event_name,
      UploadEventLevel::Debug,
      UploadEventStatus::Success,
      1,
      context_from_pairs(vec![
        (
          "stage",
          json!(match stage {
            HookStage::PreKey => "pre_key",
            HookStage::PostKey => "post_key",
          }),
        ),
        ("pluginCount", json!(plugin_count)),
      ]),
    ));
  }
}

fn failed_result(
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

fn context_from_pairs(entries: Vec<(&str, Value)>) -> HashMap<String, Value> {
  let mut context = HashMap::new();
  for (key, value) in entries {
    context.insert(key.to_string(), value);
  }
  context
}

fn context_from_pairs_with_allowlist_hash(
  mut entries: Vec<(&str, Value)>,
  allowlist_hash: Option<&String>,
) -> HashMap<String, Value> {
  if let Some(hash) = allowlist_hash {
    entries.push(("allowlistHash", json!(hash)));
  }

  context_from_pairs(entries)
}

fn compact_stack(stage: &str, error_code: impl Into<String>, details: impl Into<String>) -> String {
  format!(
    "upload::{} > {} > {}",
    stage,
    error_code.into(),
    details.into()
  )
}

#[cfg(test)]
mod tests {
  use super::UploadOrchestrator;
  use crate::contracts::{
    SettingsDraft,
    PluginConfig,
    StorageTargetConfig,
    UploadEventFilter,
    UploadFileRef,
    UploadStartPayload,
    UploadStartStatus,
  };
  use crate::domain::logging::center::LogCenter;
  use crate::runtime::adapter_runtime::AdapterRuntime;
  use crate::runtime::event_bus::EventBus;
  use crate::runtime::plugin_runtime::PluginRuntime;
  use crate::storage::key_allocator::KeyAllocator;
  use crate::storage::log_store::LogStore;
  use crate::storage::settings_store::SettingsStore;
  use serde_json::json;
  use std::sync::Arc;
  use uuid::Uuid;

  fn build_orchestrator() -> (UploadOrchestrator, LogCenter) {
    let log_store = Arc::new(LogStore::default());
    let event_bus = EventBus::new(log_store.clone());
    let log_center = LogCenter::new(event_bus, log_store);
    let settings_store = Arc::new(SettingsStore::default());
    let allocator_path = std::env::temp_dir().join(format!(
      "imgstar-orchestrator-key-{}",
      Uuid::new_v4()
    ));

    settings_store.save(SettingsDraft {
      access_key: "ak".to_string(),
      secret_key: "sk".to_string(),
      endpoint: "https://example.r2.dev".to_string(),
      bucket: "demo".to_string(),
      zone_id: Some("zone-1".to_string()),
      zone_api_token: Some("token-1".to_string()),
      cdn_base_url: Some("https://cdn.example.com".to_string()),
      region: Some("auto".to_string()),
      key_pattern: None,
      digit_count: Some(9),
      reuse_delay_ms: Some(900_000),
      preview_hash_enabled: Some(true),
      theme: Some("system".to_string()),
      language: Some("zh-CN".to_string()),
    });

    (
      UploadOrchestrator::new(
        Arc::new(
          KeyAllocator::new_with_path_and_store(settings_store.clone(), allocator_path.as_path())
            .expect("key allocator should initialize"),
        ),
        AdapterRuntime::new(settings_store),
        PluginRuntime,
        log_center.clone(),
      ),
      log_center,
    )
  }

  #[test]
  fn succeeds_and_emits_task_success_event() {
    let (orchestrator, log_center) = build_orchestrator();
    let result = orchestrator.start(UploadStartPayload {
      trace_id: None,
      files: vec![UploadFileRef {
        path: "mock/success.png".to_string(),
        name: "success.png".to_string(),
        size: 1024,
        mime_type: Some("image/png".to_string()),
        inline_content_base64: None,
      }],
      target: StorageTargetConfig {
        id: "r2-default".to_string(),
        label: "Cloudflare R2".to_string(),
      },
      plugin_chain: vec![PluginConfig {
        id: "image-compress".to_string(),
        enabled: true,
        hook_type: "upload".to_string(),
        stage: crate::contracts::HookStage::PreKey,
        priority: 1,
      }],
    });

    assert_eq!(result.status, UploadStartStatus::Success);
    let file_results = result
      .files
      .as_ref()
      .expect("file metadata should be present");
    assert_eq!(file_results.len(), 1);
    assert!(file_results[0].number.is_some());
    assert!(file_results[0].object_key.is_some());

    let events = log_center.list(UploadEventFilter {
      trace_id: Some(result.trace_id),
      ..UploadEventFilter::default()
    });
    assert!(events.iter().any(|event| event.event_name == "upload:task_success"));
  }

  #[test]
  fn emits_adapter_error_for_failed_upload() {
    let (orchestrator, log_center) = build_orchestrator();
    let result = orchestrator.start(UploadStartPayload {
      trace_id: None,
      files: vec![UploadFileRef {
        path: "mock/fail.png".to_string(),
        name: "fail.png".to_string(),
        size: 1024,
        mime_type: Some("image/png".to_string()),
        inline_content_base64: None,
      }],
      target: StorageTargetConfig {
        id: "r2-default".to_string(),
        label: "Cloudflare R2".to_string(),
      },
      plugin_chain: vec![],
    });

    assert_eq!(result.status, UploadStartStatus::Failed);
    assert_eq!(result.error, Some("ADAPTER_NETWORK_ERROR".to_string()));
    assert_eq!(result.files.as_ref().map(|items| items.len()), Some(1));

    let events = log_center.list(UploadEventFilter {
      trace_id: Some(result.trace_id),
      ..UploadEventFilter::default()
    });

    let adapter_error = events
      .iter()
      .find(|event| event.event_name == "upload:adapter_error")
      .expect("adapter error event should exist");

    assert_eq!(adapter_error.error_code, Some("ADAPTER_NETWORK_ERROR".to_string()));
    assert_eq!(adapter_error.context.get("retryCount"), Some(&json!(2)));
  }

  #[test]
  fn processes_multi_file_payload() {
    let (orchestrator, log_center) = build_orchestrator();
    let result = orchestrator.start(UploadStartPayload {
      trace_id: None,
      files: vec![
        UploadFileRef {
          path: "mock/a.png".to_string(),
          name: "a.png".to_string(),
          size: 1024,
          mime_type: Some("image/png".to_string()),
          inline_content_base64: None,
        },
        UploadFileRef {
          path: "mock/b.png".to_string(),
          name: "b.png".to_string(),
          size: 2048,
          mime_type: Some("image/png".to_string()),
          inline_content_base64: None,
        },
      ],
      target: StorageTargetConfig {
        id: "r2-default".to_string(),
        label: "Cloudflare R2".to_string(),
      },
      plugin_chain: vec![],
    });

    assert_eq!(result.status, UploadStartStatus::Success);
    assert_eq!(result.files.as_ref().map(|items| items.len()), Some(2));

    let events = log_center.list(UploadEventFilter {
      trace_id: Some(result.trace_id),
      ..UploadEventFilter::default()
    });

    let key_allocated_count = events
      .iter()
      .filter(|event| event.event_name == "upload:key_allocated")
      .count();
    assert_eq!(key_allocated_count, 2);
  }

  #[test]
  fn recycle_syncs_waf_before_purge_and_frees_number() {
    use crate::contracts::{UploadRecyclePayload, UploadFileStatus};
    use crate::storage::key_allocator::KeyState;

    let (orchestrator, log_center) = build_orchestrator();
    let upload_result = orchestrator.start(UploadStartPayload {
      trace_id: None,
      files: vec![UploadFileRef {
        path: "mock/recycle.png".to_string(),
        name: "recycle.png".to_string(),
        size: 1024,
        mime_type: Some("image/png".to_string()),
        inline_content_base64: None,
      }],
      target: StorageTargetConfig {
        id: "r2-default".to_string(),
        label: "Cloudflare R2".to_string(),
      },
      plugin_chain: vec![],
    });

    assert_eq!(upload_result.status, UploadStartStatus::Success);

    let file = upload_result
      .files
      .as_ref()
      .and_then(|items| items.first())
      .cloned()
      .expect("upload should return file metadata");

    let number = file.number.clone().expect("number should exist");
    let object_key = file.object_key.clone().expect("object key should exist");

    let recycle_result = orchestrator.recycle(UploadRecyclePayload {
      number: number.clone(),
      object_key: object_key.clone(),
      file_name: file.file_name.clone(),
      trace_id: Some("trace-recycle".to_string()),
    });

    assert_eq!(recycle_result.status, UploadFileStatus::Success);
    assert_eq!(recycle_result.cache_purged, true);
    assert_eq!(recycle_result.waf_synced, true);
    assert_eq!(orchestrator.key_allocator.state_of(number.as_str()), KeyState::Free);
    assert!(!orchestrator
      .key_allocator
      .active_numbers()
      .iter()
      .any(|item| item == &number));

    let events = log_center.list(UploadEventFilter {
      trace_id: Some("trace-recycle".to_string()),
      ..UploadEventFilter::default()
    });

    let event_names = events
      .iter()
      .map(|event| event.event_name.as_str())
      .collect::<Vec<_>>();

    let mut ordered_event_names = event_names.clone();
    ordered_event_names.reverse();

    assert_eq!(
      ordered_event_names,
      vec![
        "upload:recycle_started",
        "upload:waf_synced",
        "upload:cache_purged",
        "upload:recycle_success",
      ]
    );
  }
}
