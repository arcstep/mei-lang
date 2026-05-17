use std::collections::BTreeMap;

use crate::model::{Diagnostic, EntityDecl, FrameDecl, ResourceDecl, Severity, WorldGridDecl};

pub(super) fn apply_frame_mutations(
    frames: &mut BTreeMap<String, FrameDecl>,
    frame_default: &mut Option<FrameDecl>,
    layout: Option<crate::model::LayoutDecl>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    frame_decl_count: usize,
) {
    let Some(layout) = layout else {
        return;
    };
    match frame_decl_count {
        0 => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_frame_declaration".to_string(),
                message: "frame.set_layout(...) requires a frame(...) declaration in the same file"
                    .to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
        1 => {
            if let Some(frame_decl) = frame_default.as_mut() {
                frame_decl.layout = Some(layout);
                return;
            }
            if let Some((_id, frame_decl)) = frames.iter_mut().next() {
                frame_decl.layout = Some(layout);
            }
        }
        _ => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "ambiguous_frame_mutation".to_string(),
                message:
                    "frame.set_layout(...) requires exactly one frame(...) declaration in the file"
                        .to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
    }
}

pub(super) fn apply_world_mutations(
    worlds: &mut BTreeMap<String, crate::model::WorldDecl>,
    world_default: &mut Option<crate::model::WorldDecl>,
    resources: &[ResourceDecl],
    entities: &[EntityDecl],
    topology: Option<WorldGridDecl>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    world_decl_count: usize,
) {
    let has_mutations = !resources.is_empty() || !entities.is_empty() || topology.is_some();
    if !has_mutations {
        return;
    }
    match world_decl_count {
        0 => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_world_declaration".to_string(),
                message: "world.add_* / world.set_topology(...) requires a world(...) declaration in the same file".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
        1 => {
            if let Some(world_decl) = world_default.as_mut() {
                apply_world_mutations_to_decl(world_decl, resources, entities, topology);
                return;
            }
            if let Some((_id, world_decl)) = worlds.iter_mut().next() {
                apply_world_mutations_to_decl(world_decl, resources, entities, topology);
            }
        }
        _ => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "ambiguous_world_mutation".to_string(),
                message: "world.add_* / world.set_topology(...) requires exactly one world(...) declaration in the file".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
    }
}

pub(super) fn apply_world_mutations_to_decl(
    world_decl: &mut crate::model::WorldDecl,
    resources: &[ResourceDecl],
    entities: &[EntityDecl],
    topology: Option<WorldGridDecl>,
) {
    world_decl.resources.extend(resources.iter().cloned());
    world_decl.entities.extend(entities.iter().cloned());
    if let Some(topology) = topology {
        world_decl.topology = Some(topology);
    }
}
