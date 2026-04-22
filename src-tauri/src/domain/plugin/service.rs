use crate::contracts::{
  PluginVerificationResult,
  UploadEventLevel,
  UploadEventModule,
  UploadEventStatus,
};
use crate::domain::logging::center::{LogCenter, LogRecord};
use chrono::{SecondsFormat, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const TRUSTED_SIGNER: &str = "imgstar-source-bound";
const MAX_PLUGIN_ID_LEN: usize = 128;
const MAX_SIGNER_SOURCE_LEN: usize = 128;
const MAX_SIGNER_SOURCE_BINDINGS: usize = 1024;
const OFFICIAL_PLUGIN_SIGNER_SOURCES: [(&str, &str); 2] = [
  ("image-compress", "imgstar-official"),
  ("hidden-watermark", "imgstar-official"),
];
const REVOKED_PLUGIN_IDS: [&str; 1] = ["hidden-watermark-revoked"];

#[derive(Debug)]
enum SignerBindingRegistryError {
  SignerSourceMismatch { bound_source: String },
  CapacityExceeded,
}

#[derive(Debug)]
struct SignerBindingRegistry {
  capacity: usize,
  bindings: HashMap<String, String>,
}

impl SignerBindingRegistry {
  fn new(capacity: usize) -> Self {
    Self {
      capacity,
      bindings: HashMap::new(),
    }
  }

  fn verify_or_bind(
    &mut self,
    plugin_id: &str,
    signer_source: &str,
  ) -> Result<(), SignerBindingRegistryError> {
    if let Some(bound_source) = self.bindings.get(plugin_id) {
      if bound_source != signer_source {
        return Err(SignerBindingRegistryError::SignerSourceMismatch {
          bound_source: bound_source.clone(),
        });
      }

      return Ok(());
    }

    if self.bindings.len() >= self.capacity {
      return Err(SignerBindingRegistryError::CapacityExceeded);
    }

    self.bindings
      .insert(plugin_id.to_string(), signer_source.to_string());
    Ok(())
  }
}

fn expected_signer_source(plugin_id: &str) -> Option<&'static str> {
  OFFICIAL_PLUGIN_SIGNER_SOURCES
    .iter()
    .find(|(candidate, _)| candidate == &plugin_id)
    .map(|(_, source)| *source)
}

fn is_revoked_plugin(plugin_id: &str) -> bool {
  REVOKED_PLUGIN_IDS
    .iter()
    .any(|candidate| candidate == &plugin_id)
}

#[derive(Clone)]
pub struct PluginService {
  log_center: LogCenter,
  signer_source_bindings: Arc<Mutex<SignerBindingRegistry>>,
}

impl PluginService {
  pub fn new(log_center: LogCenter) -> Self {
    Self::with_registry_capacity(log_center, MAX_SIGNER_SOURCE_BINDINGS)
  }

  fn with_registry_capacity(log_center: LogCenter, capacity: usize) -> Self {
    Self {
      log_center,
      signer_source_bindings: Arc::new(Mutex::new(SignerBindingRegistry::new(capacity))),
    }
  }

  pub fn verify(&self, plugin_id: String, signer_source: Option<String>) -> PluginVerificationResult {
    let trace_id = self.log_center.new_trace_id();
    let Some(normalized) = normalize_bounded_text(plugin_id.as_str(), MAX_PLUGIN_ID_LEN) else {
      self.log_center.emit(LogRecord::new(
        trace_id,
        UploadEventModule::Plugin,
        "plugin:signature_rejected",
        UploadEventLevel::Warn,
        UploadEventStatus::Failed,
        6,
        context_from_pairs(vec![
          ("pluginIdLength", json!(plugin_id.trim().len())),
          ("reason", json!("invalid_plugin_id")),
        ]),
      ));

      return failed_verification(None, "SIGNATURE_VERIFY_FAILED");
    };

    let normalized_signer_source = signer_source
      .as_deref()
      .map(str::trim)
      .filter(|value| !value.is_empty())
      .map(ToString::to_string)
      .or_else(|| expected_signer_source(normalized.as_str()).map(ToString::to_string));

    if is_revoked_plugin(normalized.as_str()) {
      self.log_center.emit(LogRecord::new(
        trace_id,
        UploadEventModule::Plugin,
        "plugin:signature_revoked",
        UploadEventLevel::Warn,
        UploadEventStatus::Failed,
        6,
        context_from_pairs(vec![
          ("pluginId", json!(normalized)),
          (
            "revokedAt",
            json!(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
          ),
        ]),
      ));

      return failed_verification(normalized_signer_source, "SIGNATURE_VERIFY_FAILED");
    }

    let Some(signer_source) = normalized_signer_source else {
      self.log_center.emit(LogRecord::new(
        trace_id,
        UploadEventModule::Plugin,
        "plugin:signature_rejected",
        UploadEventLevel::Warn,
        UploadEventStatus::Failed,
        6,
        context_from_pairs(vec![
          ("pluginId", json!(normalized)),
          ("reason", json!("missing_signer_source")),
        ]),
      ));

      return failed_verification(None, "SIGNATURE_VERIFY_FAILED");
    };

    let Some(signer_source) = normalize_bounded_text(signer_source.as_str(), MAX_SIGNER_SOURCE_LEN) else {
      self.log_center.emit(LogRecord::new(
        trace_id,
        UploadEventModule::Plugin,
        "plugin:signature_rejected",
        UploadEventLevel::Warn,
        UploadEventStatus::Failed,
        6,
        context_from_pairs(vec![
          ("pluginId", json!(normalized)),
          ("reason", json!("invalid_signer_source")),
        ]),
      ));

      return failed_verification(None, "SIGNATURE_VERIFY_FAILED");
    };

    if let Some(expected_source) = expected_signer_source(normalized.as_str()) {
      if signer_source != expected_source {
        self.log_center.emit(LogRecord::new(
          trace_id,
          UploadEventModule::Plugin,
          "plugin:signature_rejected",
          UploadEventLevel::Warn,
          UploadEventStatus::Failed,
          6,
          context_from_pairs(vec![
            ("pluginId", json!(normalized)),
            ("reason", json!("untrusted_signer_source")),
            ("signerSource", json!(signer_source.clone())),
            ("expectedSignerSource", json!(expected_source)),
          ]),
        ));

        return failed_verification(Some(signer_source), "SIGNATURE_VERIFY_FAILED");
      }
    }

    let Ok(mut bindings) = self.signer_source_bindings.lock() else {
      return failed_verification(Some(signer_source), "INTERNAL_ERROR");
    };

    match bindings.verify_or_bind(normalized.as_str(), signer_source.as_str()) {
      Ok(()) => {}
      Err(SignerBindingRegistryError::SignerSourceMismatch { bound_source }) => {
        self.log_center.emit(LogRecord::new(
          trace_id,
          UploadEventModule::Plugin,
          "plugin:signature_rejected",
          UploadEventLevel::Warn,
          UploadEventStatus::Failed,
          6,
          context_from_pairs(vec![
            ("pluginId", json!(normalized)),
            ("reason", json!("signer_source_mismatch")),
            ("signerSource", json!(signer_source.clone())),
            ("boundSignerSource", json!(bound_source)),
          ]),
        ));

        return failed_verification(Some(signer_source), "SIGNATURE_VERIFY_FAILED");
      }
      Err(SignerBindingRegistryError::CapacityExceeded) => {
        self.log_center.emit(LogRecord::new(
          trace_id,
          UploadEventModule::Plugin,
          "plugin:signature_rejected",
          UploadEventLevel::Warn,
          UploadEventStatus::Failed,
          6,
          context_from_pairs(vec![
            ("pluginId", json!(normalized)),
            ("reason", json!("binding_capacity_exceeded")),
          ]),
        ));

        return failed_verification(Some(signer_source), "SIGNATURE_VERIFY_FAILED");
      }
    }

    self.log_center.emit(LogRecord::new(
      trace_id,
      UploadEventModule::Plugin,
      "plugin:signature_verified",
      UploadEventLevel::Info,
      UploadEventStatus::Success,
      6,
      context_from_pairs(vec![
        ("pluginId", json!(normalized)),
        ("signer", json!(TRUSTED_SIGNER)),
        ("signerSource", json!(signer_source.clone())),
        ("expiresAt", json!("2099-01-01T00:00:00.000Z")),
      ]),
    ));

    verified_verification(signer_source)
  }
}

fn normalize_bounded_text(value: &str, max_len: usize) -> Option<String> {
  let normalized = value.trim();
  if normalized.is_empty()
    || normalized.len() > max_len
    || normalized.chars().any(char::is_control)
  {
    return None;
  }

  Some(normalized.to_string())
}

fn failed_verification(
  signer_source: Option<String>,
  reason: &str,
) -> PluginVerificationResult {
  PluginVerificationResult {
    verified: false,
    reason: Some(reason.to_string()),
    signature_algorithm: Some("source_binding".to_string()),
    signer: Some(TRUSTED_SIGNER.to_string()),
    signer_source,
  }
}

fn verified_verification(signer_source: String) -> PluginVerificationResult {
  PluginVerificationResult {
    verified: true,
    reason: None,
    signature_algorithm: Some("source_binding".to_string()),
    signer: Some(TRUSTED_SIGNER.to_string()),
    signer_source: Some(signer_source),
  }
}

fn context_from_pairs(entries: Vec<(&str, serde_json::Value)>) -> HashMap<String, serde_json::Value> {
  let mut context = HashMap::new();
  for (key, value) in entries {
    context.insert(key.to_string(), value);
  }
  context
}

#[cfg(test)]
mod tests {
  use super::{PluginService, MAX_PLUGIN_ID_LEN};
  use crate::contracts::UploadEventFilter;
  use crate::domain::logging::center::LogCenter;
  use crate::runtime::event_bus::EventBus;
  use crate::storage::log_store::LogStore;
  use std::sync::Arc;

  fn build_service() -> (PluginService, LogCenter) {
    let log_store = Arc::new(LogStore::default());
    let event_bus = EventBus::new(log_store.clone());
    let log_center = LogCenter::new(event_bus, log_store);
    (PluginService::new(log_center.clone()), log_center)
  }

  fn build_service_with_capacity(capacity: usize) -> (PluginService, LogCenter) {
    let log_store = Arc::new(LogStore::default());
    let event_bus = EventBus::new(log_store.clone());
    let log_center = LogCenter::new(event_bus, log_store);
    (
      PluginService::with_registry_capacity(log_center.clone(), capacity),
      log_center,
    )
  }

  #[test]
  fn accepts_official_plugin_with_expected_signer_source() {
    let (service, _) = build_service();

    let result = service.verify(
      "image-compress".to_string(),
      Some("imgstar-official".to_string()),
    );

    assert!(result.verified);
    assert_eq!(result.reason, None);
    assert_eq!(result.signer_source, Some("imgstar-official".to_string()));
  }

  #[test]
  fn rejects_official_plugin_with_untrusted_signer_source() {
    let (service, log_center) = build_service();

    let result = service.verify(
      "image-compress".to_string(),
      Some("third-party".to_string()),
    );

    assert!(!result.verified);
    assert_eq!(result.reason, Some("SIGNATURE_VERIFY_FAILED".to_string()));

    let events = log_center.list(UploadEventFilter::default());
    assert!(events.iter().any(|event| {
      event.event_name == "plugin:signature_rejected"
        && event.context.get("reason") == Some(&serde_json::json!("untrusted_signer_source"))
    }));
  }

  #[test]
  fn rejects_signer_source_change_for_same_plugin_id() {
    let (service, log_center) = build_service();

    let first = service.verify("community-plugin".to_string(), Some("source-a".to_string()));
    assert!(first.verified);

    let second = service.verify("community-plugin".to_string(), Some("source-b".to_string()));
    assert!(!second.verified);
    assert_eq!(second.reason, Some("SIGNATURE_VERIFY_FAILED".to_string()));

    let events = log_center.list(UploadEventFilter::default());
    assert!(events.iter().any(|event| {
      event.event_name == "plugin:signature_rejected"
        && event.context.get("reason") == Some(&serde_json::json!("signer_source_mismatch"))
    }));
  }

  #[test]
  fn rejects_overlong_plugin_id() {
    let (service, log_center) = build_service();

    let result = service.verify("a".repeat(MAX_PLUGIN_ID_LEN + 1), Some("source-a".to_string()));

    assert!(!result.verified);
    assert_eq!(result.reason, Some("SIGNATURE_VERIFY_FAILED".to_string()));

    let events = log_center.list(UploadEventFilter::default());
    assert!(events.iter().any(|event| {
      event.event_name == "plugin:signature_rejected"
        && event.context.get("reason") == Some(&serde_json::json!("invalid_plugin_id"))
    }));
  }

  #[test]
  fn rejects_signer_source_with_control_characters() {
    let (service, log_center) = build_service();

    let result = service.verify(
      "community-plugin".to_string(),
      Some("source-a\nsource-b".to_string()),
    );

    assert!(!result.verified);
    assert_eq!(result.reason, Some("SIGNATURE_VERIFY_FAILED".to_string()));

    let events = log_center.list(UploadEventFilter::default());
    assert!(events.iter().any(|event| {
      event.event_name == "plugin:signature_rejected"
        && event.context.get("reason") == Some(&serde_json::json!("invalid_signer_source"))
    }));
  }

  #[test]
  fn rejects_new_binding_when_registry_capacity_is_exhausted() {
    let (service, log_center) = build_service_with_capacity(1);

    let first = service.verify("community-plugin-a".to_string(), Some("source-a".to_string()));
    assert!(first.verified);

    let second = service.verify("community-plugin-b".to_string(), Some("source-b".to_string()));

    assert!(!second.verified);
    assert_eq!(second.reason, Some("SIGNATURE_VERIFY_FAILED".to_string()));

    let events = log_center.list(UploadEventFilter::default());
    assert!(events.iter().any(|event| {
      event.event_name == "plugin:signature_rejected"
        && event.context.get("reason") == Some(&serde_json::json!("binding_capacity_exceeded"))
    }));
  }
}
