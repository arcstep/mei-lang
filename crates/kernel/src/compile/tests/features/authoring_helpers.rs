use std::fs;

use crate::compile::compile_app_from_root_with_options;
use crate::compile::CompileOptions;
use crate::describe_dsl_with_helpers;
use crate::eval::{evaluate_mei_file, push_authoring_helpers};
use crate::mei_config::resolve_authoring_helpers;

use super::{temp_root, write_file};

#[test]
fn workspace_authoring_helpers_merge_public_surface() {
    let source_root = temp_root("authoring-helper-surface");
    let authoring_dir = source_root.join(".stock/authoring");
    write_file(
        &authoring_dir.join("demo-fields.star"),
        r#"
def demo_filter_fields():
    return [
        filter_field(key = "region", label = "Region", column = "region", control = "multi_select"),
    ]

def demo_detail_fields():
    return ["id", "region", "amount"]
"#,
    );
    write_file(
        &source_root.join(".mei-workspace.json"),
        r#"{"paths":{"authoring":".stock/authoring"}}"#,
    );
    let helpers = resolve_authoring_helpers(&source_root).expect("resolve helpers");
    assert!(
        helpers
            .public_functions
            .contains(&"demo_filter_fields".to_string()),
        "expected demo_filter_fields, got {:?}",
        helpers.public_functions
    );
    let dsl = describe_dsl_with_helpers(Some(&helpers));
    let surface = dsl
        .get("public_surface")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        surface
            .iter()
            .any(|item| item.as_str() == Some("demo_detail_fields")),
        "describe_dsl should merge workspace helper surface"
    );
}

#[test]
fn workspace_authoring_helpers_evaluate_world_with_injected_defs() {
    let source_root = temp_root("authoring-helper-eval");
    let app_root = source_root.join("demo");
    let authoring_dir = source_root.join(".stock/authoring");
    write_file(
        &source_root.join(".mei-workspace.json"),
        r#"{"paths":{"authoring":".stock/authoring"}}"#,
    );
    write_file(
        &authoring_dir.join("demo-predicates.star"),
        r#"
def is_flagged(field = "status"):
    return in_values(field, ["yes", "Y"])
"#,
    );
    write_file(
        &app_root.join("capsule.world.mei"),
        r#"
world()
world.add_metric(
    ds.scalar_map(
        id = "flagged_count",
        label = "Flagged",
        values = {"value": ds.count(ds.where(ds.data_ref("rows"), is_flagged("status")))},
        schema = [ds.column("value", "number")],
    ),
)
world.add_dataset(
    id = "rows",
    source = ds.csv(path = "data/rows.csv"),
    schema = [
        ds.column("status", "string"),
    ],
)
"#,
    );
    write_file(&app_root.join("data/rows.csv"), "status\nyes\nno\n");
    let helpers = resolve_authoring_helpers(&source_root).expect("resolve helpers");
    let _guard = push_authoring_helpers(helpers);
    evaluate_mei_file(&app_root.join("capsule.world.mei"))
        .unwrap_or_else(|error| panic!("evaluate world with injected helper failed: {error}"));
}

#[test]
fn compile_app_with_workspace_authoring_helpers_installed() {
    let source_root = temp_root("authoring-helper-compile");
    let app_root = source_root.join("demo");
    write_file(
        &source_root.join(".mei-workspace.json"),
        r#"{"paths":{"authoring":".stock/authoring"}}"#,
    );
    write_file(
        &source_root.join(".stock/authoring/helpers.star"),
        r#"
def board_filter_fields():
    return [filter_field(key = "region", label = "Region", column = "region")]
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "demo", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame()
"#,
    );
    compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
        .unwrap_or_else(|error| panic!("compile with authoring helpers failed: {error}"));
    let _ = fs::remove_dir_all(&source_root);
}
