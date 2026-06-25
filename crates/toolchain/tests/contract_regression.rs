use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use mei_lang_kernel::RuntimeIntent;
use mei_lang_kernel::{set_mei_package_root, CompileOptions};
use mei_lang_toolchain::{
    build_world_context_snapshot, capability_catalog_descriptor_for_package_root,
    capability_catalog_descriptor_for_workspace_root, clear_compile_cache_for_app,
    compile_app_with_cache, compile_report, create_app_skeleton,
    doctor_editor_runtime_for_package_root, doctor_editor_runtime_for_workspace_root,
    editor_runtime_descriptor_for_package_root, export_knowledge_bundle_for_package_root,
    export_knowledge_bundle_for_workspace_root, init_workspace_profile,
    install_editor_runtime_support_files, query_world_dataset, query_world_dataset_metrics,
    resolve_components_root, runtime_sim_step, scaffold_editor_runtime_tooling,
    workspace_runtime_status_for_workspace_root, RESOURCE_QUERY_SCHEMA_VERSION,
};

fn package_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("mei-lang package root");
        set_mei_package_root(root.clone());
        root
    })
    .clone()
}

fn workspaces_root() -> PathBuf {
    let _ = package_root();
    if let Ok(raw) = std::env::var("MEI_TEST_SOURCE_ROOT") {
        return PathBuf::from(raw)
            .canonicalize()
            .expect("MEI_TEST_SOURCE_ROOT");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-dev")
        .canonicalize()
        .expect("workspaces/ws-dev root")
}

fn standalone_fixture_root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(build_standalone_fixture).clone()
}

fn build_standalone_fixture() -> PathBuf {
    let source = workspaces_root();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_millis();
    let fixture_root = std::env::temp_dir().join(format!(
        "mei_toolchain_standalone_fixture_{}_{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(&fixture_root).expect("create fixture root");
    fs::write(
        fixture_root.join("workspace.json"),
        r#"{"schemaVersion":2,"paths":{"apps":"apps","components":"stock/components"}}"#,
    )
    .expect("write workspace.json");
    copy_dir_recursive(
        source.join("apps/examples-core-01-single-file-doc"),
        fixture_root.join("apps/core-smoke-app"),
    );
    copy_dir_recursive(
        source.join("apps/examples-ds-01-dataset-baseline"),
        fixture_root.join("apps/ds-smoke-app"),
    );
    copy_dir_recursive(
        source.join("stock/components"),
        fixture_root.join("stock/components"),
    );
    fixture_root
}

fn copy_dir_recursive(src: PathBuf, dst: PathBuf) {
    fs::create_dir_all(&dst).expect("create destination directory");
    for entry in fs::read_dir(src).expect("read directory") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(path, target);
        } else {
            fs::copy(path, target).expect("copy file");
        }
    }
}

const DATASET_APP: &str = "examples-ds-01-dataset-baseline";
const METRIC_APP: &str = "examples-ds-04-data-table-features";
const RUNTIME_APP: &str = "examples-sim-01-fire-baseline";

#[test]
fn compile_service_reports_cache_hit_on_second_request() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let components = resolve_components_root(&root);
    let options = CompileOptions::default();
    let first = compile_app_with_cache(&root, DATASET_APP, options.clone(), components.as_path())
        .map_err(|failure| failure.error)
        .expect("first");
    let second = compile_app_with_cache(&root, DATASET_APP, options, components.as_path())
        .map_err(|failure| failure.error)
        .expect("second");
    assert!(second.cache_hit, "second compile should hit cache");
    assert_eq!(first.compile_revision, second.compile_revision);
}

#[test]
fn clear_compile_cache_for_app_invalidates_cache_hit() {
    let root = workspaces_root();
    let components = resolve_components_root(&root);
    let options = CompileOptions::default();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let _ = compile_app_with_cache(&root, DATASET_APP, options.clone(), components.as_path())
        .map_err(|failure| failure.error)
        .expect("warm");
    let cleared = clear_compile_cache_for_app(&root, DATASET_APP);
    assert!(cleared >= 1, "expected at least one cache entry cleared");
    let after_clear = compile_app_with_cache(&root, DATASET_APP, options, components.as_path())
        .map_err(|failure| failure.error)
        .expect("after clear");
    assert!(
        !after_clear.cache_hit,
        "compile after clear should miss cache"
    );
}

#[test]
fn compile_report_revision_matches_cached_outcome() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let report = compile_report(&root, DATASET_APP, CompileOptions::default()).expect("report");
    assert!(!report.revision_token.is_empty());
    let cached = compile_app_with_cache(
        &root,
        DATASET_APP,
        CompileOptions::default(),
        resolve_components_root(&root).as_path(),
    )
    .map_err(|failure| failure.error)
    .expect("cached");
    assert_eq!(report.revision_token, cached.compile_revision);
    let second =
        compile_report(&root, DATASET_APP, CompileOptions::default()).expect("second report");
    assert!(second.cache_hit);
    assert_eq!(report.revision_token, second.revision_token);
}

#[test]
fn query_world_dataset_contract_shape_is_stable() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let payload = query_world_dataset(
        &root,
        DATASET_APP,
        None,
        "sales_data",
        None,
        &BTreeMap::new(),
        None,
        None,
        None,
    )
    .expect("dataset query");
    assert_eq!(payload["id"], "sales_data");
    assert!(payload["sample_rows"].is_array());
    assert!(payload["dataset"]["schema_preview"].is_array());
    assert!(payload["observation"]["exposure"]["query_schema_version"]
        .as_str()
        .is_some_and(|version| version.contains(RESOURCE_QUERY_SCHEMA_VERSION)));
    assert!(payload["perf"]["total_ms"].as_u64().is_some());
}

#[test]
fn query_world_dataset_metrics_contract_shape_is_stable() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, METRIC_APP);
    let payload = query_world_dataset_metrics(
        &root,
        METRIC_APP,
        None,
        "orders",
        &["orders_overview".to_string()],
        None,
        &BTreeMap::new(),
        None,
        &[],
    )
    .expect("metric query");
    assert_eq!(payload["dataset_id"], "orders");
    assert!(payload["metrics"].is_array());
    assert!(!payload["metrics"].as_array().unwrap().is_empty());
    assert!(payload["analysis_contracts"].is_object() || payload["analysis_contracts"].is_array());
    assert!(payload["observation"]["compile"]["compile_ms"]
        .as_u64()
        .is_some());
    assert!(payload["perf"]["metric_eval_ms"].as_u64().is_some());
}

#[test]
fn runtime_sim_step_returns_scene_view_and_html() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, RUNTIME_APP);
    let result = runtime_sim_step(
        &root,
        RUNTIME_APP,
        None,
        RuntimeIntent {
            kind: "sync".to_string(),
            target: None,
        },
    )
    .expect("runtime sim");
    assert!(!result.html.is_empty());
    assert!(!result.scene_view.scene_id.is_empty());
}

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
fn knowledge_bundle_exports_author_assets() {
    let payload = export_knowledge_bundle_for_package_root(&package_root(), "author", None, false)
        .expect("knowledge bundle");
    assert_eq!(payload["descriptor"]["surface"], "author");
    assert!(payload["descriptor"]["available_topics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "profile"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "syntax_rules"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "author_profile"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "dsl_reference"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "workspace_config_reference"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "template_contracts"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "dsl_contracts"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "component_contracts"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "cockpit_template_index"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "example_dataset_baseline"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "example_multi_scene_app"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "example_upload_dataset_baseline"));
    assert!(payload["descriptor"]["available_topics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "templates"));
    assert!(payload["descriptor"]["available_topics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "config"));
    let overview = export_knowledge_bundle_for_package_root(
        &package_root(),
        "author",
        Some("author_runtime_overview"),
        true,
    )
    .expect("author runtime overview");
    let overview_content = overview["assets"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["content"].as_str())
        .expect("author runtime overview content");
    assert!(
        !overview_content.contains("--surface editor"),
        "public author runtime overview should not refer to the deprecated editor surface"
    );
}

#[test]
fn knowledge_bundle_exports_access_assets() {
    let payload = export_knowledge_bundle_for_package_root(&package_root(), "access", None, false)
        .expect("access knowledge bundle");
    assert_eq!(payload["descriptor"]["surface"], "access");
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "meilang_access_skill"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "access_profile"));
    assert!(payload["assets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["descriptor"]["id"] == "access_workflow"));
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

#[test]
fn capability_catalog_rejects_editor_profile_alias() {
    use mei_lang_toolchain::ai_profile_descriptor;
    assert!(ai_profile_descriptor("editor").is_none());
    assert!(ai_profile_descriptor("author").is_some());
}

#[test]
fn world_context_snapshot_includes_world_catalog_lines() {
    let root = workspaces_root();
    clear_compile_cache_for_app(&root, DATASET_APP);
    let snapshot = build_world_context_snapshot(&root, DATASET_APP, None).expect("world snapshot");
    let lines = snapshot.prompt_catalog_lines;
    assert!(
        lines.iter().any(|line| line.contains("[World — catalog]")),
        "prompt catalog should include [World — catalog]"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("[World — query tooling]")),
        "prompt catalog should include [World — query tooling]"
    );
}
