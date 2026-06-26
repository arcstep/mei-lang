use super::support::*;

#[test]
fn standalone_source_root_core_smoke_check_works() {
    let root = standalone_fixture_root();
    clear_compile_cache_for_app(&root, "core-smoke-app");
    let report =
        compile_report(&root, "core-smoke-app", CompileOptions::default()).expect("compile report");
    assert!(!report.revision_token.is_empty());
    assert!(!report
        .compiled
        .diagnostics
        .iter()
        .any(|item| matches!(item.severity, mei_lang_kernel::Severity::Error)));
}

#[test]
fn standalone_source_root_ds_smoke_query_dataset_works() {
    let root = standalone_fixture_root();
    clear_compile_cache_for_app(&root, "ds-smoke-app");
    let payload = query_world_dataset(
        &root,
        "ds-smoke-app",
        None,
        "sales_data",
        None,
        &BTreeMap::new(),
        None,
        Some(5),
        None,
    )
    .expect("standalone dataset query");
    assert_eq!(payload["id"], "sales_data");
    assert!(payload["sample_rows"].is_array());
}

#[test]
fn capability_catalog_includes_platform_assets_and_profiles() {
    let root = package_root();
    let descriptor = capability_catalog_descriptor_for_package_root(&root);
    assert_eq!(descriptor["schema_version"], "mei-capability-catalog-v1");
    assert!(descriptor["ai_profiles"].is_array());
    assert_eq!(descriptor["ai_profiles"].as_array().unwrap().len(), 2);
    assert!(descriptor["ai_profiles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == "author"
            && item["guidance_file_rel"] == "guides/author-profile.md"));
    assert!(descriptor["skill_packages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| {
            item["id"] == "meilang-author"
                && item["companion_priority"]
                    .as_array()
                    .is_some_and(|companions| {
                        companions.iter().any(|entry| entry == "dsl-reference.md")
                    })
                && item["companion_priority"]
                    .as_array()
                    .is_some_and(|companions| {
                        companions
                            .iter()
                            .any(|entry| entry == "namespace-reference.md")
                    })
        }));
    assert!(descriptor["skill_packages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| {
            item["id"] == "meilang-access"
                && item["entry_file"] == "SKILL.md"
                && item["companion_priority"]
                    .as_array()
                    .is_some_and(|companions| companions.iter().any(|entry| entry == "workflow.md"))
        }));
    assert!(descriptor["platform_assets"]["component_packs"].is_array());
    assert!(!descriptor["platform_assets"]["component_packs"]
        .as_array()
        .unwrap()
        .is_empty());
    let chart_pack = descriptor["platform_assets"]["component_packs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "chart/echarts")
        .expect("chart component pack");
    assert!(chart_pack["authoring_support"]["knowledge_asset_ids"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item == "component_contracts")));
    assert!(chart_pack["authoring_support"]["recommended_example_ids"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item == "example_chart_baseline")));
    assert!(descriptor["platform_assets"]["template_packs"].is_array());
    assert!(descriptor["platform_assets"]["template_packs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == "cockpit"));
    let cockpit_template_pack = descriptor["platform_assets"]["template_packs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "cockpit")
        .expect("cockpit template pack");
    assert!(
        cockpit_template_pack["authoring_support"]["knowledge_asset_ids"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == "cockpit_template_index"))
    );
    assert!(descriptor["host_extensions"]["extensions"].is_array());
    assert!(descriptor["host_requirements"].is_array());
    assert!(descriptor["knowledge_bundles"].is_array());
    let author_surface = descriptor["mcp_surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["surface"] == "author")
        .expect("author mcp surface");
    assert!(
        author_surface.get("surface_aliases").is_none(),
        "author surface must not expose compatibility aliases"
    );
    assert!(descriptor["mcp_surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| {
            item["surface"] == "access"
                && item["host_overlay"]["host_only_tools"]
                    .as_array()
                    .is_some_and(|tools| tools.iter().any(|tool| tool == "propose_session_patch"))
        }));
    let access_profile = descriptor["ai_profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "access")
        .expect("access profile");
    assert_eq!(access_profile["skill_package_id"], "meilang-access");
    let access_surface = descriptor["mcp_surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["surface"] == "access")
        .expect("access mcp surface");
    assert!(access_surface["tools"].as_array().is_some_and(|tools| tools
        .iter()
        .any(|tool| tool["name"] == "mei_access_knowledge")));
    assert_eq!(
        descriptor["host_requirements"][0]["consumer_id"],
        "mei-host-web"
    );
}

#[test]
fn capability_catalog_rejects_editor_profile_alias() {
    use mei_lang_toolchain::ai_profile_descriptor;
    assert!(ai_profile_descriptor("editor").is_none());
    assert!(ai_profile_descriptor("author").is_some());
}

