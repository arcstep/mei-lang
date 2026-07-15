//! Phase 8.5 ephemeral runtime policy overlay.
//!
//! Product path: effective plan is a **uniform** `defaultMode` for the whole app
//! (`launch.json.defaultMode`, or overlay `defaultMode` when set). Cleared on
//! Host/process restart; never written back to `launch.json`.
//!
//! Fine-grained overlay (targets / metricOverrides without `defaultMode`) remains
//! an e2e / advanced escape hatch.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mei_lang_kernel::{RuntimeMode, RuntimePlan, RuntimePlanApp, RuntimePlanTarget};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::paths::app_ephemeral_runtime_root;

pub const SCHEMA_RUNTIME_OVERLAY_V1: &str = "mei-runtime-overlay-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeOverlayTarget {
    pub scope: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePolicyOverlay {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub default_mode: Option<String>,
    #[serde(default)]
    pub targets: Vec<RuntimeOverlayTarget>,
    #[serde(default)]
    pub metric_overrides: BTreeMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeOverlayError {
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Invalid(String),
}

fn overlay_path(workspace: &Path, app_id: &str) -> PathBuf {
    app_ephemeral_runtime_root(workspace, app_id).join("runtime-overlay.json")
}

fn parse_mode(value: &str) -> Option<RuntimeMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "hot" => Some(RuntimeMode::Hot),
        "lazy" => Some(RuntimeMode::Lazy),
        "frozen" => Some(RuntimeMode::Frozen),
        _ => None,
    }
}

fn digest_of(overlay: &RuntimePolicyOverlay) -> String {
    let mut hasher = Sha256::new();
    let bytes = serde_json::to_vec(overlay).unwrap_or_default();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Process-local overlay cache (cleared when Host process restarts).
fn memory_overlays() -> &'static Mutex<BTreeMap<String, RuntimePolicyOverlay>> {
    static CELL: std::sync::OnceLock<Mutex<BTreeMap<String, RuntimePolicyOverlay>>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn read_runtime_overlay(workspace: &Path, app_id: &str) -> Option<RuntimePolicyOverlay> {
    if let Ok(guard) = memory_overlays().lock() {
        if let Some(overlay) = guard.get(app_id) {
            return Some(overlay.clone());
        }
    }
    let path = overlay_path(workspace, app_id);
    let bytes = fs::read(path).ok()?;
    let mut overlay: RuntimePolicyOverlay = serde_json::from_slice(&bytes).ok()?;
    if overlay.revision.trim().is_empty() {
        overlay.revision = digest_of(&overlay);
    }
    if let Ok(mut guard) = memory_overlays().lock() {
        guard.insert(app_id.to_string(), overlay.clone());
    }
    Some(overlay)
}

pub fn clear_runtime_overlay(workspace: &Path, app_id: &str) -> Result<(), RuntimeOverlayError> {
    if let Ok(mut guard) = memory_overlays().lock() {
        guard.remove(app_id);
    }
    let path = overlay_path(workspace, app_id);
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| RuntimeOverlayError::Io(e.to_string()))?;
    }
    Ok(())
}

/// Apply overlay with CAS on `expected_revision` (empty = create/replace when absent).
pub fn write_runtime_overlay(
    workspace: &Path,
    app_id: &str,
    mut overlay: RuntimePolicyOverlay,
    expected_revision: Option<&str>,
) -> Result<RuntimePolicyOverlay, RuntimeOverlayError> {
    let current = read_runtime_overlay(workspace, app_id);
    if let Some(expected) = expected_revision.map(str::trim).filter(|v| !v.is_empty()) {
        let actual = current
            .as_ref()
            .map(|doc| doc.revision.as_str())
            .unwrap_or("");
        if actual != expected {
            return Err(RuntimeOverlayError::Conflict(format!(
                "runtime overlay revision mismatch: expected `{expected}`, got `{actual}`"
            )));
        }
    }
    overlay.schema_version = SCHEMA_RUNTIME_OVERLAY_V1.to_string();
    overlay.app_id = app_id.to_string();
    for target in &overlay.targets {
        if parse_mode(target.mode.as_str()).is_none() {
            return Err(RuntimeOverlayError::Invalid(format!(
                "invalid mode `{}` for scope `{}`",
                target.mode, target.scope
            )));
        }
    }
    for (metric, mode) in &overlay.metric_overrides {
        if parse_mode(mode).is_none() {
            return Err(RuntimeOverlayError::Invalid(format!(
                "invalid metric override mode `{mode}` for `{metric}`"
            )));
        }
    }
    overlay.revision = String::new();
    overlay.revision = digest_of(&overlay);

    let path = overlay_path(workspace, app_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RuntimeOverlayError::Io(e.to_string()))?;
    }
    let bytes = serde_json::to_vec_pretty(&overlay)
        .map_err(|e| RuntimeOverlayError::Invalid(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &bytes).map_err(|e| RuntimeOverlayError::Io(e.to_string()))?;
    fs::rename(&tmp, &path).map_err(|e| RuntimeOverlayError::Io(e.to_string()))?;
    if let Ok(mut guard) = memory_overlays().lock() {
        guard.insert(app_id.to_string(), overlay.clone());
    }
    Ok(overlay)
}

/// Resolve the effective runtime plan for an app start / overlay apply.
///
/// Product semantics (Phase 8.5 control center):
/// - `launch.json.runtimePlan.defaultMode` is the **unified** startup mode for the whole app.
/// - UI / overlay `defaultMode` replaces that mode for the whole app (still ephemeral).
/// - Under a chosen mode, **all** scopes/metrics follow that single mode — launch
///   `targets` / `metricOverrides` are not applied on the product path.
///
/// Advanced / e2e: an overlay **without** `defaultMode` but with `targets` /
/// `metricOverrides` still merges onto the base plan (fine-grained escape hatch).
pub fn effective_runtime_plan(
    base: &RuntimePlan,
    app_id: &str,
    overlay: Option<&RuntimePolicyOverlay>,
) -> RuntimePlan {
    let Some(overlay) = overlay else {
        return uniform_runtime_plan(base.default_mode, app_id);
    };
    if let Some(mode) = overlay.default_mode.as_deref().and_then(parse_mode) {
        return uniform_runtime_plan(mode, app_id);
    }
    let mut plan = base.clone();
    let app_entry = plan
        .apps
        .entry(app_id.to_string())
        .or_insert_with(RuntimePlanApp::default);
    for target in &overlay.targets {
        let scope = target.scope.trim().trim_matches('/').to_string();
        if scope.is_empty() {
            continue;
        }
        let Some(mode) = parse_mode(target.mode.as_str()) else {
            continue;
        };
        if let Some(existing) = app_entry
            .targets
            .iter_mut()
            .find(|item| item.scope.trim().trim_matches('/') == scope)
        {
            existing.mode = mode;
        } else {
            app_entry.targets.push(RuntimePlanTarget { scope, mode });
        }
    }
    for (metric, mode) in &overlay.metric_overrides {
        if let Some(parsed) = parse_mode(mode) {
            app_entry
                .metric_overrides
                .insert(metric.trim().to_string(), parsed);
        }
    }
    plan
}

fn uniform_runtime_plan(mode: RuntimeMode, app_id: &str) -> RuntimePlan {
    RuntimePlan {
        default_mode: mode,
        apps: BTreeMap::from([(app_id.to_string(), RuntimePlanApp::default())]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn overlay_cas_and_effective_merge() {
        let dir = tempdir().unwrap();
        let workspace = dir.path();
        let base = RuntimePlan {
            default_mode: RuntimeMode::Frozen,
            apps: BTreeMap::from([(
                "mini-data".to_string(),
                RuntimePlanApp {
                    targets: vec![RuntimePlanTarget {
                        scope: "home/t1/r-right-rail/s-warning".into(),
                        mode: RuntimeMode::Frozen,
                    }],
                    metric_overrides: BTreeMap::new(),
                },
            )]),
        };
        let written = write_runtime_overlay(
            workspace,
            "mini-data",
            RuntimePolicyOverlay {
                targets: vec![RuntimeOverlayTarget {
                    scope: "home/t1/r-right-rail/s-warning".into(),
                    mode: "hot".into(),
                }],
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let conflict = write_runtime_overlay(
            workspace,
            "mini-data",
            RuntimePolicyOverlay {
                targets: vec![RuntimeOverlayTarget {
                    scope: "home/t1/r-right-rail/s-warning".into(),
                    mode: "lazy".into(),
                }],
                ..Default::default()
            },
            Some("stale"),
        );
        assert!(matches!(conflict, Err(RuntimeOverlayError::Conflict(_))));

        // Fine-grained overlay (no defaultMode) still merges onto base targets.
        let effective = effective_runtime_plan(&base, "mini-data", Some(&written));
        assert_eq!(
            effective.apps["mini-data"].targets[0].mode,
            RuntimeMode::Hot
        );

        clear_runtime_overlay(workspace, "mini-data").unwrap();
        assert!(read_runtime_overlay(workspace, "mini-data").is_none());
    }

    #[test]
    fn product_mode_is_uniform_and_ignores_launch_targets() {
        let base = RuntimePlan {
            default_mode: RuntimeMode::Frozen,
            apps: BTreeMap::from([(
                "mini-data".to_string(),
                RuntimePlanApp {
                    targets: vec![
                        RuntimePlanTarget {
                            scope: "home/t1/r-right-rail/s-warning".into(),
                            mode: RuntimeMode::Hot,
                        },
                        RuntimePlanTarget {
                            scope: "home/t1/r-right-rail/s-enforcement".into(),
                            mode: RuntimeMode::Frozen,
                        },
                    ],
                    metric_overrides: BTreeMap::from([("warnings_count".into(), RuntimeMode::Hot)]),
                },
            )]),
        };

        let from_git = effective_runtime_plan(&base, "mini-data", None);
        assert_eq!(from_git.default_mode, RuntimeMode::Frozen);
        assert!(from_git.apps["mini-data"].targets.is_empty());
        assert!(from_git.apps["mini-data"].metric_overrides.is_empty());

        let overlay = RuntimePolicyOverlay {
            default_mode: Some("lazy".into()),
            targets: vec![RuntimeOverlayTarget {
                scope: "should-be-ignored".into(),
                mode: "hot".into(),
            }],
            metric_overrides: BTreeMap::from([("warnings_count".into(), "frozen".into())]),
            ..Default::default()
        };
        let from_ui = effective_runtime_plan(&base, "mini-data", Some(&overlay));
        assert_eq!(from_ui.default_mode, RuntimeMode::Lazy);
        assert!(from_ui.apps["mini-data"].targets.is_empty());
        assert!(from_ui.apps["mini-data"].metric_overrides.is_empty());
    }
}
