//! Integration tests for MCG/MRG graph registry (Phase 1).

#[cfg(test)]
mod graph_mcg_tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::graph::feature::graph_registry_enabled;
    use crate::graph::mcg::assemble::{assembly_view_revision, AssemblyInputRecord};
    use crate::graph::mcg::metric_def_bundle::metric_defs_fingerprint;
    use crate::graph::mcg::scene_payload::scene_payload_revision;
    use crate::graph::mrg::invalidation::{apply_mcg_invalidation, changed_bundle_owners};
    use crate::graph::mrg::registry::{MrgRegistry, MrgSlotId, MrgSlotRecord};
    use crate::graph::mrg::slot_revision::compute_slot_revision;
    use crate::graph::types::{GraphNodeId, GraphNodeKind, MaterialState};

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
        let rev_a = assembly_view_revision(&inputs);
        let rev_b = assembly_view_revision(&inputs);
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
        let outcome = apply_mcg_invalidation(&mut mrg, true, &[]);
        assert!(outcome.scene_only_skip);
        assert_eq!(mrg.slots[0].state, MaterialState::Ready);
    }

    #[test]
    fn assemble_board_scope_hydrates_runtime_metric_defs() {
        use std::path::Path;

        use mei_lang_datasets::locate_runtime_metric_resource;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../workspaces/ws-spbjw");
        let mcg = source_root.join("zhifa/.mei/graph/registry/mcg.json");
        if !mcg.is_file() {
            return;
        }
        let (compiled, _) = crate::graph::try_assemble_scope_from_scene_payload(
            source_root.as_path(),
            "zhifa",
            Some("enforcement_personnel_analytics_board"),
            "scenes/01-执法要素.board.mei",
        )
        .expect("board scene payload assemble should succeed for prebuilt zhifa");
        let (owner, resolved) = locate_runtime_metric_resource(
            &compiled,
            "enforcement_officers",
            "scenes/01-执法要素.mei::enforcement_personnel_count::composition_by_agency",
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
        let scene_payload = source_root.join("zhifa/.mei/graph/payloads/scene/scenes-home-mei.json");
        if !scene_payload.is_file() {
            return;
        }
        let (compiled, _) = crate::graph::try_assemble_scope_from_scene_payload(
            source_root.as_path(),
            "zhifa",
            Some("home"),
            "scenes/home.mei",
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
        let board_payload = source_root
            .join("zhifa/.mei/graph/payloads/scene/scenes-04-行政处罚-board-mei.json");
        if !board_payload.is_file() {
            return;
        }
        let (compiled, _) = crate::graph::try_assemble_scope_from_scene_payload(
            source_root.as_path(),
            "zhifa",
            Some("penalty_total_analytics_board"),
            "scenes/04-行政处罚.board.mei",
        )
        .expect("penalty board assemble should backfill runtime catalog");
        assert!(
            compiled
                .resources
                .iter()
                .any(|resource| resource.id == "penalty_result_dashboard_ds"),
            "penalty board assemble must expose penalty_result_dashboard_ds"
        );
        assert!(
            compiled
                .resources
                .iter()
                .any(|resource| {
                    resource
                        .dataset
                        .as_ref()
                        .is_some_and(|dataset| dataset.has_runtime_metric_defs())
                }),
            "penalty board assemble must hydrate runtime metric defs"
        );
    }
}
