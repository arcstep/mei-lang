//! Scene-level workspace component JS bundles (access/presentation only).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::thread;

use anyhow::{Context, Result};
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{
    load_mei_config_for_app, resolve_app_root, resolve_components_root, ComponentAsset, MeiConfig,
};
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneComponentBundle {
    pub url: String,
    pub revision: String,
}

#[derive(Debug, Clone)]
pub struct SceneBundleProbe {
    pub bundle: Option<SceneComponentBundle>,
    pub cache_marker: String,
    pub build: Option<PendingSceneBundleBuild>,
}

#[derive(Debug, Clone)]
pub struct PendingSceneBundleBuild {
    pub app_id: String,
    pub scene_id: String,
    pub revision: String,
    pub components_root: PathBuf,
    pub entries: Vec<String>,
    pub cache_path: PathBuf,
}

fn inflight_builds() -> &'static Mutex<BTreeSet<String>> {
    static BUILDS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
    BUILDS.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn scene_bundle_force_disabled() -> bool {
    match std::env::var("MEI_DISABLE_SCENE_BUNDLE") {
        Ok(value) => {
            let normalized = value.trim();
            normalized == "1" || normalized.eq_ignore_ascii_case("true")
        }
        Err(_) => false,
    }
}

fn scene_bundle_force_enabled() -> bool {
    match std::env::var("MEI_ENABLE_SCENE_BUNDLE") {
        Ok(value) => {
            let normalized = value.trim();
            normalized == "1" || normalized.eq_ignore_ascii_case("true")
        }
        Err(_) => false,
    }
}

fn scene_bundle_scene_allowlist(config: &MeiConfig) -> Vec<String> {
    config
        .host
        .get("sceneBundleScenes")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::trim)
                .filter(|scene_id| !scene_id.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn scene_bundle_enabled_for_app(app_root: &Path, scene_id: &str) -> bool {
    if scene_bundle_force_disabled() {
        return false;
    }
    if scene_bundle_force_enabled() {
        return true;
    }
    let config = load_mei_config_for_app(app_root, None);
    if config.features.scene_bundle != Some(true) {
        return false;
    }
    let allowlist = scene_bundle_scene_allowlist(&config);
    allowlist.is_empty() || allowlist.iter().any(|item| item == scene_id)
}

pub fn should_build_scene_bundle(
    app_root: &Path,
    route_mode: UiRouteMode,
    scene_id: &str,
) -> bool {
    route_mode.is_access_like() && scene_bundle_enabled_for_app(app_root, scene_id)
}

pub fn scene_bundle_cache_marker(
    app_root: &Path,
    route_mode: UiRouteMode,
    scene_id: &str,
) -> String {
    if should_build_scene_bundle(app_root, route_mode, scene_id) {
        "scene-bundle:on".to_string()
    } else {
        "scene-bundle:off".to_string()
    }
}

fn scene_bundle_cache_dir(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_var_root(app_root)
        .join("cache")
        .join("scene-bundles")
}

fn build_script_path(package_root: &Path) -> PathBuf {
    package_root
        .join("scripts")
        .join("build-scene-component-bundle.mjs")
}

fn collect_entry_scripts(component_assets: &[ComponentAsset]) -> Vec<String> {
    let mut entries = component_assets
        .iter()
        .map(|asset| asset.script.trim().to_string())
        .filter(|script| !script.is_empty())
        .collect::<Vec<_>>();
    entries.sort();
    entries.dedup();
    entries
}

fn run_node_script(
    package_root: &Path,
    components_root: &Path,
    entries: &[String],
    extra_args: &[&str],
) -> Result<String> {
    let script = build_script_path(package_root);
    if !script.is_file() {
        anyhow::bail!("scene bundle script not found: {}", script.display());
    }
    let mut command = Command::new("node");
    command.arg(&script);
    command.arg("--components-root");
    command.arg(components_root);
    command.arg("--entries");
    command.arg(entries.join(","));
    for arg in extra_args {
        command.arg(arg);
    }
    let output = command
        .output()
        .with_context(|| format!("failed to spawn node for {}", script.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "scene bundle node script failed (status={}): {}",
            output.status,
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn compute_scene_bundle_revision(
    package_root: &Path,
    components_root: &Path,
    entries: &[String],
) -> Result<String> {
    if entries.is_empty() {
        anyhow::bail!("scene bundle requires at least one entry script");
    }
    let revision = run_node_script(package_root, components_root, entries, &["--revision-only"])?;
    if revision.len() != 16 || !revision.chars().all(|ch| ch.is_ascii_hexdigit()) {
        anyhow::bail!("invalid scene bundle revision: {revision}");
    }
    Ok(revision)
}

fn compute_scene_bundle_revision_cached(
    package_root: &Path,
    components_root: &Path,
    entries: &[String],
) -> Result<String> {
    compute_scene_bundle_revision(package_root, components_root, entries)
}

pub fn scene_bundle_public_url(app_id: &str, scene_id: &str, revision: &str) -> String {
    format!("/workspace-components/bundles/{app_id}/{scene_id}.{revision}.js")
}

pub fn resolve_scene_bundle_cache_path(
    app_root: &Path,
    scene_id: &str,
    revision: &str,
) -> PathBuf {
    scene_bundle_cache_dir(app_root).join(format!("{scene_id}.{revision}.js"))
}

pub fn parse_scene_bundle_request_path(
    request_path: &str,
) -> Option<(String, String, String)> {
    let normalized = request_path.trim().trim_start_matches('/');
    let rest = normalized.strip_prefix("bundles/")?;
    let (app_id, file_name) = rest.rsplit_once('/')?;
    if app_id.is_empty() || file_name.is_empty() {
        return None;
    }
    let file_name = file_name.strip_suffix(".js")?;
    let (scene_id, revision) = file_name.rsplit_once('.')?;
    if scene_id.is_empty()
        || revision.len() != 16
        || !revision.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return None;
    }
    Some((
        app_id.to_string(),
        scene_id.to_string(),
        revision.to_string(),
    ))
}

pub fn probe_scene_component_bundle(
    package_root: &Path,
    source_root: &Path,
    app_id: &str,
    scene_id: &str,
    component_assets: &[ComponentAsset],
) -> SceneBundleProbe {
    let entries = collect_entry_scripts(component_assets);
    if entries.is_empty() {
        return SceneBundleProbe {
            bundle: None,
            cache_marker: "scene-bundle:entries-empty".to_string(),
            build: None,
        };
    }
    let app_root = resolve_app_root(source_root, app_id);
    let components_root = resolve_components_root(source_root);
    let revision = match compute_scene_bundle_revision_cached(
        package_root,
        components_root.as_path(),
        &entries,
    ) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                app_id = %app_id,
                scene_id = %scene_id,
                error = %error,
                "scene bundle revision failed; falling back to per-file modules"
            );
            return SceneBundleProbe {
                bundle: None,
                cache_marker: "scene-bundle:revision-error".to_string(),
                build: None,
            };
        }
    };
    let cache_path =
        resolve_scene_bundle_cache_path(app_root.as_path(), scene_id, revision.as_str());
    if cache_path.is_file() {
        return SceneBundleProbe {
            bundle: Some(SceneComponentBundle {
                url: scene_bundle_public_url(app_id, scene_id, revision.as_str()),
                revision: revision.clone(),
            }),
            cache_marker: format!("scene-bundle:ready:{revision}"),
            build: None,
        };
    }
    SceneBundleProbe {
        bundle: None,
        cache_marker: format!("scene-bundle:missing:{revision}"),
        build: Some(PendingSceneBundleBuild {
            app_id: app_id.to_string(),
            scene_id: scene_id.to_string(),
            revision,
            components_root,
            entries,
            cache_path,
        }),
    }
}

pub fn schedule_scene_component_bundle_build(
    package_root: &Path,
    workspace_root: &Path,
    build: &PendingSceneBundleBuild,
) {
    let build_key = build.cache_path.to_string_lossy().to_string();
    let should_spawn = if let Ok(mut builds) = inflight_builds().lock() {
        builds.insert(build_key.clone())
    } else {
        false
    };
    if !should_spawn {
        return;
    }
    let package_root = package_root.to_path_buf();
    let workspace_root = workspace_root.to_path_buf();
    let app_id = build.app_id.clone();
    let scene_id = build.scene_id.clone();
    let revision = build.revision.clone();
    let components_root = build.components_root.clone();
    let entries = build.entries.clone();
    let cache_path = build.cache_path.clone();
    thread::spawn(move || {
        let result = build_scene_bundle_file(
            package_root.as_path(),
            components_root.as_path(),
            &entries,
            cache_path.as_path(),
            revision.as_str(),
        );
        match result {
            Ok(()) => {
                let cleared =
                    crate::access_page_cache::clear_access_page_render_cache_for_app(
                        workspace_root.as_path(),
                        app_id.as_str(),
                    );
                info!(
                    app_id = %app_id,
                    scene_id = %scene_id,
                    revision = %revision,
                    cleared_page_render_cache_entries = cleared,
                    "scene bundle build completed in background"
                )
            }
            Err(error) => warn!(
                app_id = %app_id,
                scene_id = %scene_id,
                revision = %revision,
                error = %error,
                "scene bundle background build failed; continuing with per-file modules"
            ),
        }
        if let Ok(mut builds) = inflight_builds().lock() {
            builds.remove(build_key.as_str());
        }
    });
}

pub fn scene_bundle_status(probe: &SceneBundleProbe) -> &'static str {
    if probe.bundle.is_some() {
        "ready"
    } else if probe.build.is_some() {
        "scheduled"
    } else if probe.cache_marker.contains("off") {
        "disabled"
    } else if probe.cache_marker.contains("entries-empty") {
        "empty"
    } else {
        "fallback"
    }
}

fn build_scene_bundle_file(
    package_root: &Path,
    components_root: &Path,
    entries: &[String],
    out_path: &Path,
    revision: &str,
) -> Result<()> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let actual_revision = run_node_script(
        package_root,
        components_root,
        entries,
        &[
            "--out",
            out_path.to_string_lossy().as_ref(),
            "--revision",
            revision,
        ],
    )?;
    if actual_revision != revision {
        anyhow::bail!(
            "scene bundle revision drifted during build: expected {revision}, got {actual_revision}"
        );
    }
    Ok(())
}
