//! Layout budget resolver and strict layout policy validation (0325).

mod padding;
mod resolve;
mod validate;

pub use padding::padding_profile_css;
pub use resolve::resolve_layout_budgets;
pub use validate::{
    materialize_fill_section_derived_heights, materialize_layout_budget_px,
    validate_layout_budget_policy, validate_layout_budget_policy_with_options,
    LayoutBudgetValidateOptions,
};

#[cfg(test)]
#[path = "layout_policy_strict.rs"]
mod layout_policy_strict;
