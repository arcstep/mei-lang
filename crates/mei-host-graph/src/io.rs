use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn read_json_registry<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read graph registry {}", path.display()))?;
    let value = serde_json::from_str::<T>(&raw)
        .with_context(|| format!("parse graph registry {}", path.display()))?;
    Ok(Some(value))
}

fn unique_tmp_path(path: &Path) -> PathBuf {
    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("registry.json");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        ".{file_name}.tmp-{}-{}-{seq}",
        std::process::id(),
        stamp
    ))
}

pub fn write_json_registry<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = unique_tmp_path(path);
    fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error).with_context(|| {
            format!(
                "atomically replace {} from {}",
                path.display(),
                tmp.display()
            )
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct Sample {
        n: u32,
    }

    #[test]
    fn concurrent_writes_do_not_fail_with_missing_tmp() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("registry.json");
        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();
        for index in 0..8 {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                write_json_registry(&path, &Sample { n: index }).expect("write");
            }));
        }
        for handle in handles {
            handle.join().expect("join");
        }
        let loaded = read_json_registry::<Sample>(&path)
            .expect("read")
            .expect("exists");
        assert!(loaded.n < 8);
    }
}
