use mei_host_graph::{assemble_scope_from_registry, discover_stage_programs, StageProgramProfile};
use mei_lang_kernel::discover_apps;
use mei_test_support::{
    conformance_workspace, ensure_imported, APP_DECK_MINIMAL, APP_DUAL_STAGE,
    APP_NARRATION_JOURNEY, APP_PAGE_REPORT,
};

#[test]
fn conformance_stage_fixture_catalog_is_complete() {
    let workspace = conformance_workspace();
    for app_id in [
        APP_DECK_MINIMAL,
        APP_DUAL_STAGE,
        APP_NARRATION_JOURNEY,
        APP_PAGE_REPORT,
    ] {
        let root = workspace.join("apps").join(app_id);
        assert!(root.is_dir(), "missing fixture {}", root.display());
        assert!(
            !discover_stage_programs(&root).is_empty(),
            "{app_id} must expose at least one Stage"
        );
    }

    let deck = discover_stage_programs(&workspace.join("apps").join(APP_DECK_MINIMAL));
    assert_eq!(deck.len(), 1);
    assert_eq!(deck[0].stage_id, "intro");
    assert_eq!(deck[0].profile, StageProgramProfile::Slides);

    let dual = discover_stage_programs(&workspace.join("apps").join(APP_DUAL_STAGE));
    assert_eq!(dual.len(), 2);
    assert_eq!(
        dual.iter()
            .find(|program| program.stage_id == "home")
            .and_then(|program| program.short_title.as_deref()),
        Some("Home")
    );
    assert_eq!(
        dual.iter()
            .find(|program| program.stage_id == "demo")
            .and_then(|program| program.short_title.as_deref()),
        Some("Demo")
    );

    let report = discover_stage_programs(&workspace.join("apps").join(APP_PAGE_REPORT));
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].profile, StageProgramProfile::Page);
    assert_eq!(report[0].short_title.as_deref(), Some("Report"));

    let apps = discover_apps(&workspace).expect("discover conformance apps");
    let dual_app = apps
        .iter()
        .find(|app| app.id == APP_DUAL_STAGE)
        .expect("dual-stage app metadata");
    assert_eq!(dual_app.title, "Conformance Dual Stage");
    assert_eq!(dual_app.short_title.as_deref(), Some("Dual"));
}

#[test]
fn conformance_stage_fixtures_compile_import_and_assemble() {
    for (app_id, stage_ids) in [
        (APP_DECK_MINIMAL, &["intro"][..]),
        (APP_DUAL_STAGE, &["home", "demo"][..]),
        (APP_NARRATION_JOURNEY, &["home", "journey"][..]),
        (APP_PAGE_REPORT, &["report"][..]),
    ] {
        let workspace = ensure_imported(app_id);
        for stage_id in stage_ids {
            let outcome = assemble_scope_from_registry(workspace.as_path(), app_id, stage_id)
                .unwrap_or_else(|error| panic!("assemble {app_id}/{stage_id}: {error:#}"))
                .unwrap_or_else(|| panic!("missing outcome for {app_id}/{stage_id}"));
            let program = outcome
                .compiled
                .stage_programs
                .get(stage_id)
                .unwrap_or_else(|| panic!("missing StageProgram for {app_id}/{stage_id}"));
            if app_id == APP_DUAL_STAGE {
                let route = outcome
                    .compiled
                    .scene_routes
                    .iter()
                    .find(|route| route.scene_id == *stage_id)
                    .unwrap_or_else(|| panic!("missing Stage route for {app_id}/{stage_id}"));
                assert_eq!(
                    route.short_title.as_deref(),
                    Some(if *stage_id == "home" { "Home" } else { "Demo" })
                );
                let descriptor = outcome
                    .compiled
                    .stage_registry
                    .get(stage_id)
                    .unwrap_or_else(|| panic!("missing Stage descriptor for {app_id}/{stage_id}"));
                assert_eq!(descriptor.short_title, route.short_title);
            }
            if app_id == APP_PAGE_REPORT {
                assert_eq!(program.surface.as_str(), "document");
                let route = outcome
                    .compiled
                    .scene_routes
                    .iter()
                    .find(|route| route.scene_id == *stage_id)
                    .expect("page Stage route");
                assert_eq!(route.short_title.as_deref(), Some("Report"));
                let descriptor = outcome
                    .compiled
                    .stage_registry
                    .get(stage_id)
                    .expect("page Stage descriptor");
                assert_eq!(descriptor.profile.as_str(), "page");
                assert_eq!(descriptor.short_title.as_deref(), Some("Report"));
            }
        }
    }
}
