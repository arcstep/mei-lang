use mei_host_graph::{assemble_scope_from_registry, discover_stage_programs};
use mei_lang_kernel::{discover_app_admin_resources, resolve_app_root, AdminDiscoverOutcome};
use mei_test_support::{conformance_workspace, ensure_imported, APP_ADMIN_MEI};

#[test]
fn conformance_admin_discover_derives_identity_route_and_provider() {
    let workspace = conformance_workspace();
    let app_root = resolve_app_root(&workspace, APP_ADMIN_MEI);
    assert!(
        app_root.join("src/admin/demo/overview.mdx").is_file(),
        "missing fx-admin-mei fixture"
    );

    let projection = match discover_app_admin_resources(&app_root, APP_ADMIN_MEI) {
        AdminDiscoverOutcome::Ok(projection) => projection,
        outcome => panic!("fx-admin-mei discovery failed: {outcome:?}"),
    };
    assert_eq!(projection.resources.len(), 1);
    let resource = &projection.resources[0];
    assert_eq!(resource.registry_entry.resource_id, "demo");
    assert_eq!(resource.registry_entry.module_id, "overview");
    assert_eq!(
        resource.registry_entry.canonical_route,
        "/admin/apps/fx-admin-mei/demo/overview"
    );
    assert_eq!(
        resource.page_program.root.scene_ref(),
        "admin.demo.overview"
    );
    assert_eq!(resource.page_program.provider_bindings.len(), 1);
    assert_eq!(
        resource.page_program.provider_bindings[0].target,
        "ops.organization"
    );
    assert!(
        discover_stage_programs(&app_root).is_empty(),
        "Admin entries must not enter the Stage Registry"
    );
}

#[test]
fn conformance_admin_compiles_imports_and_assembles_shared_scene() {
    let workspace = ensure_imported(APP_ADMIN_MEI);
    let outcome =
        assemble_scope_from_registry(workspace.as_path(), APP_ADMIN_MEI, "admin.demo.overview")
            .expect("assemble Admin scene")
            .expect("Admin scene outcome");
    assert!(
        outcome.compiled.scene_contract.is_some(),
        "Admin scene must use the ordinary host graph compositor"
    );
    assert!(
        outcome
            .compiled
            .scene_contract
            .as_ref()
            .is_some_and(|scene| !scene.panels.is_empty()),
        "Admin page scene must lower its direct panel references"
    );
    assert!(
        outcome
            .compiled
            .component_assets
            .iter()
            .any(|asset| asset.key == "admin.form-card" && asset.tag == "mei-admin-form-card"),
        "Admin bricks must resolve through the ordinary component manifest"
    );
    let structure = mei_host_graph::build_structure_full_document(&outcome.compiled, "test");
    assert!(
        !structure.nodes.is_empty(),
        "Admin page must publish non-empty ordinary structure.full"
    );
    assert_eq!(
        structure
            .frame_viewport
            .as_ref()
            .and_then(|viewport| viewport.route_mode.as_deref()),
        Some("page"),
        "Admin page profile must use document-flow compose instead of cockpit viewport scaling"
    );
}
