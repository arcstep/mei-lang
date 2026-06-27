use std::collections::BTreeMap;

use super::scope::ScopeArtifactPlan;
use super::warmup::PreparedCompileOutcome;

/// Group artifact eval pairs by dataset owner so one owner hydrates once per batch.
pub(crate) fn group_artifact_pairs_by_owner<T>(
    pairs: &[(PreparedCompileOutcome, T)],
) -> BTreeMap<String, Vec<usize>>
where
    T: OwnerBatchPlanView,
{
    let mut grouped = BTreeMap::<String, Vec<usize>>::new();
    for (idx, (_prepared, plan)) in pairs.iter().enumerate() {
        let owner = plan.primary_owner_resource_id();
        grouped.entry(owner).or_default().push(idx);
    }
    grouped
}

pub(crate) trait OwnerBatchPlanView {
    fn primary_owner_resource_id(&self) -> String;
}

impl OwnerBatchPlanView for ScopeArtifactPlan {
    fn primary_owner_resource_id(&self) -> String {
        self.metric_worksets
            .first()
            .map(|plan| plan.owner_resource_id.clone())
            .or_else(|| {
                self.dataframe_artifacts
                    .first()
                    .map(|plan| plan.owner_resource_id.clone())
            })
            .unwrap_or_else(|| "__no_owner__".to_string())
    }
}

/// Sort pairs so all plans sharing an owner run contiguously (improves dataset pool locality).
pub(crate) fn order_artifact_pairs_by_owner<T: OwnerBatchPlanView>(
    pairs: &mut [(PreparedCompileOutcome, T)],
) {
    pairs.sort_by_cached_key(|(_prepared, plan)| plan.primary_owner_resource_id());
}
