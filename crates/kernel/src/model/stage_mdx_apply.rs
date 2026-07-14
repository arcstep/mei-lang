//! Phase 4: apply Cockpit Stage MDX fills / narration onto CompiledApp ABI.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::diagnostic::{Diagnostic, Severity};
use super::narration_abi::{
    NarrationCatalog, NarrationCue, NarrationCueTarget, NarrationTrack,
};
use super::compile_out::CompiledApp;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CockpitStageDecl {
    pub stage_id: String,
    pub scene_use: String,
    pub source_anchor: String,
    pub fills: Vec<CockpitFillDecl>,
    pub steps: Vec<CockpitStepDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CockpitFillDecl {
    pub slot: String,
    pub content: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CockpitStepDecl {
    pub id: String,
    pub target: String,
    pub caption: Option<String>,
    pub speaker_notes: Option<String>,
    pub line: usize,
}

/// Validate fills against projected Slot/Capability ABI and merge authored NarrationCatalog.
pub fn apply_cockpit_stage_decl(compiled: &mut CompiledApp, decl: &CockpitStageDecl) {
    let module_key = format!("scene:{}", decl.stage_id);
    let mut diagnostics = Vec::new();

    for fill in &decl.fills {
        let Some(module) = compiled.scene_slot_modules.get(&module_key) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "stage_mdx_slot_unknown".to_string(),
                message: format!(
                    "stage `{}` has no SceneSlotModule; cannot fill slot `{}`",
                    decl.stage_id, fill.slot
                ),
                source_path: Some(decl.source_anchor.clone()),
            });
            continue;
        };
        let Some(slot) = module.get_slot(&fill.slot) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "stage_mdx_slot_unknown".to_string(),
                message: format!(
                    "@fill slot `{}` is not in public Slot ABI for stage `{}`",
                    fill.slot, decl.stage_id
                ),
                source_path: Some(format!("{}:{}", decl.source_anchor, fill.line)),
            });
            continue;
        };
        if !slot.accepted_capability_ids.contains(&fill.content)
            && !compiled.content_capabilities.contains_key(&fill.content)
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "stage_mdx_capability_mismatch".to_string(),
                message: format!(
                    "@fill content `{}` is not accepted by slot `{}`",
                    fill.content, fill.slot
                ),
                source_path: Some(format!("{}:{}", decl.source_anchor, fill.line)),
            });
        }
    }

    // Narration: authored steps replace empty projection; do not synthesize when empty.
    if !decl.steps.is_empty() {
        let mut public_targets: std::collections::BTreeSet<String> = compiled
            .scene_slot_modules
            .get(&module_key)
            .map(|m| m.slots.iter().map(|s| s.slot_id.clone()).collect())
            .unwrap_or_default();
        for id in compiled.content_capabilities.keys() {
            public_targets.insert(id.clone());
        }
        let mut cues = Vec::new();
        for step in &decl.steps {
            if !public_targets.contains(&step.target) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "narration_target_invalid".to_string(),
                    message: format!(
                        "narration step `{}` target `{}` is not a public Slot/Capability",
                        step.id, step.target
                    ),
                    source_path: Some(format!("{}:{}", decl.source_anchor, step.line)),
                });
                continue;
            }
            cues.push(NarrationCue {
                id: step.id.clone(),
                target: NarrationCueTarget::Slot(step.target.clone()),
                caption: step.caption.clone(),
                speaker_notes: step.speaker_notes.clone(),
                actions: Vec::new(),
                timing_ms: None,
                source_anchor: format!("{}:{}", decl.source_anchor, step.line),
            });
        }
        let narr_key = format!("narration:{}", decl.stage_id);
        compiled.narration_catalogs.insert(
            narr_key.clone(),
            NarrationCatalog {
                catalog_id: narr_key.clone(),
                tracks: if cues.is_empty() {
                    Vec::new()
                } else {
                    vec![NarrationTrack {
                        id: format!("{narr_key}:default"),
                        cues,
                        profile: Some("cockpit".to_string()),
                    }]
                },
                source_anchor: Some(decl.source_anchor.clone()),
            },
        );
        if let Some(program) = compiled.stage_programs.programs.get_mut(&decl.stage_id) {
            program.narration_ref = Some(narr_key.clone());
            let mut one = BTreeMap::new();
            if let Some(cat) = compiled.narration_catalogs.get(&narr_key) {
                one.insert(narr_key, cat.clone());
            }
            program.narration_digest =
                Some(super::abi_project::compute_narration_digest(&one));
        }
    }

    // Record stage mdx source on program when present.
    if let Some(program) = compiled.stage_programs.programs.get_mut(&decl.stage_id) {
        if program.source_anchor.is_empty()
            || program.source_anchor.ends_with(".mei")
            || !program.source_anchor.contains(".stage.mdx")
        {
            // Prefer Stage MDX as structure/narration authoring anchor when Native.
            program.source_anchor = decl.source_anchor.clone();
        }
        program.slot_module_ref = Some(module_key);
    }

    compiled.diagnostics.extend(diagnostics);
}
