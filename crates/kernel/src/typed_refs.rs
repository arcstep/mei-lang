use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Scene,
    World,
    Flow,
    Frame,
    Panel,
    Dataset,
    Metric,
    Resource,
    Entity,
    Component,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct SceneLocator {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_tab: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

impl SceneLocator {
    pub fn new(
        scene_id: Option<String>,
        scene_file: Option<String>,
        entry_tab: Option<String>,
        entry: Option<String>,
    ) -> Self {
        Self {
            scene_id,
            scene_file,
            entry_tab,
            entry,
        }
    }

    pub fn with_file(path: impl Into<String>) -> Self {
        Self {
            scene_id: None,
            scene_file: Some(path.into().to_string()),
            entry_tab: None,
            entry: None,
        }
    }

    pub fn with_id(id: impl Into<String>) -> Self {
        Self {
            scene_id: Some(id.into().to_string()),
            scene_file: None,
            entry_tab: None,
            entry: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefExpr {
    pub kind: RefKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(flatten)]
    pub locator: SceneLocator,
    /// `component_ref` 的 `use`（与 block `id` 二选一用于定位源组件）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_key: Option<String>,
}

impl RefExpr {
    pub fn new(kind: RefKind, id: Option<String>, locator: SceneLocator) -> Self {
        Self {
            kind,
            id,
            locator,
            use_key: None,
        }
    }

    pub fn scene(
        scene_id: Option<String>,
        scene_file: Option<String>,
        entry_tab: Option<String>,
        entry: Option<String>,
    ) -> Self {
        Self::new(
            RefKind::Scene,
            None,
            SceneLocator::new(scene_id, scene_file, entry_tab, entry),
        )
    }

    pub fn world(id: Option<String>, locator: SceneLocator) -> Self {
        Self::new(RefKind::World, id, locator)
    }

    pub fn flow(id: Option<String>, locator: SceneLocator) -> Self {
        Self::new(RefKind::Flow, id, locator)
    }

    pub fn frame(locator: SceneLocator) -> Self {
        Self::new(RefKind::Frame, None, locator)
    }

    pub fn panel(id: impl Into<String>, locator: SceneLocator) -> Self {
        Self::new(RefKind::Panel, Some(id.into()), locator)
    }

    pub fn dataset(id: impl Into<String>, locator: SceneLocator) -> Self {
        Self::new(RefKind::Dataset, Some(id.into()), locator)
    }

    pub fn metric(id: impl Into<String>, locator: SceneLocator) -> Self {
        Self::new(RefKind::Metric, Some(id.into()), locator)
    }

    pub fn resource(id: impl Into<String>, locator: SceneLocator) -> Self {
        Self::new(RefKind::Resource, Some(id.into()), locator)
    }

    pub fn entity(id: impl Into<String>, locator: SceneLocator) -> Self {
        Self::new(RefKind::Entity, Some(id.into()), locator)
    }

    pub fn component(id: Option<String>, use_key: Option<String>, locator: SceneLocator) -> Self {
        Self {
            kind: RefKind::Component,
            id,
            locator,
            use_key,
        }
    }
}

#[derive(Debug, Clone)]
pub enum BindingValue {
    Ref(RefExpr),
    Inline(Value),
}

impl BindingValue {
    pub fn as_ref(&self) -> Option<&RefExpr> {
        match self {
            Self::Ref(expr) => Some(expr),
            Self::Inline(_) => None,
        }
    }

    pub fn into_ref(self) -> Option<RefExpr> {
        match self {
            Self::Ref(expr) => Some(expr),
            Self::Inline(_) => None,
        }
    }
}

pub fn decode_ref_value(value: &Value) -> Option<RefExpr> {
    if let Some(obj) = value.as_object() {
        if let Some(kind) = obj.get("__ref").and_then(Value::as_str) {
            let ref_kind = match kind {
                "scene" => RefKind::Scene,
                "world" => RefKind::World,
                "flow" => RefKind::Flow,
                "frame" => RefKind::Frame,
                "panel" => RefKind::Panel,
                "dataset" => RefKind::Dataset,
                "metric" => RefKind::Metric,
                "resource" => RefKind::Resource,
                "entity" => RefKind::Entity,
                "component" => RefKind::Component,
                "data" => RefKind::Dataset,
                _ => return None,
            };
            let id = obj
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_string);
            let use_key = obj
                .get("use")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(str::to_string);
            let locator = decode_locator(obj);
            return Some(RefExpr {
                kind: ref_kind,
                id,
                locator,
                use_key,
            });
        }
        if let Some(kind) = obj.get("kind").and_then(Value::as_str) {
            return decode_legacy_file_ref(kind, obj);
        }
        if obj.get("path").and_then(Value::as_str).is_some() {
            return decode_legacy_file_ref("scene_file_ref", obj)
                .or_else(|| decode_legacy_file_ref("world_file_ref", obj))
                .or_else(|| decode_legacy_file_ref("frame_file_ref", obj))
                .or_else(|| decode_legacy_file_ref("flow_file_ref", obj));
        }
    }
    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return None;
        }
    }
    None
}

fn decode_locator(obj: &serde_json::Map<String, Value>) -> SceneLocator {
    let scene_id = obj
        .get("scene_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    let scene_file = obj
        .get("scene_file")
        .or_else(|| obj.get("path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string);
    let entry = obj
        .get("entry")
        .or_else(|| obj.get("entry_tab"))
        .or_else(|| obj.get("entryTab"))
        .or_else(|| obj.get("focus"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    let entry_tab = entry.clone();
    SceneLocator::new(scene_id, scene_file, entry_tab, entry)
}

fn decode_legacy_file_ref(
    expected_kind: &str,
    obj: &serde_json::Map<String, Value>,
) -> Option<RefExpr> {
    let path = obj.get("path").and_then(Value::as_str)?.trim();
    if path.is_empty() {
        return None;
    }
    let id = obj
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    let kind = match expected_kind {
        "scene_file_ref" => RefKind::Scene,
        "world_file_ref" => RefKind::World,
        "frame_file_ref" => RefKind::Frame,
        "flow_file_ref" => RefKind::Flow,
        _ => return None,
    };
    Some(RefExpr::new(
        kind,
        id,
        SceneLocator::with_file(path.to_string()),
    ))
}

pub fn decode_binding_value(value: &Value) -> Option<BindingValue> {
    if let Some(expr) = decode_ref_value(value) {
        return Some(BindingValue::Ref(expr));
    }
    if value.is_string() {
        let trimmed = value.as_str()?.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(BindingValue::Inline(Value::String(trimmed.to_string())));
    }
    if value.is_array() {
        return Some(BindingValue::Inline(value.clone()));
    }
    if value.is_object() {
        return Some(BindingValue::Inline(value.clone()));
    }
    None
}

pub fn ref_to_json(expr: &RefExpr) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "__ref".to_string(),
        Value::String(ref_kind_tag(expr.kind).to_string()),
    );
    if let Some(id) = &expr.id {
        obj.insert("id".to_string(), Value::String(id.clone()));
    }
    if let Some(scene_id) = &expr.locator.scene_id {
        obj.insert("scene_id".to_string(), Value::String(scene_id.clone()));
    }
    if let Some(scene_file) = &expr.locator.scene_file {
        obj.insert("scene_file".to_string(), Value::String(scene_file.clone()));
    }
    if let Some(entry) = &expr.locator.entry {
        obj.insert("entry".to_string(), Value::String(entry.clone()));
        // Keep legacy alias for one migration cycle.
        obj.insert("entry_tab".to_string(), Value::String(entry.clone()));
    } else if let Some(entry_tab) = &expr.locator.entry_tab {
        obj.insert("entry_tab".to_string(), Value::String(entry_tab.clone()));
    }
    Value::Object(obj)
}

fn ref_kind_tag(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Scene => "scene",
        RefKind::World => "world",
        RefKind::Flow => "flow",
        RefKind::Frame => "frame",
        RefKind::Panel => "panel",
        RefKind::Dataset => "dataset",
        RefKind::Metric => "metric",
        RefKind::Resource => "resource",
        RefKind::Entity => "entity",
        RefKind::Component => "component",
    }
}

#[derive(Debug, Default)]
pub struct SceneRegistry {
    by_id: BTreeMap<String, String>,
    by_file: BTreeMap<String, String>,
}

impl SceneRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, scene_id: String, scene_file: String) {
        let scene_file = normalize_rel_path(&scene_file);
        self.by_id.insert(scene_id.clone(), scene_file.clone());
        self.by_file.insert(scene_file, scene_id);
    }

    pub fn resolve_target(&self, locator: &SceneLocator) -> Result<(String, String), String> {
        if let Some(scene_id) = locator
            .scene_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            if let Some(file) = self.by_id.get(scene_id) {
                return Ok((scene_id.to_string(), file.clone()));
            }
            return Err(format!("unknown scene_id `{scene_id}`"));
        }
        if let Some(scene_file) = locator
            .scene_file
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            let normalized = normalize_rel_path(scene_file);
            if let Some(scene_id) = self.by_file.get(&normalized) {
                return Ok((scene_id.clone(), normalized));
            }
            return Err(format!("unknown scene_file `{scene_file}`"));
        }
        Err("scene locator requires scene_id or scene_file".to_string())
    }

    pub fn build_from_routes(routes: &[crate::model::CompiledSceneRoute]) -> Self {
        let mut registry = Self::new();
        for route in routes {
            registry.register(route.scene_id.clone(), route.target_file.clone());
        }
        registry
    }
}

pub fn normalize_rel_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decode_typed_ref_from_json() {
        let value = json!({
            "__ref": "frame",
            "scene_file": "dashboard_base.mei"
        });
        let expr = decode_ref_value(&value).expect("frame ref");
        assert_eq!(expr.kind, RefKind::Frame);
        assert_eq!(
            expr.locator.scene_file.as_deref(),
            Some("dashboard_base.mei")
        );
    }

    #[test]
    fn decode_legacy_world_file_ref() {
        let value = json!({
            "kind": "world_file_ref",
            "path": "worlds/base.mei"
        });
        let expr = decode_ref_value(&value).expect("world ref");
        assert_eq!(expr.kind, RefKind::World);
        assert_eq!(expr.locator.scene_file.as_deref(), Some("worlds/base.mei"));
    }

    #[test]
    fn scene_registry_resolves_by_file_and_id() {
        let mut registry = SceneRegistry::new();
        registry.register("home".to_string(), "scenes/home.mei".to_string());
        let (scene_id, file) = registry
            .resolve_target(&SceneLocator::with_file("scenes/home.mei"))
            .expect("resolve");
        assert_eq!(scene_id, "home");
        assert_eq!(file, "scenes/home.mei");
    }
}
