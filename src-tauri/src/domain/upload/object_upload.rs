use crate::contracts::{
  HookStage,
  StorageTargetConfig,
  UploadEventLevel,
  UploadEventModule,
  UploadEventStatus,
  UploadFileRef,
  UploadStartPayload,
};
use crate::domain::logging::center::{LogCenter, LogRecord};
use crate::domain::upload::support::{
  compact_stack,
  context_from_pairs,
  context_from_pairs_with_allowlist_hash,
  ProcessedUploadFile,
};
use crate::domain::upload::waf_sync::WafAllowlistSync;
use crate::runtime::adapter_runtime::{AdapterResult, AdapterRuntime};
use crate::runtime::plugin_runtime::PluginRuntime;
use crate::storage::key_allocator::KeyAllocator;
use serde_json::json;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct ObjectUploadCoordinator {
  key_allocator: Arc<KeyAllocator>,
  adapter_runtime: AdapterRuntime,
  plugin_runtime: PluginRuntime,
  log_center: LogCenter,
  waf_allowlist_sync: WafAllowlistSync,
}

impl ObjectUploadCoordinator {
  pub fn new(
    key_allocator: Arc<KeyAllocator>,
    adapter_runtime: AdapterRuntime,
    plugin_runtime: PluginRuntime,
    log_center: LogCenter,
    waf_allowlist_sync: WafAllowlistSync,
  ) -> Self {
    Self {
      key_allocator,
      adapter_runtime,
      plugin_runtime,
      log_center,
      waf_allowlist_sync,
    }
  }

  pub(crate) fn process_single_file<F>(
    &self,
    trace_id: &str,
    file: &UploadFileRef,
    payload: &UploadStartPayload,
    is_cancelled: &F,
  ) -> Result<ProcessedUploadFile, String>
  where
    F: Fn(&str) -> bool,
  {
    if is_cancelled(trace_id) {
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

    if is_cancelled(trace_id) {
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

    if is_cancelled(trace_id) {
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

    if is_cancelled(trace_id) {
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

    let (adapter_result, retry_count) = self.put_object_with_retry(
      trace_id,
      file,
      allocation.object_key.as_str(),
      &payload.target,
      is_cancelled,
    );

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
        .with_stack(compact_stack("adapter", error_code.clone(), error_message.clone())),
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

    if is_cancelled(trace_id) {
      let rollback_delete_ok = self
        .adapter_runtime
        .delete_object(allocation.object_key.as_str())
        .success;
      if rollback_delete_ok {
        let _ = self.key_allocator.release_active(allocation.number.as_str());
        let _ = self.waf_allowlist_sync.sync_active_object_allowlist();
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

    let (waf_result, waf_allowlist_hash) = self.waf_allowlist_sync.sync_active_object_allowlist();
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
        .with_stack(compact_stack("waf_sync", error_code.clone(), error_message.clone())),
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

  fn put_object_with_retry<F>(
    &self,
    trace_id: &str,
    file: &UploadFileRef,
    object_key: &str,
    target: &StorageTargetConfig,
    is_cancelled: &F,
  ) -> (AdapterResult, u32)
  where
    F: Fn(&str) -> bool,
  {
    let max_attempts: u32 = 3;
    let mut backoff_ms: u64 = 300;
    let mut attempt: u32 = 0;

    loop {
      if is_cancelled(trace_id) {
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