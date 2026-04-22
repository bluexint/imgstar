use crate::contracts::{
  KvReadonlyObjectEntry,
  UploadEventLevel,
  UploadEventModule,
  UploadEventStatus,
  UploadFileResult,
  UploadFileStatus,
  UploadRecyclePayload,
  UploadRecycleResult,
  UploadStartPayload,
  UploadStartResult,
  UploadStartStatus,
};
use crate::domain::logging::center::{LogCenter, LogRecord};
use crate::domain::upload::object_upload::ObjectUploadCoordinator;
use crate::domain::upload::recycle::UploadRecycleCoordinator;
use crate::domain::upload::support::{context_from_pairs, failed_result};
use crate::domain::upload::waf_sync::WafAllowlistSync;
use crate::runtime::adapter_runtime::AdapterRuntime;
use crate::runtime::plugin_runtime::PluginRuntime;
use crate::storage::key_allocator::KeyAllocator;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const CANCELLED_TRACE_TTL_MS: u64 = 3_600_000;
const MAX_CANCELLED_TRACE_ENTRIES: usize = 4_096;

#[derive(Clone)]
pub struct UploadOrchestrator {
  #[allow(dead_code)]
  pub(crate) key_allocator: Arc<KeyAllocator>,
  log_center: LogCenter,
  cancelled_traces: Arc<Mutex<HashMap<String, u64>>>,
  object_upload: ObjectUploadCoordinator,
  recycle_coordinator: UploadRecycleCoordinator,
  waf_allowlist_sync: WafAllowlistSync,
}

impl UploadOrchestrator {
  pub fn new(
    key_allocator: Arc<KeyAllocator>,
    adapter_runtime: AdapterRuntime,
    plugin_runtime: PluginRuntime,
    log_center: LogCenter,
  ) -> Self {
    let waf_allowlist_sync = WafAllowlistSync::new(
      key_allocator.clone(),
      adapter_runtime.clone(),
      log_center.clone(),
    );
    let object_upload = ObjectUploadCoordinator::new(
      key_allocator.clone(),
      adapter_runtime.clone(),
      plugin_runtime,
      log_center.clone(),
      waf_allowlist_sync.clone(),
    );
    let recycle_coordinator = UploadRecycleCoordinator::new(
      key_allocator.clone(),
      adapter_runtime,
      log_center.clone(),
      waf_allowlist_sync.clone(),
    );

    Self {
      key_allocator,
      log_center,
      cancelled_traces: Arc::new(Mutex::new(HashMap::new())),
      object_upload,
      recycle_coordinator,
      waf_allowlist_sync,
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

      let is_cancelled = |candidate: &str| self.is_cancelled(candidate);
      match self
        .object_upload
        .process_single_file(trace_id.as_str(), file, &payload, &is_cancelled)
      {
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
    self.recycle_coordinator.recycle(payload)
  }

  pub fn collect_active_object_entries(&self) -> Vec<KvReadonlyObjectEntry> {
    self.waf_allowlist_sync.collect_active_object_entries()
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

  #[allow(deprecated)]
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
      reuse_delay_ms: None,
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
