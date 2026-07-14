use crate::model::{Diagnostic, UiNodeDecl};

use super::validate::LayoutBudgetValidateOptions;

/// Public entry: validate then resolve budgets on panel forest (cockpit-strict default).
pub fn resolve_layout_budgets(
    panels: &mut [UiNodeDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    resolve_layout_budgets_with_options(
        panels,
        diagnostics,
        source_path,
        &LayoutBudgetValidateOptions::default(),
    );
}

/// Profile-aware entry used by compile/enrich (Phase 6).
pub fn resolve_layout_budgets_with_options(
    panels: &mut [UiNodeDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
    options: &LayoutBudgetValidateOptions,
) {
    super::validate::validate_layout_budget_policy_with_options(
        panels,
        diagnostics,
        source_path,
        options,
    );
    super::validate::materialize_layout_budget_px(panels, diagnostics, source_path);
}
