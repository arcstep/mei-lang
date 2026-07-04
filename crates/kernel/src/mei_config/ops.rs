use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::MeiConfig;
use super::types::{OpsBasemapEntry, OpsSourceEntry};

pub fn merge_ops_section(config: &mut MeiConfig, patch: &OpsConfigPatch) -> Result<()> {
    if let Some(themes) = &patch.themes {
        for (key, value) in themes {
            if value.is_null() {
                config.ops.themes.remove(key);
            } else {
                config.ops.themes.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(sources) = &patch.sources {
        for (key, value) in sources {
            if value.is_null() {
                config.ops.sources.remove(key);
            } else {
                let entry: OpsSourceEntry = serde_json::from_value(value.clone())
                    .with_context(|| format!("invalid ops.sources entry `{key}`"))?;
                config.ops.sources.insert(key.clone(), entry);
            }
        }
    }
    if let Some(basemaps) = &patch.basemaps {
        for (key, value) in basemaps {
            if value.is_null() {
                config.ops.basemaps.remove(key);
            } else {
                let entry: OpsBasemapEntry = serde_json::from_value(value.clone())
                    .with_context(|| format!("invalid ops.basemaps entry `{key}`"))?;
                config.ops.basemaps.insert(key.clone(), entry);
            }
        }
    }
    if let Some(params) = &patch.params {
        for (key, value) in params {
            if value.is_null() {
                config.ops.params.remove(key);
            } else {
                config.ops.params.insert(key.clone(), value.clone());
            }
        }
    }
    if let Some(layout_tuning) = &patch.layout_tuning {
        if layout_tuning.is_null() {
            config.ops.layout_tuning = None;
        } else {
            config.ops.layout_tuning = Some(layout_tuning.clone());
        }
    }
    Ok(())
}

/// 宿主写 ops 时允许的 patch 形状（仅 ops 子树，禁止其它顶层键）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpsConfigPatch {
    #[serde(default)]
    pub themes: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub sources: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub basemaps: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub params: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub layout_tuning: Option<Value>,
}

impl OpsConfigPatch {
    pub fn is_empty(&self) -> bool {
        self.themes.is_none()
            && self.sources.is_none()
            && self.basemaps.is_none()
            && self.params.is_none()
            && self.layout_tuning.is_none()
    }
}
