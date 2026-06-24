//! Build-view context export API (Markdown brief for external IDE agents).

mod context_export;
mod graph_markdown;
mod graph_registry;
mod workspace_fragment;

pub use context_export::api_build_context_export;
pub use graph_registry::{
    api_build_graph_bridge, api_build_graph_mcg, api_build_graph_mrg,
};
pub use workspace_fragment::api_build_workspace_fragment;
