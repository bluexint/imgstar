pub mod key_allocator;
pub mod log_store;
pub mod upload_queue_store;
pub mod settings_store;

use std::path::PathBuf;

pub fn resolve_app_data_dir() -> PathBuf {
	if let Some(custom) = std::env::var_os("IMGSTAR_DATA_DIR") {
		return PathBuf::from(custom);
	}

	std::env::current_dir()
		.unwrap_or_else(|_| PathBuf::from("."))
		.join(".imgstar-data")
}
