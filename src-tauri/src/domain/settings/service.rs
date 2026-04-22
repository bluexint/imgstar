use crate::contracts::{ConnectionPingResult, SaveSettingsResult, SettingsDraft, SettingsSnapshot};
use crate::domain::settings::normalizer::SettingsNormalizer;
use crate::domain::settings::ping_adapter::SettingsPingAdapter;
use crate::domain::settings::validator::SettingsValidator;
use crate::storage::settings_store::SettingsStore;
use chrono::{SecondsFormat, Utc};
use std::sync::Arc;

#[derive(Clone)]
pub struct SettingsService {
  store: Arc<SettingsStore>,
  normalizer: SettingsNormalizer,
  validator: SettingsValidator,
  ping_adapter: SettingsPingAdapter,
}

impl SettingsService {
  pub fn new(store: Arc<SettingsStore>) -> Self {
    Self::with_components(
      store,
      SettingsNormalizer,
      SettingsValidator,
      SettingsPingAdapter::new(),
    )
  }

  fn with_components(
    store: Arc<SettingsStore>,
    normalizer: SettingsNormalizer,
    validator: SettingsValidator,
    ping_adapter: SettingsPingAdapter,
  ) -> Self {
    Self {
      store,
      normalizer,
      validator,
      ping_adapter,
    }
  }

  pub fn save(&self, payload: SettingsDraft) -> Result<SaveSettingsResult, String> {
    let normalized = self.normalizer.normalize_for_save(payload);
    self.validator.validate_save(&normalized)?;

    self.store.save(normalized);

    Ok(SaveSettingsResult {
      saved_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    })
  }

  pub fn get(&self) -> SettingsDraft {
    self
      .store
      .load()
      .map(|payload| self.normalizer.normalize_for_read(payload))
      .unwrap_or_else(|| self.normalizer.default_draft())
  }

  pub fn snapshot(&self) -> SettingsSnapshot {
    let draft = self.get();
    let configured = self.validator.is_configured(&draft);

    SettingsSnapshot { draft, configured }
  }

  pub fn reset_app(&self) -> SettingsSnapshot {
    self.store.clear();
    self.snapshot()
  }

  pub async fn ping(&self) -> Result<ConnectionPingResult, String> {
    let Some(settings) = self.store.load() else {
      return Err("INVALID_CONFIG".to_string());
    };

    self.validator.validate_ping(&settings)?;
    self.ping_adapter.ping_storage(&settings).await
  }
}
