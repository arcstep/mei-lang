use std::fs;

use super::{
    compile_app_from_root, compile_app_from_root_with_options, temp_root, write_file,
    CompileOptions,
};

#[test]
fn compile_collects_scene_params_for_scene_contracts() {
    let root = temp_root("scene-params-scene-contract");
    let app_root = root.join("params-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "params-app", scene = scene_ref(scene_file = "home.mei"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(
    id = "home",
    profile = "page",
    params = {
        "metric": param(type = "metric", required = True),
        "entry": param(type = "string"),
    },
)
world(resources = [])
frame(layout = flex(direction = "column"))
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile scene params");
    let contract = compiled.scene_contract.expect("scene contract");
    assert_eq!(
        contract
            .scene
            .params
            .get("metric")
            .and_then(|value| value.get("__kind"))
            .and_then(|value| value.as_str()),
        Some("scene_param")
    );
    assert_eq!(
        compiled
            .scene_projection_assembly_by_id
            .get("home")
            .and_then(|value| value.get("params"))
            .and_then(|value| value.get("entry"))
            .and_then(|value| value.get("type"))
            .and_then(|value| value.as_str()),
        Some("string")
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_reports_invalid_scene_params_value() {
    let root = temp_root("scene-params-invalid");
    let app_root = root.join("params-invalid-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "params-invalid-app", scene = scene_ref(scene_file = "home.mei"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(
    id = "home",
    profile = "page",
    params = {
        "metric": "not-a-param",
    },
)
world(resources = [])
frame(layout = flex(direction = "column"))
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root).expect("compile invalid scene params");
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "invalid_scene_param_value"),
        "expected invalid_scene_param_value diagnostic, got {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_reports_missing_required_scene_binding() {
    let root = temp_root("scene-binding-required");
    let app_root = root.join("binding-required-app");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "binding-required-app", scene = scene_ref(scene_file = "home.mei"))
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
scene(profile = "page")
world(
    resources = [
        ds.dataset_resource(
            id = "sales",
            source = ds.csv("sales.csv"),
            binding = {
                "enabled": True,
                "required": True,
                "replace": "source",
                "accept": {"kind": "dataset"},
            },
        ),
    ],
)
frame(layout = flex(direction = "column"))
"#,
    );
    write_file(&app_root.join("sales.csv"), "name,value\nA,42\n");

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile missing required binding app");
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "missing_required_scene_binding"),
        "expected missing_required_scene_binding diagnostic, got {:?}",
        compiled.diagnostics
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_scene_file_ref_main_target_skips_scene_first_missing_diagnostics() {
    let root = temp_root("scene-file-ref-main-target");
    let app_root = root.join("fire");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "fire",
    default_scene = "room_fire_click",
)

app.add_scene(
    scene_file_ref("home.mei", id = "room_fire_click"),
)
"#,
    );
    write_file(
        &app_root.join("home.mei"),
        r#"
app.add_scene(
    id = "room_fire_click",
    profile = "simulation",
)

scene.set_frame(
    layout = flex(direction = "column"),
)
"#,
    );

    let compiled = compile_app_from_root_with_options(
        &root,
        &app_root,
        CompileOptions {
            preview_target: Some("main.mei".to_string()),
            ..CompileOptions::default()
        },
    )
    .expect("compile main target");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| diag.code != "missing_scene" && diag.code != "missing_frame"),
        "legacy scene-file-ref main target should not report scene-first missing diagnostics"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "deprecated_scene_file_ref"),
        "legacy scene_file_ref main target should emit migration warning: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "previewing main.mei should fallback to default scene payload when main is index-only"
    );
    assert_eq!(compiled.active_scene.as_deref(), Some("room_fire_click"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_refs_scenario1_local_resource_ref_in_props() {
    let root = temp_root("refs-scenario-1");
    let app_root = root.join("refs-01");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-01", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "welcome_doc", kind = "document", content = "hello")])
frame()
frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = resource_ref("welcome_doc"))],
)
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 1");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 1 should compile without errors: {:?}",
        compiled.diagnostics
    );
    assert!(compiled.scene_contract.is_some());
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "forbidden_direct_ui_data_binding"),
        "local resource_ref in props should be allowed"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_refs_scenario2_world_metrics_from_local_dataset_view() {
    let root = temp_root("refs-scenario-2");
    let app_root = root.join("refs-02");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "refs-02", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [resource(id = "sales_data", kind = "dataset", source = ds.csv(path = "data/sales.csv"))])
rows = ds.data_ref("sales_data")
world.add_dataset_view(
    id = "sales_view",
    rowset = rows,
    schema = [ds.column("label", "string"), ds.column("value", "number")],
)
world.add_metric(
    ds.scalar_map(
        id = "overview",
        schema = [ds.column("total_rows", "number")],
        values = {"total_rows": ds.count(rows)},
    ),
)
world.add_metric(
    ds.computed_metric(key = "row_count", dataset = "sales_view", op = ds.count_rows(), fallback = 0),
)
frame()
frame.add_panel(
    id = "summary",
    area = "auto",
    blocks = [doc.markdown(area = "auto", resource = metric_ref("row_count"))],
)
"#,
    );
    write_file(&app_root.join("data/sales.csv"), "label,value\nA,1\n");
    let compiled = compile_app_from_root(&root, &app_root).expect("compile refs scenario 2");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "scenario 2 should compile without errors: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.world_metrics.contains_key("overview"),
        "world ledger should include scalar_map metric"
    );
    assert!(
        compiled.world_metrics.contains_key("row_count"),
        "world ledger should include computed metric"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|item| item.id == "sales_view"),
        "dataset view should materialize for computed_metric dataset="
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_resource_base_clones_from_world_ref() {
    let root = temp_root("resource-base-clone");
    let app_root = root.join("res-base");
    write_file(
        &app_root.join("shared.mei"),
        r#"
scene(id = "s", profile = "page")
world(resources = [resource(id = "doc", kind = "document", content = "base")])
"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "res-base", default_scene = "home")
scene(id = "home", profile = "page")
world(resources = [
    resource(
        id = "doc",
        kind = "document",
        base = resource_ref(id = "doc", scene_file = "shared.mei"),
        title = "覆盖",
    ),
])
frame()
"#,
    );
    let compiled = compile_app_from_root(&root, &app_root).expect("compile resource base");
    let contract = compiled.scene_contract.expect("contract");
    let world = contract.world.expect("world");
    let res = world.resources.iter().find(|r| r.id == "doc").expect("doc");
    assert_eq!(res.title.as_deref(), Some("覆盖"));
    assert_eq!(res.kind, "document");
    let _ = fs::remove_dir_all(&root);
}
