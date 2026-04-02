use crate::contracts::{
  UploadEvent,
  UploadEventFilter,
  UploadEventLevel,
  UploadEventModule,
  UploadEventStatus,
};
use crate::runtime::event_bus::EventBus;
use crate::storage::log_store::LogStore;
use chrono::{SecondsFormat, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug)]
pub struct LogRecord {
  pub trace_id: String,
  pub module: UploadEventModule,
  pub event_name: String,
  pub level: UploadEventLevel,
  pub status: UploadEventStatus,
  pub error_code: Option<String>,
  pub error_message: Option<String>,
  pub stack: Option<String>,
  pub duration: u64,
  pub context: HashMap<String, Value>,
}

impl LogRecord {
  pub fn new(
    trace_id: impl Into<String>,
    module: UploadEventModule,
    event_name: impl Into<String>,
    level: UploadEventLevel,
    status: UploadEventStatus,
    duration: u64,
    context: HashMap<String, Value>,
  ) -> Self {
    Self {
      trace_id: trace_id.into(),
      module,
      event_name: event_name.into(),
      level,
      status,
      error_code: None,
      error_message: None,
      stack: None,
      duration,
      context,
    }
  }

  pub fn with_error(
    mut self,
    error_code: impl Into<String>,
    error_message: impl Into<String>,
  ) -> Self {
    self.error_code = Some(error_code.into());
    self.error_message = Some(error_message.into());
    self
  }

  pub fn with_stack(mut self, stack: impl Into<String>) -> Self {
    self.stack = Some(stack.into());
    self
  }
}

#[derive(Clone)]
pub struct LogCenter {
  event_bus: EventBus,
  log_store: Arc<LogStore>,
}

impl LogCenter {
  pub fn new(event_bus: EventBus, log_store: Arc<LogStore>) -> Self {
    Self {
      event_bus,
      log_store,
    }
  }

  pub fn new_trace_id(&self) -> String {
    Uuid::new_v4().to_string()
  }

  pub fn emit(&self, record: LogRecord) {
    let event = UploadEvent {
      trace_id: record.trace_id,
      timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
      module: record.module,
      event_name: record.event_name,
      level: record.level,
      status: record.status,
      error_code: record.error_code,
      error_message: record.error_message,
      stack: record.stack,
      duration: record.duration,
      context: record.context,
    };
    self.event_bus.emit(event);
  }

  pub fn list(&self, filter: UploadEventFilter) -> Vec<UploadEvent> {
    self.log_store.list(&filter)
  }

  pub fn clear(&self) {
    self.log_store.clear();
  }
}
