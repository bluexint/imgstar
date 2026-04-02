use crate::app_state::AppState;
use crate::contracts::{PreviewResult, UploadFileRef};

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_preview_get(
  state: tauri::State<'_, AppState>,
  payload: UploadFileRef,
) -> Result<PreviewResult, String> {
  let preview_service = state.preview_service.clone();
  tauri::async_runtime::spawn_blocking(move || preview_service.get_preview(payload))
    .await
    .map_err(|_| "INTERNAL_ERROR".to_string())?
}
