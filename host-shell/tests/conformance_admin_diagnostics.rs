use mei_lang_kernel::{discover_app_admin_resources, resolve_app_root, AdminDiscoverOutcome};
use mei_test_support::conformance_workspace;

#[test]
fn conformance_admin_negative_fixtures_match_frozen_codes() {
    let workspace = conformance_workspace();
    let cases = [
        (
            "fx-diag-admin-identity",
            "admin_identity_redeclaration_forbidden",
        ),
        ("fx-diag-admin-route", "admin_frontmatter_field_forbidden"),
        (
            "fx-diag-admin-template",
            "admin_frontmatter_field_forbidden",
        ),
        ("fx-diag-admin-module", "admin_frontmatter_field_forbidden"),
        ("fx-diag-admin-non-mdx", "admin_entry_module_forbidden"),
        ("fx-diag-admin-field", "admin_mdx_forbidden_presentation"),
        ("fx-diag-admin-scene-missing", "admin_scene_root_missing"),
        ("fx-diag-admin-scene-unknown", "admin_scene_root_unknown"),
        ("fx-diag-admin-json", "admin_legacy_data_json_forbidden"),
        (
            "fx-diag-admin-dual",
            "admin_legacy_dual_projection_forbidden",
        ),
        ("fx-diag-admin-target", "provider_binding_invalid"),
    ];

    for (app_id, expected) in cases {
        let app_root = resolve_app_root(&workspace, app_id);
        assert!(
            app_root.is_dir(),
            "missing conformance fixture {}",
            app_root.display()
        );
        match discover_app_admin_resources(&app_root, app_id) {
            AdminDiscoverOutcome::Err(diagnostic) => assert_eq!(
                diagnostic.kind, expected,
                "wrong diagnostic for {app_id}: {}",
                diagnostic.message
            ),
            outcome => panic!("{app_id} must fail with `{expected}`, got {outcome:?}"),
        }
    }
}
