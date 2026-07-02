mod agent_export;
mod index;
mod walker;

#[cfg(test)]
mod tests;

pub use agent_export::{format_ui_scope_agent_context, resolve_build_preview_scope, ui_scope_annotation_for_preview_path};
pub use index::{
    build_ui_layout_index, filter_roots_for_tree_mode, merge_ui_structure_root,
};
