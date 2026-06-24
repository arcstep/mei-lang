use crate::graph::types::stable_hash;

/// Slot revision = f(MetricDefBundle.rev, DataSource.rev, ScopeKey, engine).
/// Does not include app-level compile_revision (Phase 2).
pub fn compute_slot_revision(
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    scope_key: &str,
    eval_engine: &str,
) -> String {
    let body = format!(
        "mdb={metric_def_bundle_revision}\nds={data_source_revision}\nscope={scope_key}\nengine={eval_engine}"
    );
    format!("sr:{}", stable_hash(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_revision_stable() {
        let a = compute_slot_revision("mdb:abc", "ds:1", "default", "json_walk");
        let b = compute_slot_revision("mdb:abc", "ds:1", "default", "json_walk");
        assert_eq!(a, b);
        assert!(a.starts_with("sr:"));
    }
}
