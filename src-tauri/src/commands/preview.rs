use crate::app_state::AppState;
use crate::commands::run_blocking;
use crate::contracts::{PreviewResult, UploadFileRef};

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_preview_get(
  state: tauri::State<'_, AppState>,
  payload: UploadFileRef,
) -> Result<PreviewResult, String> {
  let preview_service = state.preview_service.clone();
  run_blocking(move || preview_service.get_preview(payload)).await?
}
