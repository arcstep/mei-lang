//! Layout budget resolver and strict layout policy validation (0325).

mod padding;
mod resolve;
mod validate;

pub use padding::padding_profile_css;
pub use resolve::resolve_layout_budgets;

#[cfg(test)]
#[path = "layout_policy_strict.rs"]
mod layout_policy_strict;
