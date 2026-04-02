use app_lib::app_state::AppState;
use app_lib::contracts::{
  StorageTargetConfig,
  UploadEventFilter,
  UploadFileRef,
  UploadFileStatus,
  UploadRecyclePayload,
  UploadStartPayload,
  UploadStartStatus,
};

const ONE_BY_ONE_PNG_BASE64: &str =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7+J2kAAAAASUVORK5CYII=";

#[test]
#[ignore = "requires real app config in .imgstar-data and external Cloudflare access"]
fn upload_and_recycle_with_real_app_config() {
  let state = AppState::new();
  let snapshot = state.settings_service.snapshot();

  assert!(
    snapshot.configured,
    "app settings are not configured; fill storage settings in the app first"
  );

  let payload = UploadStartPayload {
     trace_id: None,
    files: vec![UploadFileRef {
      path: "inline/waf-smoke.png".to_string(),
      name: "waf-smoke.png".to_string(),
      size: 68,
      mime_type: Some("image/png".to_string()),
      inline_content_base64: Some(ONE_BY_ONE_PNG_BASE64.to_string()),
    }],
    target: StorageTargetConfig {
      id: "r2-default".to_string(),
      label: "Cloudflare R2".to_string(),
    },
    plugin_chain: vec![],
  };

  let start_result = state.upload_orchestrator.start(payload);
  if start_result.status != UploadStartStatus::Success {
    let events = state.log_center.list(UploadEventFilter {
      trace_id: Some(start_result.trace_id.clone()),
      ..UploadEventFilter::default()
    });
    let compact_events = events
      .iter()
      .filter(|event| {
        event.event_name == "upload:waf_sync_error"
          || event.event_name == "upload:task_failed"
          || event.event_name == "upload:adapter_error"
      })
      .map(|event| {
        let error = event.error_code.as_deref().unwrap_or("none");
        let message = event.error_message.clone().unwrap_or_default();
        format!("{}:{}:{}", event.event_name, error, message)
      })
      .collect::<Vec<_>>();

    panic!(
      "upload failed: status={:?}, error={:?}, trace_id={}, events={:?}",
      start_result.status,
      start_result.error,
      start_result.trace_id,
      compact_events
    );
  }

  let uploaded = start_result
    .files
    .as_ref()
    .and_then(|items| items.first())
    .expect("upload should return one file result");

  let number = uploaded
    .number
    .as_ref()
    .expect("upload should return allocated number")
    .to_string();
  let object_key = uploaded
    .object_key
    .as_ref()
    .expect("upload should return object key")
    .to_string();

  let recycle_result = state.upload_orchestrator.recycle(UploadRecyclePayload {
    number,
    object_key,
    file_name: uploaded.file_name.clone(),
    trace_id: Some(start_result.trace_id.clone()),
  });

  if recycle_result.status != UploadFileStatus::Success {
    let events = state.log_center.list(UploadEventFilter {
      trace_id: Some(recycle_result.trace_id.clone()),
      ..UploadEventFilter::default()
    });
    let compact_events = events
      .iter()
      .filter(|event| {
        event.event_name == "upload:recycle_failed"
          || event.event_name == "upload:waf_sync_error"
          || event.event_name == "upload:task_failed"
      })
      .map(|event| {
        let error = event.error_code.as_deref().unwrap_or("none");
        let message = event.error_message.clone().unwrap_or_default();
        format!("{}:{}:{}", event.event_name, error, message)
      })
      .collect::<Vec<_>>();

    panic!(
      "recycle failed: status={:?}, error={:?}, trace_id={}, events={:?}",
      recycle_result.status,
      recycle_result.error,
      recycle_result.trace_id,
      compact_events
    );
  }
}
