mod artifacts;
mod dataset_binding;
mod graph_markdown;
mod types;

pub use graph_markdown::{
    build_eval_plan_markdown, format_eval_plan_markdown, format_semantic_graph_markdown,
};

pub use artifacts::{
    export_analysis_contracts, export_eval_plan, export_inventory_snapshot, export_runtime_trace,
    export_semantic_dag,
};
pub use types::{
    HeadlessArtifactEnvelope, HeadlessArtifactKind, HeadlessExportOptions,
    HEADLESS_EXPORT_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests {
    use super::dataset_binding::{normalize_filters, normalize_search, query_state};
    use std::collections::BTreeMap;

    #[test]
    fn normalize_filters_trims_and_drops_empty_values() {
        let mut filters = BTreeMap::new();
        filters.insert(" dept ".to_string(), " one ".to_string());
        filters.insert("empty".to_string(), "   ".to_string());
        let normalized = normalize_filters(&filters);
        assert_eq!(normalized.get("dept").map(String::as_str), Some("one"));
        assert!(!normalized.contains_key("empty"));
    }

    #[test]
    fn query_state_normalizes_top_level_search() {
        let state = query_state(&BTreeMap::new(), Some("  foo  "));
        assert_eq!(state.search.as_deref(), Some("foo"));
        assert_eq!(normalize_search(Some("   ")), None);
    }
}
