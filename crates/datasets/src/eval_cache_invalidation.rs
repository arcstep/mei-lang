use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use mei_lang_kernel::resolve_app_eval_cache_root;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct EvalCacheInvalidationReport {
    pub force_cleared: bool,
    pub removed_artifact_files: usize,
    pub removed_bytes: u64,
    pub cleared_bootstrap_scopes: usize,
    pub retained_artifact_files: usize,
}

#[derive(Debug, Clone, Default)]
pub struct EvalCacheInvalidationPlan {
    pub force_clear: bool,
    pub allowed_response_cache_keys: BTreeSet<String>,
    pub stale_bootstrap_scopes: BTreeSet<String>,
}

pub fn metric_eval_artifact_reusable(
    app_root: &Path,
    response_cache_key: &str,
    requested_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> bool {
    let Ok(Some((artifact, _))) =
        crate::result_artifact::load_metric_response_result_artifact(app_root, response_cache_key)
    else {
        return false;
    };
    if request_all_metrics {
        return artifact.complete;
    }
    requested_metric_ids
        .iter()
        .all(|metric_id| artifact.covered_metric_ids.contains(metric_id))
}

pub fn invalidate_stale_eval_artifacts(
    app_root: &Path,
    plan: &EvalCacheInvalidationPlan,
) -> Result<EvalCacheInvalidationReport> {
    let mut report = EvalCacheInvalidationReport::default();
    let eval_root = resolve_app_eval_cache_root(app_root);
    if plan.force_clear {
        report.force_cleared = true;
        if eval_root.exists() {
            report.removed_artifact_files = count_files_recursively(&eval_root);
            report.removed_bytes = dir_tree_bytes(&eval_root);
            fs::remove_dir_all(&eval_root).with_context(|| {
                format!("remove eval-cache root {}", eval_root.display())
            })?;
        }
        clear_all_client_bootstraps(app_root, &mut report)?;
        return Ok(report);
    }

    let metric_response_root = eval_root.join("metric-response");
    if metric_response_root.is_dir() {
        for entry in fs::read_dir(&metric_response_root).with_context(|| {
            format!(
                "read metric-response dir {}",
                metric_response_root.display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(cache_key) = response_cache_key_from_artifact_path(&path) else {
                report.removed_artifact_files += 1;
                report.removed_bytes += path.metadata().map(|m| m.len()).unwrap_or(0);
                fs::remove_file(&path)?;
                continue;
            };
            if plan.allowed_response_cache_keys.contains(cache_key.as_str()) {
                report.retained_artifact_files += 1;
                continue;
            }
            report.removed_artifact_files += 1;
            report.removed_bytes += path.metadata().map(|m| m.len()).unwrap_or(0);
            fs::remove_file(&path)?;
        }
    }

    for scope in &plan.stale_bootstrap_scopes {
        let path = mei_lang_kernel::resolve_app_var_root(app_root)
            .join("client-bootstrap")
            .join(format!("{scope}.json"));
        if path.is_file() {
            fs::remove_file(&path)?;
            report.cleared_bootstrap_scopes += 1;
        }
    }

    Ok(report)
}

fn clear_all_client_bootstraps(
    app_root: &Path,
    report: &mut EvalCacheInvalidationReport,
) -> Result<()> {
    let bootstrap_root = mei_lang_kernel::resolve_app_var_root(app_root).join("client-bootstrap");
    if !bootstrap_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&bootstrap_root).with_context(|| {
        format!("read client-bootstrap dir {}", bootstrap_root.display())
    })? {
        let entry = entry?;
        if entry.path().is_file() {
            report.cleared_bootstrap_scopes += 1;
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn response_cache_key_from_artifact_path(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("response_cache_key")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn count_files_recursively(path: &Path) -> usize {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .map(|child| {
            if child.is_file() {
                1
            } else if child.is_dir() {
                count_files_recursively(&child)
            } else {
                0
            }
        })
        .sum()
}

fn dir_tree_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .map(|child| {
            if child.is_file() {
                child.metadata().map(|m| m.len()).unwrap_or(0)
            } else if child.is_dir() {
                dir_tree_bytes(&child)
            } else {
                0
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_clear_removes_eval_cache_and_bootstraps() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app_root = temp.path();
        let eval_root = resolve_app_eval_cache_root(app_root);
        fs::create_dir_all(eval_root.join("metric-response")).expect("mkdir");
        fs::write(
            eval_root.join("metric-response/abc.json"),
            r#"{"response_cache_key":"k1","schema_version":"mei-metric-response-result-artifact-v1"}"#,
        )
        .expect("write");
        let bootstrap_root = mei_lang_kernel::resolve_app_var_root(app_root).join("client-bootstrap");
        fs::create_dir_all(&bootstrap_root).expect("mkdir bootstrap");
        fs::write(bootstrap_root.join("home.json"), "{}").expect("write bootstrap");
        let report = invalidate_stale_eval_artifacts(
            app_root,
            &EvalCacheInvalidationPlan {
                force_clear: true,
                ..Default::default()
            },
        )
        .expect("invalidate");
        assert!(report.force_cleared);
        assert!(report.removed_artifact_files >= 1);
        assert!(report.cleared_bootstrap_scopes >= 1);
        assert!(!eval_root.exists());
    }
}
