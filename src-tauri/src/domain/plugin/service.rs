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
const OFFICIAL_PLUGIN_SIGNER_SOURCES: [(&str, &str); 2] = [
  ("image-compress", "imgstar-official"),
  ("hidden-watermark", "imgstar-official"),
];
const REVOKED_PLUGIN_IDS: [&str; 1] = ["hidden-watermark-revoked"];

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
  signer_source_bindings: Arc<Mutex<HashMap<String, String>>>,
}

impl PluginService {
  pub fn new(log_center: LogCenter) -> Self {
    Self {
      log_center,
      signer_source_bindings: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  pub fn verify(&self, plugin_id: String, signer_source: Option<String>) -> PluginVerificationResult {
    let trace_id = self.log_center.new_trace_id();
    let normalized = plugin_id.trim().to_string();
    let normalized_signer_source = signer_source
      .as_deref()
      .map(str::trim)
      .filter(|value| !value.is_empty())
      .map(ToString::to_string)
      .or_else(|| expected_signer_source(normalized.as_str()).map(ToString::to_string));

    if normalized.is_empty() {
      self.log_center.emit(LogRecord::new(
        trace_id,
        UploadEventModule::Plugin,
        "plugin:signature_rejected",
        UploadEventLevel::Warn,
        UploadEventStatus::Failed,
        6,
        context_from_pairs(vec![
          ("pluginId", json!(normalized)),
          ("reason", json!("empty_plugin_id")),
        ]),
      ));

      return PluginVerificationResult {
        verified: false,
        reason: Some("SIGNATURE_VERIFY_FAILED".to_string()),
        signature_algorithm: Some("source_binding".to_string()),
        signer: Some(TRUSTED_SIGNER.to_string()),
        signer_source: None,
      };
    }

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

      return PluginVerificationResult {
        verified: false,
        reason: Some("SIGNATURE_VERIFY_FAILED".to_string()),
        signature_algorithm: Some("source_binding".to_string()),
        signer: Some(TRUSTED_SIGNER.to_string()),
        signer_source: normalized_signer_source,
      };
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

      return PluginVerificationResult {
        verified: false,
        reason: Some("SIGNATURE_VERIFY_FAILED".to_string()),
        signature_algorithm: Some("source_binding".to_string()),
        signer: Some(TRUSTED_SIGNER.to_string()),
        signer_source: None,
      };
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

        return PluginVerificationResult {
          verified: false,
          reason: Some("SIGNATURE_VERIFY_FAILED".to_string()),
          signature_algorithm: Some("source_binding".to_string()),
          signer: Some(TRUSTED_SIGNER.to_string()),
          signer_source: Some(signer_source),
        };
      }
    }

    let Ok(mut bindings) = self.signer_source_bindings.lock() else {
      return PluginVerificationResult {
        verified: false,
        reason: Some("INTERNAL_ERROR".to_string()),
        signature_algorithm: Some("source_binding".to_string()),
        signer: Some(TRUSTED_SIGNER.to_string()),
        signer_source: Some(signer_source),
      };
    };

    if let Some(bound_source) = bindings.get(normalized.as_str()) {
      if bound_source != &signer_source {
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
            ("boundSignerSource", json!(bound_source.clone())),
          ]),
        ));

        return PluginVerificationResult {
          verified: false,
          reason: Some("SIGNATURE_VERIFY_FAILED".to_string()),
          signature_algorithm: Some("source_binding".to_string()),
          signer: Some(TRUSTED_SIGNER.to_string()),
          signer_source: Some(signer_source),
        };
      }
    } else {
      bindings.insert(normalized.clone(), signer_source.clone());
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

    PluginVerificationResult {
      verified: true,
      reason: None,
      signature_algorithm: Some("source_binding".to_string()),
      signer: Some(TRUSTED_SIGNER.to_string()),
      signer_source: Some(signer_source),
    }
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
  use super::PluginService;
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
}
