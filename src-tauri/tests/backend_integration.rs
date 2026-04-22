use app_lib::app_state::AppState;
use app_lib::contracts::{
  StorageTargetConfig,
  SettingsDraft,
  UploadEventFilter,
  UploadFileRef,
  UploadStartPayload,
  UploadStartStatus,
};

fn build_payload(file_name: &str, size: u64) -> UploadStartPayload {
  UploadStartPayload {
    trace_id: None,
    files: vec![UploadFileRef {
      path: format!("mock/{file_name}"),
      name: file_name.to_string(),
      size,
      mime_type: Some("image/png".to_string()),
      inline_content_base64: None,
    }],
    target: StorageTargetConfig {
      id: "r2-default".to_string(),
      label: "Cloudflare R2".to_string(),
    },
    plugin_chain: vec![],
  }
}

#[allow(deprecated)]
fn build_configured_state() -> AppState {
  let state = AppState::new();
  state.settings_service.save(SettingsDraft {
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
  }).expect("upload settings should save");

  state
}

#[test]
fn failed_upload_records_adapter_error_and_task_failed() {
  let state = AppState::new();
  let result = state.upload_orchestrator.start(build_payload("fail.png", 1_024));

  assert_eq!(result.status, UploadStartStatus::Failed);
  assert_eq!(result.error, Some("ADAPTER_NETWORK_ERROR".to_string()));
  assert_eq!(result.files.as_ref().map(|items| items.len()), Some(1));

  let events = state.log_center.list(UploadEventFilter {
    trace_id: Some(result.trace_id),
    ..UploadEventFilter::default()
  });

  let adapter_error = events
    .iter()
    .find(|event| event.event_name == "upload:adapter_error")
    .expect("adapter error event should exist");
  assert_eq!(adapter_error.context.get("retryCount"), Some(&serde_json::json!(2)));
  assert!(events.iter().any(|event| event.event_name == "upload:task_failed"));
}

#[test]
fn successful_upload_records_task_success() {
  let state = build_configured_state();
  let result = state
    .upload_orchestrator
    .start(build_payload("success.png", 2_048));

  assert_eq!(
    result.status,
    UploadStartStatus::Success,
    "error={:?}",
    result.error
  );
  assert_eq!(result.error, None);
  assert_eq!(result.files.as_ref().map(|items| items.len()), Some(1));

  let events = state.log_center.list(UploadEventFilter {
    trace_id: Some(result.trace_id),
    ..UploadEventFilter::default()
  });

  assert!(events.iter().any(|event| event.event_name == "upload:task_success"));
}

#[test]
fn multi_file_payload_records_key_allocations_for_each_file() {
  let state = build_configured_state();
  let mut payload = build_payload("success-a.png", 2_048);
  payload.files.push(UploadFileRef {
    path: "mock/success-b.png".to_string(),
    name: "success-b.png".to_string(),
    size: 3_072,
    mime_type: Some("image/png".to_string()),
    inline_content_base64: None,
  });

  let result = state.upload_orchestrator.start(payload);

  assert_eq!(
    result.status,
    UploadStartStatus::Success,
    "error={:?}",
    result.error
  );
  assert_eq!(result.files.as_ref().map(|items| items.len()), Some(2));

  let events = state.log_center.list(UploadEventFilter {
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
fn plugin_verification_records_verified_event_in_open_mode() {
  let state = AppState::new();
  let verify = state
    .plugin_service
    .verify("hidden-watermark".to_string(), Some("imgstar-official".to_string()));

  assert!(verify.verified);
  assert_eq!(verify.reason, None);

  let events = state.log_center.list(UploadEventFilter::default());
  assert!(
    events
      .iter()
      .any(|event| event.event_name == "plugin:signature_verified")
  );
}
