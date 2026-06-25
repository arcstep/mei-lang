//! Access-mode compiled app slimming: strip row/metric value snapshots redundant with parquet / MRG slots.
//! `world_metrics` ledger entries are kept — SSR metric cards resolve contracts from this ledger.

use mei_lang_kernel::{CompiledApp, DatasetView};

const ACCESS_SLIM_ARTIFACTS_ENV: &str = "MEI_ACCESS_SLIM_ARTIFACTS";

/// When true (default), prebuild/access paths write v4 slim artifacts without inline rows/metrics.
pub fn access_slim_artifacts_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| match std::env::var(ACCESS_SLIM_ARTIFACTS_ENV) {
        Ok(value) => {
            let trimmed = value.trim();
            !(trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
        }
        Err(_) => true,
    })
}

const CONTENT_STORE_ENV: &str = "MEI_CONTENT_STORE";

/// Phase G: when true (default), compiled_app per-scope blobs are superseded by scene_payload CAS.
pub fn content_store_preferred() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| match std::env::var(CONTENT_STORE_ENV) {
        Ok(value) => {
            let trimmed = value.trim();
            !(trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
        }
        Err(_) => true,
    })
}

const CANONICAL_ARTIFACT_PERSIST_ENV: &str = "MEI_CANONICAL_ARTIFACT_PERSIST";

/// When true (default), only persist compiled_app for canonical scopes (not board overlay scene+target).
pub fn canonical_artifact_persist_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| match std::env::var(CANONICAL_ARTIFACT_PERSIST_ENV) {
        Ok(value) => {
            let trimmed = value.trim();
            !(trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
        }
        Err(_) => true,
    })
}

pub fn slim_dataset_for_access(dataset: &mut DatasetView) {
    dataset.rows.clear();
    dataset.metrics.clear();
}

pub fn slim_compiled_app_for_access(compiled: &CompiledApp) -> CompiledApp {
    let mut slim = compiled.clone();
    for resource in &mut slim.resources {
        if let Some(dataset) = resource.dataset.as_mut() {
            slim_dataset_for_access(dataset);
        }
    }
    slim.build_experience_index = Default::default();
    slim.build_board_index = Default::default();
    slim.build_template_index = Default::default();
    slim
}

pub fn strip_loaded_compiled_app_for_access(compiled: &mut CompiledApp) {
    for resource in &mut compiled.resources {
        if let Some(dataset) = resource.dataset.as_mut() {
            slim_dataset_for_access(dataset);
        }
    }
}

/// Board overlay scopes (scene + board target) should not get their own compiled_app blob.
pub fn should_persist_compiled_app_artifact(
    scene: Option<&str>,
    preview_target: Option<&str>,
) -> bool {
    if !canonical_artifact_persist_enabled() {
        return true;
    }
    let has_scene = scene.map(str::trim).is_some_and(|value| !value.is_empty());
    let has_target = preview_target
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    !(has_scene && has_target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use mei_lang_kernel::{LoadedResource, MetricContract, MetricShape, SourceDecl};

    fn sample_dataset() -> DatasetView {
        DatasetView {
            id: "orders".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["amount".to_string()],
            rows: vec![serde_json::json!({"amount": 1})],
            source: SourceDecl {
                kind: "xlsx".to_string(),
                path: "data/orders.xlsx".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::from([(
                "orders_total".to_string(),
                MetricContract {
                    id: "orders_total".to_string(),
                    label: None,
                    unit: None,
                    value_format: None,
                    purpose: None,
                    shape: MetricShape::Scalar,
                    schema: Vec::new(),
                    dataset: None,
                    transforms: Vec::new(),
                    value: serde_json::json!(42),
                },
            )]),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        }
    }

    #[test]
    fn slim_compiled_app_clears_rows_and_metrics() {
        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: String::new(),
            app_root: String::new(),
            scene_routes: Vec::new(),
            active_scene: None,
            active_target_file: String::new(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: Default::default(),
            scene_bindings_by_id: Default::default(),
            scene_examples_by_id: Default::default(),
            scene_projection_assembly_by_id: Default::default(),
            resources: vec![LoadedResource {
                id: "orders".to_string(),
                kind: "dataset".to_string(),
                title: None,
                document: None,
                dataset: Some(sample_dataset()),
            }],
            world_metrics: BTreeMap::from([(
                "orders_total".to_string(),
                mei_lang_kernel::WorldMetricLedgerEntry {
                    id: "orders_total".to_string(),
                    owner_resource_id: "orders".to_string(),
                    order: 0,
                    metric: MetricContract {
                        id: "orders_total".to_string(),
                        label: None,
                        unit: None,
                        value_format: None,
                        purpose: None,
                        shape: MetricShape::Scalar,
                        schema: Vec::new(),
                        dataset: None,
                        transforms: Vec::new(),
                        value: serde_json::json!(1),
                    },
                },
            )]),
            world_semantic_by_file: Default::default(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        };
        let slim = slim_compiled_app_for_access(&compiled);
        let dataset = slim.resources[0].dataset.as_ref().expect("dataset");
        assert!(dataset.rows.is_empty());
        assert!(dataset.metrics.is_empty());
        assert!(!slim.world_metrics.is_empty());
    }

    #[test]
    fn board_overlay_scope_skips_compiled_app_persist() {
        assert!(!should_persist_compiled_app_artifact(
            Some("warnings_analytics_board"),
            Some("scenes/01-执法要素.board.mei")
        ));
        assert!(should_persist_compiled_app_artifact(
            None,
            Some("scenes/01-执法要素.board.mei")
        ));
        assert!(should_persist_compiled_app_artifact(
            Some("home"),
            None
        ));
    }
}
