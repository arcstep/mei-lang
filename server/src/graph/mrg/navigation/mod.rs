pub mod resolver;
pub mod sync;
pub mod types;

pub use resolver::{
    list_navigation_entries, match_request_to_navigation, mrg_nav_gate_enabled,
    resolve_default_scope,
};
pub use sync::sync_navigation_registry;
pub use types::NavigationMatch;
