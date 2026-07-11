use std::path::Path;

use serde_json::Value;

use crate::compile::load_external::load_panel_from_scene_file;
use crate::compile::panel_normalize::{
    seed_metric_block_vertical_align_from_base, seed_metric_desc_runtime_from_shell,
    seed_metric_slot_vertical_align_defaults_from_base,
};
use crate::model::{Diagnostic, Severity, UiNodeDecl};
use crate::typed_refs::{decode_ref_value, RefExpr, RefKind, SceneRegistry};

use super::super::super::normalize::normalize_ui_nodes;
use super::super::merge_decl::merge_panel_decl;
use crate::compile::entry_payload::import_scope::rewrite_panel_import_refs;

use super::path::{push_invalid_base_kind, resolve_ref_path};

pub(crate) fn resolve_panel_ref(
    app_root: &Path,
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<UiNodeDecl> {
    if expr.kind != RefKind::Panel {
        push_invalid_base_kind(diagnostics, target_file, "panel", RefKind::Panel, expr.kind);
        return None;
    }
    let panel_id = expr.id.as_deref().unwrap_or_default().trim();
    if panel_id.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_panel_ref".to_string(),
            message: "panel_ref(...) requires panel id".to_string(),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    let Some(path) = resolve_ref_path(
        expr,
        scene_registry,
        diagnostics,
        target_file,
        "panel_base_not_resolved",
    ) else {
        return None;
    };
    let panel = match load_panel_from_scene_file(app_root, path.as_str(), panel_id) {
        Ok(panel) => panel,
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "panel_base_not_resolved".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            });
            return None;
        }
    };
    let Some(panel_value) = serde_json::to_value(&panel).ok() else {
        return Some(panel);
    };
    if let Some(base_value) = panel_value.get("base").filter(|value| !value.is_null()) {
        let Some(base_expr) = decode_ref_value(base_value) else {
            return Some(panel);
        };
        let Some(base_panel) = resolve_panel_ref(
            app_root,
            &base_expr,
            scene_registry,
            diagnostics,
            target_file,
        ) else {
            return None;
        };
        let mut overlay = panel_value.clone();
        if let Some(obj) = overlay.as_object_mut() {
            if obj
                .get("blocks")
                .and_then(Value::as_array)
                .is_some_and(|blocks| blocks.is_empty())
            {
                obj.remove("blocks");
            }
            obj.remove("base");
        }
        if let Ok(mut overlay_panel) = serde_json::from_value::<UiNodeDecl>(overlay.clone()) {
            rewrite_panel_import_refs(&mut overlay_panel, &path);
            if let Ok(rewritten) = serde_json::to_value(&overlay_panel) {
                if let Ok(mut merged) = merge_panel_decl(base_panel.clone(), &rewritten) {
                    seed_metric_slot_vertical_align_defaults_from_base(
                        &base_panel,
                        &mut merged,
                        &rewritten,
                    );
                    seed_metric_desc_runtime_from_shell(&mut merged);
                    seed_metric_block_vertical_align_from_base(&base_panel, &mut merged);
                    merged.blocks = normalize_ui_nodes(
                        app_root,
                        &merged.blocks,
                        scene_registry,
                        diagnostics,
                        target_file,
                    );
                    merged.base = None;
                    merged.import_scope = Some(path.clone());
                    return Some(merged);
                }
            }
        }
        let mut merged = merge_panel_decl(base_panel.clone(), &overlay).ok()?;
        seed_metric_slot_vertical_align_defaults_from_base(&base_panel, &mut merged, &overlay);
        seed_metric_desc_runtime_from_shell(&mut merged);
        seed_metric_block_vertical_align_from_base(&base_panel, &mut merged);
        merged.blocks = normalize_ui_nodes(
            app_root,
            &merged.blocks,
            scene_registry,
            diagnostics,
            target_file,
        );
        merged.base = None;
        merged.import_scope = Some(path.clone());
        return Some(merged);
    }
    let mut panel = panel;
    rewrite_panel_import_refs(&mut panel, &path);
    panel.import_scope = Some(path);
    Some(panel)
}
