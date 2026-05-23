//! `decl(base = *_ref(...))` 克隆与字段级覆盖归一。

mod merge;
mod normalize;
mod refs;

pub(crate) use merge::deep_merge_json;
pub(crate) use normalize::{
    collect_ref_scene_files, normalize_flow_decl, normalize_scene_value, resolve_entity_slot,
    resolve_resource_slot,
};
pub(crate) use refs::{normalize_frame_decl, normalize_world_decl, resolve_panel_slot};
