use crate::app_state::AppState;
use crate::contracts::{ConnectionPingResult, SaveSettingsResult, SettingsDraft, SettingsSnapshot};

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_settings_save(
  state: tauri::State<'_, AppState>,
  payload: SettingsDraft,
) -> Result<SaveSettingsResult, String> {
  state.settings_service.save(payload)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_settings_get(
  state: tauri::State<'_, AppState>,
) -> Result<SettingsDraft, String> {
  Ok(state.settings_service.get())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_settings_get_snapshot(
  state: tauri::State<'_, AppState>,
) -> Result<SettingsSnapshot, String> {
  Ok(state.settings_service.snapshot())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_settings_reset_app(
  state: tauri::State<'_, AppState>,
) -> Result<SettingsSnapshot, String> {
  Ok(state.reset_app())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_settings_ping(
  state: tauri::State<'_, AppState>,
) -> Result<ConnectionPingResult, String> {
  state.settings_service.ping().await
}
