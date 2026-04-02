use crate::contracts::UploadEvent;
use crate::storage::log_store::LogStore;
use std::sync::Arc;

#[derive(Clone)]
pub struct EventBus {
  log_store: Arc<LogStore>,
}

impl EventBus {
  pub fn new(log_store: Arc<LogStore>) -> Self {
    Self { log_store }
  }

  pub fn emit(&self, event: UploadEvent) {
    self.log_store.append(event);
  }
}
