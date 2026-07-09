//! Build-view context export API (Markdown brief for external IDE agents).

mod context_export;
mod graph_markdown;
mod graph_registry;
mod mcg_resource;
mod content_panel;
mod panel_lookup;
mod panel_render;
mod runtime_snapshot;

pub use context_export::api_build_context_export;
pub use graph_registry::{
    api_build_graph_bridge, api_build_graph_mcg, api_build_graph_mrg,
};
pub use mcg_resource::{api_build_graph_mcg_artifact, api_build_graph_mcg_node};
pub use content_panel::api_build_content_panel;
pub use panel_render::api_build_panel_render;
pub use runtime_snapshot::api_runtime_snapshot;
