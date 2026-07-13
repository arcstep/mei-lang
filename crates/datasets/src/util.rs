use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::eval_cache_io_stats::record_artifact_read;

pub(crate) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

/// Read a JSON artifact; IO/parse failures are treated as cache miss (not fatal).
pub(crate) fn read_json_artifact_lenient<T: DeserializeOwned>(
    path: &Path,
    artifact_kind: &str,
) -> Result<Option<T>> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                %error,
                artifact_kind,
                "read artifact failed; treating as cache miss"
            );
            return Ok(None);
        }
    };
    record_artifact_read(raw.len() as u64);
    match serde_json::from_str::<T>(&raw) {
        Ok(artifact) => Ok(Some(artifact)),
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                %error,
                artifact_kind,
                "parse artifact failed; treating as cache miss"
            );
            Ok(None)
        }
    }
}

pub(crate) fn value_to_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        other => other.to_string(),
    }
}
