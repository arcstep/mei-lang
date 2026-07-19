//! Apply structural Cockpit Stage MDX fills onto CompiledApp ABI.

use serde::{Deserialize, Serialize};

use super::compile_out::CompiledApp;
use super::diagnostic::{Diagnostic, Severity};

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

/// Validate fills against projected Slot/Capability ABI.
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
