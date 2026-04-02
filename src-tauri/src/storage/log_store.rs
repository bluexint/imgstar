use crate::contracts::{UploadEvent, UploadEventFilter};
use chrono::{DateTime, Utc};
use crate::storage::resolve_app_data_dir;
use heed::types::{Bytes, Str};
use heed::{Database, Env, EnvOpenOptions};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug)]
pub struct LogStore {
  env: Env,
  db: Database<Str, Bytes>,
  gate: Mutex<()>,
}

impl Default for LogStore {
  fn default() -> Self {
    let test_path = std::env::temp_dir().join(format!("imgstar-logs-{}", Uuid::new_v4()));
    Self::new_with_path(&test_path).expect("log store should initialize")
  }
}

impl LogStore {
  pub fn for_app() -> Result<Self, String> {
    let path = resolve_app_data_dir().join("logs");
    Self::new_with_path(&path)
  }

  pub fn new_with_path(path: &Path) -> Result<Self, String> {
    std::fs::create_dir_all(path).map_err(|_| "INTERNAL_ERROR".to_string())?;

    let env = unsafe {
      EnvOpenOptions::new()
        .max_dbs(8)
        .map_size(64 * 1024 * 1024)
        .open(path)
        .map_err(|_| "INTERNAL_ERROR".to_string())?
    };

    let mut wtxn = env.write_txn().map_err(|_| "INTERNAL_ERROR".to_string())?;
    let db: Database<Str, Bytes> = env
      .create_database(&mut wtxn, Some("log_store"))
      .map_err(|_| "INTERNAL_ERROR".to_string())?;
    wtxn.commit().map_err(|_| "INTERNAL_ERROR".to_string())?;

    Ok(Self {
      env,
      db,
      gate: Mutex::new(()),
    })
  }

  pub fn append(&self, event: UploadEvent) {
    let Ok(_guard) = self.gate.lock() else {
      return;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return;
    };

    let next_id = match self.read_last_id_rw(&wtxn) {
      Ok(last_id) => last_id + 1,
      Err(_) => 1,
    };
    let _ = self.write_last_id(&mut wtxn, next_id);

    let event_key = format!("event:{next_id:020}");
    if let Ok(encoded) = serde_json::to_vec(&event) {
      let _ = self.db.put(&mut wtxn, event_key.as_str(), encoded.as_slice());
      let _ = wtxn.commit();
    }
  }

  pub fn list(&self, filter: &UploadEventFilter) -> Vec<UploadEvent> {
    let Ok(rtxn) = self.env.read_txn() else {
      return vec![];
    };

    let Ok(iter) = self.db.iter(&rtxn) else {
      return vec![];
    };

    let mut events = vec![];
    for item in iter {
      let Ok((key, value)) = item else {
        continue;
      };
      if !key.starts_with("event:") {
        continue;
      }
      if let Ok(event) = serde_json::from_slice::<UploadEvent>(value) {
        events.push(event);
      }
    }
    events.reverse();

    events
      .iter()
      .filter(|event| {
        if let Some(module) = &filter.module {
          if event.module != *module {
            return false;
          }
        }

        if let Some(level) = &filter.level {
          if event.level != *level {
            return false;
          }
        }

        if let Some(trace_id) = &filter.trace_id {
          if event.trace_id != *trace_id {
            return false;
          }
        }

        if let Some(error_code) = &filter.error_code {
          if event.error_code.as_ref() != Some(error_code) {
            return false;
          }
        }

        within_time_range(event.timestamp.as_str(), filter)
      })
      .cloned()
      .collect()
  }

  pub fn clear(&self) {
    let Ok(_guard) = self.gate.lock() else {
      return;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return;
    };

    let mut keys = vec![];
    if let Ok(iter) = self.db.iter(&wtxn) {
      for item in iter {
        let Ok((key, _)) = item else {
          continue;
        };
        if key.starts_with("event:") || key == "meta:last_id" {
          keys.push(key.to_string());
        }
      }
    }

    for key in keys {
      let _ = self.db.delete(&mut wtxn, key.as_str());
    }

    let _ = wtxn.commit();
  }

  fn read_last_id_rw(&self, txn: &heed::RwTxn<'_>) -> Result<u64, String> {
    let raw = self
      .db
      .get(txn, "meta:last_id")
      .map_err(|_| "INTERNAL_ERROR".to_string())?;

    if let Some(bytes) = raw {
      let text = std::str::from_utf8(bytes).map_err(|_| "INTERNAL_ERROR".to_string())?;
      text.parse::<u64>().map_err(|_| "INTERNAL_ERROR".to_string())
    } else {
      Ok(0)
    }
  }

  fn write_last_id(&self, txn: &mut heed::RwTxn<'_>, last_id: u64) -> Result<(), String> {
    let value = last_id.to_string();
    self
      .db
      .put(txn, "meta:last_id", value.as_bytes())
      .map_err(|_| "INTERNAL_ERROR".to_string())
  }
}

fn within_time_range(timestamp: &str, filter: &UploadEventFilter) -> bool {
  let Some(event_time) = parse_time(timestamp) else {
    return false;
  };

  if let Some(start_at) = &filter.start_at {
    let Some(start_time) = parse_time(start_at) else {
      return false;
    };
    if event_time < start_time {
      return false;
    }
  }

  if let Some(end_at) = &filter.end_at {
    let Some(end_time) = parse_time(end_at) else {
      return false;
    };
    if event_time > end_time {
      return false;
    }
  }

  true
}

fn parse_time(input: &str) -> Option<DateTime<Utc>> {
  DateTime::parse_from_rfc3339(input)
    .ok()
    .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
  use super::LogStore;
  use crate::contracts::{
    UploadEvent,
    UploadEventFilter,
    UploadEventLevel,
    UploadEventModule,
    UploadEventStatus,
  };
  use serde_json::json;
  use std::collections::HashMap;
  use uuid::Uuid;

  #[test]
  fn persists_log_events_across_reopen() {
    let path = std::env::temp_dir().join(format!("imgstar-log-persist-{}", Uuid::new_v4()));
    let store = LogStore::new_with_path(&path).expect("store should initialize");

    let mut context = HashMap::new();
    context.insert("k".to_string(), json!("v"));

    store.append(UploadEvent {
      trace_id: "trace-1".to_string(),
      timestamp: "2026-03-30T00:00:00.000Z".to_string(),
      module: UploadEventModule::Upload,
      event_name: "upload:task_success".to_string(),
      level: UploadEventLevel::Info,
      status: UploadEventStatus::Success,
      error_code: None,
      error_message: None,
      stack: None,
      duration: 12,
      context,
    });

    drop(store);

    let reopened = LogStore::new_with_path(&path).expect("store should reopen");
    let events = reopened.list(&UploadEventFilter::default());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].trace_id, "trace-1");
  }

  #[test]
  fn clears_persisted_log_events() {
    let path = std::env::temp_dir().join(format!("imgstar-log-clear-{}", Uuid::new_v4()));
    let store = LogStore::new_with_path(&path).expect("store should initialize");

    let mut context = HashMap::new();
    context.insert("k".to_string(), json!("v"));

    store.append(UploadEvent {
      trace_id: "trace-clear".to_string(),
      timestamp: "2026-03-30T00:00:00.000Z".to_string(),
      module: UploadEventModule::Upload,
      event_name: "upload:task_success".to_string(),
      level: UploadEventLevel::Info,
      status: UploadEventStatus::Success,
      error_code: None,
      error_message: None,
      stack: None,
      duration: 8,
      context,
    });

    store.clear();
    let events = store.list(&UploadEventFilter::default());
    assert!(events.is_empty());
  }
}
