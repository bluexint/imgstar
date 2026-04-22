use crate::contracts::{ConnectionPingResult, SettingsDraft};
use crate::runtime::adapter_runtime::resolve_bucket_base_url;
use chrono::{SecondsFormat, Utc};
use std::time::{Duration, Instant};

const PING_TIMEOUT_SECS: u64 = 5;

#[derive(Clone)]
pub struct SettingsPingAdapter {
  client: Option<reqwest::Client>,
}

impl SettingsPingAdapter {
  pub fn new() -> Self {
    let client = reqwest::Client::builder()
      .timeout(Duration::from_secs(PING_TIMEOUT_SECS))
      .build()
      .ok();

    Self { client }
  }

  pub async fn ping_storage(&self, settings: &SettingsDraft) -> Result<ConnectionPingResult, String> {
    let probe_url = resolve_bucket_base_url(settings.endpoint.as_str(), settings.bucket.as_str());
    let client = self
      .client
      .as_ref()
      .ok_or_else(|| "INTERNAL_ERROR".to_string())?;

    let started_at = Instant::now();
    match client
      .head(probe_url)
      .header("x-imgstar-ping", "1")
      .send()
      .await
    {
      Ok(_) => Ok(ConnectionPingResult {
        latency_ms: started_at.elapsed().as_millis() as u64,
        checked_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
      }),
      Err(error) => {
        if error.is_timeout() {
          return Err("ADAPTER_TIMEOUT".to_string());
        }

        if error.is_connect() || error.is_request() {
          return Err("ADAPTER_NETWORK_ERROR".to_string());
        }

        Err("ADAPTER_SERVER_ERROR".to_string())
      }
    }
  }
}

impl Default for SettingsPingAdapter {
  fn default() -> Self {
    Self::new()
  }
}