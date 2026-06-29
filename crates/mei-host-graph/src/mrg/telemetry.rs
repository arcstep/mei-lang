use std::path::Path;
use std::sync::Mutex;

use mei_lang_kernel::resolve_app_root;

use crate::mrg::client_bootstrap::client_bootstrap_root;
use crate::mrg::registry::{MrgRegistryWriter, MrgTelemetrySummary};

static TELEMETRY: Mutex<MrgTelemetrySummary> = Mutex::new(MrgTelemetrySummary {
    assemble_count: 0,
    metrics_api_count: 0,
    cache_hits: 0,
    cache_misses: 0,
});

#[derive(Debug, Clone, Copy)]
pub enum MrgAccessKind {
    Assemble,
    MetricsApi,
}

pub fn record_access(kind: MrgAccessKind, cache_hit: bool) {
    let Ok(mut summary) = TELEMETRY.lock() else {
        return;
    };
    match kind {
        MrgAccessKind::Assemble => summary.assemble_count += 1,
        MrgAccessKind::MetricsApi => summary.metrics_api_count += 1,
    };
    if cache_hit {
        summary.cache_hits += 1;
    } else {
        summary.cache_misses += 1;
    }
}

pub fn flush_telemetry_to_registry(source_root: &Path, app_id: &str) -> anyhow::Result<()> {
    let snapshot = TELEMETRY
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    registry.telemetry_summary = Some(snapshot);
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)
}

pub fn mrg_status_json(source_root: &Path, app_id: &str) -> anyhow::Result<serde_json::Value> {
    let registry = MrgRegistryWriter::load(source_root, app_id);
    let (disk_ready, memory_resident, client_eligible) = registry.tier_counts();
    let app_root = resolve_app_root(source_root, app_id);
    let bootstrap_root = client_bootstrap_root(app_root.as_path());
    let mut bootstrap_scopes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&bootstrap_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            if let Some(scope) = path.file_stem().and_then(|stem| stem.to_str()) {
                bootstrap_scopes.push(scope.to_string());
            }
        }
    }
    bootstrap_scopes.sort();
    let mut scope_counts = std::collections::BTreeMap::<String, usize>::new();
    for slot in &registry.slots {
        *scope_counts
            .entry(slot.slot_id.scope_key.clone())
            .or_insert(0) += 1;
    }
    let mut hot_scopes: Vec<_> = scope_counts.into_iter().collect();
    hot_scopes.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(serde_json::json!({
        "appId": registry.app_id,
        "schemaVersion": registry.schema_version,
        "slotCount": registry.slots.len(),
        "slotsByTier": {
            "diskReady": disk_ready,
            "memoryResident": memory_resident,
            "clientEligible": client_eligible,
        },
        "hotScopes": hot_scopes.into_iter().take(10).map(|(scope, count)| {
            serde_json::json!({ "scope": scope, "slots": count })
        }).collect::<Vec<_>>(),
        "telemetry": registry.telemetry_summary,
        "edgeCount": registry.edges.len(),
        "bootstrapManifestCount": bootstrap_scopes.len(),
        "bootstrapScopes": bootstrap_scopes,
    }))
}
