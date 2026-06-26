use super::{build_explain_slots, build_root_metric_slot, lookup_metric_contract, parse_metric_ref_id};

use serde_json::{Map, Value};

use crate::model::{Diagnostic, Severity};

pub(crate) fn expand_drilldown_tabs(
    metric_ref: &Value,
    include_hero: bool,
    _default_slot: Option<usize>,
    resources: &[crate::model::LoadedResource],
    world_hint: Option<&Value>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<Vec<Map<String, Value>>> {
    let metric_id = parse_metric_ref_id(metric_ref)?;
    let (dataset_id, contract) =
        lookup_metric_contract(metric_id, resources, world_hint, diagnostics, target_file)?;

    let mut slots = Vec::new();
    if include_hero {
        slots.push(build_root_metric_slot(
            metric_id,
            &dataset_id,
            contract.as_ref(),
            "metric_card",
        ));
    }
    slots.extend(build_explain_slots(
        metric_id,
        &dataset_id,
        contract.as_ref(),
    ));

    if slots.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "projection_slots_empty".to_string(),
            message: format!("no projection slots for metric `{metric_id}`"),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }

    Some(slots)
}

