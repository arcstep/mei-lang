pub mod resolver;
pub mod sync;
pub mod types;

#[derive(Debug, Clone, Copy, Default)]
pub struct NavigationResolveOpts {
    pub silent: bool,
}

pub use resolver::{
    list_navigation_entries, match_request_to_navigation, match_request_to_navigation_with_opts,
    resolve_default_scope, resolve_default_scope_with_opts,
};
pub use sync::{sync_navigation_for_compile_scopes, sync_navigation_registry, CompileScopeNav};
pub use types::NavigationMatch;
