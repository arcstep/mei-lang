use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use mei_lang_kernel::{catalog_scene_routes_from_app_root, compile_app_from_root, resolve_app_root};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub struct PresentationCompileRequest {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub source: String,
    #[serde(rename = "sceneId")]
    pub scene_id: Option<String>,
    #[serde(rename = "presentationId")]
    pub presentation_id: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PresentationCompileDiagnostic {
    pub level: String,
    pub code: String,
    pub message: String,
    #[serde(rename = "stepId", skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(rename = "refKind", skip_serializing_if = "Option::is_none")]
    pub ref_kind: Option<String>,
    #[serde(rename = "refId", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
}

#[derive(Default)]
struct PresentationSurfaceIndex {
    viewpoints: BTreeSet<String>,
    pages: BTreeSet<String>,
    metrics: BTreeSet<String>,
}

fn compile_script_path(package_root: &Path) -> std::path::PathBuf {
    package_root.join("scripts").join("compile-presentation.mjs")
}

fn compile_manifest_via_node(
    package_root: &Path,
    source: &str,
    options: &Value,
) -> Result<Value> {
    let script = compile_script_path(package_root);
    if !script.is_file() {
        anyhow::bail!("presentation compile script not found: {}", script.display());
    }
    let payload = json!({
        "source": source,
        "options": options,
    });
    let mut command = Command::new("node");
    command.arg(&script);
    command.arg("--stdin-json");
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn node for {}", script.display()))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(payload.to_string().as_bytes())
            .context("failed to write compile payload to node stdin")?;
    }
    let output = child
        .wait_with_output()
        .context("failed to wait for presentation compile node process")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "presentation compile script failed (status={}): {}",
            output.status,
            stderr.trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest = serde_json::from_str::<Value>(stdout.trim())
        .context("failed to parse manifest JSON from node stdout")?;
    Ok(manifest)
}

fn build_surface_index(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Result<PresentationSurfaceIndex> {
    let app_root = resolve_app_root(workspace_root, app_id);
    let compiled = compile_app_from_root(workspace_root, app_root.as_path())
        .with_context(|| format!("failed to compile app `{app_id}` for presentation validation"))?;
    let mut surfaces = PresentationSurfaceIndex::default();
    for route in catalog_scene_routes_from_app_root(app_root.as_path()) {
        let scene_id = route.scene_id.trim();
        if !scene_id.is_empty() {
            surfaces.pages.insert(scene_id.to_string());
        }
    }
    for resource in &compiled.resources {
        if let Some(dataset) = resource.dataset.as_ref() {
            for metric_id in dataset.metrics.keys() {
                let metric_id = metric_id.trim();
                if !metric_id.is_empty() {
                    surfaces.metrics.insert(metric_id.to_string());
                }
            }
            for metric_id in dataset.runtime_metric_defs.keys() {
                let metric_id = metric_id.trim();
                if !metric_id.is_empty() {
                    surfaces.metrics.insert(metric_id.to_string());
                }
            }
        }
    }
    if let Ok(Some(outcome)) =
        mei_host_graph::assemble_scope_from_registry(workspace_root, app_id, scene_id)
    {
        if let Some(viewpoints) = outcome
            .presentation_map
            .get("viewpoints")
            .and_then(Value::as_object)
        {
            for viewpoint_id in viewpoints.keys() {
                let viewpoint_id = viewpoint_id.trim();
                if !viewpoint_id.is_empty() {
                    surfaces.viewpoints.insert(viewpoint_id.to_string());
                }
            }
        }
    }
    Ok(surfaces)
}

fn diagnostic(
    code: &str,
    message: impl Into<String>,
    step_id: Option<&str>,
    ref_kind: Option<&str>,
    ref_id: Option<&str>,
) -> PresentationCompileDiagnostic {
    PresentationCompileDiagnostic {
        level: "error".to_string(),
        code: code.to_string(),
        message: message.into(),
        step_id: step_id.map(str::to_string),
        ref_kind: ref_kind.map(str::to_string),
        ref_id: ref_id.map(str::to_string),
    }
}

fn warn(code: &str, message: impl Into<String>) -> PresentationCompileDiagnostic {
    PresentationCompileDiagnostic {
        level: "warn".to_string(),
        code: code.to_string(),
        message: message.into(),
        step_id: None,
        ref_kind: None,
        ref_id: None,
    }
}

fn step_actions(step: &Map<String, Value>) -> Vec<Value> {
    if let Some(actions) = step.get("actions").and_then(Value::as_array) {
        return actions.to_vec();
    }
    step.get("cockpit")
        .and_then(Value::as_object)
        .and_then(|cockpit| cockpit.get("actions"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn validate_manifest_refs(
    manifest: &Value,
    surfaces: &PresentationSurfaceIndex,
) -> (Vec<PresentationCompileDiagnostic>, Vec<PresentationCompileDiagnostic>) {
    let mut diagnostics = Vec::new();
    let mut warnings = Vec::new();
    let mut warned_unvalidated_chart = false;
    let mut warned_unvalidated_image = false;
    let Some(steps) = manifest.get("steps").and_then(Value::as_array) else {
        diagnostics.push(diagnostic(
            "manifest_steps_missing",
            "presentation manifest 缺少 steps 数组",
            None,
            None,
            None,
        ));
        return (diagnostics, warnings);
    };
    for step in steps {
        let Some(step_map) = step.as_object() else {
            continue;
        };
        let step_id = step_map.get("id").and_then(Value::as_str);
        for action in step_actions(step_map) {
            let Some(action_map) = action.as_object() else {
                continue;
            };
            let action_type = action_map
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            match action_type {
                "highlight" | "focus" => {
                    if let Some(viewpoint_id) =
                        action_map.get("viewpoint").and_then(Value::as_str).map(str::trim)
                    {
                        if !viewpoint_id.is_empty() && !surfaces.viewpoints.contains(viewpoint_id) {
                            diagnostics.push(diagnostic(
                                "unknown_viewpoint",
                                format!("未知 viewpoint `{viewpoint_id}`"),
                                step_id,
                                Some("viewpoint"),
                                Some(viewpoint_id),
                            ));
                        }
                    }
                }
                "open_t2_page" => {
                    if let Some(page_scene_id) =
                        action_map.get("pageSceneId").and_then(Value::as_str).map(str::trim)
                    {
                        if !page_scene_id.is_empty() && !surfaces.pages.contains(page_scene_id) {
                            diagnostics.push(diagnostic(
                                "unknown_page_scene",
                                format!("未知 page_scene_id `{page_scene_id}`"),
                                step_id,
                                Some("page_scene_id"),
                                Some(page_scene_id),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        let slot_arrays = step_map
            .get("slide")
            .and_then(Value::as_object)
            .and_then(|slide| slide.get("slots"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for slot in slot_arrays {
            let Some(slot_map) = slot.as_object() else {
                continue;
            };
            let embeds = slot_map
                .get("embeds")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for embed in embeds {
                let Some(embed_map) = embed.as_object() else {
                    continue;
                };
                let kind = embed_map.get("kind").and_then(Value::as_str).unwrap_or("").trim();
                let ref_id = embed_map.get("ref").and_then(Value::as_str).unwrap_or("").trim();
                if ref_id.is_empty() {
                    continue;
                }
                match kind {
                    "embed" => {
                        if !surfaces.viewpoints.contains(ref_id) {
                            diagnostics.push(diagnostic(
                                "unknown_embed_viewpoint",
                                format!("未知 embed viewpoint `{ref_id}`"),
                                step_id,
                                Some("viewpoint"),
                                Some(ref_id),
                            ));
                        }
                    }
                    "metric" => {
                        if !surfaces.metrics.contains(ref_id) {
                            diagnostics.push(diagnostic(
                                "unknown_metric",
                                format!("未知 metric `{ref_id}`"),
                                step_id,
                                Some("metric"),
                                Some(ref_id),
                            ));
                        }
                    }
                    "chart" if !warned_unvalidated_chart => {
                        warned_unvalidated_chart = true;
                        warnings.push(warn(
                            "chart_validation_not_enabled",
                            "当前临时 compile API 尚未对 chart 引用做严格存在性校验",
                        ));
                    }
                    "image" if !warned_unvalidated_image => {
                        warned_unvalidated_image = true;
                        warnings.push(warn(
                            "image_validation_not_enabled",
                            "当前临时 compile API 尚未对 image 引用做严格存在性校验",
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    (diagnostics, warnings)
}

pub async fn api_presentation_compile(
    State(state): State<SharedState>,
    Json(request): Json<PresentationCompileRequest>,
) -> Response {
    let app_id = request.app_id.trim();
    let source = request.source.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "manifest": Value::Null,
                "diagnostics": [{
                    "level": "error",
                    "code": "app_id_required",
                    "message": "appId 不能为空"
                }],
                "warnings": [],
            })),
        )
            .into_response();
    }
    if source.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "manifest": Value::Null,
                "diagnostics": [{
                    "level": "error",
                    "code": "source_required",
                    "message": "source 不能为空"
                }],
                "warnings": [],
            })),
        )
            .into_response();
    }
    if let Some(mode) = request.mode.as_deref() {
        let mode = mode.trim();
        if !mode.is_empty() && mode != "ephemeral" {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "manifest": Value::Null,
                    "diagnostics": [{
                        "level": "error",
                        "code": "unsupported_mode",
                        "message": format!("仅支持 mode=ephemeral，收到 `{mode}`")
                    }],
                    "warnings": [],
                })),
            )
                .into_response();
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    let package_root = guard.package_root.clone();
    drop(guard);
    let scene_id = request
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");
    let options = json!({
        "id": request.presentation_id.as_deref().map(str::trim).filter(|value| !value.is_empty()).unwrap_or("ephemeral"),
        "defaultScene": scene_id,
    });
    let manifest = match compile_manifest_via_node(package_root.as_path(), source, &options) {
        Ok(manifest) => manifest,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "manifest": Value::Null,
                    "diagnostics": [{
                        "level": "error",
                        "code": "compile_failed",
                        "message": error.to_string(),
                    }],
                    "warnings": [],
                })),
            )
                .into_response();
        }
    };
    let surfaces = match build_surface_index(workspace_root.as_path(), app_id, scene_id) {
        Ok(index) => index,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "manifest": Value::Null,
                    "diagnostics": [{
                        "level": "error",
                        "code": "surface_index_failed",
                        "message": error.to_string(),
                    }],
                    "warnings": [],
                })),
            )
                .into_response();
        }
    };
    let (diagnostics, warnings) = validate_manifest_refs(&manifest, &surfaces);
    if !diagnostics.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "manifest": Value::Null,
                "diagnostics": diagnostics,
                "warnings": warnings,
            })),
        )
            .into_response();
    }
    Json(json!({
        "manifest": manifest,
        "diagnostics": diagnostics,
        "warnings": warnings,
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_manifest_refs_reports_unknown_viewpoint_page_and_metric() {
        let manifest = json!({
            "id": "ephemeral",
            "steps": [{
                "id": "step_1",
                "actions": [
                    { "type": "highlight", "viewpoint": "missing_viewpoint" },
                    { "type": "open_t2_page", "pageSceneId": "missing_page" }
                ],
                "slide": {
                    "slots": [{
                        "name": "evidence",
                        "embeds": [
                            { "kind": "embed", "ref": "missing_viewpoint" },
                            { "kind": "metric", "ref": "missing_metric" }
                        ]
                    }]
                }
            }]
        });
        let surfaces = PresentationSurfaceIndex::default();
        let (diagnostics, warnings) = validate_manifest_refs(&manifest, &surfaces);
        assert!(warnings.is_empty());
        let codes = diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"unknown_viewpoint"));
        assert!(codes.contains(&"unknown_page_scene"));
        assert!(codes.contains(&"unknown_embed_viewpoint"));
        assert!(codes.contains(&"unknown_metric"));
    }

    #[test]
    fn validate_manifest_refs_accepts_known_refs() {
        let manifest = json!({
            "id": "ephemeral",
            "steps": [{
                "id": "step_1",
                "actions": [
                    { "type": "highlight", "viewpoint": "known_viewpoint" },
                    { "type": "open_t2_page", "pageSceneId": "known_page" }
                ],
                "slide": {
                    "slots": [{
                        "name": "evidence",
                        "embeds": [
                            { "kind": "embed", "ref": "known_viewpoint" },
                            { "kind": "metric", "ref": "known_metric" }
                        ]
                    }]
                }
            }]
        });
        let surfaces = PresentationSurfaceIndex {
            viewpoints: BTreeSet::from(["known_viewpoint".to_string()]),
            pages: BTreeSet::from(["known_page".to_string()]),
            metrics: BTreeSet::from(["known_metric".to_string()]),
        };
        let (diagnostics, warnings) = validate_manifest_refs(&manifest, &surfaces);
        assert!(diagnostics.is_empty());
        assert!(warnings.is_empty());
    }
}
