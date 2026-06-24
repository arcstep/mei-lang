use mei_lang_kernel::CompiledApp;
use serde::{Deserialize, Serialize};

/// Explicit assembly inputs for AssemblyView (CompiledApp) derivation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyInputRecord {
    pub kind: String,
    pub key: String,
    pub revision: String,
}

#[derive(Debug, Clone, Default)]
pub struct AssemblyViewInputs {
    pub scene_payload: Option<AssemblyInputRecord>,
    pub metric_def_bundles: Vec<AssemblyInputRecord>,
    pub panel_contracts: Vec<AssemblyInputRecord>,
}

/// `assemble_assembly_view` is the explicit API boundary for AssemblyView derivation.
/// Today the kernel `finish_compiled_app` already performs this merge; callers pass
/// the resulting `CompiledApp` plus input revisions for registry metadata.
pub fn assemble_assembly_view(
    compiled: CompiledApp,
    inputs: AssemblyViewInputs,
) -> (CompiledApp, Vec<AssemblyInputRecord>) {
    let mut assembly_inputs = Vec::new();
    if let Some(scene) = inputs.scene_payload {
        assembly_inputs.push(scene);
    }
    assembly_inputs.extend(inputs.metric_def_bundles);
    assembly_inputs.extend(inputs.panel_contracts);
    (compiled, assembly_inputs)
}

pub fn assembly_view_revision(inputs: &[AssemblyInputRecord]) -> String {
    use crate::graph::types::stable_hash;
    let mut parts = inputs
        .iter()
        .map(|input| format!("{}:{}={}", input.kind, input.key, input.revision))
        .collect::<Vec<_>>();
    parts.sort();
    format!("av:{}", stable_hash(&parts.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembly_view_revision_stable() {
        let inputs = vec![AssemblyInputRecord {
            kind: "metric_def_bundle".to_string(),
            key: "ds1".to_string(),
            revision: "mdb:abc".to_string(),
        }];
        let rev = assembly_view_revision(&inputs);
        assert!(rev.starts_with("av:"));
    }
}
