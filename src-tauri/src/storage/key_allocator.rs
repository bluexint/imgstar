use crate::contracts::KvReadonlyObjectEntry;
use crate::storage::resolve_app_data_dir;
use crate::storage::settings_store::SettingsStore;
use heed::types::{Bytes, Str};
use heed::{Database, Env, EnvOpenOptions};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const KV_BUCKET_SIZE: u64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
  Free,
  Reserved,
  Active,
  Deleted,
  Cooling,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyAllocation {
  pub number: String,
  pub object_key: String,
}

#[derive(Debug)]
pub struct KeyAllocator {
  env: Env,
  db: Database<Str, Bytes>,
  gate: Mutex<()>,
  settings_store: Option<Arc<SettingsStore>>,
  digit_count: u32,
}

impl Default for KeyAllocator {
  fn default() -> Self {
    let test_path = std::env::temp_dir().join(format!("imgstar-key-{}", Uuid::new_v4()));
    Self::new_with_path(9, &test_path).expect("key allocator should initialize")
  }
}

impl KeyAllocator {
  pub fn for_app(settings_store: Arc<SettingsStore>) -> Result<Self, String> {
    let path = resolve_app_data_dir().join("key_allocator");
    Self::new_with_path_and_store(settings_store, &path)
  }

  pub fn new_with_path(digit_count: u32, path: &Path) -> Result<Self, String> {
    Self::new_with_path_inner(None, digit_count, path)
  }

  pub fn new_with_path_and_store(
    settings_store: Arc<SettingsStore>,
    path: &Path,
  ) -> Result<Self, String> {
    Self::new_with_path_inner(Some(settings_store), 9, path)
  }

  fn new_with_path_inner(
    settings_store: Option<Arc<SettingsStore>>,
    digit_count: u32,
    path: &Path,
  ) -> Result<Self, String> {
    std::fs::create_dir_all(path).map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    let env = unsafe {
      EnvOpenOptions::new()
        .max_dbs(8)
        .map_size(32 * 1024 * 1024)
        .open(path)
        .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?
    };

    let mut wtxn = env
      .write_txn()
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
    let db: Database<Str, Bytes> = env
      .create_database(&mut wtxn, Some("key_allocator"))
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
    wtxn
      .commit()
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    Ok(Self {
      env,
      db,
      gate: Mutex::new(()),
      settings_store,
      digit_count: digit_count.max(1),
    })
  }

  pub fn allocate(&self, file_name: &str) -> Result<KeyAllocation, String> {
    let _guard = self
      .gate
      .lock()
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    let mut wtxn = self
      .env
      .write_txn()
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
    self.promote_expired_cooling(&mut wtxn)?;

    let ext = Self::extract_extension(file_name);
    let bucket_index: u64;

    let number = if let Some((reusable, reusable_index)) = self.read_reusable_number_rw(&wtxn)? {
      bucket_index = reusable_index / KV_BUCKET_SIZE;
      reusable
    } else {
      let next_number = self.read_next_number_rw(&wtxn)?;
      let digit_count = self.current_digit_count();
      bucket_index = next_number / KV_BUCKET_SIZE;
      self.write_next_number(&mut wtxn, next_number + 1)?;
      format!(
        "{:0width$}",
        next_number,
        width = digit_count as usize
      )
    };

    let object_key = format!("img/public/{number}.{ext}");

    self.write_state(&mut wtxn, number.as_str(), KeyState::Reserved)?;
    self.clear_cooling_until(&mut wtxn, number.as_str())?;
    self.write_object_key(&mut wtxn, number.as_str(), object_key.as_str())?;
    self.track_extension(&mut wtxn, ext.as_str())?;
    self.track_suffix_bucket(&mut wtxn, ext.as_str(), bucket_index)?;
    wtxn
      .commit()
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    Ok(KeyAllocation { number, object_key })
  }

  pub fn tracked_extensions(&self) -> Vec<String> {
    let Ok(_guard) = self.gate.lock() else {
      return Vec::new();
    };

    let Ok(rtxn) = self.env.read_txn() else {
      return Vec::new();
    };

    self.read_extensions_ro(&rtxn).ok().flatten().unwrap_or_default()
  }

  pub fn digit_count(&self) -> u32 {
    self.current_digit_count()
  }

  pub fn active_numbers(&self) -> Vec<String> {
    let Ok(_guard) = self.gate.lock() else {
      return Vec::new();
    };

    let Ok(rtxn) = self.env.read_txn() else {
      return Vec::new();
    };

    let mut numbers = Vec::new();

    if let Ok(iter) = self.db.iter(&rtxn) {
      for item in iter {
        let Ok((key, value)) = item else {
          continue;
        };

        let Some(number) = key.strip_prefix("state:") else {
          continue;
        };

        let Ok(text) = std::str::from_utf8(value) else {
          continue;
        };

        if KeyState::from_db(text).ok() == Some(KeyState::Active) {
          numbers.push(number.to_string());
        }
      }
    }

    numbers.sort();
    numbers
  }

  pub fn active_object_entries(&self) -> Vec<KvReadonlyObjectEntry> {
    let mut entries = Vec::new();
    for number in self.active_numbers() {
      if let Some(object_key) = self.object_key_for_number(number.as_str()) {
        entries.push(KvReadonlyObjectEntry { number, object_key });
      }
    }

    entries
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
        keys.push(key.to_string());
      }
    }

    for key in keys {
      let _ = self.db.delete(&mut wtxn, key.as_str());
    }

    let _ = wtxn.commit();
  }

  pub fn activate(&self, number: &str) -> bool {
    self.transition(number, KeyState::Reserved, KeyState::Active)
  }

  pub fn release_reserved(&self, number: &str) -> bool {
    self.transition(number, KeyState::Reserved, KeyState::Free)
  }

  pub fn mark_deleted(&self, number: &str) -> bool {
    self.transition(number, KeyState::Active, KeyState::Deleted)
  }

  pub fn release_active(&self, number: &str) -> bool {
    self.transition(number, KeyState::Active, KeyState::Free)
  }

  pub fn restore_active(&self, number: &str) -> bool {
    self.transition(number, KeyState::Deleted, KeyState::Active)
  }

  pub fn mark_cooling(&self, number: &str) -> bool {
    let Ok(_guard) = self.gate.lock() else {
      return false;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return false;
    };

    let Ok(current_state) = self.read_state_rw(&wtxn, number) else {
      return false;
    };

    if current_state.unwrap_or(KeyState::Free) != KeyState::Deleted {
      return false;
    }

    if self.write_state(&mut wtxn, number, KeyState::Cooling).is_err() {
      return false;
    }

    let now_ms = Self::timestamp_ms();
    let reuse_delay_ms = self.current_reuse_delay_ms();
    if reuse_delay_ms > 0 {
      if self
        .write_cooling_until(&mut wtxn, number, now_ms.saturating_add(reuse_delay_ms))
        .is_err()
      {
        return false;
      }
    } else if self.clear_cooling_until(&mut wtxn, number).is_err() {
      return false;
    }

    wtxn.commit().is_ok()
  }

  pub fn mark_free(&self, number: &str) -> bool {
    let Ok(_guard) = self.gate.lock() else {
      return false;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return false;
    };

    let Ok(current_state) = self.read_state_rw(&wtxn, number) else {
      return false;
    };

    if current_state.unwrap_or(KeyState::Free) != KeyState::Cooling {
      return false;
    }

    let now_ms = Self::timestamp_ms();
    if let Ok(Some(cooling_until_ms)) = self.read_cooling_until_rw(&wtxn, number) {
      if now_ms < cooling_until_ms {
        return false;
      }
    }

    if self.write_state(&mut wtxn, number, KeyState::Free).is_err() {
      return false;
    }

    if self.clear_cooling_until(&mut wtxn, number).is_err() {
      return false;
    }

    wtxn.commit().is_ok()
  }

  pub fn mark_free_immediately(&self, number: &str) -> bool {
    let Ok(_guard) = self.gate.lock() else {
      return false;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return false;
    };

    let Ok(current_state) = self.read_state_rw(&wtxn, number) else {
      return false;
    };

    if current_state.unwrap_or(KeyState::Free) != KeyState::Cooling {
      return false;
    }

    if self.write_state(&mut wtxn, number, KeyState::Free).is_err() {
      return false;
    }

    if self.clear_cooling_until(&mut wtxn, number).is_err() {
      return false;
    }

    wtxn.commit().is_ok()
  }

  pub fn state_of(&self, number: &str) -> KeyState {
    let Ok(rtxn) = self.env.read_txn() else {
      return KeyState::Free;
    };

    self
      .read_state_ro(&rtxn, number)
      .ok()
      .flatten()
      .unwrap_or(KeyState::Free)
  }

  fn transition(&self, number: &str, from: KeyState, to: KeyState) -> bool {
    let Ok(_guard) = self.gate.lock() else {
      return false;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return false;
    };

    let Ok(current_state) = self.read_state_rw(&wtxn, number) else {
      return false;
    };

    if current_state.unwrap_or(KeyState::Free) != from {
      return false;
    }

    if self.write_state(&mut wtxn, number, to).is_err() {
      return false;
    }

    wtxn.commit().is_ok()
  }

  fn next_number_key() -> &'static str {
    "meta:next_number"
  }

  fn current_digit_count(&self) -> u32 {
    self
      .settings_store
      .as_ref()
      .and_then(|store| store.load())
      .and_then(|settings| settings.digit_count)
      .unwrap_or(self.digit_count)
      .clamp(1, 20)
  }

  fn extensions_key() -> &'static str {
    "meta:extensions"
  }

  fn suffix_buckets_key() -> &'static str {
    "meta:suffix_buckets"
  }

  fn state_key(number: &str) -> String {
    format!("state:{number}")
  }

  fn cooling_until_key(number: &str) -> String {
    format!("cooling_until:{number}")
  }

  fn object_key_key(number: &str) -> String {
    format!("object_key:{number}")
  }

  #[allow(deprecated)]
  fn current_reuse_delay_ms(&self) -> u64 {
    0
  }

  fn timestamp_ms() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|duration| duration.as_millis() as u64)
      .unwrap_or(0)
  }

  fn read_reusable_number_rw(
    &self,
    txn: &heed::RwTxn<'_>,
  ) -> Result<Option<(String, u64)>, String> {
    let mut reusable: Option<(String, u64)> = None;

    let iter = self
      .db
      .iter(txn)
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    for item in iter {
      let (key, value) = item.map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
      let Some(number) = key.strip_prefix("state:") else {
        continue;
      };

      let state_text = std::str::from_utf8(value)
        .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
      if KeyState::from_db(state_text)? != KeyState::Free {
        continue;
      }

      let Ok(parsed_number) = number.parse::<u64>() else {
        continue;
      };

      let should_replace = reusable
        .as_ref()
        .map(|(_, candidate)| parsed_number < *candidate)
        .unwrap_or(true);

      if should_replace {
        reusable = Some((number.to_string(), parsed_number));
      }
    }

    Ok(reusable)
  }

  fn write_cooling_until(
    &self,
    txn: &mut heed::RwTxn<'_>,
    number: &str,
    cooling_until_ms: u64,
  ) -> Result<(), String> {
    let key = Self::cooling_until_key(number);
    let value = cooling_until_ms.to_string();
    self
      .db
      .put(txn, key.as_str(), value.as_bytes())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())
  }

  fn read_cooling_until_rw(
    &self,
    txn: &heed::RwTxn<'_>,
    number: &str,
  ) -> Result<Option<u64>, String> {
    let key = Self::cooling_until_key(number);
    let raw = self
      .db
      .get(txn, key.as_str())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    if let Some(bytes) = raw {
      let text = std::str::from_utf8(bytes).map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
      return text
        .parse::<u64>()
        .map(Some)
        .map_err(|_| "KEY_ALLOCATION_FAILED".to_string());
    }

    Ok(None)
  }

  fn clear_cooling_until(
    &self,
    txn: &mut heed::RwTxn<'_>,
    number: &str,
  ) -> Result<(), String> {
    let key = Self::cooling_until_key(number);
    self
      .db
      .delete(txn, key.as_str())
      .map(|_| ())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())
  }

  fn promote_expired_cooling(&self, txn: &mut heed::RwTxn<'_>) -> Result<(), String> {
    let now_ms = Self::timestamp_ms();
    let mut ready_numbers: Vec<String> = Vec::new();

    let iter = self
      .db
      .iter(txn)
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    for item in iter {
      let (key, value) = item.map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
      let Some(number) = key.strip_prefix("cooling_until:") else {
        continue;
      };

      let cooling_until_ms = std::str::from_utf8(value)
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(0);

      if cooling_until_ms <= now_ms {
        ready_numbers.push(number.to_string());
      }
    }

    for number in ready_numbers {
      let current_state = self.read_state_rw(txn, number.as_str())?;
      if current_state == Some(KeyState::Cooling) {
        self.write_state(txn, number.as_str(), KeyState::Free)?;
      }
      self.clear_cooling_until(txn, number.as_str())?;
    }

    Ok(())
  }

  fn read_next_number_rw(&self, txn: &heed::RwTxn<'_>) -> Result<u64, String> {
    let raw = self
      .db
      .get(txn, Self::next_number_key())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    if let Some(bytes) = raw {
      let text = std::str::from_utf8(bytes).map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
      text
        .parse::<u64>()
        .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())
    } else {
      Ok(0)
    }
  }

  fn write_next_number(
    &self,
    txn: &mut heed::RwTxn<'_>,
    next_number: u64,
  ) -> Result<(), String> {
    let value = next_number.to_string();
    self
      .db
      .put(txn, Self::next_number_key(), value.as_bytes())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())
  }

  fn read_state_rw(&self, txn: &heed::RwTxn<'_>, number: &str) -> Result<Option<KeyState>, String> {
    let key = Self::state_key(number);
    let raw = self
      .db
      .get(txn, key.as_str())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    raw
      .map(|bytes| {
        let text = std::str::from_utf8(bytes)
          .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
        KeyState::from_db(text)
      })
      .transpose()
  }

  fn read_state_ro(&self, txn: &heed::RoTxn<'_>, number: &str) -> Result<Option<KeyState>, String> {
    let key = Self::state_key(number);
    let raw = self
      .db
      .get(txn, key.as_str())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    raw
      .map(|bytes| {
        let text = std::str::from_utf8(bytes)
          .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
        KeyState::from_db(text)
      })
      .transpose()
  }

  fn write_state(
    &self,
    txn: &mut heed::RwTxn<'_>,
    number: &str,
    state: KeyState,
  ) -> Result<(), String> {
    let key = Self::state_key(number);
    let value = state.as_db();
    self
      .db
      .put(txn, key.as_str(), value.as_bytes())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())
  }

  fn write_object_key(
    &self,
    txn: &mut heed::RwTxn<'_>,
    number: &str,
    object_key: &str,
  ) -> Result<(), String> {
    let key = Self::object_key_key(number);
    self
      .db
      .put(txn, key.as_str(), object_key.as_bytes())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())
  }

  fn object_key_for_number_rw(
    &self,
    txn: &heed::RoTxn<'_>,
    number: &str,
  ) -> Option<String> {
    let key = Self::object_key_key(number);
    let raw = self.db.get(txn, key.as_str()).ok()??;
    std::str::from_utf8(raw).ok().map(|value| value.to_string())
  }

  pub fn object_key_for_number(&self, number: &str) -> Option<String> {
    let Ok(_guard) = self.gate.lock() else {
      return None;
    };

    let Ok(rtxn) = self.env.read_txn() else {
      return None;
    };

    self.object_key_for_number_rw(&rtxn, number)
  }

  fn track_extension(&self, txn: &mut heed::RwTxn<'_>, extension: &str) -> Result<(), String> {
    let mut extensions = self.read_extensions_rw(txn)?.unwrap_or_default();
    let normalized = Self::normalize_extension(extension);

    if !extensions.iter().any(|item| item == &normalized) {
      extensions.push(normalized);
      extensions.sort();
      let encoded = serde_json::to_vec(&extensions)
        .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
      self
        .db
        .put(txn, Self::extensions_key(), encoded.as_slice())
        .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
    }

    Ok(())
  }

  fn track_suffix_bucket(
    &self,
    txn: &mut heed::RwTxn<'_>,
    extension: &str,
    bucket_index: u64,
  ) -> Result<(), String> {
    let mut mapping = self.read_suffix_buckets_rw(txn)?.unwrap_or_default();
    let normalized = Self::normalize_extension(extension);
    let bucket_indexes = mapping.entry(normalized).or_default();

    if !bucket_indexes.iter().any(|value| *value == bucket_index) {
      bucket_indexes.push(bucket_index);
      bucket_indexes.sort();
    }

    let encoded = serde_json::to_vec(&mapping)
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
    self
      .db
      .put(txn, Self::suffix_buckets_key(), encoded.as_slice())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())
  }

  fn read_extensions_rw(&self, txn: &heed::RwTxn<'_>) -> Result<Option<Vec<String>>, String> {
    let raw = self
      .db
      .get(txn, Self::extensions_key())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    raw
      .map(|bytes| Self::decode_extensions(bytes))
      .transpose()
  }

  fn read_extensions_ro(&self, txn: &heed::RoTxn<'_>) -> Result<Option<Vec<String>>, String> {
    let raw = self
      .db
      .get(txn, Self::extensions_key())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    raw
      .map(|bytes| Self::decode_extensions(bytes))
      .transpose()
  }

  fn read_suffix_buckets_rw(
    &self,
    txn: &heed::RwTxn<'_>,
  ) -> Result<Option<BTreeMap<String, Vec<u64>>>, String> {
    let raw = self
      .db
      .get(txn, Self::suffix_buckets_key())
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;

    raw
      .map(|bytes| Self::decode_suffix_buckets(bytes))
      .transpose()
  }

  fn decode_extensions(bytes: &[u8]) -> Result<Vec<String>, String> {
    let decoded = serde_json::from_slice::<Vec<String>>(bytes)
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
    let mut normalized = BTreeSet::new();

    for value in decoded {
      normalized.insert(Self::normalize_extension(value.as_str()));
    }

    Ok(normalized.into_iter().collect())
  }

  fn decode_suffix_buckets(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u64>>, String> {
    let decoded = serde_json::from_slice::<BTreeMap<String, Vec<u64>>>(bytes)
      .map_err(|_| "KEY_ALLOCATION_FAILED".to_string())?;
    let mut normalized = BTreeMap::new();

    for (suffix, buckets) in decoded {
      let clean_suffix = Self::normalize_extension(suffix.as_str());
      let entry = normalized.entry(clean_suffix).or_insert_with(Vec::new);
      for bucket_index in buckets {
        if !entry.iter().any(|value| *value == bucket_index) {
          entry.push(bucket_index);
        }
      }
      entry.sort();
    }

    Ok(normalized)
  }

  fn extract_extension(file_name: &str) -> String {
    let candidate = file_name
      .rsplit_once('.')
      .map(|(_, suffix)| suffix)
      .unwrap_or("bin");
    Self::normalize_extension(candidate)
  }

  fn normalize_extension(value: &str) -> String {
    let cleaned = value
      .trim()
      .trim_start_matches('.')
      .chars()
      .filter(|ch| ch.is_ascii_alphanumeric())
      .collect::<String>()
      .to_ascii_lowercase();

    if cleaned.is_empty() {
      "bin".to_string()
    } else {
      cleaned
    }
  }

}

impl KeyState {
  fn as_db(self) -> &'static str {
    match self {
      KeyState::Free => "free",
      KeyState::Reserved => "reserved",
      KeyState::Active => "active",
      KeyState::Deleted => "deleted",
      KeyState::Cooling => "cooling",
    }
  }

  fn from_db(input: &str) -> Result<Self, String> {
    match input {
      "free" => Ok(KeyState::Free),
      "reserved" => Ok(KeyState::Reserved),
      "active" => Ok(KeyState::Active),
      "deleted" => Ok(KeyState::Deleted),
      "cooling" => Ok(KeyState::Cooling),
      _ => Err("KEY_ALLOCATION_FAILED".to_string()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{KeyAllocator, KeyState};
  use crate::contracts::SettingsDraft;
  use crate::storage::settings_store::SettingsStore;
  use std::sync::Arc;
  use uuid::Uuid;

  #[test]
  fn supports_full_state_cycle() {
    let allocator = KeyAllocator::default();
    let allocation = allocator
      .allocate("sample.png")
      .expect("allocation should succeed");

    assert_eq!(allocator.state_of(&allocation.number), KeyState::Reserved);
    assert!(allocator.activate(&allocation.number));
    assert_eq!(allocator.state_of(&allocation.number), KeyState::Active);
    assert!(allocator.mark_deleted(&allocation.number));
    assert_eq!(allocator.state_of(&allocation.number), KeyState::Deleted);
    assert!(allocator.mark_cooling(&allocation.number));
    assert_eq!(allocator.state_of(&allocation.number), KeyState::Cooling);
    assert!(allocator.mark_free(&allocation.number));
    assert_eq!(allocator.state_of(&allocation.number), KeyState::Free);
  }

  #[test]
  fn rejects_invalid_transition() {
    let allocator = KeyAllocator::default();
    let allocation = allocator
      .allocate("sample.png")
      .expect("allocation should succeed");

    assert!(!allocator.mark_deleted(&allocation.number));
    assert_eq!(allocator.state_of(&allocation.number), KeyState::Reserved);
  }

  #[test]
  fn supports_deleted_to_active_rollback() {
    let allocator = KeyAllocator::default();
    let allocation = allocator
      .allocate("sample.png")
      .expect("allocation should succeed");

    assert!(allocator.activate(&allocation.number));
    assert!(allocator.mark_deleted(&allocation.number));
    assert!(allocator.restore_active(&allocation.number));
    assert_eq!(allocator.state_of(&allocation.number), KeyState::Active);
  }

  #[test]
  fn persists_sequence_across_reopen() {
    let path = std::env::temp_dir().join(format!("imgstar-key-persist-{}", Uuid::new_v4()));
    let allocator = KeyAllocator::new_with_path(9, &path)
      .expect("allocator should initialize");
    let first = allocator.allocate("a.png").expect("first allocation should succeed");
    assert_eq!(first.number, "000000000");

    drop(allocator);

    let reopened = KeyAllocator::new_with_path(9, &path)
      .expect("reopened allocator should initialize");
    let second = reopened
      .allocate("b.png")
      .expect("second allocation should succeed");
    assert_eq!(second.number, "000000001");
  }

  #[test]
  fn tracks_extensions_in_sorted_order() {
    let allocator = KeyAllocator::default();
    let _ = allocator.allocate("sample.webp").expect("allocation should succeed");
    let _ = allocator.allocate("sample.PNG").expect("allocation should succeed");
    let _ = allocator.allocate("sample.jpeg").expect("allocation should succeed");
    let _ = allocator.allocate("sample").expect("allocation should succeed");

    assert_eq!(
      allocator.tracked_extensions(),
      vec![
        "bin".to_string(),
        "jpeg".to_string(),
        "png".to_string(),
        "webp".to_string(),
      ]
    );
  }

  #[test]
  fn persists_suffix_bucket_mapping_snapshot() {
    let path = std::env::temp_dir().join(format!("imgstar-key-map-{}", Uuid::new_v4()));
    let allocator = KeyAllocator::new_with_path(9, &path)
      .expect("allocator should initialize");
    let _ = allocator.allocate("first.png").expect("allocation should succeed");
    let _ = allocator.allocate("second.webp").expect("allocation should succeed");

    drop(allocator);

    let reopened = KeyAllocator::new_with_path(9, &path)
      .expect("reopened allocator should initialize");
    let entries = reopened.active_object_entries();

    assert!(entries.is_empty());
  }

  #[test]
  fn reuses_lowest_free_number_before_incrementing_sequence() {
    let allocator = KeyAllocator::default();

    let first = allocator
      .allocate("first.png")
      .expect("first allocation should succeed");
    assert!(allocator.release_reserved(first.number.as_str()));

    let second = allocator
      .allocate("second.jpg")
      .expect("second allocation should succeed");

    assert_eq!(second.number, first.number);
    assert_eq!(second.object_key, format!("img/public/{}.jpg", first.number));
  }

  #[test]
  #[allow(deprecated)]
  fn releases_recycled_number_immediately_after_cooling_transition() {
    let path = std::env::temp_dir().join(format!("imgstar-key-cooling-{}", Uuid::new_v4()));
    let settings_store = Arc::new(SettingsStore::default());
    settings_store.save(SettingsDraft {
      access_key: "ak".to_string(),
      secret_key: "sk".to_string(),
      endpoint: "https://example.r2.dev".to_string(),
      bucket: "demo".to_string(),
      zone_id: None,
      zone_api_token: None,
      cdn_base_url: None,
      region: Some("auto".to_string()),
      key_pattern: None,
      digit_count: Some(9),
      reuse_delay_ms: None,
      preview_hash_enabled: Some(true),
      theme: Some("system".to_string()),
      language: Some("zh-CN".to_string()),
    });

    let allocator = KeyAllocator::new_with_path_and_store(settings_store, &path)
      .expect("allocator should initialize");
    let allocation = allocator
      .allocate("sample.png")
      .expect("allocation should succeed");

    assert!(allocator.activate(allocation.number.as_str()));
    assert!(allocator.mark_deleted(allocation.number.as_str()));
    assert!(allocator.mark_cooling(allocation.number.as_str()));
    assert_eq!(allocator.state_of(allocation.number.as_str()), KeyState::Cooling);
    assert!(allocator.mark_free_immediately(allocation.number.as_str()));
    assert_eq!(allocator.state_of(allocation.number.as_str()), KeyState::Free);
  }
}
