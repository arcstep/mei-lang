//! Training pack manifest + member lists (curriculum scope).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackManifest {
    #[serde(default)]
    #[allow(dead_code)]
    pub schema_version: u32,
    pub packs: Vec<PackManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackManifestEntry {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub order: u32,
    #[serde(default)]
    pub item_file: Option<String>,
    #[serde(default)]
    pub unlock: UnlockRule,
    #[serde(default)]
    #[allow(dead_code)]
    pub target_ladder: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnlockRule {
    DefaultOpen,
    Manual,
    PackMastery {
        requires: String,
        #[serde(default = "default_min_introduced", rename = "minIntroducedPct")]
        min_introduced_pct: u32,
        #[serde(default, rename = "minL1Pct")]
        min_l1_pct: u32,
        #[serde(default, rename = "minL2Pct")]
        min_l2_pct: u32,
    },
}

impl Default for UnlockRule {
    fn default() -> Self {
        Self::Manual
    }
}

fn default_min_introduced() -> u32 {
    100
}

#[derive(Debug, Clone, Deserialize)]
struct PackFile {
    #[allow(dead_code)]
    id: String,
    chars: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PackDef {
    pub id: String,
    pub title: String,
    pub tier: String,
    pub order: u32,
    pub chars: Vec<String>,
    pub unlock: UnlockRule,
}

#[derive(Debug, Clone, Default)]
pub struct PackCatalog {
    pub packs: BTreeMap<String, PackDef>,
    pub order: Vec<String>,
}

impl PackCatalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn get(&self, id: &str) -> Option<&PackDef> {
        self.packs.get(id)
    }

    pub fn char_item_ids(&self, pack_id: &str) -> Vec<String> {
        self.packs
            .get(pack_id)
            .map(|p| p.chars.iter().map(|c| format!("char:{c}")).collect())
            .unwrap_or_default()
    }

    pub fn union_char_item_ids(&self, pack_ids: &[String]) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for pid in pack_ids {
            for id in self.char_item_ids(pid) {
                if seen.insert(id.clone()) {
                    out.push(id);
                }
            }
        }
        out
    }

    pub fn all_b_pack_ids(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|id| self.packs.get(*id).map(|p| p.tier == "B").unwrap_or(false))
            .cloned()
            .collect()
    }

    pub fn next_locked_after(&self, open: &[String]) -> Option<&PackDef> {
        for id in &self.order {
            if open.iter().any(|o| o == id) {
                continue;
            }
            let Some(p) = self.packs.get(id) else {
                continue;
            };
            if matches!(p.unlock, UnlockRule::Manual) || p.chars.is_empty() {
                continue;
            }
            return Some(p);
        }
        None
    }
}

/// Load `data/packs/manifest.json` and member files. Missing dir → empty catalog.
pub fn load_pack_catalog(app_root: &Path) -> Result<PackCatalog> {
    let packs_dir = app_root.join("data/packs");
    let manifest_path = packs_dir.join("manifest.json");
    if !manifest_path.is_file() {
        return Ok(PackCatalog::empty());
    }
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let manifest: PackManifest = serde_json::from_str(&raw)
        .with_context(|| format!("parse {}", manifest_path.display()))?;

    let mut packs = BTreeMap::new();
    let mut order = Vec::new();
    for entry in manifest.packs {
        order.push(entry.id.clone());
        let chars = if let Some(file) = entry.item_file.as_deref().filter(|s| !s.is_empty()) {
            let path = packs_dir.join(file);
            if !path.is_file() {
                bail!("pack file missing: {}", path.display());
            }
            let pf: PackFile = serde_json::from_str(
                &fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?,
            )
            .with_context(|| format!("parse {}", path.display()))?;
            pf.chars
        } else {
            Vec::new()
        };
        packs.insert(
            entry.id.clone(),
            PackDef {
                id: entry.id,
                title: entry.title,
                tier: entry.tier,
                order: entry.order,
                chars,
                unlock: entry.unlock,
            },
        );
    }
    Ok(PackCatalog { packs, order })
}

#[cfg(test)]
mod load_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn load_wubi_packs_from_mei_test_workspace() {
        // Optional monorepo probe: set MEI_TEST_WORKSPACE to a workspace root that
        // contains apps/wubi/data/packs. Missing → skip (no sibling path hard fail).
        let Ok(ws) = std::env::var("MEI_TEST_WORKSPACE") else {
            return;
        };
        let app = PathBuf::from(ws).join("apps/wubi");
        if !app.join("data/packs/manifest.json").is_file() {
            return;
        }
        let cat = load_pack_catalog(&app).expect("load packs");
        assert!(cat.packs.contains_key("pack-a"));
        assert_eq!(cat.all_b_pack_ids().len(), 35);
        assert_eq!(cat.char_item_ids("pack-b1").len(), 100);
    }
}
