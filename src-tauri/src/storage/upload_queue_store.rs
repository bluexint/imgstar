use crate::contracts::UploadQueueSnapshot;
use crate::storage::resolve_app_data_dir;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug)]
pub struct UploadQueueStore {
  path: PathBuf,
  gate: Mutex<()>,
}

impl Default for UploadQueueStore {
  fn default() -> Self {
    let test_path = std::env::temp_dir().join(format!("imgstar-upload-queue-{}", Uuid::new_v4()));
    Self::new_with_path(&test_path).expect("upload queue store should initialize")
  }
}

impl UploadQueueStore {
  pub fn for_app() -> Result<Self, String> {
    let path = resolve_app_data_dir().join("upload_queue").join("snapshot.json");
    Self::new_with_path(&path)
  }

  pub fn new_with_path(path: &Path) -> Result<Self, String> {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(|_| "INTERNAL_ERROR".to_string())?;
    }

    Ok(Self {
      path: path.to_path_buf(),
      gate: Mutex::new(()),
    })
  }

  pub fn save(&self, snapshot: UploadQueueSnapshot) -> Result<(), String> {
    let Ok(_guard) = self.gate.lock() else {
      return Err("INTERNAL_ERROR".to_string());
    };

    let encoded = serde_json::to_vec(&snapshot).map_err(|_| "INTERNAL_ERROR".to_string())?;
    let temp_path = self.path.with_extension("json.tmp");

    std::fs::write(&temp_path, encoded).map_err(|_| "INTERNAL_ERROR".to_string())?;
    let _ = std::fs::remove_file(&self.path);
    std::fs::rename(&temp_path, &self.path).map_err(|_| {
      let _ = std::fs::remove_file(&temp_path);
      "INTERNAL_ERROR".to_string()
    })
  }

  pub fn load(&self) -> Result<Option<UploadQueueSnapshot>, String> {
    let Ok(_guard) = self.gate.lock() else {
      return Err("INTERNAL_ERROR".to_string());
    };

    if !self.path.exists() {
      return Ok(None);
    }

    let bytes = std::fs::read(&self.path).map_err(|_| "INTERNAL_ERROR".to_string())?;
    let snapshot = serde_json::from_slice::<UploadQueueSnapshot>(bytes.as_slice())
      .map_err(|_| "INTERNAL_ERROR".to_string())?;
    Ok(Some(snapshot))
  }

  pub fn clear(&self) -> Result<(), String> {
    let Ok(_guard) = self.gate.lock() else {
      return Err("INTERNAL_ERROR".to_string());
    };

    if self.path.exists() {
      std::fs::remove_file(&self.path).map_err(|_| "INTERNAL_ERROR".to_string())?;
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::UploadQueueStore;
  use crate::contracts::{
    UploadFileRef,
    UploadQueueSnapshot,
    UploadTaskSnapshot,
    UploadTaskStatus,
  };
  use std::collections::HashMap;
  use uuid::Uuid;

  #[test]
  fn persists_queue_snapshot_across_reopen() {
    let path = std::env::temp_dir().join(format!("imgstar-upload-queue-persist-{}", Uuid::new_v4()));
    let store = UploadQueueStore::new_with_path(&path).expect("store should initialize");
    let mut thumbnails = HashMap::new();
    thumbnails.insert("task-1".to_string(), "data:image/png;base64,AA==".to_string());

    store
      .save(UploadQueueSnapshot {
        tasks: vec![UploadTaskSnapshot {
          id: "task-1".to_string(),
          file: UploadFileRef {
            path: "picked/a.png".to_string(),
            name: "a.png".to_string(),
            size: 1,
            mime_type: Some("image/png".to_string()),
            inline_content_base64: Some("AA==".to_string()),
          },
          trace_id: Some("trace-1".to_string()),
          number: None,
          object_key: None,
          progress: 0,
          status: UploadTaskStatus::Queued,
          error: None,
          started_at: None,
          completed_at: None,
          speed_bps: None,
        }],
        thumbnails,
        target_id: "r2-default".to_string(),
      })
      .expect("queue snapshot should save");

    drop(store);

    let reopened = UploadQueueStore::new_with_path(&path).expect("store should reopen");
    let snapshot = reopened
      .load()
      .expect("snapshot should load")
      .expect("snapshot should exist");

    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].file.name, "a.png");
    assert_eq!(snapshot.target_id, "r2-default");
  }

  #[test]
  fn clears_saved_queue_snapshot() {
    let path = std::env::temp_dir().join(format!("imgstar-upload-queue-clear-{}", Uuid::new_v4()));
    let store = UploadQueueStore::new_with_path(&path).expect("store should initialize");

    store
      .save(UploadQueueSnapshot {
        tasks: vec![],
        thumbnails: HashMap::new(),
        target_id: "r2-default".to_string(),
      })
      .expect("queue snapshot should save");

    store.clear().expect("queue snapshot should clear");
    assert!(store.load().expect("snapshot load should succeed").is_none());
  }
}