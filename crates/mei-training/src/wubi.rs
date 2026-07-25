use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::packs::{load_pack_catalog, PackCatalog};
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
    pub packs: PackCatalog,
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
                // Legacy path: keep for tests without packs; prefer pack-based pools in queue.
                let mut ids: Vec<String> = self
                    .chars
                    .values()
                    .filter(|c| match char_pool {
                        "d2" | "all" => true,
                        _ => c.tier == "d1" || c.tier == "pack",
                    })
                    .map(|c| format!("char:{}", c.ch))
                    .collect();
                ids.sort();
                ids
            }
        }
    }

    pub fn all_char_item_ids(&self) -> Vec<String> {
        self.chars.keys().map(|ch| format!("char:{ch}")).collect()
    }

    pub fn radical_item_ids(&self) -> Vec<String> {
        self.radicals
            .keys()
            .map(|k| format!("radical:{k}"))
            .collect()
    }

    pub fn payload_for(&self, item_id: &str, show_hint: bool) -> Value {
        if let Some(c) = self.resolve_char(item_id) {
            let brief = level1_brief_key(&c.ch);
            let mut obj = json!({
                "kind": "char",
                "char": c.ch,
                "tier": c.tier,
                "full_code": c.code,
            });
            if let Some(b) = brief {
                obj["brief_code"] = json!(b);
                obj["code_mode"] = json!("level1_brief");
            }
            if show_hint {
                let hint = brief
                    .map(|s| s.to_string())
                    .or_else(|| c.code.chars().next().map(|ch| ch.to_string()));
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
            let full = c.code.to_ascii_lowercase();
            let got = answer.unwrap_or("").trim().to_ascii_lowercase();
            // 一级简码：练习目标为单键；全码也接受。
            if let Some(brief) = level1_brief_key(&c.ch) {
                let brief = brief.to_ascii_lowercase();
                if got == brief || got == full {
                    return (true, Some(brief));
                }
                return (false, Some(brief));
            }
            return (got == full, Some(full));
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

/// 新世纪常用一级简码（键 → 字）。练习目标为单键；与 `build_training_packs.py` 对齐。
pub fn level1_brief_key(ch: &str) -> Option<&'static str> {
    Some(match ch {
        "一" => "g",
        "地" => "f",
        "在" => "d",
        "要" => "s",
        "工" => "a",
        "上" => "h",
        "是" => "j",
        "中" => "k",
        "国" => "l",
        "同" => "m",
        "和" => "t",
        "的" => "r",
        "有" => "e",
        "人" => "w",
        "我" => "q",
        "主" => "y",
        "产" => "u",
        "不" => "i",
        "为" => "o",
        "这" => "p",
        "民" => "n",
        "了" => "b",
        "发" => "v",
        "以" => "c",
        "经" => "x",
        _ => return None,
    })
}

/// Single-stroke 成字字根 — often absent from 通规8105 Rime export; still required by Pack A.
const SINGLE_STROKE_EXTRAS: &[(&str, &str)] = &[
    ("一", "ggll"),
    ("丨", "hhll"),
    ("丿", "ttll"),
    ("丶", "yyll"),
    ("乙", "nnll"),
];

fn ensure_single_stroke_chars(chars: &mut BTreeMap<String, WubiCharItem>) {
    for (ch, code) in SINGLE_STROKE_EXTRAS {
        chars.entry((*ch).to_string()).or_insert_with(|| WubiCharItem {
            ch: (*ch).to_string(),
            code: (*code).to_string(),
            tier: "pack".to_string(),
        });
    }
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

    // Prefer CJK Unified Ideographs. Tier is informational; curriculum uses packs.
    let mut ordered: Vec<(String, CharBundleEntry)> = bundle
        .chars
        .into_iter()
        .filter(|(ch, _)| is_practice_han(ch))
        .collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));

    let packs = load_pack_catalog(app_root).unwrap_or_else(|err| {
        eprintln!(
            "[mei-training] pack catalog load failed ({}): {err:#}",
            app_root.display()
        );
        PackCatalog::empty()
    });

    let pack_chars: BTreeSet<String> = packs
        .packs
        .values()
        .flat_map(|p| p.chars.iter().cloned())
        .collect();

    let mut chars = BTreeMap::new();
    for (ch, entry) in ordered {
        let tier = if pack_chars.contains(&ch) {
            "pack".to_string()
        } else {
            "extra".to_string()
        };
        chars.insert(
            ch.clone(),
            WubiCharItem {
                ch,
                code: entry.code.to_ascii_lowercase(),
                tier,
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

    ensure_single_stroke_chars(&mut chars);

    Ok(WubiCatalog {
        chars,
        radicals,
        packs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn catalog_with(chars: &[(&str, &str)]) -> WubiCatalog {
        let mut map = BTreeMap::new();
        for (ch, code) in chars {
            map.insert(
                (*ch).to_string(),
                WubiCharItem {
                    ch: (*ch).to_string(),
                    code: (*code).to_string(),
                    tier: "pack".into(),
                },
            );
        }
        WubiCatalog {
            chars: map,
            radicals: BTreeMap::new(),
            packs: PackCatalog::empty(),
        }
    }

    #[test]
    fn level1_brief_accepts_single_key_and_full() {
        let cat = catalog_with(&[("地", "fbn")]);
        let (ok, exp) = cat.judge("char:地", Some("f"), None);
        assert!(ok);
        assert_eq!(exp.as_deref(), Some("f"));
        let (ok2, _) = cat.judge("char:地", Some("fbn"), None);
        assert!(ok2);
        let (bad, exp_bad) = cat.judge("char:地", Some("g"), None);
        assert!(!bad);
        assert_eq!(exp_bad.as_deref(), Some("f"));
    }

    #[test]
    fn non_brief_still_requires_full_code() {
        let cat = catalog_with(&[("王", "gggg")]);
        let (ok, exp) = cat.judge("char:王", Some("g"), None);
        assert!(!ok);
        assert_eq!(exp.as_deref(), Some("gggg"));
        let (ok2, _) = cat.judge("char:王", Some("gggg"), None);
        assert!(ok2);
    }
}
