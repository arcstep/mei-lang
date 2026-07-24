use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::{
    eval::evaluate_mei_file,
    mei_config::{
        app_mei_config_path, load_app_manifest, load_mei_config_for_app, resolve_app_entry_main,
        resolve_app_main_path, APP_TOML_FILENAME, MEI_CONFIG_FILENAME,
    },
    model::{AppDecl, CompiledSceneRoute},
    typed_refs::SceneRegistry,
    workspace::load_component_assets_for_app,
};

use crate::compile::app_decl::decode_app_decl;
use crate::compile::catalog::{
    build_dataset_catalog_filter, resolve_dataset_catalog_compile_rels, DatasetCatalogFilter,
};
use crate::compile::dependency_graph::DependencyGraph;
use crate::compile::discover_routes::{
    catalog_focus_target, inject_discovered_entry_scene_routes, is_dataset_manage_preview,
    is_manage_preview_only_compile, CompileOptions, CompileRevisionPlan, CompileWatchedFile,
};
use crate::compile::scene::{find_scene_route, resolve_scene_routes};

pub fn resolve_default_scene_from_root(app_root: &Path) -> Result<Option<String>> {
    if let Some(stage) = crate::mei_config::load_app_manifest(app_root)
        .default_stage
        .filter(|s| !s.trim().is_empty())
    {
        return Ok(Some(stage));
    }
    let app_main = resolve_app_main_path(app_root);
    match evaluate_mei_file(&app_main) {
        Ok(app_decls) => {
            let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
            if let Some(app_decl) = app_decl {
                let route_registry =
                    resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);
                return Ok(route_registry
                    .default_scene_id
                    .or_else(|| {
                        route_registry
                            .routes
                            .first()
                            .map(|route| route.scene_id.clone())
                    })
                    .map(|scene_id| scene_id.trim().to_string())
                    .filter(|scene_id| !scene_id.is_empty()));
            }
            if let Some(scene_id) = default_scene_from_skeleton_or_navigation(&app_decls) {
                return Ok(Some(scene_id));
            }
        }
        Err(_) => {
            // graph-native app.mei (app_skeleton) is not surface-evaluable; scan source instead.
        }
    }
    Ok(scan_default_scene_from_app_source(app_root))
}

/// Scene / stage ids: Stage Program MDX enumeration first (0119), then union with
/// classic `app(...)` routes / graph-native `navigation(...)` (T2 pages etc.).
pub fn resolve_scene_ids_from_root(app_root: &Path) -> Result<Vec<String>> {
    let mut scenes = Vec::new();
    let mut seen = BTreeSet::new();
    let push = |scene: &str, scenes: &mut Vec<String>, seen: &mut BTreeSet<String>| {
        let trimmed = scene.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            return;
        }
        scenes.push(trimmed.to_string());
    };

    for prog in mei_syntax::discover_stage_programs(app_root) {
        push(prog.stage_id.as_str(), &mut scenes, &mut seen);
    }

    let app_main = resolve_app_main_path(app_root);
    match evaluate_mei_file(&app_main) {
        Ok(app_decls) => {
            let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
            if let Some(app_decl) = app_decl {
                let route_registry =
                    resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);
                for route in route_registry.routes {
                    push(route.scene_id.as_str(), &mut scenes, &mut seen);
                }
            } else if let Some(values) = app_decls.as_array() {
                for value in values {
                    if value.get("kind").and_then(Value::as_str) != Some("navigation") {
                        continue;
                    }
                    if let Some(scene) = value
                        .get("scene")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        push(scene, &mut scenes, &mut seen);
                    }
                }
            }
        }
        Err(_) => {}
    }
    for scene in scan_navigation_scenes_from_app_source(app_root) {
        push(scene.as_str(), &mut scenes, &mut seen);
    }
    Ok(scenes)
}

pub fn app_declares_scene_from_root(app_root: &Path, scene_id: &str) -> bool {
    let wanted = scene_id.trim();
    if wanted.is_empty() {
        return false;
    }
    resolve_scene_ids_from_root(app_root)
        .ok()
        .map(|scenes| scenes.iter().any(|scene| scene == wanted))
        .unwrap_or(false)
}

fn default_scene_from_skeleton_or_navigation(raw: &Value) -> Option<String> {
    let values = raw.as_array()?;
    for value in values {
        if value.get("kind").and_then(Value::as_str) != Some("app_skeleton") {
            continue;
        }
        if let Some(scene) = value
            .get("default_stage")
            .or_else(|| value.get("default_scene"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(scene.to_string());
        }
    }
    for value in values {
        if value.get("kind").and_then(Value::as_str) != Some("navigation") {
            continue;
        }
        if let Some(scene) = value
            .get("scene")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(scene.to_string());
        }
    }
    None
}

fn scan_default_scene_from_app_source(app_root: &Path) -> Option<String> {
    let programs = mei_syntax::discover_stage_programs(app_root);
    if let Some(home) = programs.iter().find(|p| p.stage_id == "home") {
        return Some(home.stage_id.clone());
    }
    if programs.len() == 1 {
        return Some(programs[0].stage_id.clone());
    }
    let app_main = resolve_app_main_path(app_root);
    let Ok(source) = std::fs::read_to_string(app_main) else {
        return programs.into_iter().next().map(|p| p.stage_id);
    };
    if let Some(scene) = first_quoted_assignment(source.as_str(), "default_stage") {
        return Some(scene);
    }
    // Phase 9: temporary read of removed authoring field (no silent home fallback).
    if let Some(scene) = first_quoted_assignment(source.as_str(), "default_scene") {
        return Some(scene);
    }
    scan_navigation_scenes_from_source(source.as_str())
        .into_iter()
        .next()
}

fn scan_navigation_scenes_from_app_source(app_root: &Path) -> Vec<String> {
    let app_main = resolve_app_main_path(app_root);
    let Ok(source) = std::fs::read_to_string(app_main) else {
        return Vec::new();
    };
    scan_navigation_scenes_from_source(source.as_str())
}

fn scan_navigation_scenes_from_source(source: &str) -> Vec<String> {
    let mut scenes = Vec::new();
    let mut seen = BTreeSet::new();
    // Prefer `scene = "..."` assignments that appear after a `navigation(` opener.
    let mut in_navigation = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("navigation(") || trimmed.contains("navigation(") {
            in_navigation = true;
        }
        if in_navigation {
            if let Some(scene) = first_quoted_assignment(trimmed, "scene") {
                if seen.insert(scene.clone()) {
                    scenes.push(scene);
                }
            }
        }
        if in_navigation && trimmed.contains(')') {
            in_navigation = false;
        }
    }
    if scenes.is_empty() {
        // Fallback: any scene = "..." that is not default_scene.
        for scene in all_quoted_assignments(source, "scene") {
            if seen.insert(scene.clone()) {
                scenes.push(scene);
            }
        }
    }
    scenes
}

fn first_quoted_assignment(source: &str, key: &str) -> Option<String> {
    all_quoted_assignments(source, key).into_iter().next()
}

fn all_quoted_assignments(source: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(idx) = rest.find(key) {
        let after_key = &rest[idx + key.len()..];
        let trimmed = after_key.trim_start();
        let Some(after_eq) = trimmed.strip_prefix('=') else {
            rest = &rest[idx + key.len()..];
            continue;
        };
        let trimmed = after_eq.trim_start();
        if let Some(value) = parse_double_quoted(trimmed) {
            out.push(value);
        }
        rest = &rest[idx + key.len()..];
    }
    out
}

fn parse_double_quoted(input: &str) -> Option<String> {
    let input = input.trim_start();
    let mut chars = input.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    for ch in chars {
        if ch == '"' {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

pub fn compile_revision_plan_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: &CompileOptions,
) -> Result<CompileRevisionPlan> {
    let app_entry_main = resolve_app_entry_main(app_root);
    let (app_main, app_decls, app_decl, mut diagnostics) = if app_entry_main.is_empty() {
        let manifest = load_app_manifest(app_root);
        let app_id = manifest.app_id.clone().unwrap_or_else(|| {
            app_root
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("app")
                .to_string()
        });
        let app_decl = AppDecl {
            kind: "app".to_string(),
            id: app_id,
            title: manifest.title,
            default_stage: manifest.default_stage,
            scene: None,
        };
        (
            app_root.join(APP_TOML_FILENAME),
            serde_json::to_value(vec![app_decl.clone()])?,
            app_decl,
            Vec::new(),
        )
    } else {
        let app_main = resolve_app_main_path(app_root);
        let app_decls = evaluate_mei_file(&app_main)?;
        let (app_decl, diagnostics) = decode_app_decl(&app_main, &app_decls);
        let app_decl = app_decl
            .ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
        (app_main, app_decls, app_decl, diagnostics)
    };
    let mut route_registry =
        resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);
    let asset_map = load_component_assets_for_app(source_root, app_decl.id.as_str())?;
    let preview_only = is_manage_preview_only_compile(options, app_entry_main.as_str());
    let scene_registry = SceneRegistry::build_from_routes(&route_registry.routes);
    inject_discovered_entry_scene_routes(
        app_root,
        source_root,
        &app_decls,
        &asset_map,
        &mut route_registry.routes,
        &scene_registry,
        options.preview_target.as_deref(),
        options.scene.as_deref(),
        preview_only,
    );
    let active_route_meta = if let Some(requested) = options.scene.as_deref() {
        let selected = find_scene_route(&route_registry.routes, requested).cloned();
        if selected.is_none() {
            let preview_route = options
                .preview_target
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty())
                .and_then(|target| {
                    route_registry
                        .routes
                        .iter()
                        .find(|route| route.target_file == target)
                        .cloned()
                });
            preview_route.or_else(|| {
                route_registry
                    .default_scene_id
                    .as_deref()
                    .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
                    .cloned()
                    .or_else(|| route_registry.routes.first().cloned())
            })
        } else {
            selected
        }
    } else {
        route_registry
            .default_scene_id
            .as_deref()
            .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
            .cloned()
            .or_else(|| route_registry.routes.first().cloned())
    };
    let dependency_graph_routes =
        scoped_dependency_graph_routes(&route_registry.routes, active_route_meta.as_ref(), options);
    let dependency_graph =
        DependencyGraph::build_cached(app_root, &app_decls, &dependency_graph_routes);

    let selected_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|value| value.to_string());
    let primary_target = selected_target
        .or_else(|| {
            active_route_meta
                .as_ref()
                .map(|route| route.target_file.clone())
        })
        .unwrap_or_else(|| app_entry_main.clone());

    let dataset_manage_preview = is_dataset_manage_preview(options, app_entry_main.as_str());
    let catalog_focus = catalog_focus_target(options, Some(primary_target.as_str()));
    let catalog_filter = if app_entry_main.is_empty() {
        DatasetCatalogFilter::all_data_modules(app_root)
    } else if dataset_manage_preview {
        DatasetCatalogFilter::default()
    } else {
        build_dataset_catalog_filter(app_root, &app_decls, &dependency_graph, catalog_focus)
    };
    Ok(build_compile_revision_plan_from_inputs(
        source_root,
        app_root,
        app_entry_main.as_str(),
        &app_decls,
        &dependency_graph,
        primary_target.as_str(),
        dataset_manage_preview,
        &catalog_filter,
    ))
}

pub fn compile_revision_token_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: &CompileOptions,
) -> Result<String> {
    Ok(compile_revision_plan_from_root_with_options(source_root, app_root, options)?.token)
}

pub(crate) fn build_compile_revision_plan_from_inputs(
    source_root: &Path,
    app_root: &Path,
    app_entry_main: &str,
    app_decls: &serde_json::Value,
    dependency_graph: &DependencyGraph,
    primary_target: &str,
    dataset_manage_preview: bool,
    catalog_filter: &DatasetCatalogFilter,
) -> CompileRevisionPlan {
    let mut token_parts = BTreeMap::<String, String>::new();
    let mut watched_paths = BTreeSet::<String>::new();
    watched_paths.insert(app_entry_main.to_string());
    if let Some(main_token) =
        dependency_graph.dependency_fingerprint_for_target(app_root, app_decls, app_entry_main)
    {
        token_parts.insert("main".to_string(), main_token);
        watched_paths.extend(dependency_graph.closure_for_target(
            app_root,
            app_decls,
            app_entry_main,
        ));
    }
    if let Some(primary_token) =
        dependency_graph.dependency_fingerprint_for_target(app_root, app_decls, primary_target)
    {
        token_parts.insert(format!("target:{primary_target}"), primary_token);
        watched_paths.extend(dependency_graph.closure_for_target(
            app_root,
            app_decls,
            primary_target,
        ));
    }
    if !dataset_manage_preview {
        for rel in resolve_dataset_catalog_compile_rels(app_root, catalog_filter)
            .into_iter()
            .filter(|rel| rel != primary_target)
        {
            if let Some(token) =
                dependency_graph.dependency_fingerprint_for_target(app_root, app_decls, &rel)
            {
                token_parts.insert(format!("catalog:{rel}"), token);
                watched_paths
                    .extend(dependency_graph.closure_for_target(app_root, app_decls, &rel));
            }
        }
    }
    for path in crate::model::discover_narration_track_paths(app_root) {
        let rel = path
            .strip_prefix(app_root)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        watched_paths.insert(rel.clone());
        token_parts.insert(
            format!("narration:{rel}"),
            crate::compile::source_file_content_signature(path.as_path(), rel.as_str()),
        );
    }

    let app_toml_path = app_root.join(APP_TOML_FILENAME);
    let config_path = app_mei_config_path(app_root);
    if app_toml_path.is_file() || config_path.is_file() {
        watched_paths.insert(
            if app_toml_path.is_file() {
                APP_TOML_FILENAME
            } else {
                MEI_CONFIG_FILENAME
            }
            .to_string(),
        );
        let config = load_mei_config_for_app(app_root, None);
        token_parts.insert(
            "mei-config".to_string(),
            crate::mei_config::mei_config_compile_revision_digest(&config),
        );
        let themes_rev = crate::mei_config::ops_themes_revision_digest(&config);
        if !themes_rev.is_empty() {
            token_parts.insert("ops-themes".to_string(), themes_rev);
        }
        append_ops_source_revision_tokens(app_root, &mut token_parts, &mut watched_paths);
    }

    let components_revision = crate::compile::scene_payload_cache::components_revision(source_root);
    token_parts.insert("components".to_string(), components_revision.to_string());
    let watched_files = watched_paths
        .into_iter()
        .map(|rel_path| {
            let path = crate::mei_config::resolve_app_mei_file_path(app_root, &rel_path);
            let metadata = std::fs::metadata(&path).ok();
            CompileWatchedFile {
                content_signature: path.is_file().then(|| {
                    crate::compile::source_file_content_signature(path.as_path(), &rel_path)
                }),
                rel_path,
                modified_ms: crate::compile::scene_payload_cache::file_mtime_ms(&path),
                size_bytes: metadata.map(|meta| meta.len()).unwrap_or(0),
            }
        })
        .collect();
    CompileRevisionPlan {
        token: token_parts.into_values().collect::<Vec<_>>().join("||"),
        watched_files,
        components_revision,
    }
}

fn append_ops_source_revision_tokens(
    app_root: &Path,
    token_parts: &mut BTreeMap<String, String>,
    watched_paths: &mut BTreeSet<String>,
) {
    let config = load_mei_config_for_app(app_root, None);
    for (source_id, entry) in &config.ops.sources {
        let rel = entry.path.trim().replace('\\', "/");
        if rel.is_empty() {
            continue;
        }
        let resolved = crate::resolve_versioned_source_identifier(app_root, rel.as_str());
        watched_paths.insert(resolved.clone());
        let absolute = crate::resolve_versioned_source_path(app_root, rel.as_str());
        let content_signature =
            crate::compile::source_file_content_signature(absolute.as_path(), resolved.as_str());
        token_parts.insert(
            format!("source:{source_id}"),
            format!("content:{content_signature}"),
        );
    }
}

pub(crate) fn scoped_dependency_graph_routes(
    routes: &[CompiledSceneRoute],
    active_route_meta: Option<&CompiledSceneRoute>,
    options: &CompileOptions,
) -> Vec<CompiledSceneRoute> {
    let explicit_scene_scope = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || options
            .preview_target
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some();
    if !explicit_scene_scope {
        return routes.to_vec();
    }
    let mut scoped = BTreeMap::<String, CompiledSceneRoute>::new();
    if let Some(route) = active_route_meta.cloned() {
        scoped.insert(route.target_file.clone(), route);
    }
    if let Some(preview_route) = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .and_then(|target| routes.iter().find(|route| route.target_file == target))
        .cloned()
    {
        scoped.insert(preview_route.target_file.clone(), preview_route);
    }
    if scoped.is_empty() {
        return routes.to_vec();
    }
    scoped.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::append_ops_source_revision_tokens;
    use crate::compile::source_file_content_signature;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;

    fn temp_app_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mei-app-compile-revision-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn ops_sources_revision_token_uses_content_signature() {
        let app_root = temp_app_root("ops-source");
        fs::create_dir_all(app_root.join("upload")).expect("create upload dir");
        fs::write(app_root.join("upload/test.xlsx"), b"hello world").expect("write source");
        fs::write(
            app_root.join("app.toml"),
            r#"schema_version = "mei-app-v1"
app_id = "demo"

[ops.sources.ledger]
kind = "xlsx"
path = "upload/test.xlsx"
"#,
        )
        .expect("write config");

        let mut token_parts = BTreeMap::new();
        let mut watched_paths = BTreeSet::new();
        append_ops_source_revision_tokens(app_root.as_path(), &mut token_parts, &mut watched_paths);

        let expected_rel = "upload/test.xlsx";
        let expected_sig =
            source_file_content_signature(&app_root.join(expected_rel), expected_rel);
        let expected_token = format!("content:{expected_sig}");
        assert_eq!(
            token_parts.get("source:ledger").map(String::as_str),
            Some(expected_token.as_str())
        );
        assert!(watched_paths.contains(expected_rel));

        let _ = fs::remove_dir_all(app_root);
    }
}

#[cfg(test)]
mod default_scene_v2_tests {
    use super::*;
    use std::fs;

    #[test]
    fn mei_tutorial_resolves_intro_default_scene() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().to_path_buf();
        fs::write(
            root.join("app.toml"),
            r#"
schema_version = "mei-app-v1"
title = "MeiLang Tutorial Fixture"
default_stage = "intro"
app_id = "mei-tutorial"
"#,
        )
        .expect("write app.toml");
        let deck = root.join("src/presentation/intro/intro.deck.mdx");
        fs::create_dir_all(deck.parent().expect("parent")).expect("mkdir");
        fs::write(
            &deck,
            r#"---
id: intro
title: Intro
theme: presentation
---

# Intro
"#,
        )
        .expect("write deck");

        let scene = resolve_default_scene_from_root(&root)
            .expect("resolve")
            .expect("default scene");
        assert_eq!(scene, "intro");
        assert!(app_declares_scene_from_root(&root, "intro"));
        assert!(!app_declares_scene_from_root(&root, "home"));
    }
}
