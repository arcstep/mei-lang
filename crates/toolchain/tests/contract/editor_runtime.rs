use super::support::*;

#[test]
fn editor_runtime_descriptor_exposes_tooling_templates() {
    let descriptor = editor_runtime_descriptor_for_package_root(&package_root());
    assert_eq!(descriptor.schema_version, "mei-editor-runtime-v1");
    assert!(descriptor
        .tooling_templates
        .iter()
        .any(|item| item.tool == "cursor"));
}

#[test]
fn editor_runtime_doctor_passes_for_source_tree_package_root() {
    let report = doctor_editor_runtime_for_package_root(&package_root());
    assert!(report.ok, "doctor should pass for source-tree package root");
    assert!(report
        .checks
        .iter()
        .any(|item| item.id == "author_profile" && item.ok));
    assert!(report
        .checks
        .iter()
        .any(|item| item.id == "knowledge_asset:author_profile" && item.ok));
}

#[test]
fn scaffold_editor_runtime_tooling_writes_cursor_files() {
    let root = std::env::temp_dir().join(format!(
        "mei_editor_runtime_scaffold_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    fs::create_dir_all(&root).expect("create scaffold root");
    let report = scaffold_editor_runtime_tooling(
        &root,
        &package_root(),
        &[
            "cursor".to_string(),
            "vscode".to_string(),
            "trae".to_string(),
            "codex".to_string(),
            "claude-code".to_string(),
            "opencode".to_string(),
        ],
        false,
    )
    .expect("scaffold");
    assert!(report
        .files
        .iter()
        .any(|item| item.rel_path == ".cursor/mcp.json"));
    assert!(
        !root.join("runtime/platform/version.json").exists(),
        "scaffold must not install runtime metadata"
    );
    assert!(
        !root.join("runtime/platform/editor-runtime.json").exists(),
        "scaffold must not install runtime descriptor"
    );
    assert!(root.join(".cursor/rules/meilang-authoring.mdc").is_file());
    assert!(root.join(".vscode/settings.json").is_file());
    assert!(root.join(".trae/mcp.json").is_file());
    assert!(root.join("runtime/platform/tooling/codex/mcp.json").is_file());
    assert!(root.join("runtime/platform/tooling/claude-code/mcp.json").is_file());
    assert!(root.join("runtime/platform/tooling/opencode/mcp.json").is_file());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn install_editor_runtime_support_files_writes_version_metadata() {
    let root = std::env::temp_dir().join(format!(
        "mei_editor_runtime_install_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    fs::create_dir_all(&root).expect("create install root");
    install_editor_runtime_support_files(&root, &package_root(), true).expect("install runtime");
    let version: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("runtime/platform/version.json")).expect("read version"),
    )
    .expect("parse version");
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("toolchain/MANIFEST.json")).expect("read manifest"),
    )
    .expect("parse manifest");
    let expected_line = format!(
        "mei-{}",
        env!("CARGO_PKG_VERSION")
            .split('.')
            .next()
            .expect("major version")
    );
    assert_eq!(version["toolchain_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(version["compatibility"]["line"], expected_line);
    assert_eq!(manifest["toolchain_version"], env!("CARGO_PKG_VERSION"));
    assert!(manifest["bundle_id"]
        .as_str()
        .is_some_and(|value| value.starts_with("mei-lang-")));
    assert!(root.join("runtime/platform/catalog/capability-catalog.json").is_file());
    assert!(root.join("runtime/platform/catalog/author-surface.json").is_file());
    assert!(root.join("runtime/platform/catalog/access-surface.json").is_file());
    assert!(root.join("runtime/platform/profiles/author.md").is_file());
    assert!(root.join("runtime/platform/profiles/access.md").is_file());
    assert_eq!(manifest["artifacts"]["mei_toolchain"], "bin/mei-toolchain");
    assert_eq!(manifest["artifacts"]["mei_lsp"], "bin/mei-lsp");
    assert_eq!(manifest["artifacts"]["mei_host_web"], "bin/mei-host-web");
    assert!(
        manifest["provenance"]["package_root"]
            .as_str()
            .is_some_and(|value| !value.contains('/')),
        "manifest provenance must avoid machine-local absolute paths"
    );
    assert!(root.join("runtime/platform/skills/meilang-author/SKILL.md").is_file());
    assert!(root.join("runtime/platform/skills/meilang-access/SKILL.md").is_file());
    assert!(root.join("toolchain/bin/mei-toolchain").is_file());
    assert!(root.join("toolchain/bin/mei-lsp").is_file());
    assert!(root.join("toolchain/bin/mei-host-web").is_file());
    assert!(root.join("toolchain/bin/author-mcp-adapter").is_file());
    assert!(root.join("toolchain/bin/access-mcp-adapter").is_file());
    assert!(root.join("deploy/start.sh").is_file());
    assert!(!root.join("runtime/platform/catalog/editor-surface.json").exists());
    for rel in [
        "runtime/platform/editor-runtime.json",
        "runtime/platform/knowledge/author-runtime.json",
        "toolchain/MANIFEST.json",
        "runtime/platform/catalog/capability-catalog.json",
        "runtime/platform/catalog/author-surface.json",
        "runtime/platform/catalog/access-surface.json",
    ] {
        let content = fs::read_to_string(root.join(rel)).expect("read installed descriptor");
        assert!(
            !content.contains(&root.display().to_string()),
            "installed runtime descriptor {rel} must not embed workspace-local absolute paths"
        );
        assert!(
            !content.contains(&package_root().display().to_string()),
            "installed runtime descriptor {rel} must not embed source-tree absolute paths"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn editor_runtime_doctor_checks_workspace_runtime_metadata() {
    let root = std::env::temp_dir().join(format!(
        "mei_editor_runtime_doctor_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    fs::create_dir_all(&root).expect("create doctor root");
    install_editor_runtime_support_files(&root, &package_root(), true).expect("install runtime");
    let report = doctor_editor_runtime_for_workspace_root(&package_root(), &root);
    assert!(report.ok, "workspace doctor should pass after install");
    assert_eq!(report.workspace_root, Some(root.display().to_string()));
    assert!(report
        .checks
        .iter()
        .any(|item| item.id == "workspace_version_descriptor" && item.ok));
    assert!(report
        .checks
        .iter()
        .any(|item| item.id == "workspace_runtime_manifest" && item.ok));
    assert!(report
        .checks
        .iter()
        .any(|item| item.id == "workspace_author_skill" && item.ok));
    assert!(report
        .checks
        .iter()
        .any(|item| item.id == "workspace_capability_catalog" && item.ok));
    assert!(report
        .checks
        .iter()
        .any(|item| item.id == "workspace_access_skill" && item.ok));
    assert!(report
        .checks
        .iter()
        .any(|item| item.id == "workspace_access_mcp_adapter" && item.ok));
    let status = workspace_runtime_status_for_workspace_root(&package_root(), &root);
    assert!(status.installed, "runtime status should report installed");
    assert!(
        !status.fallback_to_source_tree,
        "workspace-local assets should avoid source-tree fallback"
    );
    let bundle = export_knowledge_bundle_for_workspace_root(
        &root,
        &package_root(),
        "author",
        Some("author_profile"),
        true,
    )
    .expect("workspace knowledge bundle");
    let author_profile = bundle["assets"]
        .as_array()
        .and_then(|items| items.first())
        .expect("workspace author profile asset");
    assert!(author_profile["content"]
        .as_str()
        .is_some_and(|content| content.contains("Author")));
    let component_contracts = export_knowledge_bundle_for_workspace_root(
        &root,
        &package_root(),
        "author",
        Some("component_contracts"),
        true,
    )
    .expect("workspace component contracts");
    let component_contracts_content = component_contracts["assets"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["content"].as_str())
        .expect("component contracts content");
    assert!(
        component_contracts_content.contains("meilang-author-component-contracts-v1"),
        "workspace component contracts should expose the public contract index"
    );
    let example_pack = export_knowledge_bundle_for_workspace_root(
        &root,
        &package_root(),
        "author",
        Some("example_dataset_baseline"),
        true,
    )
    .expect("workspace example pack");
    let example_content = example_pack["assets"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["content"].as_str())
        .expect("workspace example content");
    assert!(
        example_content.contains("dataset.table"),
        "workspace example bundle should expose the curated standalone examples"
    );
    let access_bundle = export_knowledge_bundle_for_workspace_root(
        &root,
        &package_root(),
        "access",
        Some("meilang_access_skill"),
        true,
    )
    .expect("workspace access skill");
    let access_skill_content = access_bundle["assets"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["content"].as_str())
        .expect("access skill content");
    assert!(
        access_skill_content.contains("MeiLang Access"),
        "workspace access bundle should expose the installed access skill entry"
    );
    let catalog = capability_catalog_descriptor_for_workspace_root(&root, &package_root());
    assert_eq!(catalog["workspace_root"], ".");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_runtime_status_fails_when_core_binary_is_missing() {
    let root = std::env::temp_dir().join(format!(
        "mei_editor_runtime_missing_bin_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    fs::create_dir_all(&root).expect("create missing-bin root");
    install_editor_runtime_support_files(&root, &package_root(), true).expect("install runtime");
    fs::remove_file(root.join("toolchain/bin/mei-host-web")).expect("remove host binary");
    let status = workspace_runtime_status_for_workspace_root(&package_root(), &root);
    assert!(
        !status.installed,
        "runtime status must fail when a required workspace-local binary is missing"
    );
    let doctor = doctor_editor_runtime_for_workspace_root(&package_root(), &root);
    assert!(
        !doctor.ok,
        "doctor must fail when a required workspace-local binary is missing"
    );
    assert!(doctor
        .checks
        .iter()
        .any(|item| item.id == "workspace_mei_host_web_bin" && !item.ok));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_bootstrap_cli_installs_runtime_for_new_source_workspace() {
    let root = std::env::temp_dir().join(format!(
        "mei_workspace_bootstrap_cli_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_millis()
    ));
    let status = Command::new("cargo")
        .arg("run")
        .arg("-q")
        .arg("-p")
        .arg("mei-lang-server")
        .arg("--bin")
        .arg("mei-toolchain")
        .arg("--")
        .arg("workspace")
        .arg("bootstrap")
        .arg("--source-root")
        .arg(&root)
        .arg("--app")
        .arg("demo")
        .arg("--tool")
        .arg("cursor")
        .current_dir(package_root())
        .status()
        .expect("run bootstrap command");
    assert!(status.success(), "workspace bootstrap CLI should succeed");
    assert!(root.join("toolchain/bin/mei-toolchain").is_file());
    assert!(root.join("toolchain/bin/mei-lsp").is_file());
    assert!(root.join("toolchain/bin/mei-host-web").is_file());
    assert!(root.join("apps/demo/src/main.mei").is_file());
    assert!(root.join("deploy/start.sh").is_file());
    let status = workspace_runtime_status_for_workspace_root(&package_root(), &root);
    assert!(
        status.installed,
        "bootstrapped source workspace should report installed after runtime setup"
    );
    let _ = fs::remove_dir_all(root);
}

