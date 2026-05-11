use std::collections::BTreeMap;

use leptos::prelude::*;
use mei_lang_kernel::{BlockDecl, CompiledApp, LoadedResource, SceneContract};
use serde_json::Value;

pub(super) fn preview_view(compiled: &CompiledApp) -> AnyView {
    let resource_map = compiled
        .resources
        .iter()
        .map(|resource| (resource.id.clone(), resource.clone()))
        .collect::<BTreeMap<_, _>>();

    if let Some(scene_contract) = &compiled.scene_contract {
        if let Some(frame) = &scene_contract.frame {
            let panels = scene_contract
                .panels
                .iter()
                .map(|panel| {
                    panel_view(
                        panel,
                        frame.layout.as_ref(),
                        compiled,
                        scene_contract,
                        &resource_map,
                    )
                })
                .collect_view();
            return view! {
                <section class="preview-surface" style=surface_layout_style(frame.layout.as_ref())>
                    {panels}
                </section>
            }
            .into_any();
        }

        return view! {
            <section class="scene-placeholder">
                <h3>{scene_contract.scene.id.clone()}</h3>
                <p>{scene_contract.scene.summary.clone().unwrap_or_else(|| "已生成 scene contract，运行态将在后续阶段接入。".to_string())}</p>
                <ul>
                    <li>{format!("观察面区块：{}", scene_contract.panels.len())}</li>
                    <li>{format!("目标：{}", scene_contract.scene.goal.clone().unwrap_or_else(|| "未声明".to_string()))}</li>
                </ul>
            </section>
        }
        .into_any();
    }

    view! { <div class="empty-preview">"当前入口还没有可渲染的 frame 或 scene。"</div> }.into_any()
}

fn panel_view(
    panel: &mei_lang_kernel::PanelDecl,
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
) -> AnyView {
    let blocks = panel
        .blocks
        .iter()
        .map(|block| block_view(block, compiled, scene_contract, resources))
        .collect_view();
    let title = panel.title.clone().unwrap_or_else(|| panel.id.clone());
    view! {
        <section class="preview-card" style=panel_style(panel.area.as_deref(), layout)>
            <div class="panel-heading">
                <h3>{title}</h3>
                <p>{panel.area.clone().unwrap_or_else(|| "auto".to_string())}</p>
            </div>
            <div class="panel-body">
                {blocks}
            </div>
        </section>
    }
    .into_any()
}

fn block_view(
    block: &BlockDecl,
    compiled: &CompiledApp,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
) -> AnyView {
    let props = attach_host_meta(resolve_value(&block.props, scene_contract, resources), compiled);
    let tag = compiled
        .component_assets
        .iter()
        .find(|asset| asset.key == block.use_key)
        .map(|asset| asset.tag.clone())
        .unwrap_or_else(|| "mei-missing-component".to_string());
    let html = component_html(tag.as_str(), &props);
    view! {
        <section class="component-card">
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}

fn attach_host_meta(mut props: Value, compiled: &CompiledApp) -> Value {
    if let Some(map) = props.as_object_mut() {
        map.insert(
            "_mei".to_string(),
            serde_json::json!({
                "app_id": compiled.app_id,
                "entry_target": compiled.entry_target,
                "step_api": format!("/api/sim/step/{}", compiled.app_id),
            }),
        );
    }
    props
}

fn resolve_value(
    value: &Value,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
) -> Value {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("world") {
                if let Some(id) = map.get("id").and_then(Value::as_str) {
                    if let Some(resource) = resources.get(id) {
                        return serde_json::to_value(resource).unwrap_or(Value::Null);
                    }
                }
            }
            if map.get("__ref").and_then(Value::as_str) == Some("scene") {
                return serde_json::to_value(scene_contract).unwrap_or(Value::Null);
            }
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some(dataset) = resolve_data_ref(map, resources) {
                    return serde_json::to_value(dataset).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                if let Some(metric) = resolve_metric_ref(map, resources) {
                    return serde_json::to_value(metric).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            if map.get("metric").and_then(Value::as_str).is_some() {
                let mut compat = serde_json::Map::new();
                compat.insert("__ref".to_string(), Value::String("metric".to_string()));
                if let Some(id) = map.get("metric").cloned() {
                    compat.insert("id".to_string(), id);
                }
                if let Some(from) = map
                    .get("from_dataset")
                    .cloned()
                    .or_else(|| map.get("from").cloned())
                {
                    compat.insert("from_dataset".to_string(), from);
                }
                if let Some(metric) = resolve_metric_ref(&compat, resources) {
                    return serde_json::to_value(metric).unwrap_or(Value::Null);
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("rows")
            {
                if let Some(dataset) = resolve_rows_expr(map, resources) {
                    return serde_json::to_value(dataset).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            let mut out = serde_json::Map::new();
            for (key, entry) in map {
                out.insert(key.clone(), resolve_value(entry, scene_contract, resources));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_value(item, scene_contract, resources))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn resolve_data_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::DatasetView> {
    let id = map.get("id").and_then(Value::as_str)?;
    let from_dataset = map.get("from_dataset").and_then(Value::as_str);
    let dataset_id = from_dataset.unwrap_or(id);
    resources.get(dataset_id)?.dataset.clone()
}

fn resolve_metric_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::MetricContract> {
    let metric_id = map.get("id").and_then(Value::as_str)?;
    if let Some(dataset_id) = map.get("from_dataset").and_then(Value::as_str) {
        return resources
            .get(dataset_id)?
            .dataset
            .as_ref()?
            .metrics
            .get(metric_id)
            .cloned();
    }
    resources
        .values()
        .filter_map(|resource| resource.dataset.as_ref())
        .find_map(|dataset| dataset.metrics.get(metric_id).cloned())
}

fn resolve_rows_expr(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::DatasetView> {
    let dataset = map
        .get("dataset")
        .and_then(Value::as_str)
        .map(|value| value.strip_prefix("dataset.").unwrap_or(value).to_string())?;
    resources.get(&dataset)?.dataset.clone()
}

fn component_html(tag: &str, props: &Value) -> String {
    let props = escape_html_attr(&serde_json::to_string(props).unwrap_or_else(|_| "{}".to_string()));
    format!("<{tag} data-props=\"{props}\"></{tag}>")
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn surface_layout_style(layout: Option<&mei_lang_kernel::LayoutDecl>) -> String {
    let Some(layout) = layout else {
        return "display:grid;gap:16px;".to_string();
    };
    match layout.layout_type.as_str() {
        "flex" => format!(
            "display:flex;flex-direction:{};gap:{};padding:{};",
            layout.direction.clone().unwrap_or_else(|| "column".to_string()),
            layout.gap.clone().unwrap_or_else(|| "16px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
        _ => format!(
            "display:grid;grid-template-columns:{};grid-template-rows:{};{}gap:{};padding:{};",
            layout
                .columns
                .clone()
                .unwrap_or_else(|| vec!["1fr".to_string()])
                .join(" "),
            layout
                .rows
                .clone()
                .unwrap_or_else(|| vec!["auto".to_string()])
                .join(" "),
            grid_template_areas_style(layout),
            layout.gap.clone().unwrap_or_else(|| "16px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
    }
}

fn panel_style(area: Option<&str>, layout: Option<&mei_lang_kernel::LayoutDecl>) -> String {
    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && layout
            .and_then(|value| value.areas.as_ref())
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    {
        if let Some(area) = area {
            return format!("grid-area:{};", area);
        }
    }
    String::new()
}

fn grid_template_areas_style(layout: &mei_lang_kernel::LayoutDecl) -> String {
    let Some(rows) = layout.areas.as_ref() else {
        return String::new();
    };
    let rows = rows
        .iter()
        .filter(|row| !row.is_empty())
        .map(|row| {
            let template = row
                .iter()
                .map(|area| {
                    let area = area.trim();
                    if area.is_empty() { "." } else { area }
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("'{template}'")
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        String::new()
    } else {
        format!("grid-template-areas:{};", rows.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{panel_style, resolve_value, surface_layout_style};
    use mei_lang_kernel::{
        ColumnSchema, DatasetView, LayoutDecl, LoadedResource, MetricContract, MetricShape,
        SceneContract, SceneDecl, SourceDecl,
    };
    use serde_json::json;

    fn grid_layout() -> LayoutDecl {
        LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["1fr".to_string(), "2fr".to_string()]),
            rows: None,
            areas: Some(vec![vec!["doc".to_string(), "table".to_string()]]),
            gap: Some("16px".to_string()),
            padding: Some("20px".to_string()),
        }
    }

    #[test]
    fn surface_layout_style_emits_grid_template_areas() {
        let layout = grid_layout();
        let style = surface_layout_style(Some(&layout));
        assert!(style.contains("grid-template-areas:'doc table';"));
    }

    #[test]
    fn panel_style_requires_named_grid_areas() {
        let mut layout = grid_layout();
        layout.areas = None;
        assert_eq!(panel_style(Some("doc"), Some(&layout)), "");
    }

    #[test]
    fn resolve_value_supports_data_and_metric_refs() {
        let scene_contract = SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                profile: None,
                summary: None,
                goal: None,
                state: json!({}),
            },
            world: None,
            flow: None,
            frame: None,
            panels: vec![],
        };
        let mut resources = BTreeMap::new();
        resources.insert(
            "sales_metrics".to_string(),
            LoadedResource {
                id: "sales_metrics".to_string(),
                kind: "dataset".to_string(),
                title: Some("Sales".to_string()),
                document: None,
                dataset: Some(DatasetView {
                    id: "sales_metrics".to_string(),
                    title: Some("Sales".to_string()),
                    purpose: None,
                    schema: vec![
                        ColumnSchema {
                            name: "label".to_string(),
                            type_name: "string".to_string(),
                            source: None,
                            optional: false,
                            unit: None,
                        },
                        ColumnSchema {
                            name: "value".to_string(),
                            type_name: "number".to_string(),
                            source: None,
                            optional: false,
                            unit: Some("元".to_string()),
                        },
                    ],
                    stage_schema: Vec::new(),
                    columns: vec!["label".to_string(), "value".to_string()],
                    rows: vec![json!({"label":"A","value":"100"})],
                    source: SourceDecl {
                        kind: "derived".to_string(),
                        path: "dataset_view:sales_metrics".to_string(),
                        content: None,
                    },
                    sources: Vec::new(),
                    metrics: BTreeMap::from([(
                        "sales_total".to_string(),
                        MetricContract {
                            id: "sales_total".to_string(),
                            label: Some("销售总额".to_string()),
                            purpose: None,
                            shape: MetricShape::Scalar,
                            schema: vec![ColumnSchema {
                                name: "total_value".to_string(),
                                type_name: "number".to_string(),
                                source: None,
                                optional: false,
                                unit: Some("元".to_string()),
                            }],
                            dataset: None,
                            transforms: Vec::new(),
                            value: json!({"total_value": 100}),
                        },
                    )]),
                }),
            },
        );

        let data_ref = json!({"__ref":"data","id":"sales_metrics"});
        let resolved_data = resolve_value(&data_ref, &scene_contract, &resources);
        assert_eq!(resolved_data.get("id").and_then(|value| value.as_str()), Some("sales_metrics"));

        let metric_ref = json!({"__ref":"metric","id":"sales_total","from_dataset":"sales_metrics"});
        let resolved_metric = resolve_value(&metric_ref, &scene_contract, &resources);
        assert_eq!(
            resolved_metric.get("id").and_then(|value| value.as_str()),
            Some("sales_total")
        );
    }
}
