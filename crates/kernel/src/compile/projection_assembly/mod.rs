mod metric;
mod panel;

pub use panel::enrich_runtime_board_assembly_projection_slots;
pub(crate) use panel::{
    enrich_scene_projection_assembly_preview, lower_scene_links_in_panels,
    scene_shell_contract_from_scene_contract,
};
