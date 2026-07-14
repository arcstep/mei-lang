//! Phase 5: serialize Stage Registry / Programs / Narration for client bootstrap.

use mei_lang_kernel::CompiledApp;
use serde_json::{json, Value};

/// Slim Stage Registry payload for `__mei.stage_registry`.
pub fn stage_registry_bootstrap(compiled: &CompiledApp) -> Value {
    let stages: Vec<Value> = compiled
        .stage_registry
        .stages
        .iter()
        .map(|s| {
            json!({
                "stage_id": s.id.as_str(),
                "profile": s.profile.as_str(),
                "surface": mei_lang_kernel::StageSurface::from_profile(s.profile).as_str(),
                "title": s.title,
                "is_default": s.is_default,
                "legacy_scene_id": s.legacy_scene_id,
                "source_anchor": s.source_anchor.replace('\\', "/"),
            })
        })
        .collect();
    json!({
        "stages": stages,
        "default_stage_id": compiled
            .stage_registry
            .default_stage_id
            .as_ref()
            .map(|id| id.as_str().to_string()),
    })
}

/// Slim StageProgram index for `__mei.stage_programs`.
pub fn stage_programs_bootstrap(compiled: &CompiledApp) -> Value {
    let mut map = serde_json::Map::new();
    for (id, program) in &compiled.stage_programs.programs {
        map.insert(
            id.clone(),
            json!({
                "stage_id": program.stage_id.as_str(),
                "profile": program.profile.as_str(),
                "surface": program.surface.as_str(),
                "unit_count": program.units.len(),
                "unit_ids": program.unit_ids(),
                "state_namespace": program.state_namespace,
                "narration_ref": program.narration_ref,
                "source_anchor": program.source_anchor.replace('\\', "/"),
            }),
        );
    }
    Value::Object(map)
}

/// Narration catalogs for `__mei.narration_catalogs` (full cue payloads for FAB).
pub fn narration_catalogs_bootstrap(compiled: &CompiledApp) -> Value {
    serde_json::to_value(&compiled.narration_catalogs).unwrap_or_else(|_| json!({}))
}
