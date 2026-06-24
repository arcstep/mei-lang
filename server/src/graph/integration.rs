//! Graph registry integration helpers.

use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::{CompiledApp, CompileOptions};

use crate::graph::feature::graph_registry_enabled;
use crate::graph::mcg::metric_def_bundle::DatasetRuntimePayloadView;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mcg::update::update_mcg_after_compile;

pub fn maybe_update_graph_after_compile(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    compiled: &CompiledApp,
    compile_revision: &str,
    dataset_runtime_payloads: &BTreeMap<String, DatasetRuntimePayloadView>,
) {
    if !graph_registry_enabled() {
        return;
    }
    let dependency_fingerprint = compile_revision.to_string();
    match update_mcg_after_compile(
        source_root,
        app_id,
        options,
        compiled,
        compile_revision,
        dependency_fingerprint.as_str(),
        dataset_runtime_payloads,
    ) {
        Ok(outcome) => {
            if let Some(rev) = outcome.scene_payload_revision.as_deref() {
                tracing::debug!(
                    app_id = %app_id,
                    scene_payload_revision = %rev,
                    "MCG scene payload updated"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                app_id = %app_id,
                error = %error,
                "failed to update MCG registry after compile"
            );
        }
    }
}

pub fn runtime_payloads_from_compiled(compiled: &CompiledApp) -> BTreeMap<String, DatasetRuntimePayloadView> {
    let mut payloads = BTreeMap::new();
    for resource in &compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset.runtime_metric_defs.is_empty() {
            continue;
        }
        payloads.insert(
            resource.id.clone(),
            DatasetRuntimePayloadView {
                runtime_metric_defs: dataset.runtime_metric_defs.clone(),
            },
        );
    }
    payloads
}

/// Skip scope_artifacts eval for owners whose MetricDefBundle revision is unchanged.
pub fn bundle_unchanged_owners(source_root: &Path, app_id: &str) -> BTreeMap<String, String> {
    if !graph_registry_enabled() {
        return BTreeMap::new();
    }
    McgRegistryWriter::load(source_root, app_id)
        .nodes
        .into_iter()
        .filter(|node| node.id.kind == crate::graph::types::GraphNodeKind::MetricDefBundle)
        .map(|node| (node.id.key, node.revision))
        .collect()
}

pub fn app_graph_fingerprint(source_root: &Path, app_id: &str) -> String {
    if !graph_registry_enabled() {
        return String::new();
    }
    let mcg = McgRegistryWriter::load(source_root, app_id);
    format!("mcg={}", mcg.registry_revision)
}
