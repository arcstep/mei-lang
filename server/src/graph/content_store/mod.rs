//! Content-addressed artifact store (Phase G).
//!
//! Registry nodes reference payloads via `PayloadRef.contentHash`; bytes live under
//! `apps/{appId}/build/active/store/content/{kind}/{sha256}.json`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::Context;
use mei_lang_kernel::resolve_app_build_root;
use sha2::{Digest, Sha256};

use crate::graph::types::PayloadRef;

#[derive(Debug, Clone)]
pub struct PutResult {
    pub content_hash: String,
    pub path: PathBuf,
    pub created: bool,
}

pub fn content_store_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MEI_CONTENT_STORE")
            .map(|value| {
                let trimmed = value.trim();
                !(trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
            })
            .unwrap_or(true)
    })
}

pub fn content_store_root(app_root: &Path) -> PathBuf {
    resolve_app_build_root(app_root).join("store").join("content")
}

pub fn content_hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn put_if_absent(app_root: &Path, kind: &str, bytes: &[u8]) -> anyhow::Result<PutResult> {
    let content_hash = content_hash_bytes(bytes);
    let dir = content_store_root(app_root).join(kind);
    std::fs::create_dir_all(&dir).with_context(|| format!("create content store dir {}", dir.display()))?;
    let path = dir.join(format!("{content_hash}.json"));
    if path.is_file() {
        return Ok(PutResult {
            content_hash,
            path,
            created: false,
        });
    }
    std::fs::write(&path, bytes)
        .with_context(|| format!("write content store blob {}", path.display()))?;
    Ok(PutResult {
        content_hash,
        path,
        created: true,
    })
}

pub fn get(app_root: &Path, kind: &str, content_hash: &str) -> Option<PathBuf> {
    let hash = content_hash.trim();
    if hash.is_empty() {
        return None;
    }
    let path = content_store_root(app_root)
        .join(kind)
        .join(format!("{hash}.json"));
    path.is_file().then_some(path)
}

pub fn resolve_payload_ref(app_root: &Path, pref: &PayloadRef) -> Option<PathBuf> {
    if let Some(hash) = pref
        .content_hash
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(path) = get(app_root, pref.kind.as_str(), hash) {
            return Some(path);
        }
    }
    let rel = pref.relative_path.trim();
    if rel.is_empty() {
        return None;
    }
    let legacy = app_root.join(rel);
    if legacy.is_file() {
        return Some(legacy);
    }
    let under_build = resolve_app_build_root(app_root).join(rel.trim_start_matches('/'));
    under_build.is_file().then_some(under_build)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_if_absent_dedupes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = tmp.path();
        std::fs::create_dir_all(app.join("build/active")).expect("mkdir");
        let bytes = br#"{"ok":true}"#;
        let first = put_if_absent(app, "scene_payload", bytes).expect("put");
        assert!(first.created);
        let second = put_if_absent(app, "scene_payload", bytes).expect("put");
        assert!(!second.created);
        assert_eq!(first.content_hash, second.content_hash);
    }
}
