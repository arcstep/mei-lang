use mei_lang_kernel::{discover_app_admin_resources, resolve_app_root, AdminDiscoverOutcome};

#[test]
fn optional_v2_admin_entries_project_plain_page_programs() {
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
    let Ok(paths) = mei_lang_kernel::discover_admin_mdx_paths(&app_root) else {
        return;
    };
    if paths.is_empty() {
        return;
    }
    match discover_app_admin_resources(&app_root, "mini-data") {
        AdminDiscoverOutcome::Ok(projection) => {
            assert!(!projection.resources.is_empty());
            assert!(projection.resources.iter().all(|entry| {
                entry.page_program.surface.as_str() == "document"
                    && entry.page_program.source_anchor.starts_with("src/admin/")
                    && entry.page_program.source_anchor.ends_with(".mdx")
                    && entry.registry_entry.module_id
                        == entry
                            .registry_entry
                            .canonical_route
                            .rsplit('/')
                            .next()
                            .unwrap_or_default()
            }));
        }
        outcome => panic!("mini-data Admin MDX discovery failed: {outcome:?}"),
    }
}
