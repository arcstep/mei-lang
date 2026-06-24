use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::compile::resolve_default_scene_from_root;
use crate::mei_config::{
    load_workspace_config, resolve_app_entry_main, resolve_app_root, RuntimeWarmupApp,
    RuntimeWarmupDatasetRequest, RuntimeWarmupManifest, RuntimeWarmupXlsxSource,
    WorkspaceWarmupDatasetConfig, WorkspaceWarmupXlsxConfig, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};
use crate::workspace::discover_apps;

pub const WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION: &str = "mei-runtime-warmup-manifest-v2";

/// Load `.mei/runtime/warmup-manifest.json`, or synthesize from `.mei-workspace.json` when missing.
pub fn resolve_runtime_warmup_manifest(
    source_root: &Path,
) -> Result<Option<RuntimeWarmupManifest>> {
    let manifest_path = source_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL);
    if manifest_path.is_file() {
        let raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("read warmup manifest {}", manifest_path.display()))?;
        let manifest = serde_json::from_str::<RuntimeWarmupManifest>(&raw)
            .with_context(|| format!("parse warmup manifest {}", manifest_path.display()))?;
        return Ok(Some(manifest));
    }

    let workspace_config = load_workspace_config(source_root);
    if !workspace_config.warmup.is_enabled() {
        return Ok(None);
    }

    build_runtime_warmup_manifest(source_root).map(Some)
}

pub fn build_runtime_warmup_manifest(source_root: &Path) -> Result<RuntimeWarmupManifest> {
    let workspace_config = load_workspace_config(source_root);
    if !workspace_config.warmup.is_enabled() {
        return Ok(RuntimeWarmupManifest {
            schema_version: WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION.to_string(),
            enabled: false,
            apps: Vec::new(),
        });
    }

    let apps = discover_apps(source_root)?;
    let mut warmup_apps = Vec::new();
    for app in apps {
        let app_root = resolve_app_root(source_root, &app.id);
        let default_scene = resolve_default_scene_from_root(&app_root).ok().flatten();
        let app_config = workspace_config.warmup.apps.get(&app.id);
        let hot_scenes = normalize_hot_scenes(
            app_config
                .map(|config| config.hot_scenes.as_slice())
                .unwrap_or(&[]),
        );
        let scenes = merge_warmup_scenes(default_scene.as_deref(), hot_scenes.as_slice());
        let mut focuses = normalize_focuses(
            app_config
                .map(|config| config.focuses.as_slice())
                .unwrap_or(&[]),
        );
        if focuses.is_empty() {
            let entry_main = resolve_app_entry_main(&app_root);
            if !entry_main.trim().is_empty() {
                focuses.push(entry_main);
            }
        }
        let merged_datasets = crate::warmup_board_autogen::merge_workspace_and_board_warmup_requests(
            app_config
                .map(|config| config.datasets.as_slice())
                .unwrap_or(&[]),
            app_root.as_path(),
        )?;
        let datasets = normalize_warmup_dataset_requests(merged_datasets.as_slice());
        let xlsx_sources = normalize_warmup_xlsx_sources(
            app_config
                .map(|config| config.xlsx_sources.as_slice())
                .unwrap_or(&[]),
        );
        warmup_apps.push(RuntimeWarmupApp {
            app_id: app.id,
            default_scene,
            hot_scenes,
            scenes,
            focuses,
            datasets,
            xlsx_sources,
        });
    }

    Ok(RuntimeWarmupManifest {
        schema_version: WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION.to_string(),
        enabled: true,
        apps: warmup_apps,
    })
}

fn normalize_hot_scenes(hot_scenes: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for scene in hot_scenes {
        let scene = scene.trim();
        if scene.is_empty() || !seen.insert(scene.to_string()) {
            continue;
        }
        normalized.push(scene.to_string());
    }
    normalized
}

fn normalize_focuses(focuses: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for focus in focuses {
        let focus = focus.trim();
        if focus.is_empty() || !seen.insert(focus.to_string()) {
            continue;
        }
        normalized.push(focus.to_string());
    }
    normalized
}

fn merge_warmup_scenes(default_scene: Option<&str>, hot_scenes: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(default_scene) = default_scene
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if seen.insert(default_scene.to_string()) {
            merged.push(default_scene.to_string());
        }
    }
    for scene in hot_scenes {
        let scene = scene.trim();
        if scene.is_empty() || !seen.insert(scene.to_string()) {
            continue;
        }
        merged.push(scene.to_string());
    }
    merged
}

fn normalize_warmup_dataset_requests(
    requests: &[WorkspaceWarmupDatasetConfig],
) -> Vec<RuntimeWarmupDatasetRequest> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for request in requests {
        let dataset_id = request.dataset_id.trim();
        if dataset_id.is_empty() {
            continue;
        }
        let scene_id = request
            .scene_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let metric_id = request
            .metric_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let mut metric_ids = request
            .metric_ids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if let Some(metric_id) = metric_id.as_deref() {
            metric_ids.push(metric_id.to_string());
        }
        metric_ids.sort();
        metric_ids.dedup();
        let focus = request
            .focus
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let priority = request
            .priority
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| match value.to_ascii_lowercase().as_str() {
                "hot" => "critical".to_string(),
                "full" => "deferred".to_string(),
                other => other.to_string(),
            });
        let dedupe_key = format!(
            "{}|{}|{}|{}|{}|{}",
            scene_id.as_deref().unwrap_or(""),
            focus.as_deref().unwrap_or(""),
            dataset_id,
            priority.as_deref().unwrap_or(""),
            metric_id.as_deref().unwrap_or(""),
            serde_json::to_string(&metric_ids).unwrap_or_default()
        );
        if !seen.insert(dedupe_key) {
            continue;
        }
        normalized.push(RuntimeWarmupDatasetRequest {
            scene_id,
            dataset_id: dataset_id.to_string(),
            priority,
            metric_id,
            metric_ids,
            focus,
        });
    }
    normalized
}

fn normalize_warmup_xlsx_sources(
    sources: &[WorkspaceWarmupXlsxConfig],
) -> Vec<RuntimeWarmupXlsxSource> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for source in sources {
        let path = source.path.trim();
        if path.is_empty() {
            continue;
        }
        let sheet = source
            .sheet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let header_row = source.header_row.unwrap_or(1).max(1);
        let dedupe_key = format!("{}|{}|{header_row}", path, sheet.as_deref().unwrap_or(""));
        if !seen.insert(dedupe_key) {
            continue;
        }
        normalized.push(RuntimeWarmupXlsxSource {
            path: path.to_string(),
            sheet,
            header_row: Some(header_row),
        });
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::mei_config::{app_mei_config_path, write_mei_config, AppEntryConfig, MeiConfig};

    fn temp_workspace_root(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("mei-warmup-manifest-{name}-{nanos}"))
    }

    #[test]
    fn build_manifest_includes_default_entry_focus() {
        let workspace_root = temp_workspace_root("focus");
        let app_root = workspace_root.join("demo");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::write(
            app_root.join("main.mei"),
            "app(id=\"demo\")\nscene(id=\"home\", target=\"home.mei\")\n",
        )
        .expect("write main");
        fs::write(app_root.join("home.mei"), "frame()").expect("write scene");
        fs::write(
            workspace_root.join(".mei-workspace.json"),
            r#"{"warmup":{"apps":{"demo":{"hotScenes":["home"]}}}}"#,
        )
        .expect("write workspace config");

        let manifest = build_runtime_warmup_manifest(&workspace_root).expect("build manifest");
        assert!(manifest.enabled);
        assert_eq!(manifest.apps.len(), 1);
        assert_eq!(manifest.apps[0].focuses, vec!["main.mei".to_string()]);

        let _ = fs::remove_dir_all(workspace_root);
    }

    #[test]
    fn resolve_falls_back_to_workspace_config_when_manifest_missing() {
        let workspace_root = temp_workspace_root("fallback");
        let app_root = workspace_root.join("demo");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::write(
            app_root.join("main.mei"),
            "app(id=\"demo\")\nscene(id=\"home\", target=\"home.mei\")\n",
        )
        .expect("write main");
        fs::write(app_root.join("home.mei"), "frame()").expect("write scene");
        fs::write(
            workspace_root.join(".mei-workspace.json"),
            r#"{"warmup":{"enabled":true,"apps":{"demo":{"hotScenes":["home"]}}}}"#,
        )
        .expect("write workspace config");

        let manifest = resolve_runtime_warmup_manifest(&workspace_root).expect("resolve manifest");
        assert!(manifest.expect("manifest").enabled);

        let _ = fs::remove_dir_all(workspace_root);
    }

    #[test]
    fn dataset_requests_keep_explicit_focus_only() {
        let requests = normalize_warmup_dataset_requests(&[WorkspaceWarmupDatasetConfig {
            scene_id: Some("home".to_string()),
            dataset_id: "warning_list".to_string(),
            priority: None,
            metric_id: Some("case_total".to_string()),
            metric_ids: vec!["case_delta".to_string(), "case_total".to_string()],
            focus: Some("main.mei".to_string()),
        }]);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].focus.as_deref(), Some("main.mei"));
        assert_eq!(
            requests[0].metric_ids,
            vec!["case_delta".to_string(), "case_total".to_string()]
        );
    }

    #[test]
    fn dataset_requests_without_focus_stay_scene_only() {
        let requests = normalize_warmup_dataset_requests(&[WorkspaceWarmupDatasetConfig {
            scene_id: Some("home".to_string()),
            dataset_id: "warning_list".to_string(),
            priority: None,
            metric_id: Some("case_total".to_string()),
            metric_ids: Vec::new(),
            focus: None,
        }]);
        assert_eq!(requests.len(), 1);
        assert!(requests[0].focus.is_none());
    }

    #[test]
    fn dataset_requests_normalize_priority_aliases() {
        let requests = normalize_warmup_dataset_requests(&[
            WorkspaceWarmupDatasetConfig {
                scene_id: Some("home".to_string()),
                dataset_id: "warning_list".to_string(),
                priority: Some("hot".to_string()),
                metric_id: None,
                metric_ids: Vec::new(),
                focus: None,
            },
            WorkspaceWarmupDatasetConfig {
                scene_id: Some("home".to_string()),
                dataset_id: "warning_detail".to_string(),
                priority: Some("full".to_string()),
                metric_id: None,
                metric_ids: Vec::new(),
                focus: None,
            },
        ]);
        assert_eq!(requests[0].priority.as_deref(), Some("critical"));
        assert_eq!(requests[1].priority.as_deref(), Some("deferred"));
    }

    #[test]
    fn custom_entry_main_becomes_default_focus() {
        let workspace_root = temp_workspace_root("custom-main");
        let app_root = workspace_root.join("demo");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::write(
            app_root.join("main.mei"),
            "app(id=\"demo\")\nscene(id=\"home\", target=\"home.mei\")\n",
        )
        .expect("write main");
        fs::write(app_root.join("home.mei"), "frame()").expect("write scene");
        let mut mei_config = MeiConfig::default();
        mei_config.entry = AppEntryConfig {
            main: "custom.mei".to_string(),
        };
        write_mei_config(&app_mei_config_path(&app_root), &mei_config).expect("write mei config");
        fs::write(
            workspace_root.join(".mei-workspace.json"),
            r#"{"warmup":{"apps":{"demo":{}}}}"#,
        )
        .expect("write workspace config");

        let manifest = build_runtime_warmup_manifest(&workspace_root).expect("build manifest");
        assert_eq!(manifest.apps[0].focuses, vec!["custom.mei".to_string()]);

        let _ = fs::remove_dir_all(workspace_root);
    }
}
