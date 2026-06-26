use super::*;

use super::*;

use super::*;
use mei_lang_kernel::{BoardFileEntry, CompiledApp, CompiledSceneRoute, DatasetView, LoadedResource, SourceDecl};
use serde_json::json;

fn test_outcome(active_scene: &str, active_target_file: &str) -> SharedCompileOutcome {
    SharedCompileOutcome {
        compiled: Arc::new(CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: "/tmp/demo".to_string(),
            active_scene: Some(active_scene.to_string()),
            active_target_file: active_target_file.to_string(),
            scene_routes: vec![CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "scenes/home.mei".to_string(),
                kind: "page".to_string(),
                title: None,
                is_default: true,
                access_export: true,
            }],
            file_tree: Vec::new(),
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        }),
        cache_hit: true,
        artifact_cache_hit: true,
        compile_revision: "rev-a".to_string(),
        cache_lookup_ms: 0,
        artifact_load_ms: 0,
        compile_ms: 0,
    }
}

#[test]
fn default_scope_rejects_non_default_active_target() {
    let outcome = test_outcome("home", "scenes/07-问题办理.board.mei");
    assert!(!compile_outcome_matches_scope(
        &CompileScope::default_scope(),
        &outcome.compiled
    ));
}

#[test]
fn finalize_coverage_report_computes_missing_artifacts() {
    let mut coverage = PrebuildCoverageReport {
        compile_artifacts_planned: 3,
        compile_artifacts_ready: 2,
        dataset_import_artifacts_planned: 2,
        dataset_import_artifacts_ready: 2,
        metric_response_artifacts_planned: 5,
        metric_response_artifacts_ready: 3,
        metric_dataframe_artifacts_planned: 4,
        metric_dataframe_artifacts_ready: 1,
        ..PrebuildCoverageReport::default()
    };

    finalize_coverage_report(&mut coverage);

    assert_eq!(coverage.compile_artifacts_missing, 1);
    assert_eq!(coverage.dataset_import_artifacts_missing, 0);
    assert_eq!(coverage.metric_response_artifacts_missing, 2);
    assert_eq!(coverage.metric_dataframe_artifacts_missing, 3);
    assert_eq!(coverage.total_missing_artifacts, 6);
}

fn test_dataset_resource(id: &str) -> LoadedResource {
    LoadedResource {
        id: id.to_string(),
        kind: "dataset".to_string(),
        title: None,
        document: None,
        dataset: Some(DatasetView {
            id: id.to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["value".to_string()],
            rows: vec![json!({"value": 1})],
            source: SourceDecl {
                kind: "csv".to_string(),
                path: "data/demo.csv".to_string(),
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
            metrics: Default::default(),
            runtime_metric_defs: Default::default(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        }),
    }
}

#[test]
fn prebuild_report_summary_omits_compile_revision() {
    let report = PrebuildReport {
        schema_version: "mei-prebuild-report-v1".to_string(),
        mode: PrebuildMode::Verify,
        scope_profile: PrebuildScopeProfile::Full,
        clean: false,
        clean_wall_ms: 0,
        total_wall_ms: 1200,
        source_root: "/tmp/ws".to_string(),
        manifest_path: "/tmp/ws/.mei/runtime/warmup-manifest.json".to_string(),
        manifest_source: "workspace_config_fallback".to_string(),
        ok: true,
        succeeded_apps: vec!["zhifa".to_string()],
        failed_apps: Vec::new(),
        error_summary: Vec::new(),
        diagnostics: PrebuildDiagnosticsReport::default(),
        apps: vec![PrebuildAppReport {
            app_id: "zhifa".to_string(),
            compile_scopes: vec![PrebuildScopeReport {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: Some("scenes/01-执法要素.mei".to_string()),
                active_scene_id: Some("home".to_string()),
                active_target_file: "scenes/01-执法要素.mei".to_string(),
                cache_hit: true,
                artifact_cache_hit: true,
                compile_revision: "very-long-revision-token".to_string(),
                cache_lookup_ms: 0,
                artifact_load_ms: 12,
                compile_ms: 0,
            }],
            coverage: PrebuildCoverageReport::default(),
            timings: PrebuildTimingReport::default(),
            data_snapshots: None,
            diagnostics: PrebuildDiagnosticsReport::default(),
            warnings: Vec::new(),
        }],
    };
    let json = serde_json::to_string(&report.summary()).expect("serialize summary");
    assert!(!json.contains("compile_revision"));
    assert!(!json.contains("very-long-revision-token"));
    assert!(json.contains("scenes/01-执法要素.mei"));
}

#[test]
fn compile_scopes_follow_explicit_manifest_closure() {
    let app = RuntimeWarmupApp {
        app_id: "demo".to_string(),
        default_scene: Some("home".to_string()),
        hot_scenes: vec!["dashboard".to_string()],
        scenes: vec!["home".to_string()],
        focuses: vec!["scenes/02-inspection.mei".to_string()],
        datasets: vec![RuntimeWarmupDatasetRequest {
            scene_id: Some("details".to_string()),
            focus: Some("scenes/details.mei".to_string()),
            dataset_id: "demo_ds".to_string(),
            priority: None,
            metric_id: None,
            metric_ids: Vec::new(),
        }],
        xlsx_sources: Vec::new(),
    };
    let scope_keys = compile_scopes_for_app(&app, PrebuildScopeProfile::Full)
        .into_iter()
        .map(|scope| scope.key())
        .collect::<BTreeSet<_>>();

    assert!(scope_keys.contains("|"));
    assert!(scope_keys.contains("home|"));
    assert!(scope_keys.contains("dashboard|"));
    assert!(scope_keys.contains("|scenes/02-inspection.mei"));
    assert!(scope_keys.contains("details|scenes/details.mei"));
    assert!(scope_keys.contains("home|scenes/02-inspection.mei"));
    assert!(scope_keys.contains("dashboard|scenes/02-inspection.mei"));
    assert!(!scope_keys.contains("details|scenes/02-inspection.mei"));
}

