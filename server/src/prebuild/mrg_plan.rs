use std::collections::BTreeSet;
use std::path::Path;

use super::PrebuildScopeProfile;
use super::warmup::PreparedCompileOutcome;

#[derive(Debug, Clone)]
pub(crate) struct MrgEvalFrontier {
    pub plan_source: &'static str,
    pub dirty_slot_count: usize,
    pub dirty_slot_keys: BTreeSet<String>,
    pub dirty_scope_keys: BTreeSet<String>,
}

/// Build MRG-driven eval frontier from dirty slots; manifest scopes remain compile-only input.
pub fn build_mrg_eval_frontier(
    source_root: &Path,
    app_id: &str,
    _scope_profile: PrebuildScopeProfile,
) -> MrgEvalFrontier {
    if !crate::graph::feature::graph_registry_dedup_enabled() {
        return MrgEvalFrontier {
            plan_source: "manifest_override",
            dirty_slot_count: 0,
            dirty_slot_keys: BTreeSet::new(),
            dirty_scope_keys: BTreeSet::new(),
        };
    }
    let registry = crate::graph::load_mrg_registry(source_root, app_id);
    let dirty = registry.dirty_slots();
    let dirty_slot_keys = dirty
        .iter()
        .map(|slot| slot.slot_id.node.key.clone())
        .collect::<BTreeSet<_>>();
    let dirty_scope_keys = dirty
        .iter()
        .map(|slot| slot.slot_id.scope_key.clone())
        .collect::<BTreeSet<_>>();
    let plan_source = if dirty.is_empty() {
        "manifest_override"
    } else {
        "mrg_dirty"
    };
    MrgEvalFrontier {
        plan_source,
        dirty_slot_count: dirty.len(),
        dirty_slot_keys,
        dirty_scope_keys,
    }
}

/// Reorder artifact plans so MRG-dirty scopes are evaluated first.
pub(crate) fn prioritize_artifact_plans_by_frontier<T>(
    plans: &mut [(PreparedCompileOutcome, T)],
    frontier: &MrgEvalFrontier,
) {
    if frontier.dirty_scope_keys.is_empty() {
        return;
    }
    plans.sort_by_key(|(prepared, _)| {
        let scope_key = crate::graph::mrg_eval_scope_key(
            prepared
                .scope
                .requested_scene_id
                .as_deref()
                .unwrap_or(""),
            prepared.scope.requested_target_file.as_deref(),
        );
        let scope_dirty = frontier.dirty_scope_keys.contains(scope_key.as_str());
        let slot_dirty = plan_has_dirty_slot(prepared, &frontier.dirty_slot_keys);
        !(scope_dirty || slot_dirty)
    });
}

pub(crate) fn retain_dirty_artifact_plans<T>(
    plans: &mut Vec<(PreparedCompileOutcome, T)>,
    frontier: &MrgEvalFrontier,
) {
    if frontier.dirty_slot_keys.is_empty() && frontier.dirty_scope_keys.is_empty() {
        plans.clear();
        return;
    }
    plans.retain(|(prepared, _)| {
        let scope_key = crate::graph::mrg_eval_scope_key(
            prepared
                .scope
                .requested_scene_id
                .as_deref()
                .unwrap_or(""),
            prepared.scope.requested_target_file.as_deref(),
        );
        frontier.dirty_scope_keys.contains(scope_key.as_str())
            || plan_has_dirty_slot(prepared, &frontier.dirty_slot_keys)
    });
}

fn plan_has_dirty_slot(prepared: &PreparedCompileOutcome, dirty_keys: &BTreeSet<String>) -> bool {
    dirty_keys.is_empty()
        || dirty_keys.iter().any(|key| {
            prepared
                .outcome
                .compiled
                .world_metrics
                .contains_key(key.as_str())
        })
}

/// Scoped build: retain only dirty worksets within a single scope plan.
pub(crate) fn retain_dirty_scope_plan(
    plan: &mut super::ScopeArtifactPlan,
    frontier: &MrgEvalFrontier,
) {
    if frontier.dirty_slot_keys.is_empty() {
        return;
    }
    plan.metric_worksets.retain(|workset| {
        frontier.dirty_slot_keys.contains(workset.logical_node_id.as_str())
            || frontier.dirty_slot_keys.contains(workset.owner_resource_id.as_str())
    });
    plan.dataframe_artifacts.retain(|artifact| {
        frontier.dirty_slot_keys.contains(artifact.logical_node_id.as_str())
            || frontier.dirty_slot_keys.contains(artifact.owner_resource_id.as_str())
    });
}

pub(crate) fn artifact_plan_matches_continue_target(
    prepared: &PreparedCompileOutcome,
    plan: &super::ScopeArtifactPlan,
    target: &str,
) -> bool {
    let scope_key = crate::graph::mrg_eval_scope_key(
        prepared
            .scope
            .requested_scene_id
            .as_deref()
            .unwrap_or(""),
        prepared.scope.requested_target_file.as_deref(),
    );
    if scope_key.contains(target) {
        return true;
    }
    plan.metric_worksets.iter().any(|workset| {
        workset.logical_node_id.contains(target)
            || workset.owner_resource_id.contains(target)
            || workset.materialization_key.contains(target)
            || workset.dataset_selector.contains(target)
    }) || plan.dataframe_artifacts.iter().any(|artifact| {
        artifact.logical_node_id.contains(target)
            || artifact.owner_resource_id.contains(target)
            || artifact.artifact_key.contains(target)
    })
}
