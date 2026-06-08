use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{
    load_workspace_config, resolve_app_entry_main, resolve_app_root, workspace_config_path,
    WorkspaceConfig, DEFAULT_APP_ENTRY_MAIN,
};
use mei_lang_kernel::{CompileOptions, Severity};
use serde_json::Value;
use walkdir::{DirEntry, WalkDir};

use crate::compile_report::compile_report;
use crate::compile_service::inspect_source_layout;
use crate::semantic_summary::summarize_compiled_app_semantics;
use crate::types::{DiagnosticCountSummary, WorkspaceAppSummary, WorkspaceSummary};

fn should_skip_dir(entry: &DirEntry, skip_directories: &[String]) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    if name == ".git" || name == ".mei" || name == ".stock" || name == "node_modules" {
        return true;
    }
    if name.starts_with('.') {
        return true;
    }
    skip_directories.iter().any(|item| item == name.as_ref())
}

fn menu_group_ids(menu: &Value) -> Vec<String> {
    menu.get("groups")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("id").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn discover_app_ids(source_root: &Path, config: &WorkspaceConfig) -> Vec<String> {
    let skip_directories = config.discover_skip_directories();
    let mut app_ids = Vec::new();
    let walker = WalkDir::new(source_root)
        .into_iter()
        .filter_entry(|entry| !should_skip_dir(entry, &skip_directories));

    for entry in walker.flatten().filter(|entry| entry.file_type().is_file()) {
        if entry.file_name() != DEFAULT_APP_ENTRY_MAIN {
            continue;
        }
        let Some(app_root) = entry.path().parent() else {
            continue;
        };
        if let Ok(relative) = app_root.strip_prefix(source_root) {
            let app_id = relative.to_string_lossy().replace('\\', "/");
            if !app_id.is_empty() {
                app_ids.push(app_id);
            }
        }
    }
    app_ids.sort();
    app_ids.dedup();
    app_ids
}

fn summarize_compile_diagnostics(
    diagnostics: &[mei_lang_kernel::Diagnostic],
) -> DiagnosticCountSummary {
    let mut summary = DiagnosticCountSummary::default();
    for item in diagnostics {
        match item.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warning => summary.warnings += 1,
            Severity::Info => summary.infos += 1,
        }
    }
    summary
}

pub fn build_workspace_summary(source_root: &Path) -> Result<WorkspaceSummary> {
    let config_path = workspace_config_path(source_root);
    let config = load_workspace_config(source_root);
    let app_ids = discover_app_ids(source_root, &config);
    let mut apps = Vec::new();
    let mut healthy_app_count = 0usize;

    for app_id in app_ids {
        let layout = inspect_source_layout(source_root, &app_id);
        if layout.ok {
            healthy_app_count += 1;
        }
        let mut error_count = 0usize;
        let mut warning_count = 0usize;
        let mut info_count = 0usize;
        for item in &layout.checks {
            match item.level.as_str() {
                "error" => error_count += 1,
                "warning" => warning_count += 1,
                _ => info_count += 1,
            }
        }
        let app_root = resolve_app_root(source_root, &app_id);
        let compile = compile_report(source_root, &app_id, CompileOptions::default());
        let (
            title,
            active_scene,
            default_scene_id,
            app_kind,
            scene_profile,
            semantic_hint,
            business_explanation,
            semantic_tags,
            compile_error,
            route_count,
            loaded_resource_count,
            dataset_resource_count,
            component_asset_count,
            compile_diagnostics,
        ) = match compile {
            Ok(report) => {
                let compiled = report.compiled;
                let default_scene_id = compiled
                    .scene_routes
                    .iter()
                    .find(|route| route.is_default)
                    .map(|route| route.scene_id.clone());
                let semantic = summarize_compiled_app_semantics(&compiled);
                let compile_diagnostics = summarize_compile_diagnostics(&compiled.diagnostics);
                let semantic_hint = Some(format!(
                    "kind=`{}` title=`{}` active_scene=`{}` tags={}",
                    semantic.app_kind,
                    compiled.title,
                    compiled.active_scene.as_deref().unwrap_or("-"),
                    semantic.semantic_tags.join(", ")
                ));
                (
                    Some(compiled.title),
                    compiled.active_scene,
                    default_scene_id,
                    Some(semantic.app_kind),
                    semantic.scene_profile,
                    semantic_hint,
                    Some(semantic.business_explanation),
                    semantic.semantic_tags,
                    None,
                    Some(semantic.route_count),
                    Some(semantic.loaded_resource_count),
                    Some(semantic.dataset_resource_count),
                    Some(semantic.component_asset_count),
                    Some(compile_diagnostics),
                )
            }
            Err(error) => (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Vec::new(),
                Some(error.to_string()),
                None,
                None,
                None,
                None,
                None,
            ),
        };
        apps.push(WorkspaceAppSummary {
            app_id: app_id.clone(),
            app_root: app_root.display().to_string(),
            entry_main: resolve_app_entry_main(&app_root),
            layout_ok: layout.ok,
            error_count,
            warning_count,
            info_count,
            title,
            active_scene,
            default_scene_id,
            app_kind,
            scene_profile,
            semantic_hint,
            business_explanation,
            semantic_tags,
            compile_error,
            route_count,
            loaded_resource_count,
            dataset_resource_count,
            component_asset_count,
            compile_diagnostics,
        });
    }

    let app_count = apps.len();
    let app_aliases = config
        .discover
        .app_aliases
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let workspace_id = config.workspace.id.clone();
    let workspace_label = config.workspace.label.clone();
    let discover_skip_directories = config.discover_skip_directories();
    let menu_group_ids = menu_group_ids(&config.menu);
    let mut narrative = vec![format!("workspace_config: {}", config_path.display())];
    if let Some(id) = workspace_id.as_deref() {
        narrative.push(format!("workspace_id: {id}"));
    }
    if let Some(label) = workspace_label
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        narrative.push(format!("workspace_label: {label}"));
    }
    narrative.push(format!("apps: {healthy_app_count}/{app_count} layout_ok"));
    let compile_ready_count = apps
        .iter()
        .filter(|item| item.compile_error.is_none())
        .count();
    narrative.push(format!(
        "compile_ready_apps: {compile_ready_count}/{app_count}"
    ));
    let semantic_app_counts = apps.iter().filter_map(|item| item.app_kind.as_deref()).fold(
        BTreeMap::<String, usize>::new(),
        |mut acc, kind| {
            *acc.entry(kind.to_string()).or_insert(0) += 1;
            acc
        },
    );
    if !semantic_app_counts.is_empty() {
        let kinds = semantic_app_counts
            .iter()
            .map(|(kind, count)| format!("{kind}={count}"))
            .collect::<Vec<_>>();
        narrative.push(format!("app_kinds: {}", kinds.join(", ")));
    }
    if !app_aliases.is_empty() {
        let aliases = app_aliases
            .iter()
            .take(8)
            .map(|(key, value)| format!("{key}->{value}"))
            .collect::<Vec<_>>();
        narrative.push(format!("app_aliases: {}", aliases.join(", ")));
    }
    if !menu_group_ids.is_empty() {
        narrative.push(format!("menu_groups: {}", menu_group_ids.join(", ")));
    }

    Ok(WorkspaceSummary {
        source_root: source_root.display().to_string(),
        workspace_id,
        workspace_label,
        app_count,
        healthy_app_count,
        discover_skip_directories,
        app_aliases,
        menu_group_ids,
        narrative,
        apps,
    })
}
