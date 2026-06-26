use super::support::*;

#[test]
fn workspace_init_does_not_install_runtime_assets() {
    let root = std::env::temp_dir().join(format!(
        "mei_workspace_init_no_runtime_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    fs::create_dir_all(&root).expect("create init root");
    let profile_root =
        init_workspace_profile(&root, "profile-a", Some("test"), &package_root())
            .expect("init profile");
    assert!(
        !profile_root.join("runtime/platform/version.json").exists(),
        "workspace init must not install runtime metadata"
    );
    assert!(
        !profile_root
            .join("runtime/platform/skills/meilang-author/SKILL.md")
            .exists(),
        "workspace init must not install author skill package"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_runtime_update_preserves_local_state_files() {
    let root = std::env::temp_dir().join(format!(
        "mei_workspace_runtime_local_state_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    fs::create_dir_all(root.join("runtime/hosts")).expect("create local hosts");
    let local_state_path = root.join("runtime/hosts/preserved.state.json");
    let local_state = r#"{"schemaVersion":1,"hostId":"test","auth":{"users":[]}}"#;
    fs::write(&local_state_path, local_state).expect("write local state");

    install_editor_runtime_support_files(&root, &package_root(), true).expect("install runtime");
    install_editor_runtime_support_files(&root, &package_root(), true).expect("update runtime");

    assert_eq!(
        fs::read_to_string(&local_state_path).expect("read preserved local state"),
        local_state,
        "runtime update must preserve runtime/hosts/** content"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_knowledge_requires_runtime_install_without_package_fallback() {
    let root = std::env::temp_dir().join(format!(
        "mei_workspace_knowledge_gate_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    fs::create_dir_all(&root).expect("create knowledge root");
    let error = export_knowledge_bundle_for_workspace_root(
        &root,
        &package_root(),
        "author",
        Some("author_profile"),
        true,
    )
    .expect_err("workspace knowledge must fail before runtime install");
    assert!(
        error.to_string().contains("workspace runtime install"),
        "error should point to runtime install, got: {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn standalone_workspace_init_install_create_app_and_check_form_a_smoke_path() {
    let parent = std::env::temp_dir().join(format!(
        "mei_workspace_authoring_smoke_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    fs::create_dir_all(&parent).expect("create smoke parent");
    let workspace_root = init_workspace_profile(
        &parent,
        "standalone-smoke",
        Some("Standalone Smoke"),
        &package_root(),
    )
    .expect("init source workspace");
    install_editor_runtime_support_files(&workspace_root, &package_root(), true)
        .expect("install runtime");
    let app_root = create_app_skeleton(&workspace_root, "demo").expect("create app");
    assert!(app_root.join("src/main.mei").is_file());
    let config_bundle = export_knowledge_bundle_for_workspace_root(
        &workspace_root,
        &package_root(),
        "author",
        Some("workspace_config_reference"),
        true,
    )
    .expect("workspace config reference");
    let config_content = config_bundle["assets"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["content"].as_str())
        .expect("config reference content");
    assert!(
        config_content.contains("workspace.json") && config_content.contains("theme_ref"),
        "workspace config reference should be available in standalone installs"
    );
    let report = compile_report(&workspace_root, "demo", CompileOptions::default())
        .expect("compile created app");
    assert!(
        !report
            .compiled
            .diagnostics
            .iter()
            .any(|item| matches!(item.severity, mei_lang_kernel::Severity::Error)),
        "created standalone app should compile without errors"
    );
    let _ = fs::remove_dir_all(parent);
}

