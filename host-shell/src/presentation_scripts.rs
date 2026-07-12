use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    Extension,
};
use mei_host_auth::AuthPrincipal;
use mei_lang_kernel::{load_mei_config_for_app, resolve_app_root};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::presentation_compile::presentation_image_assets_for_app;
use crate::state::SharedState;

const SCRIPT_SUFFIX_PRESENTATION: &str = ".presentation.mdx";
const SCRIPT_SUFFIX_SCENE: &str = ".scene.mdx";
const MAX_SCRIPT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone)]
struct StageRoot {
    /// e.g. `presentation/supervision` or `scene/home`
    target: String,
    /// Absolute directory that holds the stage entry + colocated MDX.
    dir: PathBuf,
    /// Relative path from app root (for API `path` fields).
    rel_dir: String,
    kind: StageKind,
    /// Scene/presentation id segment (`home`, `supervision`).
    stage_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageKind {
    Scene,
    Presentation,
}

#[derive(Debug, Clone, Serialize)]
struct PresentationScriptEntry {
    id: String,
    title: String,
    path: String,
    #[serde(rename = "modifiedMs", skip_serializing_if = "Option::is_none")]
    modified_ms: Option<i64>,
    #[serde(rename = "isDefault", default)]
    is_default: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    target: String,
}

#[derive(Debug, Deserialize)]
pub struct PutPresentationScriptRequest {
    pub source: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Default)]
struct FrontmatterMeta {
    script_id: Option<String>,
    title: Option<String>,
    target_stage: Option<String>,
    default_for_stage: bool,
}

fn sanitize_script_id(raw: &str) -> Result<String, (StatusCode, String)> {
    let id = raw.trim();
    if id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "scriptId 不能为空".to_string()));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "scriptId 仅允许字母、数字、_ 和 -".to_string(),
        ));
    }
    Ok(id.to_string())
}

fn sanitize_rel_dir(raw: &str) -> Result<String, (StatusCode, String)> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    for component in Path::new(&trimmed).components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "演说稿路径不能越出应用源码目录".to_string(),
                ));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(trimmed.trim_start_matches('/').to_string())
}

pub fn default_presentation_rel_path() -> &'static str {
    "src/presentation"
}

pub fn default_scene_rel_path() -> &'static str {
    "src/scene"
}

fn resolve_src_rel(app_root: &Path, workspace_root: &Path, key: &str, fallback: &str) -> PathBuf {
    let config = load_mei_config_for_app(app_root, Some(workspace_root));
    let rel = if key == "presentation" {
        config
            .paths
            .presentation
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(fallback)
    } else {
        fallback
    };
    app_root.join(rel)
}

fn discover_stage_roots(workspace_root: &Path, app_id: &str) -> Vec<StageRoot> {
    let app_root = resolve_app_root(workspace_root, app_id);
    let mut stages = Vec::new();

    let scene_root = resolve_src_rel(
        app_root.as_path(),
        workspace_root,
        "scene",
        default_scene_rel_path(),
    );
    if scene_root.is_dir() {
        if let Ok(read_dir) = fs::read_dir(&scene_root) {
            for item in read_dir.flatten() {
                let path = item.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                    continue;
                };
                let Some(stem) = name.strip_suffix(".mei") else {
                    continue;
                };
                if stem.is_empty() || stem.contains('.') {
                    continue;
                }
                let rel_dir = path_relative_to_app(&app_root, &scene_root)
                    .unwrap_or_else(|| default_scene_rel_path().to_string());
                stages.push(StageRoot {
                    target: format!("scene/{stem}"),
                    dir: scene_root.clone(),
                    rel_dir,
                    kind: StageKind::Scene,
                    stage_id: stem.to_string(),
                });
            }
        }
    }

    let presentation_root = resolve_src_rel(
        app_root.as_path(),
        workspace_root,
        "presentation",
        default_presentation_rel_path(),
    );
    if presentation_root.is_dir() {
        if let Ok(read_dir) = fs::read_dir(&presentation_root) {
            for item in read_dir.flatten() {
                let dir = item.path();
                if !dir.is_dir() {
                    continue;
                }
                let Some(stage_id) = dir.file_name().and_then(|v| v.to_str()).map(str::to_string)
                else {
                    continue;
                };
                if stage_id.starts_with('.') {
                    continue;
                }
                let entry = dir.join("presentation.mei");
                if !entry.is_file() {
                    continue;
                }
                let rel_dir = path_relative_to_app(&app_root, &dir)
                    .unwrap_or_else(|| format!("{}/{}", default_presentation_rel_path(), stage_id));
                stages.push(StageRoot {
                    target: format!("presentation/{stage_id}"),
                    dir,
                    rel_dir,
                    kind: StageKind::Presentation,
                    stage_id,
                });
            }
        }
    }

    stages.sort_by(|a, b| a.target.cmp(&b.target));
    stages
}

fn path_relative_to_app(app_root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(app_root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

fn script_id_from_file_name(file_name: &str) -> Option<String> {
    let name = file_name.trim();
    let stem = name
        .strip_suffix(SCRIPT_SUFFIX_PRESENTATION)
        .or_else(|| name.strip_suffix(SCRIPT_SUFFIX_SCENE))?;
    if stem.is_empty() {
        return None;
    }
    sanitize_script_id(stem).ok()
}

fn scene_mdx_belongs_to_stage(file_stem: &str, stage_id: &str) -> bool {
    file_stem == stage_id || file_stem.starts_with(&format!("{stage_id}-"))
}

fn system_time_to_epoch_ms(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis() as i64)
}

fn parse_frontmatter(source: &str) -> FrontmatterMeta {
    let mut meta = FrontmatterMeta::default();
    let trimmed = source.trim_start();
    if !trimmed.starts_with("---") {
        return meta;
    }
    let Some(rest) = trimmed.strip_prefix("---") else {
        return meta;
    };
    let Some(end) = rest.find("\n---") else {
        return meta;
    };
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("script_id:") {
            let id = value.trim().trim_matches('"').trim_matches('\'');
            if !id.is_empty() {
                meta.script_id = Some(id.to_string());
            }
        } else if let Some(value) = line.strip_prefix("title:") {
            let title = value.trim().trim_matches('"').trim_matches('\'');
            if !title.is_empty() {
                meta.title = Some(title.to_string());
            }
        } else if let Some(value) = line.strip_prefix("target_stage:") {
            let target = value.trim().trim_matches('"').trim_matches('\'');
            if !target.is_empty() {
                meta.target_stage = Some(target.to_string());
            }
        } else if let Some(value) = line.strip_prefix("default_for_stage:") {
            let raw = value.trim().to_ascii_lowercase();
            meta.default_for_stage = matches!(raw.as_str(), "true" | "yes" | "1");
        }
    }
    meta
}

fn parse_title_from_source(source: &str) -> Option<String> {
    parse_frontmatter(source).title
}

fn read_script_title(path: &Path, source: &str) -> String {
    parse_title_from_source(source)
        .or_else(|| {
            path.file_name()
                .and_then(|value| value.to_str())
                .and_then(script_id_from_file_name)
        })
        .unwrap_or_else(|| "未命名演说稿".to_string())
}

fn scan_stage_scripts(stage: &StageRoot) -> Vec<PresentationScriptEntry> {
    let Ok(read_dir) = fs::read_dir(&stage.dir) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for item in read_dir.flatten() {
        let path = item.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let is_presentation = file_name.ends_with(SCRIPT_SUFFIX_PRESENTATION);
        let is_scene = file_name.ends_with(SCRIPT_SUFFIX_SCENE);
        if !is_presentation && !is_scene {
            continue;
        }
        match stage.kind {
            StageKind::Presentation if !is_presentation => continue,
            StageKind::Scene if !is_scene => continue,
            _ => {}
        }
        let Some(file_stem) = script_id_from_file_name(file_name) else {
            continue;
        };
        if stage.kind == StageKind::Scene
            && !scene_mdx_belongs_to_stage(&file_stem, &stage.stage_id)
        {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap_or_default();
        let meta = parse_frontmatter(&source);
        if let Some(declared) = meta.target_stage.as_deref() {
            if declared != stage.target {
                eprintln!(
                    "presentation script `{}`: frontmatter target_stage=`{}` ignored; path derives `{}`",
                    file_name, declared, stage.target
                );
            }
        }
        let id = meta
            .script_id
            .as_deref()
            .and_then(|raw| sanitize_script_id(raw).ok())
            .unwrap_or(file_stem);
        let modified_ms = item
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(system_time_to_epoch_ms);
        let rel_path = if stage.rel_dir.is_empty() {
            file_name.to_string()
        } else {
            format!("{}/{}", stage.rel_dir.trim_end_matches('/'), file_name)
        };
        entries.push(PresentationScriptEntry {
            id,
            title: meta
                .title
                .unwrap_or_else(|| read_script_title(&path, &source)),
            path: rel_path,
            modified_ms,
            is_default: meta.default_for_stage,
            target: stage.target.clone(),
        });
    }
    entries
}

fn list_all_script_entries(
    workspace_root: &Path,
    app_id: &str,
) -> Result<Vec<PresentationScriptEntry>, (StatusCode, String)> {
    let stages = discover_stage_roots(workspace_root, app_id);
    let mut entries = Vec::new();
    let mut seen_ids = BTreeMap::<String, String>::new();
    for stage in &stages {
        for entry in scan_stage_scripts(stage) {
            if let Some(existing_target) = seen_ids.get(&entry.id) {
                if existing_target != &entry.target {
                    return Err((
                        StatusCode::CONFLICT,
                        format!(
                            "演说稿 id `{}` 在多个 stage 重复（{} 与 {}）",
                            entry.id, existing_target, entry.target
                        ),
                    ));
                }
                continue;
            }
            seen_ids.insert(entry.id.clone(), entry.target.clone());
            entries.push(entry);
        }
    }

    // Resolve default flags per stage: prefer explicit default_for_stage;
    // if multiple, keep the newest modified; if none, leave all false.
    let mut by_stage: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        by_stage
            .entry(entry.target.clone())
            .or_default()
            .push(index);
    }
    for indexes in by_stage.values() {
        let defaults: Vec<usize> = indexes
            .iter()
            .copied()
            .filter(|index| entries[*index].is_default)
            .collect();
        let chosen = if defaults.len() == 1 {
            Some(defaults[0])
        } else if defaults.len() > 1 {
            defaults
                .into_iter()
                .max_by_key(|index| entries[*index].modified_ms.unwrap_or(0))
        } else {
            None
        };
        for index in indexes {
            entries[*index].is_default = chosen == Some(*index);
        }
    }

    entries.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(entries)
}

fn default_by_stage_map(entries: &[PresentationScriptEntry]) -> BTreeMap<String, Option<String>> {
    let mut map = BTreeMap::new();
    for entry in entries {
        if entry.target.is_empty() {
            continue;
        }
        map.entry(entry.target.clone()).or_insert(None);
        if entry.is_default {
            map.insert(entry.target.clone(), Some(entry.id.clone()));
        }
    }
    map
}

fn find_script_entry<'a>(
    entries: &'a [PresentationScriptEntry],
    script_id: &str,
) -> Option<&'a PresentationScriptEntry> {
    entries.iter().find(|entry| entry.id == script_id)
}

fn resolve_script_abs_path(
    workspace_root: &Path,
    app_id: &str,
    entry: &PresentationScriptEntry,
) -> Result<PathBuf, (StatusCode, String)> {
    let app_root = resolve_app_root(workspace_root, app_id);
    let rel = sanitize_rel_dir(&entry.path)?;
    let path = app_root.join(&rel);
    let canonical_app = app_root
        .canonicalize()
        .map_err(|error| (StatusCode::NOT_FOUND, format!("应用目录不可用: {error}")))?;
    if path.exists() {
        let canonical = path
            .canonicalize()
            .map_err(|error| (StatusCode::NOT_FOUND, format!("演说稿路径不可用: {error}")))?;
        if !canonical.starts_with(&canonical_app) {
            return Err((StatusCode::BAD_REQUEST, "演说稿路径越界".to_string()));
        }
        return Ok(canonical);
    }
    Ok(path)
}

fn upsert_frontmatter_default(source: &str, default_for_stage: bool) -> String {
    let trimmed = source.trim_start();
    if !trimmed.starts_with("---") {
        let flag = if default_for_stage { "true" } else { "false" };
        return format!("---\ndefault_for_stage: {flag}\n---\n{source}");
    }
    let Some(rest) = trimmed.strip_prefix("---") else {
        return source.to_string();
    };
    let Some(end) = rest.find("\n---") else {
        return source.to_string();
    };
    let frontmatter = &rest[..end];
    let body = &rest[end + "\n---".len()..];
    let mut lines = Vec::new();
    let mut replaced = false;
    for line in frontmatter.lines() {
        if line.trim().starts_with("default_for_stage:") {
            lines.push(format!(
                "default_for_stage: {}",
                if default_for_stage { "true" } else { "false" }
            ));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !replaced && default_for_stage {
        lines.push("default_for_stage: true".to_string());
    }
    if !replaced && !default_for_stage {
        // omit false default
    }
    let fm = lines.join("\n");
    let fm = fm.trim_matches('\n');
    format!("---\n{fm}\n---{body}")
}

pub fn read_default_script_id(workspace_root: &Path, app_id: &str) -> Option<String> {
    let entries = list_all_script_entries(workspace_root, app_id).ok()?;
    if let Some(entry) = entries.iter().find(|entry| entry.is_default) {
        return Some(entry.id.clone());
    }
    entries.first().map(|entry| entry.id.clone())
}

pub async fn api_list_presentation_scripts(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(app_id): AxumPath<String>,
) -> Response {
    let app_id = app_id.trim();
    if let Some(Extension(principal)) = principal.as_ref() {
        if !principal.can_access_app(app_id) {
            return forbidden(app_id);
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let scripts = match list_all_script_entries(workspace_root.as_path(), app_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    let default_by_stage = default_by_stage_map(&scripts);
    let default_script_id = scripts
        .iter()
        .find(|entry| entry.is_default)
        .map(|entry| entry.id.clone())
        .or_else(|| scripts.first().map(|entry| entry.id.clone()))
        .unwrap_or_default();
    Json(json!({
        "appId": app_id,
        "root": default_presentation_rel_path(),
        "defaultScriptId": default_script_id,
        "defaultByStage": default_by_stage,
        "scripts": scripts,
        "imageAssets": presentation_image_assets_for_app(workspace_root.as_path(), app_id),
    }))
    .into_response()
}

pub async fn api_get_presentation_script(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath((app_id, script_id)): AxumPath<(String, String)>,
) -> Response {
    let app_id = app_id.trim();
    let script_id = match sanitize_script_id(&script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Some(Extension(principal)) = principal.as_ref() {
        if !principal.can_access_app(app_id) {
            return forbidden(app_id);
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let entries = match list_all_script_entries(workspace_root.as_path(), app_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    let Some(entry) = find_script_entry(&entries, &script_id) else {
        return error_json(
            StatusCode::NOT_FOUND,
            format!("演说稿 `{script_id}` 不存在"),
        );
    };
    let path = match resolve_script_abs_path(workspace_root.as_path(), app_id, entry) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if !path.is_file() {
        return error_json(
            StatusCode::NOT_FOUND,
            format!("演说稿 `{script_id}` 不存在"),
        );
    }
    let source = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(error) => {
            return error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("读取演说稿失败: {error}"),
            )
        }
    };
    Json(json!({
        "appId": app_id,
        "id": script_id,
        "path": entry.path,
        "title": read_script_title(&path, &source),
        "source": source,
        "target": entry.target,
        "isDefault": entry.is_default,
    }))
    .into_response()
}

pub async fn api_put_presentation_script(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath((app_id, script_id)): AxumPath<(String, String)>,
    Json(request): Json<PutPresentationScriptRequest>,
) -> Response {
    let app_id = app_id.trim();
    let script_id = match sanitize_script_id(&script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Some(Extension(principal)) = principal.as_ref() {
        if !principal.can_access_app(app_id) {
            return forbidden(app_id);
        }
    }
    let source = request.source.trim();
    if source.is_empty() {
        return error_json(StatusCode::BAD_REQUEST, "source 不能为空");
    }
    if source.len() > MAX_SCRIPT_BYTES {
        return error_json(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("演说稿超过 {} 字节上限", MAX_SCRIPT_BYTES),
        );
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let entries = match list_all_script_entries(workspace_root.as_path(), app_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    let Some(entry) = find_script_entry(&entries, &script_id) else {
        return error_json(
            StatusCode::NOT_FOUND,
            format!("演说稿 `{script_id}` 不存在；请将 MDX 放在对应 stage 根目录"),
        );
    };
    let path = match resolve_script_abs_path(workspace_root.as_path(), app_id, entry) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            return error_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("创建演说稿目录失败: {error}"),
            );
        }
    }
    if let Err(error) = fs::write(&path, format!("{source}\n")) {
        return error_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("保存演说稿失败: {error}"),
        );
    }
    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| read_script_title(&path, source));
    Json(json!({
        "ok": true,
        "appId": app_id,
        "id": script_id,
        "path": entry.path,
        "title": title,
        "target": entry.target,
        "isDefault": entry.is_default,
    }))
    .into_response()
}

pub async fn api_set_default_presentation_script(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath((app_id, script_id)): AxumPath<(String, String)>,
) -> Response {
    let app_id = app_id.trim();
    let script_id = match sanitize_script_id(&script_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    if let Some(Extension(principal)) = principal.as_ref() {
        if !principal.can_access_app(app_id) {
            return forbidden(app_id);
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let entries = match list_all_script_entries(workspace_root.as_path(), app_id) {
        Ok(value) => value,
        Err((status, message)) => return error_json(status, message),
    };
    let Some(chosen) = find_script_entry(&entries, &script_id).cloned() else {
        return error_json(
            StatusCode::NOT_FOUND,
            format!("演说稿 `{script_id}` 不存在"),
        );
    };
    for entry in &entries {
        if entry.target != chosen.target {
            continue;
        }
        let path = match resolve_script_abs_path(workspace_root.as_path(), app_id, entry) {
            Ok(value) => value,
            Err((status, message)) => return error_json(status, message),
        };
        if !path.is_file() {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let next = upsert_frontmatter_default(&source, entry.id == chosen.id);
        if next != source {
            if let Err(error) = fs::write(&path, next) {
                return error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("更新默认讲稿标记失败: {error}"),
                );
            }
        }
    }
    Json(json!({
        "ok": true,
        "appId": app_id,
        "defaultScriptId": script_id,
        "target": chosen.target,
    }))
    .into_response()
}

fn forbidden(app_id: &str) -> Response {
    error_json(
        StatusCode::FORBIDDEN,
        format!("当前账号无权访问 app `{app_id}`"),
    )
}

fn error_json(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({
            "ok": false,
            "error": message.into(),
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_id_from_file_name_parses_presentation_and_scene_mdx() {
        assert_eq!(
            script_id_from_file_name("intro.presentation.mdx").as_deref(),
            Some("intro")
        );
        assert_eq!(
            script_id_from_file_name("home.scene.mdx").as_deref(),
            Some("home")
        );
        assert!(script_id_from_file_name("intro.mdx").is_none());
    }

    #[test]
    fn parse_frontmatter_reads_default_and_script_id() {
        let source = "---\nscript_id: home-tour\ntitle: 驾驶舱导览\ntarget_stage: scene/home\ndefault_for_stage: true\n---\n";
        let meta = parse_frontmatter(source);
        assert_eq!(meta.script_id.as_deref(), Some("home-tour"));
        assert_eq!(meta.title.as_deref(), Some("驾驶舱导览"));
        assert_eq!(meta.target_stage.as_deref(), Some("scene/home"));
        assert!(meta.default_for_stage);
    }

    #[test]
    fn scene_mdx_belongs_to_stage_matches_prefix() {
        assert!(scene_mdx_belongs_to_stage("home", "home"));
        assert!(scene_mdx_belongs_to_stage("home-tour", "home"));
        assert!(!scene_mdx_belongs_to_stage("other", "home"));
    }

    #[test]
    fn upsert_frontmatter_default_sets_flag() {
        let source = "---\ntitle: Demo\n---\nbody\n";
        let next = upsert_frontmatter_default(source, true);
        assert!(next.contains("default_for_stage: true"));
        assert!(next.contains("title: Demo"));
        assert!(next.contains("body"));
    }
}
