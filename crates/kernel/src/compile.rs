use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{anyhow, Context, Result};
use csv::StringRecord;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    eval::evaluate_mei_file,
    model::{
        AppDecl, CompiledApp, ComponentAsset, DatasetView, Diagnostic, EntryDecl, FlowDecl,
        FrameDecl, LoadedResource, PanelDecl, ResourceDecl, SceneContract, SceneDecl, Severity,
        SourceDecl,
    },
    workspace::{load_component_assets, source_tree},
};

pub fn compile_app(source_root: &Path, app_id: &str) -> Result<CompiledApp> {
    let app_root = source_root.join(app_id);
    compile_app_from_root(source_root, &app_root)
}

pub fn compile_app_from_root(source_root: &Path, app_root: &Path) -> Result<CompiledApp> {
    let app_main = app_root.join("main.mei");
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let (entry_target, entry_decls) =
        resolve_scene_source(app_root, &app_main, &app_decl, &app_decls, &mut diagnostics)?;

    let mut frame: Option<FrameDecl> = None;
    let mut scene: Option<SceneDecl> = None;
    let mut world: Option<crate::model::WorldDecl> = None;
    let mut flow: Option<FlowDecl> = None;
    let mut panels: Vec<PanelDecl> = Vec::new();

    if let Some(values) = entry_decls.as_array() {
        for value in values {
            let Some(kind) = value.get("kind").and_then(Value::as_str) else {
                continue;
            };
            match kind {
                "frame" => frame = Some(serde_json::from_value(value.clone())?),
                "scene" => scene = Some(serde_json::from_value(value.clone())?),
                "world" => world = Some(serde_json::from_value(value.clone())?),
                "flow" => flow = Some(serde_json::from_value(value.clone())?),
                "panel" => panels.push(serde_json::from_value(value.clone())?),
                "app" | "app_scene_ref" => {}
                _ => diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_decl".to_string(),
                    message: format!("unknown declaration kind `{kind}`"),
                    source_path: Some(entry_target.clone()),
                }),
            }
        }
    }

    let asset_map = load_component_assets(source_root)?;
    let mut asset_keys = BTreeSet::new();
    for panel in &panels {
        for block in &panel.blocks {
            asset_keys.insert(block.use_key.clone());
        }
    }
    let component_assets = asset_keys
        .into_iter()
        .filter_map(|key| asset_map.get(&key).cloned())
        .collect::<Vec<ComponentAsset>>();

    if scene.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene".to_string(),
            message: "entry file must declare scene(...) for scene-first authoring".to_string(),
            source_path: Some(entry_target.clone()),
        });
    }

    if frame.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "missing_frame".to_string(),
            message: "scene entry should declare frame(...) to define UI layout".to_string(),
            source_path: Some(entry_target.clone()),
        });
    }

    let title = app_decl
        .title
        .clone()
        .unwrap_or_else(|| app_decl.id.clone());

    let resources = match world.as_ref() {
        Some(world_decl) => load_resources(app_root, &world_decl.resources)?,
        None => Vec::new(),
    };

    let scene_contract = scene.map(|scene_decl| SceneContract {
        scene: scene_decl,
        world,
        flow,
        frame: frame.clone(),
        panels,
    });

    Ok(CompiledApp {
        app_id: app_decl.id.clone(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        entry_target,
        file_tree: source_tree(app_root)?,
        scene_contract,
        resources,
        component_assets,
        diagnostics,
    })
}

fn decode_app_decl(path: &Path, raw: &Value) -> (Option<AppDecl>, Vec<Diagnostic>) {
    let mut app_decl = None;
    let mut diagnostics = Vec::new();
    if let Some(values) = raw.as_array() {
        for value in values {
            if value.get("kind").and_then(Value::as_str) == Some("app") {
                match serde_json::from_value::<AppDecl>(value.clone()) {
                    Ok(decl) => app_decl = Some(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_app_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(path.to_string_lossy().to_string()),
                    }),
                }
            }
        }
    }
    (app_decl, diagnostics)
}

#[derive(Debug, Clone, Deserialize)]
struct SceneFileRefDecl {
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub id: Option<String>,
}

fn resolve_scene_source(
    app_root: &Path,
    app_main: &Path,
    app_decl: &AppDecl,
    app_decls: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(String, Value)> {
    if let Some(entry) = app_decl.entries.first().cloned() {
        let entry_target = entry_source(&entry).unwrap_or_else(|| "main.mei".to_string());
        let entry_path = app_root.join(&entry_target);
        return Ok((entry_target, evaluate_mei_file(&entry_path)?));
    }

    if let Some(default_scene) = app_decl.default_scene.as_deref() {
        if let Some(target) = resolve_default_scene_target(app_decls, default_scene) {
            if target == "main.mei" {
                return Ok((target, app_decls.clone()));
            }
            let entry_path = app_root.join(&target);
            return Ok((target, evaluate_mei_file(&entry_path)?));
        }

        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_default_scene".to_string(),
            message: format!(
                "default_scene `{default_scene}` did not match an inline scene or app.add_scene(scene_file_ref(...))"
            ),
            source_path: Some(app_main.to_string_lossy().to_string()),
        });
    }

    if has_inline_scene(app_decls, None) {
        return Ok(("main.mei".to_string(), app_decls.clone()));
    }

    if let Some(target) = first_scene_file_ref_target(app_decls) {
        let entry_path = app_root.join(&target);
        return Ok((target, evaluate_mei_file(&entry_path)?));
    }

    Ok(("main.mei".to_string(), app_decls.clone()))
}

fn resolve_default_scene_target(raw: &Value, default_scene: &str) -> Option<String> {
    if has_inline_scene(raw, Some(default_scene)) {
        return Some("main.mei".to_string());
    }

    scene_file_ref_target(raw, default_scene)
}

fn has_inline_scene(raw: &Value, scene_id: Option<&str>) -> bool {
    raw.as_array().is_some_and(|values| {
        values.iter().any(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("scene") {
                return false;
            }
            match scene_id {
                Some(expected) => value.get("id").and_then(Value::as_str) == Some(expected),
                None => true,
            }
        })
    })
}

fn scene_file_ref_target(raw: &Value, scene_id: &str) -> Option<String> {
    raw.as_array().and_then(|values| {
        values.iter().find_map(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
                return None;
            }
            let scene_ref = serde_json::from_value::<SceneFileRefDecl>(
                value.get("scene").cloned().unwrap_or(Value::Null),
            )
            .ok()?;
            if scene_ref.kind != "scene_file_ref" {
                return None;
            }
            if scene_ref.id.as_deref() == Some(scene_id)
                || (scene_ref.id.is_none() && scene_name_from_path(&scene_ref.path) == scene_id)
            {
                return Some(scene_ref.path);
            }
            None
        })
    })
}

fn first_scene_file_ref_target(raw: &Value) -> Option<String> {
    raw.as_array().and_then(|values| {
        values.iter().find_map(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
                return None;
            }
            let scene_ref = serde_json::from_value::<SceneFileRefDecl>(
                value.get("scene").cloned().unwrap_or(Value::Null),
            )
            .ok()?;
            if scene_ref.kind == "scene_file_ref" {
                Some(scene_ref.path)
            } else {
                None
            }
        })
    })
}

fn scene_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

fn entry_source(entry: &EntryDecl) -> Option<String> {
    entry
        .scene
        .clone()
        .or_else(|| entry.frame.clone())
}

fn load_resources(app_root: &Path, resources: &[ResourceDecl]) -> Result<Vec<LoadedResource>> {
    resources
        .iter()
        .map(|resource| load_resource(app_root, resource))
        .collect()
}

fn load_resource(app_root: &Path, resource: &ResourceDecl) -> Result<LoadedResource> {
    match resource.kind.as_str() {
        "document" => {
            let document = match (&resource.content, &resource.source) {
                (Some(content), _) => Some(content.clone()),
                (_, Some(source)) if source.kind == "markdown" => {
                    Some(load_markdown_content(app_root, source)?)
                }
                _ => None,
            };
            Ok(LoadedResource {
                id: resource.id.clone(),
                kind: resource.kind.clone(),
                title: resource.title.clone(),
                document,
                dataset: None,
            })
        }
        "dataset" => Ok(LoadedResource {
            id: resource.id.clone(),
            kind: resource.kind.clone(),
            title: resource.title.clone(),
            document: None,
            dataset: Some(load_dataset_view(app_root, resource)?),
        }),
        _ => Ok(LoadedResource {
            id: resource.id.clone(),
            kind: resource.kind.clone(),
            title: resource.title.clone(),
            document: resource.content.clone(),
            dataset: None,
        }),
    }
}

fn load_markdown_content(app_root: &Path, source: &SourceDecl) -> Result<String> {
    let path = app_root.join(&source.path);
    std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read markdown resource {}", path.display()))
}

fn load_dataset_view(app_root: &Path, resource: &ResourceDecl) -> Result<DatasetView> {
    let source = resource
        .source
        .as_ref()
        .ok_or_else(|| anyhow!("dataset resource `{}` missing source", resource.id))?;
    let path = app_root.join(&source.path);
    let mut reader = csv::Reader::from_path(&path)
        .with_context(|| format!("failed to open dataset {}", path.display()))?;
    let headers = reader
        .headers()
        .context("failed to read csv headers")?
        .clone();
    let columns = headers.iter().map(|value| value.to_string()).collect::<Vec<_>>();
    let rows = reader
        .records()
        .map(|record| {
            let record = record.context("failed to read csv row")?;
            Ok::<_, anyhow::Error>(csv_record_to_json(&headers, &record))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DatasetView {
        id: resource.id.clone(),
        title: resource.title.clone(),
        columns,
        rows,
        source: source.clone(),
    })
}

fn csv_record_to_json(headers: &StringRecord, record: &StringRecord) -> Value {
    let mut out = BTreeMap::new();
    for (idx, header) in headers.iter().enumerate() {
        let value = record.get(idx).unwrap_or_default();
        out.insert(header.to_string(), Value::String(value.to_string()));
    }
    json!(out)
}

#[cfg(test)]
mod tests {
    use super::compile_app_from_root;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mei-lang-kernel-{name}-{nonce}"))
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn compile_supports_inline_default_scene_authoring() {
        let root = temp_root("inline-default-scene");
        let app_root = root.join("demo");
        write_file(
            &app_root.join("main.mei"),
            r#"
app(
    id = "demo",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

scene.set_world(
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
    ],
)
"#,
        );

        let compiled = compile_app_from_root(&root, &app_root).expect("compile inline scene app");
        assert_eq!(compiled.entry_target, "main.mei");
        let contract = compiled.scene_contract.expect("scene contract");
        assert_eq!(contract.scene.id, "home");
        assert_eq!(contract.world.expect("world").resources.len(), 1);
        assert_eq!(contract.panels.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn compile_supports_scene_file_ref_authoring() {
        let root = temp_root("scene-file-ref");
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

frame.add_panel(
    id = "status",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "hello"),
    ],
)
"#,
        );

        let compiled = compile_app_from_root(&root, &app_root).expect("compile external scene app");
        assert_eq!(compiled.entry_target, "home.mei");
        let contract = compiled.scene_contract.expect("scene contract");
        assert_eq!(contract.scene.id, "room_fire_click");
        assert_eq!(contract.panels.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }
}
