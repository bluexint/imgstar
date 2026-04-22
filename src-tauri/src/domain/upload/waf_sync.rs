use crate::contracts::{KvReadonlyObjectEntry, UploadEventFilter, UploadEventModule};
use crate::domain::logging::center::LogCenter;
use crate::runtime::adapter_runtime::{AdapterResult, AdapterRuntime};
use crate::storage::key_allocator::KeyAllocator;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct WafAllowlistSync {
  key_allocator: Arc<KeyAllocator>,
  adapter_runtime: AdapterRuntime,
  log_center: LogCenter,
}

impl WafAllowlistSync {
  pub fn new(
    key_allocator: Arc<KeyAllocator>,
    adapter_runtime: AdapterRuntime,
    log_center: LogCenter,
  ) -> Self {
    Self {
      key_allocator,
      adapter_runtime,
      log_center,
    }
  }

  pub fn collect_active_object_entries(&self) -> Vec<KvReadonlyObjectEntry> {
    let active_numbers = self.key_allocator.active_numbers();
    let mut entries = Vec::with_capacity(active_numbers.len());

    for number in active_numbers {
      if let Some(object_key) = self.key_allocator.object_key_for_number(number.as_str()) {
        entries.push(KvReadonlyObjectEntry { number, object_key });
        continue;
      }

      if let Some(object_key) = self.find_object_key_from_logs(number.as_str()) {
        entries.push(KvReadonlyObjectEntry { number, object_key });
      }
    }

    entries
  }

  pub fn sync_active_object_allowlist(&self) -> (AdapterResult, Option<String>) {
    let object_keys = self
      .collect_active_object_entries()
      .into_iter()
      .map(|entry| entry.object_key)
      .collect::<Vec<_>>();

    let allowlist_hash = self
      .adapter_runtime
      .waf_allowlist_fingerprint(object_keys.as_slice());
    let result = self
      .adapter_runtime
      .sync_waf_object_allowlist(object_keys.as_slice());

    (result, allowlist_hash)
  }

  fn find_object_key_from_logs(&self, number: &str) -> Option<String> {
    self
      .log_center
      .list(UploadEventFilter::default())
      .into_iter()
      .find_map(|event| {
        if event.module != UploadEventModule::Upload {
          return None;
        }

        if event.event_name != "upload:adapter_success"
          && event.event_name != "upload:key_allocated"
        {
          return None;
        }

        if event.context.get("number").and_then(Value::as_str) != Some(number) {
          return None;
        }

        event
          .context
          .get("objectKey")
          .and_then(Value::as_str)
          .map(|value| value.to_string())
      })
  }
}