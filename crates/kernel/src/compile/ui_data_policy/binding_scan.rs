use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::eval::evaluate_mei_file;
use crate::model::{
    BlockDecl, Diagnostic, LoadedResource, PanelDecl, PanelRefEmbedDecl, Severity, UiNodeDecl,
};

use super::resource_refs::collect_resource_ref_issues;
use super::rules::collect_forbidden_paths;
use crate::compile::entry_payload::import_scope::{
    load_namespaced_capsule_resources, rewrite_panel_import_refs,
};

pub(super) fn validate_embed_capsule_ui_bindings(
    app_root: &Path,
    embed: &PanelRefEmbedDecl,
    _host_resources: &[LoadedResource],
    _target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = embed.scene_file.trim();
    if path.is_empty() {
        return;
    }
    let capsule_resources = load_namespaced_capsule_resources(app_root, path).unwrap_or_default();
    let (resource_ids, metric_ids) =
        crate::compile::entry_payload::import_scope::resource_and_metric_ids_for_scope(
            &capsule_resources,
            path,
        );
    let Ok(decls) = evaluate_mei_file(app_root.join(path)) else {
        return;
    };
    let Some(values) = decls.as_array() else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) == Some("panel") {
            if let Ok(mut panel) = serde_json::from_value::<PanelDecl>(value.clone()) {
                rewrite_panel_import_refs(&mut panel, path);
                scan_panel_props(
                    &panel,
                    &resource_ids,
                    &metric_ids,
                    &resource_ids,
                    &metric_ids,
                    path,
                    diagnostics,
                );
                for node in &panel.blocks {
                    scan_ui_node(
                        node,
                        &resource_ids,
                        &metric_ids,
                        &resource_ids,
                        &metric_ids,
                        path,
                        diagnostics,
                    );
                }
            }
        }
        if value.get("kind").and_then(Value::as_str) == Some("block") {
            if let Ok(block) = serde_json::from_value::<BlockDecl>(value.clone()) {
                push_violations(
                    &block.props,
                    &resource_ids,
                    &metric_ids,
                    &resource_ids,
                    &metric_ids,
                    &format!("block `{}` props", block.id.as_deref().unwrap_or("?")),
                    path,
                    diagnostics,
                );
            }
        }
    }
}

pub(super) fn scan_deprecated_embed_nodes(
    node: &UiNodeDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node {
        UiNodeDecl::Panel(panel) => {
            for child in &panel.blocks {
                scan_deprecated_embed_nodes(child, target_file, diagnostics);
            }
        }
        UiNodeDecl::PanelRefEmbed(embed) => {
            if let Some(legacy) = embed.compat_source.as_deref() {
                let code = match legacy {
                    "frame_ref" => "deprecated_frame_ref_block_embed",
                    "panel_capsule_ref" => "deprecated_panel_capsule_ref",
                    _ => "deprecated_panel_ref_embed",
                };
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: code.to_string(),
                    message: format!(
                        "legacy {legacy} block embed `{}` is removed; use frame.panels panel_ref(id = ..., scene_file = ...)",
                        embed.scene_file
                    ),
                    source_path: Some(target_file.to_string()),
                });
            }
        }
        UiNodeDecl::Block(_) => {}
    }
}

pub(super) fn scan_panel_props(
    panel: &PanelDecl,
    host_resource_ids: &BTreeSet<String>,
    host_metric_ids: &BTreeSet<String>,
    merged_resource_ids: &BTreeSet<String>,
    merged_metric_ids: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    push_violations(
        &panel.props,
        host_resource_ids,
        host_metric_ids,
        merged_resource_ids,
        merged_metric_ids,
        &format!("panel `{}` props", panel.id),
        target_file,
        diagnostics,
    );
}

pub(super) fn scan_ui_node(
    node: &UiNodeDecl,
    host_resource_ids: &BTreeSet<String>,
    host_metric_ids: &BTreeSet<String>,
    merged_resource_ids: &BTreeSet<String>,
    merged_metric_ids: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node {
        UiNodeDecl::Panel(panel) => {
            scan_panel_props(
                panel,
                host_resource_ids,
                host_metric_ids,
                merged_resource_ids,
                merged_metric_ids,
                target_file,
                diagnostics,
            );
            for child in &panel.blocks {
                scan_ui_node(
                    child,
                    host_resource_ids,
                    host_metric_ids,
                    merged_resource_ids,
                    merged_metric_ids,
                    target_file,
                    diagnostics,
                );
            }
        }
        UiNodeDecl::Block(block) => {
            push_violations(
                &block.props,
                host_resource_ids,
                host_metric_ids,
                merged_resource_ids,
                merged_metric_ids,
                &format!(
                    "block `{}` (use `{}`) props",
                    block.id.as_deref().unwrap_or("?"),
                    block.use_key
                ),
                target_file,
                diagnostics,
            );
        }
        UiNodeDecl::PanelRefEmbed(_) => {}
    }
}

pub(super) fn push_violations(
    value: &Value,
    host_resource_ids: &BTreeSet<String>,
    host_metric_ids: &BTreeSet<String>,
    merged_resource_ids: &BTreeSet<String>,
    merged_metric_ids: &BTreeSet<String>,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let paths = collect_forbidden_paths(value, "$");
    for path in paths {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "forbidden_direct_ui_data_binding".to_string(),
            message: format!(
                "{context}：在 `{path}` 发现禁止的数据直连（`ds.data_ref` / `__ref:\"data\"` / `analysis_expr` rows）；请改为 `dataset_ref(id=...)` / `resource_ref(id=...)`，并确保该 id 已在当前 scene world 资源账本中"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    let ref_issues = collect_resource_ref_issues(
        value,
        "$",
        host_resource_ids,
        host_metric_ids,
        merged_resource_ids,
        merged_metric_ids,
    );
    for (path, code, message) in ref_issues {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code,
            message: format!("{context}：在 `{path}` {message}"),
            source_path: Some(target_file.to_string()),
        });
    }
}
