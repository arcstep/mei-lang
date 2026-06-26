use std::fs;

use super::{compile_app_from_root, temp_root, write_file};

#[test]
fn compile_panel_base_imports_direct_world_metrics_from_multiple_sources() {
    let root = temp_root("panel-ref-import-world-metrics");
    let app_root = root.join("panel-ref-import-world-metrics");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "panel-ref-import-world-metrics", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame(
    panels = [
        panel(
            id = "summary_a",
            area = "auto",
            base = panel_ref(id = "summary_panel_a", scene_file = "panels/a.mei"),
        ),
        panel(
            id = "summary_b",
            area = "auto",
            base = panel_ref(id = "summary_panel_b", scene_file = "panels/b.mei"),
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/a.mei"),
        r#"
scene(id = "panel_a", profile = "page")
world()
world.add_metric(
    ds.scalar_map(
        id = "warning_models",
        label = "预警模型",
        values = {"value": 15},
        unit = "个",
        schema = [ds.column("value", "number")],
    ),
)
frame()
frame.add_panel(
    id = "summary_panel_a",
    area = "auto",
    blocks = [
        metric_card(
            id = "metric_a",
            source = metric_ref("warning_models"),
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/b.mei"),
        r#"
scene(id = "panel_b", profile = "page")
world()
world.add_metric(
    ds.scalar_map(
        id = "warning_supervision",
        label = "监督事项",
        values = {"value": 2000},
        unit = "项",
        schema = [ds.column("value", "number")],
    ),
)
frame()
frame.add_panel(
    id = "summary_panel_b",
    area = "auto",
    blocks = [
        metric_card(
            id = "metric_b",
            source = metric_ref("warning_supervision"),
        ),
    ],
)
"#,
    );

    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile panel_ref imported world metrics");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "panel(base=panel_ref(...)) should import external direct world metrics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .world_metrics
            .contains_key("panels/a.mei::warning_models"),
        "panel a metrics are namespaced"
    );
    assert!(
        compiled
            .world_metrics
            .contains_key("panels/b.mei::warning_supervision"),
        "panel b metrics are namespaced"
    );
    let resource_ids = compiled
        .resources
        .iter()
        .map(|resource| resource.id.as_str())
        .collect::<Vec<_>>();
    assert!(
        resource_ids
            .iter()
            .any(|id| id.contains("panels/a.mei::metrics")),
        "resources should contain imported world metrics dataset for panels/a.mei, got {resource_ids:?}"
    );
    assert!(
        resource_ids
            .iter()
            .any(|id| id.contains("panels/b.mei::metrics")),
        "resources should contain imported world metrics dataset for panels/b.mei, got {resource_ids:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_nested_panel_refs_import_direct_world_metrics_from_grandchild_source() {
    let root = temp_root("nested-panel-ref-import-world-metrics");
    let app_root = root.join("nested-panel-ref-import-world-metrics");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "nested-panel-ref-import-world-metrics", default_scene = "home")
scene(id = "home", profile = "page")
world()
frame(
    panels = [
        panel(
            id = "host",
            area = "auto",
            base = panel_ref(id = "layout_panel", scene_file = "panels/layout.mei"),
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/layout.mei"),
        r#"
scene(id = "layout_scene", profile = "page")
world()
frame()
frame.add_panel(
    id = "layout_panel",
    area = "auto",
    blocks = [
        panel(
            base = panel_ref(id = "summary_panel", scene_file = "panels/detail.mei"),
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("panels/detail.mei"),
        r#"
scene(id = "detail_scene", profile = "page")
world()
world.add_metric(
    ds.scalar_map(
        id = "warning_supervision",
        label = "监督事项",
        values = {"value": 2000},
        unit = "项",
        schema = [ds.column("value", "number")],
    ),
)
frame()
frame.add_panel(
    id = "summary_panel",
    area = "auto",
    blocks = [
        metric_card(
            id = "metric_detail",
            source = metric_ref("warning_supervision"),
        ),
    ],
)
"#,
    );

    let compiled = compile_app_from_root(&root, &app_root)
        .expect("compile nested panel_ref imported world metrics");
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
        "nested panel(base=panel_ref(...)) should import grandchild direct world metrics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .world_metrics
            .contains_key("panels/detail.mei::warning_supervision"),
        "imported panel metrics are namespaced by capsule path"
    );
    assert!(
        !compiled.world_metrics.contains_key("warning_supervision"),
        "flat metric id must not leak into host world_metrics ledger"
    );
    let resource_ids = compiled
        .resources
        .iter()
        .map(|resource| resource.id.as_str())
        .collect::<Vec<_>>();
    assert!(
        resource_ids
            .iter()
            .any(|id| id.contains("panels/detail.mei::metrics")),
        "resources should contain imported world metrics dataset for panels/detail.mei, got {resource_ids:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_metric_card_base_inherits_template_slot_vertical_align_defaults() {
    let root = temp_root("metric-card-base-v-align");
    let app_root = root.join("metric-card-base-v-align");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "metric-card-base-v-align", default_scene = "home")
scene(id = "home", profile = "cockpit", theme = "cockpit")
world(resources = [])
frame(
    panels = [
        panel(
            id = "row",
            area = "auto",
            show_heading = False,
            blocks = [
                metric_card(
                    base = metric_card_ref(id = "shell", scene_file = "templates/shell.mei"),
                    id = "live",
                    source = {"label": "现场", "value": "42", "unit": "项"},
                ),
                metric_card(
                    base = metric_card_ref(id = "shell", scene_file = "templates/shell.mei"),
                    id = "override",
                    value_vertical_align = "end",
                    source = {"label": "覆写", "value": "9", "unit": "项"},
                ),
            ],
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("templates/shell.mei"),
        r#"
scene(id = "shell_tpl", profile = "cockpit", theme = "cockpit")
world(resources = [])
frame()
frame.add_panel(
    id = "shell",
    area = "auto",
    show_heading = False,
    chrome = "bare",
    variant = "container",
    props = {
        "width": "120px",
        "height": "100px",
        "__mei_metric_card": True,
        "__mei_metric_template": "stack",
        "__mei_metric_value_v_align": "top",
    },
    layout = layout_metric_stack(),
    blocks = [
        label("模板", area = "label", vertical_align = "center"),
        value("--", area = "value"),
        unit("", area = "unit"),
    ],
)
"#,
    );
    let compiled =
        compile_app_from_root(&root, &app_root).expect("compile metric_card base v_align");
    let contract = compiled.scene_contract.expect("contract");
    fn find_panel_by_id<'a>(
        panels: &'a [crate::PanelDecl],
        target: &str,
    ) -> Option<&'a crate::PanelDecl> {
        for panel in panels {
            if panel.id == target {
                return Some(panel);
            }
            for node in &panel.blocks {
                if let crate::UiNodeDecl::Panel(nested) = node {
                    if let Some(found) = find_panel_by_id(std::slice::from_ref(nested), target) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
    fn value_slot_v_align(panel: &crate::PanelDecl) -> Option<&str> {
        panel.blocks.iter().find_map(|node| {
            let crate::UiNodeDecl::Block(block) = node else {
                return None;
            };
            if block.props.get("metric_role").and_then(|v| v.as_str()) != Some("value") {
                return None;
            }
            block.props.get("metric_v_align").and_then(|v| v.as_str())
        })
    }
    let live = find_panel_by_id(&contract.panels, "live").expect("live");
    assert_eq!(
        value_slot_v_align(live),
        Some("top"),
        "template __mei_metric_value_v_align should apply after base+source clone"
    );
