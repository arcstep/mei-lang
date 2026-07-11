//! Shared enrich pipeline for host-shell Build and server access assembly.

use std::path::Path;

use mei_lang_kernel::{
    build_ui_layout_index, load_mei_config_for_app, materialize_fill_section_derived_heights,
    materialize_layout_budget_px, resolve_app_root, validate_layout_budget_policy_with_options,
    CompiledApp, LayoutBudgetValidateOptions,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnrichCompiledScopeOptions {
    pub materialize_px: bool,
}

impl Default for EnrichCompiledScopeOptions {
    fn default() -> Self {
        Self {
            materialize_px: false,
        }
    }
}

pub fn enrich_compiled_scope(
    mut compiled: CompiledApp,
    workspace_root: &Path,
    app_id: &str,
    options: EnrichCompiledScopeOptions,
) -> CompiledApp {
    let app_root = resolve_app_root(workspace_root, app_id);
    let mei_config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    compiled.ui_layout_index = build_ui_layout_index(&compiled).index;
    let theme_id = compiled
        .scene_contract
        .as_ref()
        .and_then(|c| c.scene.theme.clone())
        .unwrap_or_else(|| "cockpit".to_string());
    crate::theme_layout_merge::merge_theme_layout_into_compiled(
        &mut compiled,
        theme_id.as_str(),
        &mei_config.ops.themes,
    );
    let layout_options = LayoutBudgetValidateOptions {
        strict_t1_fill_down: mei_config.ops.strict_fill_down,
        strict_t2_fill_down: mei_config.ops.fill_down,
    };
    if let Some(contract) = compiled.scene_contract.as_mut() {
        let source_path = compiled.active_target_file.as_str();
        if options.materialize_px {
            validate_layout_budget_policy_with_options(
                &mut contract.panels,
                &mut compiled.diagnostics,
                source_path,
                &layout_options,
            );
            materialize_layout_budget_px(
                &mut contract.panels,
                &mut compiled.diagnostics,
                source_path,
            );
        } else {
            validate_layout_budget_policy_with_options(
                &mut contract.panels,
                &mut compiled.diagnostics,
                source_path,
                &layout_options,
            );
            materialize_fill_section_derived_heights(
                &mut contract.panels,
                &mut compiled.diagnostics,
                source_path,
            );
        }
    }
    compiled.ui_layout_index = build_ui_layout_index(&compiled).index;
    compiled
}
