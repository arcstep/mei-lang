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
        clear_all_client_bootstraps(app_root, &mut report)?;
        report.removed_artifact_files += crate::clear_small_artifacts(app_root)?;
        if eval_root.exists() {
            for entry in fs::read_dir(&eval_root)
                .with_context(|| format!("read eval-cache root {}", eval_root.display()))?
            {
                let path = entry?.path();
                if path == crate::small_artifact_store_path(app_root) {
                    continue;
                }
                report.removed_artifact_files += if path.is_dir() {
                    count_files_recursively(&path)
                } else {
                    1
                };
                report.removed_bytes += if path.is_dir() {
                    dir_tree_bytes(&path)
                } else {
                    path.metadata().map(|metadata| metadata.len()).unwrap_or(0)
                };
                if path.is_dir() {
                    fs::remove_dir_all(path)?;
                } else {
                    fs::remove_file(path)?;
                }
            }
        }
        return Ok(report);
    }

    report.removed_artifact_files += crate::retain_small_artifact_keys(
        app_root,
        "metric-response-lite",
        &plan.allowed_response_cache_keys,
    )?;

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
            if plan
                .allowed_response_cache_keys
                .contains(cache_key.as_str())
            {
                report.retained_artifact_files += 1;
                continue;
            }
            report.removed_artifact_files += 1;
            report.removed_bytes += path.metadata().map(|m| m.len()).unwrap_or(0);
            fs::remove_file(&path)?;
        }
    }

    for scope in &plan.stale_bootstrap_scopes {
        let removed_manifest =
            crate::remove_small_artifact(app_root, "client-bootstrap", scope.as_str())?;
        let scene_prefix = format!("{scope}|");
        let removed_scene = crate::remove_small_artifacts_with_prefix(
            app_root,
            "scene-bootstrap",
            scene_prefix.as_str(),
        )?;
        if removed_manifest || removed_scene > 0 {
            report.cleared_bootstrap_scopes += 1;
        }
    }

    Ok(report)
}

fn clear_all_client_bootstraps(
    app_root: &Path,
    report: &mut EvalCacheInvalidationReport,
) -> Result<()> {
    let empty = BTreeSet::new();
    let manifests = crate::retain_small_artifact_keys(app_root, "client-bootstrap", &empty)?;
    let scenes = crate::retain_small_artifact_keys(app_root, "scene-bootstrap", &empty)?;
    report.cleared_bootstrap_scopes = report
        .cleared_bootstrap_scopes
        .saturating_add(manifests.saturating_add(scenes));
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
        let env_dir = app_root.join("env").join("WS-20260720.0");
        fs::create_dir_all(env_dir.join("build")).expect("mkdir build");
        fs::create_dir_all(env_dir.join("var")).expect("mkdir var");
        let current = app_root.join("env").join("current");
        #[cfg(unix)]
        std::os::unix::fs::symlink("WS-20260720.0", &current).expect("symlink env/current");
        #[cfg(not(unix))]
        fs::create_dir_all(&current).expect("mkdir env/current");
        let eval_root = resolve_app_eval_cache_root(app_root);
        fs::create_dir_all(eval_root.join("metric-response")).expect("mkdir");
        fs::write(
            eval_root.join("metric-response/abc.json"),
            r#"{"response_cache_key":"k1","schema_version":"mei-metric-response-result-artifact-v1"}"#,
        )
        .expect("write");
        crate::store_small_artifact(app_root, "client-bootstrap", "home", &serde_json::json!({}))
            .expect("write bootstrap");
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
        assert!(!eval_root.join("metric-response").exists());
        assert!(crate::load_small_artifact::<serde_json::Value>(
            app_root,
            "client-bootstrap",
            "home"
        )
        .expect("load")
        .is_none());
    }
}
