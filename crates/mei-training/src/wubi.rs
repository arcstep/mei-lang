use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::queue::TrainingMode;

#[derive(Debug, Clone)]
pub struct WubiCharItem {
    pub ch: String,
    pub code: String,
    pub tier: String,
}

#[derive(Debug, Clone)]
pub struct WubiRadicalItem {
    pub key: String,
    pub mnemonic: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct WubiCatalog {
    pub chars: BTreeMap<String, WubiCharItem>,
    pub radicals: BTreeMap<String, WubiRadicalItem>,
}

impl WubiCatalog {
    pub fn contains(&self, item_id: &str) -> bool {
        self.resolve_char(item_id).is_some() || self.resolve_radical(item_id).is_some()
    }

    pub fn item_ids_for_mode(&self, mode: TrainingMode, char_pool: &str) -> Vec<String> {
        match mode {
            TrainingMode::RadicalKey => self
                .radicals
                .keys()
                .map(|k| format!("radical:{k}"))
                .collect(),
            TrainingMode::CharToCode => {
                let mut ids: Vec<String> = self
                    .chars
                    .values()
                    .filter(|c| match char_pool {
                        "d2" => true,
                        _ => c.tier == "d1",
                    })
                    .map(|c| format!("char:{}", c.ch))
                    .collect();
                ids.sort();
                ids
            }
        }
    }

    pub fn payload_for(&self, item_id: &str, show_hint: bool) -> Value {
        if let Some(c) = self.resolve_char(item_id) {
            let mut obj = json!({
                "kind": "char",
                "char": c.ch,
                "tier": c.tier,
            });
            if show_hint {
                let hint = c.code.chars().next().map(|ch| ch.to_string());
                obj["hint"] = json!(hint);
            }
            return obj;
        }
        if let Some(r) = self.resolve_radical(item_id) {
            return json!({
                "kind": "radical",
                "key": r.key,
                "mnemonic": r.mnemonic,
                "examples": r.examples,
            });
        }
        json!({ "kind": "unknown", "item_id": item_id })
    }

    pub fn judge(
        &self,
        item_id: &str,
        answer: Option<&str>,
        client_correct: Option<bool>,
    ) -> (bool, Option<String>) {
        if let Some(c) = self.resolve_char(item_id) {
            let expected = c.code.to_ascii_lowercase();
            let got = answer.unwrap_or("").trim().to_ascii_lowercase();
            return (got == expected, Some(expected));
        }
        if let Some(r) = self.resolve_radical(item_id) {
            let expected = r.key.to_ascii_lowercase();
            let got = answer.unwrap_or("").trim().to_ascii_lowercase();
            return (got == expected, Some(expected));
        }
        (client_correct.unwrap_or(false), None)
    }

    fn resolve_char(&self, item_id: &str) -> Option<&WubiCharItem> {
        let ch = item_id.strip_prefix("char:")?;
        self.chars.get(ch)
    }

    fn resolve_radical(&self, item_id: &str) -> Option<&WubiRadicalItem> {
        let key = item_id.strip_prefix("radical:")?;
        self.radicals.get(key)
    }
}

#[derive(Debug, Deserialize)]
struct CharBundleFile {
    chars: BTreeMap<String, CharBundleEntry>,
}

#[derive(Debug, Deserialize)]
struct CharBundleEntry {
    code: String,
    #[serde(default)]
    tier: Option<String>,
    #[serde(default)]
    level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RadicalsFile {
    /// Preferred shape: `{ "keys": { "G": { ... }, ... }, "schemaVersion"?: … }`.
    #[serde(default)]
    keys: BTreeMap<String, RadicalEntry>,
}

#[derive(Debug, Deserialize)]
struct RadicalEntry {
    #[serde(default)]
    mnemonic: String,
    #[serde(default)]
    examples: Vec<String>,
}

fn is_practice_han(ch: &str) -> bool {
    let mut it = ch.chars();
    let Some(c) = it.next() else {
        return false;
    };
    if it.next().is_some() {
        return false;
    }
    // CJK Unified Ideographs only. Excludes 〇 (U+3007) and Extension A (U+3400+)
    // which Unicode-sort to the front and look like empty / unreadable glyphs.
    matches!(c as u32, 0x4E00..=0x9FFF)
}

fn parse_radicals_file(raw: &str) -> Result<BTreeMap<String, RadicalEntry>> {
    // Nested document with metadata (schemaVersion / variant / keys).
    if let Ok(file) = serde_json::from_str::<RadicalsFile>(raw) {
        if !file.keys.is_empty() {
            return Ok(file.keys);
        }
    }
    // Flat map: { "G": { mnemonic, examples }, ... } (no wrapper).
    let flat: BTreeMap<String, RadicalEntry> = serde_json::from_str(raw)?;
    Ok(flat)
}

/// Load wubi catalog from app data directory.
/// Prefers `data/bundles/wubi06-tygf8105.json`, falls back to `data/samples/wubi06-sample.json`.
pub fn load_wubi_catalog(app_root: &Path) -> Result<WubiCatalog> {
    let bundle_candidates = [
        app_root.join("data/bundles/wubi06-tygf8105.json"),
        app_root.join("data/samples/wubi06-sample.json"),
    ];
    let bundle_path = bundle_candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .with_context(|| {
            format!(
                "no wubi char bundle under {} (expected data/bundles or data/samples)",
                app_root.display()
            )
        })?;

    let radical_candidates = [
        app_root.join("data/radicals-wubi06.json"),
        app_root.join("data/bundles/radicals-wubi06.json"),
    ];
    let radical_path = radical_candidates.iter().find(|p| p.is_file()).cloned();

    let raw = fs::read_to_string(&bundle_path)
        .with_context(|| format!("read {}", bundle_path.display()))?;
    let bundle: CharBundleFile =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", bundle_path.display()))?;

    // Prefer CJK Unified Ideographs; order by codepoint so intro starts near 一/丁/七…
    let mut ordered: Vec<(String, CharBundleEntry)> = bundle
        .chars
        .into_iter()
        .filter(|(ch, _)| is_practice_han(ch))
        .collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));

    let mut chars = BTreeMap::new();
    for (idx, (ch, entry)) in ordered.into_iter().enumerate() {
        // Re-tier by filtered order: first 3500 ≈ 一级常用；忽略码表里被 Unicode 排序污染的 tier。
        let tier = if idx < 3500 { "d1" } else { "d2" };
        chars.insert(
            ch.clone(),
            WubiCharItem {
                ch,
                code: entry.code.to_ascii_lowercase(),
                tier: tier.to_string(),
            },
        );
    }

    let mut radicals = BTreeMap::new();
    if let Some(path) = radical_path {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let source = parse_radicals_file(&raw)
            .with_context(|| format!("parse {}", path.display()))?;
        for (key, entry) in source {
            let key_up = key.to_ascii_uppercase();
            if key_up.len() != 1 {
                continue;
            }
            radicals.insert(
                key_up.clone(),
                WubiRadicalItem {
                    key: key_up,
                    mnemonic: entry.mnemonic,
                    examples: entry.examples,
                },
            );
        }
    }

    if chars.is_empty() {
        bail!("wubi char bundle is empty: {}", bundle_path.display());
    }

    Ok(WubiCatalog { chars, radicals })
}
