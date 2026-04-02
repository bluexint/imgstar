use crate::contracts::SettingsDraft;
use crate::storage::resolve_app_data_dir;
use heed::types::{Bytes, Str};
use heed::{Database, Env, EnvOpenOptions};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

const CURRENT_SETTINGS_KEY: &str = "current";
const CACHED_WAF_RULESET_IDS_KEY: &str = "cloudflare_waf_ruleset_ids";

#[derive(Debug)]
pub struct SettingsStore {
  env: Env,
  db: Database<Str, Bytes>,
  gate: Mutex<()>,
}

impl Default for SettingsStore {
  fn default() -> Self {
    let test_path = std::env::temp_dir().join(format!("imgstar-settings-{}", Uuid::new_v4()));
    Self::new_with_path(&test_path).expect("settings store should initialize")
  }
}

impl SettingsStore {
  pub fn for_app() -> Result<Self, String> {
    let path = resolve_app_data_dir().join("settings");
    Self::new_with_path(&path)
  }

  pub fn new_with_path(path: &Path) -> Result<Self, String> {
    std::fs::create_dir_all(path).map_err(|_| "INTERNAL_ERROR".to_string())?;

    let env = unsafe {
      EnvOpenOptions::new()
        .max_dbs(8)
        .map_size(16 * 1024 * 1024)
        .open(path)
        .map_err(|_| "INTERNAL_ERROR".to_string())?
    };

    let mut wtxn = env.write_txn().map_err(|_| "INTERNAL_ERROR".to_string())?;
    let db: Database<Str, Bytes> = env
      .create_database(&mut wtxn, Some("settings_store"))
      .map_err(|_| "INTERNAL_ERROR".to_string())?;
    wtxn.commit().map_err(|_| "INTERNAL_ERROR".to_string())?;

    Ok(Self {
      env,
      db,
      gate: Mutex::new(()),
    })
  }

  pub fn save(&self, settings: SettingsDraft) {
    let Ok(_guard) = self.gate.lock() else {
      return;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return;
    };
    let Ok(encoded) = serde_json::to_vec(&settings) else {
      return;
    };

    let _ = self.db.put(&mut wtxn, CURRENT_SETTINGS_KEY, encoded.as_slice());
    let _ = wtxn.commit();
  }

  pub fn load(&self) -> Option<SettingsDraft> {
    let rtxn = self.env.read_txn().ok()?;
    let bytes = self.db.get(&rtxn, CURRENT_SETTINGS_KEY).ok()??;
    serde_json::from_slice(bytes).ok()
  }

  pub fn save_cached_waf_ruleset_id(&self, zone_id: &str, ruleset_id: Option<&str>) {
    let Ok(_guard) = self.gate.lock() else {
      return;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return;
    };

    let zone_id = zone_id.trim();
    if zone_id.is_empty() {
      return;
    }

    let mut cached_ruleset_ids = self.load_cached_waf_ruleset_ids().unwrap_or_default();
    if ruleset_id
      .map(str::trim)
      .filter(|value| !value.is_empty())
      .is_some()
    {
      if let Some(value) = ruleset_id.map(str::trim).filter(|value| !value.is_empty()) {
        cached_ruleset_ids.insert(zone_id.to_string(), value.to_string());
      }
    } else {
      cached_ruleset_ids.remove(zone_id);
    }

    let Ok(encoded) = serde_json::to_vec(&cached_ruleset_ids) else {
      return;
    };

    let _ = self.db.put(&mut wtxn, CACHED_WAF_RULESET_IDS_KEY, encoded.as_slice());

    let _ = wtxn.commit();
  }

  pub fn load_cached_waf_ruleset_id(&self, zone_id: &str) -> Option<String> {
    let zone_id = zone_id.trim();
    if zone_id.is_empty() {
      return None;
    }

    self
      .load_cached_waf_ruleset_ids()
      .and_then(|cached_ruleset_ids| cached_ruleset_ids.get(zone_id).cloned())
  }

  fn load_cached_waf_ruleset_ids(&self) -> Option<std::collections::HashMap<String, String>> {
    let rtxn = self.env.read_txn().ok()?;
    let bytes = self.db.get(&rtxn, CACHED_WAF_RULESET_IDS_KEY).ok()??;
    serde_json::from_slice(bytes).ok()
  }

  pub fn clear(&self) {
    let Ok(_guard) = self.gate.lock() else {
      return;
    };
    let Ok(mut wtxn) = self.env.write_txn() else {
      return;
    };

    let _ = self.db.delete(&mut wtxn, CURRENT_SETTINGS_KEY);
    let _ = self.db.delete(&mut wtxn, CACHED_WAF_RULESET_IDS_KEY);
    let _ = wtxn.commit();
  }
}

#[cfg(test)]
mod tests {
  use super::SettingsStore;
  use crate::contracts::SettingsDraft;
  use uuid::Uuid;

  #[test]
  fn persists_saved_settings_across_reopen() {
    let path = std::env::temp_dir().join(format!("imgstar-settings-persist-{}", Uuid::new_v4()));
    let store = SettingsStore::new_with_path(&path).expect("store should initialize");
    store.save(SettingsDraft {
      access_key: "ak".to_string(),
      secret_key: "sk".to_string(),
      endpoint: "https://example.r2.dev".to_string(),
      bucket: "demo".to_string(),
      zone_id: Some("zone-1".to_string()),
      zone_api_token: Some("token-1".to_string()),
      cdn_base_url: Some("https://cdn.example.com".to_string()),
      region: Some("auto".to_string()),
      key_pattern: None,
      digit_count: Some(9),
      reuse_delay_ms: Some(900_000),
      preview_hash_enabled: Some(true),
      theme: Some("system".to_string()),
      language: Some("zh-CN".to_string()),
    });

    drop(store);

    let reopened = SettingsStore::new_with_path(&path).expect("store should reopen");
    let loaded = reopened.load().expect("settings should persist");
    assert_eq!(loaded.access_key, "ak");
    assert_eq!(loaded.bucket, "demo");
  }

  #[test]
  fn clears_saved_settings() {
    let path = std::env::temp_dir().join(format!("imgstar-settings-clear-{}", Uuid::new_v4()));
    let store = SettingsStore::new_with_path(&path).expect("store should initialize");
    store.save(SettingsDraft {
      access_key: "ak".to_string(),
      secret_key: "sk".to_string(),
      endpoint: "https://example.r2.dev".to_string(),
      bucket: "demo".to_string(),
      zone_id: Some("zone-1".to_string()),
      zone_api_token: Some("token-1".to_string()),
      cdn_base_url: Some("https://cdn.example.com".to_string()),
      region: Some("auto".to_string()),
      key_pattern: None,
      digit_count: Some(9),
      reuse_delay_ms: Some(900_000),
      preview_hash_enabled: Some(true),
      theme: Some("system".to_string()),
      language: Some("zh-CN".to_string()),
    });

    store.clear();

    assert!(store.load().is_none());
    assert!(store.load_cached_waf_ruleset_id("account-1").is_none());
  }

  #[test]
  fn persists_cached_waf_ruleset_id_across_reopen() {
    let path = std::env::temp_dir().join(format!("imgstar-settings-waf-{}", Uuid::new_v4()));
    let store = SettingsStore::new_with_path(&path).expect("store should initialize");
    store.save_cached_waf_ruleset_id("zone-1", Some("ruleset-123"));

    drop(store);

    let reopened = SettingsStore::new_with_path(&path).expect("store should reopen");
    assert_eq!(reopened.load_cached_waf_ruleset_id("zone-1").as_deref(), Some("ruleset-123"));
  }
}
