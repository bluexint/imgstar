pub mod app_state;
pub mod commands;
pub mod contracts;
pub mod domain;
pub mod runtime;
pub mod storage;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(app_state::AppState::new())
    .invoke_handler(tauri::generate_handler![
      commands::upload::cmd_upload_start,
      commands::upload::cmd_upload_cancel,
      commands::upload::cmd_upload_recycle,
      commands::upload::cmd_upload_queue_get_snapshot,
      commands::upload::cmd_upload_queue_save_snapshot,
      commands::upload::cmd_upload_queue_clear_snapshot,
      commands::upload::cmd_upload_release_reserved_number,
      commands::preview::cmd_preview_get,
      commands::plugins::cmd_plugin_verify,
      commands::settings::cmd_settings_get,
      commands::settings::cmd_settings_get_snapshot,
      commands::settings::cmd_settings_save,
      commands::settings::cmd_settings_reset_app,
      commands::settings::cmd_settings_ping,
      commands::logs::cmd_logs_list,
      commands::logs::cmd_logs_clear,
      commands::logs::cmd_logs_kv_readonly_snapshot,
    ])
    .setup(|app| {
      if let Some(window) = app.get_webview_window("main") {
        if let Some(icon) = app.handle().default_window_icon().cloned() {
          let _ = window.set_icon(icon);
        }
      }

      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
