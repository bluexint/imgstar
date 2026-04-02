use crate::app_state::AppState;
use crate::contracts::{KvReadonlySnapshot, UploadEvent, UploadEventFilter};

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_logs_list(
  state: tauri::State<'_, AppState>,
  payload: UploadEventFilter,
) -> Result<Vec<UploadEvent>, String> {
  Ok(state.log_center.list(payload))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_logs_clear(
  state: tauri::State<'_, AppState>,
) -> Result<(), String> {
  state.log_center.clear();
  Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_logs_kv_readonly_snapshot(
  state: tauri::State<'_, AppState>,
) -> Result<KvReadonlySnapshot, String> {
  Ok(KvReadonlySnapshot {
    digit_count: state.key_allocator.digit_count(),
    objects: state.upload_orchestrator.collect_active_object_entries(),
  })
}
