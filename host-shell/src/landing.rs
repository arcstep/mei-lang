use std::path::Path;

use mei_host_graph::McgRegistryWriter;
use mei_lang_kernel::{discover_apps, resolve_app_root, WorkspaceAppMeta};

pub fn app_has_prebuilt_access_entry(source_root: &Path, app_id: &str) -> bool {
    let app_root = resolve_app_root(source_root, app_id);
    let current = app_root.join("env/current");
    if !current.exists() && !current.is_symlink() {
        return false;
    }
    let registry = McgRegistryWriter::load(source_root, app_id);
    !registry.nodes.is_empty()
}

#[cfg(test)]
pub fn choose_default_app<'a>(
    source_root: &Path,
    apps: &'a [WorkspaceAppMeta],
) -> Option<&'a WorkspaceAppMeta> {
    use mei_lang_kernel::{load_workspace_config, resolve_app_id};

    let workspace = load_workspace_config(source_root);
    if let Some(preferred) = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let canonical = resolve_app_id(source_root, preferred);
        if let Some(app) = apps
            .iter()
            .find(|app| app.id == canonical || app.id == preferred)
        {
            if app_has_prebuilt_access_entry(source_root, app.id.as_str()) {
                return Some(app);
            }
        }
    }
    apps.iter()
        .find(|app| app_has_prebuilt_access_entry(source_root, app.id.as_str()))
}

pub fn build_discovered_app_summaries(shell: &crate::state::ShellState) -> Vec<serde_json::Value> {
    use serde_json::json;

    let workspace = shell.ctx.workspace_root.as_path();
    let discovered = discover_workspace_apps(workspace).unwrap_or_default();
    discovered
        .iter()
        .map(|app| {
            let materialized = app_has_prebuilt_access_entry(workspace, app.id.as_str());
            let access_ready = shell.data_plane_enabled && materialized;
            let is_default = shell.default_app() == Some(app.id.as_str());
            let has_plug_ds = shell.plug_ds_endpoint_for(app.id.as_str()).is_some();
            let phase = if !shell.data_plane_enabled {
                "disabled"
            } else if !materialized {
                "missing"
            } else if is_default && shell.warmed_up {
                "ready"
            } else if is_default {
                "bound"
            } else {
                "bound"
            };
            json!({
                "appId": app.id,
                "accessReady": access_ready,
                "hasRegistry": materialized,
                "hasPlugDs": has_plug_ds,
                "isDefault": is_default,
                "phase": phase,
            })
        })
        .collect()
}

pub fn discover_workspace_apps(source_root: &Path) -> anyhow::Result<Vec<WorkspaceAppMeta>> {
    discover_apps(source_root)
}

pub fn menu_label_for_app(
    topbar_menu: &mei_lang_app::TopbarMenuContext,
    app_id: &str,
) -> Option<String> {
    let from_root = topbar_menu.root.as_ref().and_then(|menu| {
        menu.items
            .iter()
            .find(|item| item.app_id == app_id)
            .and_then(|item| item.label.clone())
    });
    if from_root.is_some() {
        return from_root;
    }
    topbar_menu.by_segment.values().find_map(|menu| {
        menu.items
            .iter()
            .find(|item| item.app_id == app_id)
            .and_then(|item| item.label.clone())
    })
}

pub fn enrich_discovered_apps(
    apps: &[WorkspaceAppMeta],
    topbar_menu: &mei_lang_app::TopbarMenuContext,
) -> Vec<WorkspaceAppMeta> {
    apps.iter()
        .map(|app| {
            let mut enriched = app.clone();
            if let Some(label) = menu_label_for_app(topbar_menu, app.id.as_str()) {
                enriched.title = label;
            }
            enriched
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::WorkspaceAppMeta;

    #[test]
    fn choose_default_app_prefers_workspace_default_when_ready() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":1,"workspace":{"id":"test","defaultApp":"data-demo"}}"#,
        )
        .expect("workspace.json");
        let apps_dir = tmp.path().join("apps");
        std::fs::create_dir_all(apps_dir.join("data-demo")).expect("data-demo dir");
        std::fs::create_dir_all(apps_dir.join("mini-park")).expect("mini-park dir");
        std::fs::write(
            apps_dir.join("data-demo/app.config.json"),
            r#"{"schemaVersion":1,"app":{"id":"data-demo"}}"#,
        )
        .expect("data-demo config");
        std::fs::write(
            apps_dir.join("mini-park/app.config.json"),
            r#"{"schemaVersion":1,"app":{"id":"mini-park"}}"#,
        )
        .expect("mini-park config");
        let env_dir = tmp.path().join("apps/data-demo/env/WS-20260712.0");
        let mrg_dir = env_dir.join("build/registry");
        std::fs::create_dir_all(&mrg_dir).expect("registry dir");
        std::os::unix::fs::symlink(
            "WS-20260712.0",
            tmp.path().join("apps/data-demo/env/current"),
        )
        .expect("env/current");
        std::fs::write(
            mrg_dir.join("mcg-registry.json"),
            r#"{
  "schemaVersion": "mei-mcg-registry-v2",
  "appId": "data-demo",
  "registryRevision": "test-rev",
  "updatedAtMs": 1,
  "nodes": [
    {
      "id": { "kind": "app_skeleton", "key": "app_skeleton:data-demo" },
      "revision": "blk:test",
      "state": "ready",
      "layer": "import"
    }
  ]
}"#,
        )
        .expect("mcg");
        let apps = vec![
            WorkspaceAppMeta {
                id: "data-demo".to_string(),
                title: "data-demo".to_string(),
                root: apps_dir.join("data-demo").display().to_string(),
            },
            WorkspaceAppMeta {
                id: "mini-park".to_string(),
                title: "mini-park".to_string(),
                root: apps_dir.join("mini-park").display().to_string(),
            },
        ];
        let chosen = choose_default_app(tmp.path(), apps.as_slice()).expect("default app");
        assert_eq!(chosen.id, "data-demo");
    }
}
