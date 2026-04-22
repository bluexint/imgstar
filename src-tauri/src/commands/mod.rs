pub mod logs;
pub mod plugins;
pub mod preview;
pub mod settings;
pub mod upload;

pub(crate) async fn run_blocking<T, F>(operation: F) -> Result<T, String>
where
	T: Send + 'static,
	F: FnOnce() -> T + Send + 'static,
{
	tauri::async_runtime::spawn_blocking(operation)
		.await
		.map_err(|_| "INTERNAL_ERROR".to_string())
}
