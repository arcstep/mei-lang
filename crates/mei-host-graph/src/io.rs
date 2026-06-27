use std::fs;
use std::path::Path;

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

pub fn write_json_registry<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
