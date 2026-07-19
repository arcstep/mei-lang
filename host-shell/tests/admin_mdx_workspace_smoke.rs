use mei_lang_kernel::{discover_app_admin_resources, resolve_app_root, AdminDiscoverOutcome};

#[test]
fn optional_mini_data_admin_mdx_projects_page_and_theme_resources() {
    let Some(workspace) = mei_test_support::optional_external_workspace() else {
        return;
    };
    let app_root = resolve_app_root(&workspace, "mini-data");
    if !app_root.is_dir() {
        eprintln!(
            "skip: MEI_TEST_WORKSPACE has no mini-data app at {}",
            app_root.display()
        );
        return;
    }
    match discover_app_admin_resources(&app_root, "mini-data") {
        AdminDiscoverOutcome::Ok(projection) => {
            let ids = projection
                .resources
                .iter()
                .map(|resource| resource.resource_id.as_str())
                .collect::<Vec<_>>();
            assert_eq!(ids, vec!["datasources", "organization", "theme"]);
            let theme = projection
                .resources
                .iter()
                .find(|resource| resource.resource_id == "theme")
                .expect("theme resource");
            assert_eq!(theme.config_path.as_deref(), Some("ops.themes.cockpit"));
            assert_eq!(theme.page_program.page.surface.as_str(), "document");
            assert!(theme
                .page_program
                .page
                .source_anchor
                .ends_with("src/admin/theme.admin.mdx"));
        }
        outcome => panic!("mini-data Admin MDX discovery failed: {outcome:?}"),
    }
}
