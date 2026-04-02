use crate::contracts::{HookStage, PluginConfig};
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct PluginRuntime;

const KNOWN_UPLOAD_PLUGINS: [&str; 2] = ["image-compress", "hidden-watermark"];
const MAX_PLUGINS_PER_STAGE: usize = 16;
const MAX_PLUGIN_ID_LENGTH: usize = 64;

fn is_known_plugin(plugin_id: &str) -> bool {
  KNOWN_UPLOAD_PLUGINS
    .iter()
    .any(|candidate| candidate == &plugin_id)
}

impl PluginRuntime {
  pub fn execute_stage(
    &self,
    stage: HookStage,
    plugin_chain: &[PluginConfig],
  ) -> Result<(), String> {
    let mut applicable_plugins = plugin_chain
      .iter()
      .filter(|plugin| plugin.enabled && plugin.stage == stage)
      .collect::<Vec<_>>();
    applicable_plugins.sort_by_key(|plugin| plugin.priority);

    if applicable_plugins.len() > MAX_PLUGINS_PER_STAGE {
      return Err("HOOK_EXECUTION_FAILED".to_string());
    }

    let mut seen_plugin_ids = HashSet::new();

    for plugin in applicable_plugins {
      let normalized_id = plugin.id.trim();
      let normalized_hook_type = plugin.hook_type.trim().to_ascii_lowercase();

      if normalized_id.is_empty() || normalized_id.len() > MAX_PLUGIN_ID_LENGTH {
        return Err("HOOK_EXECUTION_FAILED".to_string());
      }

      if plugin.priority < 0 {
        return Err("HOOK_EXECUTION_FAILED".to_string());
      }

      if !seen_plugin_ids.insert(normalized_id.to_string()) {
        return Err("HOOK_EXECUTION_FAILED".to_string());
      }

      if normalized_hook_type != "upload" {
        return Err("HOOK_EXECUTION_FAILED".to_string());
      }

      if !is_known_plugin(normalized_id) {
        return Err("HOOK_EXECUTION_FAILED".to_string());
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::PluginRuntime;
  use crate::contracts::{HookStage, PluginConfig};

  fn plugin(id: &str, hook_type: &str, stage: HookStage, priority: i32) -> PluginConfig {
    PluginConfig {
      id: id.to_string(),
      enabled: true,
      hook_type: hook_type.to_string(),
      stage,
      priority,
    }
  }

  #[test]
  fn accepts_known_upload_plugin() {
    let runtime = PluginRuntime;
    let chain = vec![plugin("image-compress", "upload", HookStage::PreKey, 1)];

    assert!(runtime.execute_stage(HookStage::PreKey, &chain).is_ok());
  }

  #[test]
  fn rejects_unknown_upload_plugin() {
    let runtime = PluginRuntime;
    let chain = vec![plugin("third-party-plugin", "upload", HookStage::PreKey, 1)];

    assert!(runtime.execute_stage(HookStage::PreKey, &chain).is_err());
  }

  #[test]
  fn rejects_unsupported_hook_type() {
    let runtime = PluginRuntime;
    let chain = vec![plugin("image-compress", "transform", HookStage::PreKey, 1)];

    assert!(runtime.execute_stage(HookStage::PreKey, &chain).is_err());
  }

  #[test]
  fn rejects_duplicate_plugin_id_within_stage() {
    let runtime = PluginRuntime;
    let chain = vec![
      plugin("image-compress", "upload", HookStage::PreKey, 1),
      plugin("image-compress", "upload", HookStage::PreKey, 2),
    ];

    assert!(runtime.execute_stage(HookStage::PreKey, &chain).is_err());
  }

  #[test]
  fn rejects_negative_priority() {
    let runtime = PluginRuntime;
    let chain = vec![plugin("image-compress", "upload", HookStage::PreKey, -1)];

    assert!(runtime.execute_stage(HookStage::PreKey, &chain).is_err());
  }

  #[test]
  fn rejects_excessive_plugins_per_stage() {
    let runtime = PluginRuntime;
    let chain = (0..17)
      .map(|idx| plugin(&format!("plugin-{idx}"), "upload", HookStage::PreKey, idx))
      .collect::<Vec<_>>();

    assert!(runtime.execute_stage(HookStage::PreKey, &chain).is_err());
  }
}
