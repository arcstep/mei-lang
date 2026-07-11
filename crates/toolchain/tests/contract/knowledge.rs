use super::support::*;

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
