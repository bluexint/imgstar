use crate::app_state::AppState;
use crate::commands::run_blocking;
use crate::contracts::{
  UploadQueueSnapshot,
  UploadRecyclePayload,
  UploadRecycleResult,
  UploadStartPayload,
  UploadStartResult,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_upload_start(
  state: tauri::State<'_, AppState>,
  payload: UploadStartPayload,
) -> Result<UploadStartResult, String> {
  let orchestrator = state.upload_orchestrator.clone();
  run_blocking(move || orchestrator.start(payload)).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_upload_cancel(
  state: tauri::State<'_, AppState>,
  trace_id: String,
) -> Result<(), String> {
  state.upload_orchestrator.cancel(trace_id);
  Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_upload_recycle(
  state: tauri::State<'_, AppState>,
  payload: UploadRecyclePayload,
) -> Result<UploadRecycleResult, String> {
  let orchestrator = state.upload_orchestrator.clone();
  run_blocking(move || orchestrator.recycle(payload)).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_upload_queue_get_snapshot(
  state: tauri::State<'_, AppState>,
) -> Result<UploadQueueSnapshot, String> {
  let upload_queue_store = state.upload_queue_store.clone();
  run_blocking(move || {
    upload_queue_store.load().map(|snapshot| {
      snapshot.unwrap_or_else(|| UploadQueueSnapshot {
        tasks: vec![],
        thumbnails: std::collections::HashMap::new(),
        target_id: "r2-default".to_string(),
      })
    })
  })
  .await?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_upload_queue_save_snapshot(
  state: tauri::State<'_, AppState>,
  payload: UploadQueueSnapshot,
) -> Result<(), String> {
  let upload_queue_store = state.upload_queue_store.clone();
  run_blocking(move || upload_queue_store.save(payload)).await?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_upload_queue_clear_snapshot(
  state: tauri::State<'_, AppState>,
) -> Result<(), String> {
  let upload_queue_store = state.upload_queue_store.clone();
  run_blocking(move || upload_queue_store.clear()).await?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_upload_release_reserved_number(
  state: tauri::State<'_, AppState>,
  number: String,
) -> Result<bool, String> {
  let key_allocator = state.key_allocator.clone();
  run_blocking(move || key_allocator.release_reserved(number.as_str())).await
}
