use crate::app_state::AppState;
use crate::contracts::PluginVerificationResult;

#[tauri::command(rename_all = "camelCase")]
pub async fn cmd_plugin_verify(
  state: tauri::State<'_, AppState>,
  plugin_id: String,
  signer_source: Option<String>,
) -> Result<PluginVerificationResult, String> {
  Ok(state.plugin_service.verify(plugin_id, signer_source))
}
