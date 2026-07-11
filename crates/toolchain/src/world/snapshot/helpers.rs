use std::collections::BTreeMap;

use mei_lang_kernel::Severity;

use crate::types::{DiagnosticCountSummary, ResourceInventoryItem, WorldRuntimeBundle};

pub(super) fn top_kind_lines(counts: &BTreeMap<String, usize>, label: &str) -> Option<String> {
    if counts.is_empty() {
        return None;
    }
    let top = counts
        .iter()
        .take(4)
        .map(|(kind, count)| format!("{kind}={count}"))
        .collect::<Vec<_>>();
    Some(format!("{label}: {}", top.join(", ")))
}

pub(super) fn resource_inventory_map<'a>(
    items: &'a [ResourceInventoryItem],
) -> BTreeMap<&'a str, &'a ResourceInventoryItem> {
    let mut out = BTreeMap::new();
    for item in items {
        out.insert(item.id.as_str(), item);
    }
    out
}

pub(super) fn summarize_diagnostics(bundle: &WorldRuntimeBundle) -> DiagnosticCountSummary {
    let mut summary = DiagnosticCountSummary::default();
    for item in &bundle.compiled.diagnostics {
        match item.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warning => summary.warnings += 1,
            Severity::Info => summary.infos += 1,
        }
    }
    summary
}
