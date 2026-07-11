//! Content-addressed artifact store (Phase G / 1.3.0).
//!
//! Registry nodes reference payloads via `PayloadRef.contentHash`; bytes live under
//! `apps/{appId}/build/active/store/content/{kind}/{sha256}.json`.

mod kinds;

pub use kinds::*;

use std::path::{Path, PathBuf};

use anyhow::Context;
use mei_lang_kernel::resolve_app_build_root;
use sha2::{Digest, Sha256};

use crate::graph::types::PayloadRef;

#[derive(Debug, Clone)]
pub struct PutResult {
    pub content_hash: String,
    pub created: bool,
}

pub fn content_store_root(app_root: &Path) -> PathBuf {
    resolve_app_build_root(app_root)
        .join("store")
        .join("content")
}

pub fn content_hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn put_if_absent(app_root: &Path, kind: &str, bytes: &[u8]) -> anyhow::Result<PutResult> {
    let content_hash = content_hash_bytes(bytes);
    let dir = content_store_root(app_root).join(kind);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create content store dir {}", dir.display()))?;
    let path = dir.join(format!("{content_hash}.json"));
    if path.is_file() {
        return Ok(PutResult {
            content_hash,
            created: false,
        });
    }
    std::fs::write(&path, bytes)
        .with_context(|| format!("write content store blob {}", path.display()))?;
    Ok(PutResult {
        content_hash,
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
    let hash = pref.content_hash.trim();
    if hash.is_empty() {
        return None;
    }
    get(app_root, pref.kind.as_str(), hash)
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
