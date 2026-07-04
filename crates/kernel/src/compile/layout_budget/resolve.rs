use crate::model::{Diagnostic, PanelDecl};

/// Public entry: validate then resolve budgets on panel forest.
pub fn resolve_layout_budgets(
    panels: &mut [PanelDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    super::validate::emit_layout_budget_policy_diagnostics(panels, diagnostics, source_path);
}
