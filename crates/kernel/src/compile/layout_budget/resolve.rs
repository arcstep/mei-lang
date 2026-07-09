use crate::model::{Diagnostic, UiNodeDecl};

/// Public entry: validate then resolve budgets on panel forest.
pub fn resolve_layout_budgets(
    panels: &mut [UiNodeDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    super::validate::emit_layout_budget_policy_diagnostics(panels, diagnostics, source_path);
}
