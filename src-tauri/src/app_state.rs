use crate::domain::logging::center::LogCenter;
use crate::domain::plugin::service::PluginService;
use crate::domain::preview::service::PreviewService;
use crate::domain::settings::service::SettingsService;
use crate::domain::upload::orchestrator::UploadOrchestrator;
use crate::runtime::adapter_runtime::AdapterRuntime;
use crate::runtime::event_bus::EventBus;
use crate::runtime::plugin_runtime::PluginRuntime;
use crate::storage::key_allocator::KeyAllocator;
use crate::storage::log_store::LogStore;
use crate::storage::upload_queue_store::UploadQueueStore;
use crate::storage::settings_store::SettingsStore;
use std::sync::Arc;

pub struct AppState {
  pub key_allocator: Arc<KeyAllocator>,
  pub upload_queue_store: Arc<UploadQueueStore>,
  pub upload_orchestrator: UploadOrchestrator,
  pub preview_service: PreviewService,
  pub plugin_service: PluginService,
  pub settings_service: SettingsService,
  pub log_center: LogCenter,
}

impl AppState {
  pub fn new() -> Self {
    let log_store = Arc::new(LogStore::for_app().unwrap_or_else(|_| LogStore::default()));
    let event_bus = EventBus::new(log_store.clone());
    let log_center = LogCenter::new(event_bus, log_store);

    let settings_store = Arc::new(
      SettingsStore::for_app().unwrap_or_else(|_| SettingsStore::default())
    );

    let key_allocator = Arc::new(
      KeyAllocator::for_app(settings_store.clone()).unwrap_or_else(|_| KeyAllocator::default())
    );
    let upload_queue_store = Arc::new(
      UploadQueueStore::for_app().unwrap_or_else(|_| UploadQueueStore::default())
    );
    let adapter_runtime = AdapterRuntime::new(settings_store.clone());

    let upload_orchestrator = UploadOrchestrator::new(
      key_allocator.clone(),
      adapter_runtime,
      PluginRuntime,
      log_center.clone(),
    );

    Self {
      key_allocator,
      upload_queue_store,
      upload_orchestrator,
      preview_service: PreviewService::new(settings_store.clone()),
      plugin_service: PluginService::new(log_center.clone()),
      settings_service: SettingsService::new(settings_store),
      log_center,
    }
  }

  pub fn reset_app(&self) -> crate::contracts::SettingsSnapshot {
    let snapshot = self.settings_service.reset_app();
    self.key_allocator.clear();
    let _ = self.upload_queue_store.clear();
    self.log_center.clear();
    snapshot
  }
}

impl Default for AppState {
  fn default() -> Self {
    Self::new()
  }
}
