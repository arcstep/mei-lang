//! Integration tests for MCG/MRG graph registry (Phase 1).

#[cfg(test)]
mod graph_mcg_tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::graph::feature::graph_registry_enabled;
    use crate::graph::mcg::assemble::{page_instance_revision, AssemblyInputRecord};
    use crate::graph::mcg::metric_def_bundle::metric_defs_fingerprint;
    use crate::graph::mcg::scene_payload::scene_payload_revision;
    use crate::graph::mrg::invalidation::{apply_mcg_invalidation, changed_bundle_owners};
    use crate::graph::mrg::registry::{MrgRegistry, MrgSlotId, MrgSlotRecord};
    use crate::graph::mrg::slot_revision::compute_slot_revision;
    use crate::graph::types::{GraphNodeId, GraphNodeKind, MaterialState};

    #[test]
    fn discover_world_metrics_owner_ids_normalizes_src_prefix() {
        use std::path::Path;

        use mei_lang_kernel::{CompiledApp, DatasetView, LoadedResource, SourceDecl};

        let mut compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: String::new(),
            app_root: String::new(),
            active_scene: None,
            stage_registry: Default::default(),
            stage_programs: Default::default(),
            scene_slot_modules: Default::default(),
            content_capabilities: Default::default(),
            narration_catalogs: Default::default(),
            active_target_file: String::new(),
            file_tree: Vec::new(),
            scene_routes: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        };
        compiled.resources.push(LoadedResource {
            id: "__world_metrics__::src/scenes/x.mei::metrics".to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(DatasetView {
                id: "__world_metrics__::src/scenes/x.mei::metrics".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: Vec::new(),
                rows: Vec::new(),
                source: SourceDecl {
                    kind: "world_metrics".to_string(),
                    path: String::new(),
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
                metrics: BTreeMap::new(),
                runtime_metric_defs: BTreeMap::new(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: Default::default(),
            }),
        });
        let owners =
            crate::graph::discover_world_metrics_owner_ids(Path::new("."), "demo", &compiled);
        assert!(owners.contains("__world_metrics__::scenes/x.mei::metrics"));
        assert!(!owners.contains("__world_metrics__::src/scenes/x.mei::metrics"));
    }

    #[test]
    fn discover_world_metrics_owner_ids_reads_compiled_resources() {
        use std::path::Path;

        use mei_lang_kernel::{CompiledApp, DatasetView, LoadedResource, SourceDecl};

        let mut compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: String::new(),
            app_root: String::new(),
            active_scene: None,
            stage_registry: Default::default(),
            stage_programs: Default::default(),
            scene_slot_modules: Default::default(),
            content_capabilities: Default::default(),
            narration_catalogs: Default::default(),
            active_target_file: String::new(),
            file_tree: Vec::new(),
            scene_routes: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        };
        compiled.resources.push(LoadedResource {
            id: "__world_metrics__::scenes/x.mei::metrics".to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(DatasetView {
                id: "__world_metrics__::scenes/x.mei::metrics".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: Vec::new(),
                rows: Vec::new(),
                source: SourceDecl {
                    kind: "world_metrics".to_string(),
                    path: String::new(),
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
                metrics: BTreeMap::new(),
                runtime_metric_defs: BTreeMap::new(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: Default::default(),
            }),
        });
        let owners =
            crate::graph::discover_world_metrics_owner_ids(Path::new("."), "demo", &compiled);
        assert!(owners.contains("__world_metrics__::scenes/x.mei::metrics"));
    }

    #[test]
    fn ui_tweak_invalidation_bundle_unchanged() {
        let mut current = BTreeMap::new();
        current.insert("ds_home".to_string(), "mdb:abc".to_string());
        assert!(changed_bundle_owners(&current, &current).is_empty());

        let mut changed = current.clone();
        let mut defs = BTreeMap::new();
        defs.insert("m2".to_string(), json!({"shape": "dataframe"}));
        changed.insert(
            "ds_home".to_string(),
            format!("mdb:{}", metric_defs_fingerprint(&defs)),
        );
        assert_eq!(changed_bundle_owners(&current, &changed).len(), 1);
    }

    #[test]
    fn assemble_inputs_metadata_stable() {
        let inputs = vec![
            AssemblyInputRecord {
                kind: "scene_payload".to_string(),
                key: "scenes/home.mei".to_string(),
                revision: scene_payload_revision("scenes/home.mei", "dep1"),
            },
            AssemblyInputRecord {
                kind: "metric_def_bundle".to_string(),
                key: "ds_home".to_string(),
                revision: "mdb:abc".to_string(),
            },
        ];
        let rev_a = page_instance_revision(&inputs);
        let rev_b = page_instance_revision(&inputs);
        assert_eq!(rev_a, rev_b);
    }

    #[test]
    fn graph_registry_default_off() {
        std::env::remove_var("MEI_GRAPH_REGISTRY");
        assert!(!graph_registry_enabled());
    }

    #[test]
    fn slot_revision_differs_from_compile_revision() {
        let slot = compute_slot_revision("mdb:abc", "ds:1", "default", "json_walk");
        assert!(slot.starts_with("sr:"));
        assert_ne!(slot, "compile:token".to_string());
    }

    #[test]
    fn scene_only_invalidates_nothing() {
        let mut mrg = MrgRegistry::empty("zhifa");
        mrg.slots.push(MrgSlotRecord {
            slot_id: MrgSlotId {
                node: GraphNodeId::new(GraphNodeKind::MaterialSlot, "m1"),
                scope_key: "default".to_string(),
            },
            slot_revision: "sr:1".to_string(),
            state: MaterialState::Ready,
            owner_resource_id: "ds_home".to_string(),
            metric_def_bundle_revision: "mdb:1".to_string(),
            data_source_revision: String::new(),
            payload_ref: None,
            cache_policy: "artifact_sealed".to_string(),
            eval_engine: "json_walk".to_string(),
            last_eval: None,
        });
        use std::collections::BTreeMap;
        let bridge = crate::graph::bridge::export_bridge("zhifa", &BTreeMap::new());
        let outcome = apply_mcg_invalidation(&mut mrg, &bridge, true, &[]);
        assert!(outcome.scene_only_skip);
        assert_eq!(mrg.slots[0].state, MaterialState::Ready);
    }

    #[test]
    fn assemble_board_scope_hydrates_runtime_metric_defs() {
        use std::path::Path;

        use mei_lang_datasets::locate_runtime_metric_resource;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../workspaces/ws-spbjw");
        // Phase 5: apps live under apps/; v1 *.board.mei removed.
        let mcg = source_root.join("apps/zhifa/build/active/graph/registry/mcg.json");
        if !mcg.is_file() {
            return;
        }
        let (compiled, _) = crate::graph::try_assemble_scope_from_scene_payload(
            source_root.as_path(),
            "zhifa",
            Some("enforcement_personnel_analytics_page"),
            "src/scene/home/t2/r-drilldown/s-enforcement-elements/c-enforcement-personnel-analytics/content.mei",
        )
        .expect("page_instance scene payload assemble should succeed for prebuilt zhifa");
        let (owner, resolved) = locate_runtime_metric_resource(
            &compiled,
            "enforcement_officers",
            "enforcement_personnel_count::composition_by_agency",
        )
        .expect("metric should resolve after runtime payload hydrate");
        assert_eq!(owner.id, "__world_metrics__");
        assert_eq!(
            resolved,
            "enforcement_personnel_count::composition_by_agency"
        );
    }

    #[test]
    fn assemble_home_scene_payload_without_mcg_registry() {
        use std::path::Path;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../workspaces/ws-spbjw");
        let mcg = source_root.join("apps/zhifa/build/active/graph/registry/mcg.json");
        if !mcg.is_file() {
            return;
        }
        let (compiled, _) = crate::graph::try_assemble_scope_from_scene_payload(
            source_root.as_path(),
            "zhifa",
            Some("home"),
            "src/scene/home/assembly.mei",
        )
        .expect("home scene payload assemble should succeed without MCG registry");
        assert!(
            !compiled.world_metrics.is_empty(),
            "assembled home view must carry world_metrics for SSR metric cards"
        );
    }

    #[test]
    fn assemble_penalty_board_backfills_runtime_catalog() {
        use std::path::Path;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../workspaces/ws-spbjw");
        let mcg = source_root.join("apps/zhifa/build/active/graph/registry/mcg.json");
        if !mcg.is_file() {
            return;
        }
        let (compiled, _) = crate::graph::try_assemble_scope_from_scene_payload(
            source_root.as_path(),
            "zhifa",
            Some("penalty_total_analytics_page"),
            "src/scene/home/t2/r-drilldown/s-penalty-dashboard/c-penalty-total-analytics/content.mei",
        )
        .expect("penalty page_instance assemble should backfill runtime catalog");
        assert!(
            compiled.resources.iter().any(|resource| {
                resource.id.contains("penalty")
                    || resource
                        .dataset
                        .as_ref()
                        .is_some_and(|dataset| dataset.has_runtime_metric_defs())
            }),
            "penalty page_instance assemble must expose penalty resources or runtime metric defs"
        );
    }
}
