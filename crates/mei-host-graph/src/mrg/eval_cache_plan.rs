use std::collections::BTreeSet;

use mei_lang_datasets::EvalCacheInvalidationPlan;
use mei_lang_kernel::resolve_app_root;

use crate::mrg::registry::MrgRegistry;
use crate::types::MaterialState;

pub fn build_eval_cache_invalidation_plan(
    workspace_root: &std::path::Path,
    app_id: &str,
    force_clear: bool,
) -> EvalCacheInvalidationPlan {
    if force_clear {
        return EvalCacheInvalidationPlan {
            force_clear: true,
            ..Default::default()
        };
    }
    let registry = crate::mrg::registry::MrgRegistryWriter::load(workspace_root, app_id);
    build_eval_cache_invalidation_plan_from_registry(&registry)
}

pub fn build_eval_cache_invalidation_plan_from_registry(
    registry: &MrgRegistry,
) -> EvalCacheInvalidationPlan {
    let mut allowed_response_cache_keys = BTreeSet::new();
    let mut stale_bootstrap_scopes = BTreeSet::new();
    for slot in &registry.slots {
        if slot.client_eligible && matches!(slot.state, MaterialState::Stale) {
            stale_bootstrap_scopes.insert(slot.slot_id.scope_key.clone());
        }
        if !matches!(slot.state, MaterialState::Ready) {
            continue;
        }
        if let Some(payload_ref) = slot.payload_ref.as_ref() {
            if payload_ref.kind == "metric_response" {
                allowed_response_cache_keys.insert(payload_ref.content_hash.clone());
            }
        }
    }
    EvalCacheInvalidationPlan {
        force_clear: false,
        allowed_response_cache_keys,
        stale_bootstrap_scopes,
    }
}

pub fn invalidate_app_eval_cache(
    workspace_root: &std::path::Path,
    app_id: &str,
    force_clear: bool,
) -> anyhow::Result<mei_lang_datasets::EvalCacheInvalidationReport> {
    let app_root = resolve_app_root(workspace_root, app_id);
    let plan = build_eval_cache_invalidation_plan(workspace_root, app_id, force_clear);
    mei_lang_datasets::invalidate_stale_eval_artifacts(app_root.as_path(), &plan)
}
