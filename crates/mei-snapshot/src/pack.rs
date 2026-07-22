use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::manifest::{
    DataModeHint, ManifestFileEntry, SnapshotAppEntry, SnapshotManifest, FORMAT_NAME,
    FORMAT_VERSION_V1, FORMAT_VERSION_V2,
};
use crate::paths::{resolve_app_env_root, resolve_bundle_path};
use crate::portable_config::build_portable_app_toml;
use crate::resources::{ResourceEntry, ResourceSeverity, ResourceState, ResourcesDocument};

const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "webm", "mov", "avi", "mkv", "m4v", "mp3", "wav", "pdf",
];
const STRUCTURED_EXTENSIONS: &[&str] = &["csv", "json", "geojson"];
const TABLE_EXTENSIONS: &[&str] = &["xlsx", "xls"];

#[derive(Debug, Clone)]
pub struct PackOptions {
    pub workspace: PathBuf,
    pub app_id: String,
    pub out: PathBuf,
    pub include_data: bool,
    pub include_cache: bool,
    pub default_scene: Option<String>,
    pub compiler_version: Option<String>,
}

/// Options for portable multi-app snapshot (formatVersion 2).
#[derive(Debug, Clone)]
pub struct PortablePackOptions {
    pub workspace: PathBuf,
    pub app_ids: Vec<String>,
    pub out: PathBuf,
    pub default_scene: Option<String>,
    pub compiler_version: Option<String>,
    pub workspace_label: Option<String>,
    /// Optional path to platform package root (for stock revision / overlay diff).
    pub package_root: Option<PathBuf>,
    /// When true, also bundle media files under upload/ (default false).
    pub include_media: bool,
}

/// Legacy v1 pack: single app, exchange/ + optional data-snapshots.
pub fn pack_snapshot(opts: &PackOptions) -> anyhow::Result<SnapshotManifest> {
    let env_root = resolve_app_env_root(&opts.workspace, &opts.app_id)?;
    let bundle = resolve_bundle_path(&env_root, &opts.app_id)?;

    let staging = tempfile_dir(&opts.out)?;
    let exchange_dir = staging.join("exchange");
    fs::create_dir_all(&exchange_dir)?;
    let bundle_name = format!("{}.meibundle", opts.app_id);
    let staged_bundle = exchange_dir.join(&bundle_name);
    fs::copy(&bundle, &staged_bundle)?;

    let mut data_mode = DataModeHint::Static;
    if opts.include_data {
        let src = env_root.join("var").join("data-snapshots");
        if src.is_dir() {
            copy_dir_contents(&src, &staging.join("data-snapshots"))?;
            data_mode = DataModeHint::Eval;
        }
    }
    if opts.include_cache {
        let mrg = env_root.join("var").join("mrg");
        if mrg.is_dir() {
            copy_dir_contents(&mrg, &staging.join("cache").join("mrg"))?;
        }
    }

    let mut files = collect_file_entries(&staging)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let manifest = SnapshotManifest {
        format: FORMAT_NAME.to_string(),
        format_version: FORMAT_VERSION_V1,
        app_id: opts.app_id.clone(),
        default_scene: opts.default_scene.clone(),
        compiler_version: opts.compiler_version.clone(),
        data_mode_hint: data_mode,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        files,
        apps: Vec::new(),
        workspace_label: None,
        platform_stock_revision: None,
        resources_path: None,
    };
    manifest.validate()?;

    let manifest_path = staging.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    if let Some(parent) = opts.out.parent() {
        fs::create_dir_all(parent)?;
    }
    write_zip(&staging, &opts.out)?;
    let _ = fs::remove_dir_all(&staging);
    Ok(manifest)
}

/// Portable v2 pack: multi-app, portable config, sealed data, resources manifest.
pub fn pack_portable_snapshot(opts: &PortablePackOptions) -> anyhow::Result<SnapshotManifest> {
    if opts.app_ids.is_empty() {
        anyhow::bail!("portable pack requires at least one app id");
    }
    let staging = tempfile_dir(&opts.out)?;
    let mut resources: Vec<ResourceEntry> = Vec::new();
    let mut app_entries: Vec<SnapshotAppEntry> = Vec::new();
    let mut overall_hint = DataModeHint::Static;

    for app_id in &opts.app_ids {
        let app_root = opts.workspace.join("apps").join(app_id);
        if !app_root.is_dir() {
            anyhow::bail!("app directory missing: {}", app_root.display());
        }
        let env_root = resolve_app_env_root(&opts.workspace, app_id)?;
        let bundle = resolve_bundle_path(&env_root, app_id)?;

        let app_stage = staging.join("apps").join(app_id);
        let exchange_dir = app_stage.join("exchange");
        fs::create_dir_all(&exchange_dir)?;
        let bundle_name = format!("{app_id}.meibundle");
        fs::copy(&bundle, exchange_dir.join(&bundle_name))?;
        let bundle_rel = format!("apps/{app_id}/exchange/{bundle_name}");
        // Required runtime closure — without meibundle the receiver cannot load.
        resources.push(ResourceEntry {
            id: format!("{app_id}.meibundle"),
            app_id: app_id.clone(),
            kind: "meibundle".into(),
            state: ResourceState::Bundled,
            target_path: format!("apps/{app_id}/env/current/build/exchange/{bundle_name}"),
            required_for: Some("runtime".into()),
            severity: ResourceSeverity::Blocking,
            sha256: sha256_file(&bundle).ok(),
            bytes: fs::metadata(&bundle).ok().map(|m| m.len()),
            hint: Some("必要：场景/指标图结构；导入后禁止再 compile/prebuild 冲掉".into()),
            recovery: None,
        });

        // Portable runtime config
        let portable = build_portable_app_toml(&app_root, app_id)?;
        let runtime_dir = app_stage.join("runtime");
        fs::create_dir_all(&runtime_dir)?;
        fs::write(runtime_dir.join("app.toml"), portable.toml.as_bytes())?;

        // Marker for sealed data mode after materialize
        fs::write(app_stage.join(".mei-portable-snapshot"), b"1\n")?;

        let data_mode = DataModeHint::Eval;
        let ds_src = env_root.join("var").join("data-snapshots");
        let has_parquet = dir_has_extension(&ds_src, "parquet");
        // Portable sealed demos need parquet in-pack. Raw upload xlsx is optional when
        // parquet exists; videos are always optional (`include_media`).
        if !has_parquet {
            anyhow::bail!(
                "portable pack for `{app_id}` requires apps/{app_id}/env/current/var/data-snapshots/*.parquet \
                 (necessary for sealed eval). Run prebuild on the author workspace first; \
                 do not rely on upload/ videos or raw xlsx as the runtime data closure."
            );
        }
        copy_dir_contents(&ds_src, &app_stage.join("data-snapshots"))?;
        overall_hint = DataModeHint::Eval;
        resources.push(ResourceEntry {
            id: format!("{app_id}.data-snapshots"),
            app_id: app_id.clone(),
            kind: "data-snapshots".into(),
            state: ResourceState::Bundled,
            target_path: format!("apps/{app_id}/env/current/var/data-snapshots"),
            required_for: Some("eval".into()),
            severity: ResourceSeverity::Blocking,
            sha256: None,
            bytes: None,
            hint: Some("必要：表格指标用包内 parquet；与大媒体无关".into()),
            recovery: None,
        });

        // Assets
        let assets_src = app_root.join("assets");
        if assets_src.is_dir() {
            copy_dir_contents(&assets_src, &app_stage.join("assets"))?;
            resources.push(ResourceEntry {
                id: format!("{app_id}.assets"),
                app_id: app_id.clone(),
                kind: "assets".into(),
                state: ResourceState::Bundled,
                target_path: format!("apps/{app_id}/assets"),
                required_for: Some("ui".into()),
                severity: ResourceSeverity::Degrade,
                sha256: None,
                bytes: None,
                hint: None,
                recovery: None,
            });
        }

        // Sealed view/eval store — required so Viewer can skip prebuild yet still
        // serve theme tokens, metric skins, and KPI eval slots (map layers etc.).
        let store_packed = pack_sealed_store_content(&env_root, &app_stage)?;
        if store_packed > 0 {
            resources.push(ResourceEntry {
                id: format!("{app_id}.sealed-store"),
                app_id: app_id.clone(),
                kind: "sealed-store".into(),
                state: ResourceState::Bundled,
                target_path: format!("apps/{app_id}/store-content"),
                required_for: Some("eval".into()),
                severity: ResourceSeverity::Blocking,
                sha256: None,
                bytes: None,
                hint: Some(format!(
                    "必要：已打包 {store_packed} 个 sealed store 文件（theme/eval/content_panel/…）"
                )),
                recovery: None,
            });
        }

        // MCG/MRG registries — view-revision assemble reads these; without them
        // Access returns assemble unavailable / blank map layers after sealed import.
        let registry_packed = pack_sealed_registry(&env_root, &app_stage)?;
        if registry_packed > 0 {
            resources.push(ResourceEntry {
                id: format!("{app_id}.sealed-registry"),
                app_id: app_id.clone(),
                kind: "sealed-registry".into(),
                state: ResourceState::Bundled,
                target_path: format!("apps/{app_id}/registry"),
                required_for: Some("assemble".into()),
                severity: ResourceSeverity::Blocking,
                sha256: None,
                bytes: None,
                hint: Some(format!(
                    "必要：已打包 {registry_packed} 个 registry 文件（mcg/mrg/bridge/admin）"
                )),
                recovery: None,
            });
        }

        // Prototype (if referenced / present)
        let proto_src = app_root.join("prototype");
        if proto_src.is_dir() {
            copy_dir_contents(&proto_src, &app_stage.join("prototype"))?;
        }

        // Classify upload sources from portable config
        let upload_root = app_root.join("upload");
        for src in &portable.sources {
            let rel = src.path.trim().trim_start_matches("./");
            let abs = app_root.join(rel);
            let ext = Path::new(rel)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            if TABLE_EXTENSIONS.contains(&ext.as_str()) {
                // Parquet is required above; original xlsx is optional (edit-source only).
                resources.push(ResourceEntry {
                    id: format!("{app_id}.source.{}", src.id),
                    app_id: app_id.clone(),
                    kind: "xlsx".into(),
                    state: if abs.is_file() {
                        ResourceState::External
                    } else {
                        ResourceState::Missing
                    },
                    target_path: format!("apps/{app_id}/{rel}"),
                    required_for: Some("edit-source".into()),
                    severity: ResourceSeverity::Info,
                    sha256: abs.is_file().then(|| sha256_file(&abs).ok()).flatten(),
                    bytes: abs
                        .is_file()
                        .then(|| fs::metadata(&abs).ok().map(|m| m.len()))
                        .flatten(),
                    hint: Some("可选：演示用包内 parquet；改表时再补原 xlsx".into()),
                    recovery: Some("import_file".into()),
                });
            } else if STRUCTURED_EXTENSIONS.contains(&ext.as_str()) {
                if abs.is_file() {
                    let dest = app_stage.join("portable-data").join(rel);
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&abs, &dest)?;
                    resources.push(ResourceEntry {
                        id: format!("{app_id}.source.{}", src.id),
                        app_id: app_id.clone(),
                        kind: src.kind.clone(),
                        state: ResourceState::Bundled,
                        target_path: format!("apps/{app_id}/{rel}"),
                        required_for: Some("eval".into()),
                        severity: ResourceSeverity::Degrade,
                        sha256: sha256_file(&abs).ok(),
                        bytes: fs::metadata(&abs).ok().map(|m| m.len()),
                        hint: None,
                        recovery: None,
                    });
                } else {
                    resources.push(ResourceEntry {
                        id: format!("{app_id}.source.{}", src.id),
                        app_id: app_id.clone(),
                        kind: src.kind.clone(),
                        state: ResourceState::Missing,
                        target_path: format!("apps/{app_id}/{rel}"),
                        required_for: Some("eval".into()),
                        severity: ResourceSeverity::Degrade,
                        sha256: None,
                        bytes: None,
                        hint: Some(format!("导出时未找到 {rel}")),
                        recovery: Some("import_file".into()),
                    });
                }
            } else {
                resources.push(ResourceEntry {
                    id: format!("{app_id}.source.{}", src.id),
                    app_id: app_id.clone(),
                    kind: src.kind.clone(),
                    state: if abs.is_file() {
                        ResourceState::External
                    } else {
                        ResourceState::Missing
                    },
                    target_path: format!("apps/{app_id}/{rel}"),
                    required_for: Some("eval".into()),
                    severity: ResourceSeverity::Degrade,
                    sha256: abs.is_file().then(|| sha256_file(&abs).ok()).flatten(),
                    bytes: abs
                        .is_file()
                        .then(|| fs::metadata(&abs).ok().map(|m| m.len()))
                        .flatten(),
                    hint: Some("该数据源未自动入包，需另行补齐".into()),
                    recovery: Some("import_file".into()),
                });
            }
        }

        // Scan upload for media → external by default
        if upload_root.is_dir() {
            for entry in WalkDir::new(&upload_root)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                let rel = path
                    .strip_prefix(&app_root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !MEDIA_EXTENSIONS.contains(&ext.as_str()) {
                    continue;
                }
                if opts.include_media {
                    let dest = app_stage.join("portable-data").join(&rel);
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(path, &dest)?;
                    resources.push(ResourceEntry {
                        id: format!("{app_id}.media.{}", rel.replace('/', ".")),
                        app_id: app_id.clone(),
                        kind: "video".into(),
                        state: ResourceState::Bundled,
                        target_path: format!("apps/{app_id}/{rel}"),
                        required_for: Some("media-playback".into()),
                        severity: ResourceSeverity::Degrade,
                        sha256: sha256_file(path).ok(),
                        bytes: fs::metadata(path).ok().map(|m| m.len()),
                        hint: Some("可选：大媒体（已勾选「包含大媒体」）".into()),
                        recovery: None,
                    });
                } else {
                    resources.push(ResourceEntry {
                        id: format!("{app_id}.media.{}", rel.replace('/', ".")),
                        app_id: app_id.clone(),
                        kind: if ext == "pdf" { "pdf" } else { "video" }.into(),
                        state: ResourceState::External,
                        target_path: format!("apps/{app_id}/{rel}"),
                        required_for: Some("media-playback".into()),
                        severity: ResourceSeverity::Info,
                        sha256: sha256_file(path).ok(),
                        bytes: fs::metadata(path).ok().map(|m| m.len()),
                        hint: Some(
                            "可选：大媒体默认外置，不影响图表；Viewer「补齐资源」可导入".into(),
                        ),
                        recovery: Some("import_file".into()),
                    });
                }
            }
        }

        // GIS: workspace stock/gis is packed once at workspace level (see pack_workspace_gis).
        // Per-app marker remains for replenish UI when tiles were missing at pack time.
        let gis_tiles = opts.workspace.join("stock").join("gis").join("tiles");
        let has_tiles = gis_tiles.is_dir()
            && std::fs::read_dir(&gis_tiles)
                .map(|rd| {
                    rd.filter_map(|e| e.ok()).any(|e| {
                        e.path()
                            .extension()
                            .and_then(|x| x.to_str())
                            .map(|x| x.eq_ignore_ascii_case("mbtiles"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
        resources.push(ResourceEntry {
            id: format!("{app_id}.gis.basemap"),
            app_id: app_id.clone(),
            kind: "gis".into(),
            state: if has_tiles {
                ResourceState::Bundled
            } else {
                ResourceState::External
            },
            target_path: "stock/gis/tiles".into(),
            required_for: Some("basemap".into()),
            severity: ResourceSeverity::Degrade,
            sha256: None,
            bytes: None,
            hint: Some(if has_tiles {
                "工作区 stock/gis/tiles 已打入包；Host 启动时自动托管 Martin".into()
            } else {
                "工作区缺少 stock/gis/tiles/*.mbtiles；无底图时地图其他功能仍可用".into()
            }),
            recovery: if has_tiles {
                None
            } else {
                Some("place_mbtiles".into())
            },
        });

        for remote in &portable.dropped_remote_sources {
            resources.push(ResourceEntry {
                id: format!("{app_id}.remote.{remote}"),
                app_id: app_id.clone(),
                kind: "remote-source".into(),
                state: ResourceState::External,
                target_path: format!("apps/{app_id}"),
                required_for: Some("eval".into()),
                severity: ResourceSeverity::Degrade,
                sha256: None,
                bytes: None,
                hint: Some(format!("远端/数据库源 `{remote}` 未打包，对应指标将降级")),
                recovery: None,
            });
        }

        app_entries.push(SnapshotAppEntry {
            app_id: app_id.clone(),
            default_scene: opts.default_scene.clone(),
            data_mode_hint: data_mode,
            bundle_path: bundle_rel,
            compiler_version: opts.compiler_version.clone(),
        });
    }

    // Stock overlay: workspace stock files that differ from package (when package_root set)
    pack_stock_overlay(
        &opts.workspace,
        opts.package_root.as_deref(),
        &staging,
        &mut resources,
    )?;

    // Workspace GIS tiles (mbtiles) — default bundled for portable demos.
    pack_workspace_gis(&opts.workspace, &staging, &mut resources)?;

    // Workspace scene theme library (colors / role maps). Without this, Viewer
    // home workspace.json has no ops.sceneThemes and Access falls back to Host
    // shell defaults — font_scale in app.toml alone cannot restore cockpit look.
    pack_workspace_scene_ops(&opts.workspace, &staging, &mut resources)?;

    let platform_stock_revision = opts
        .package_root
        .as_ref()
        .and_then(|root| read_stock_revision_hint(root));

    let resources_doc = ResourcesDocument::new(resources);
    fs::write(
        staging.join("resources.json"),
        serde_json::to_vec_pretty(&resources_doc)?,
    )?;

    let readme = build_readme(&opts.app_ids, &resources_doc);
    fs::write(staging.join("README.txt"), readme.as_bytes())?;

    let mut files = collect_file_entries(&staging)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let primary = opts.app_ids[0].clone();
    let manifest = SnapshotManifest {
        format: FORMAT_NAME.to_string(),
        format_version: FORMAT_VERSION_V2,
        app_id: primary,
        default_scene: opts.default_scene.clone(),
        compiler_version: opts.compiler_version.clone(),
        data_mode_hint: overall_hint,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        files,
        apps: app_entries,
        workspace_label: opts.workspace_label.clone(),
        platform_stock_revision,
        resources_path: Some("resources.json".into()),
    };
    manifest.validate()?;

    fs::write(
        staging.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    if let Some(parent) = opts.out.parent() {
        fs::create_dir_all(parent)?;
    }
    write_zip(&staging, &opts.out)?;
    let _ = fs::remove_dir_all(&staging);
    Ok(manifest)
}

fn pack_stock_overlay(
    workspace: &Path,
    package_root: Option<&Path>,
    staging: &Path,
    resources: &mut Vec<ResourceEntry>,
) -> anyhow::Result<()> {
    let ws_stock = workspace.join("stock");
    if !ws_stock.is_dir() {
        return Ok(());
    }
    let mut overlay_count = 0usize;
    for sub in ["components", "templates"] {
        let from = ws_stock.join(sub);
        if !from.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&from).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let rel = path
                .strip_prefix(&ws_stock)?
                .to_string_lossy()
                .replace('\\', "/");
            let include = match package_root {
                Some(pkg) => {
                    let platform = pkg.join("stock").join(&rel);
                    if !platform.is_file() {
                        true
                    } else {
                        let a = fs::read(path)?;
                        let b = fs::read(&platform)?;
                        a != b
                    }
                }
                // Without package root, only include non-empty custom trees if
                // a `.mei-stock-overlay` marker exists (avoid shipping full platform stock).
                None => workspace.join(".mei-stock-overlay").is_file(),
            };
            if !include {
                continue;
            }
            let dest = staging.join("stock-overlay").join(&rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &dest)?;
            overlay_count += 1;
        }
    }
    if overlay_count > 0 {
        resources.push(ResourceEntry {
            id: "stock.overlay".into(),
            app_id: "*".into(),
            kind: "stock-overlay".into(),
            state: ResourceState::Bundled,
            target_path: "stock".into(),
            required_for: Some("components".into()),
            severity: ResourceSeverity::Info,
            sha256: None,
            bytes: None,
            hint: Some(format!("已打包 {overlay_count} 个工作区自定义 stock 文件")),
            recovery: None,
        });
    }
    Ok(())
}

/// Pack workspace-level scene theme library into `workspace-ops.json`.
/// Scene colors live here (not in app.toml); see docs 0310 / 0540.
fn pack_workspace_scene_ops(
    workspace: &Path,
    staging: &Path,
    resources: &mut Vec<ResourceEntry>,
) -> anyhow::Result<()> {
    let ws_json = workspace.join("workspace.json");
    if !ws_json.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&ws_json)?;
    let doc: serde_json::Value = serde_json::from_str(&raw)?;
    let Some(ops) = doc.get("ops").and_then(|v| v.as_object()) else {
        return Ok(());
    };
    let mut out = serde_json::Map::new();
    for key in [
        "sceneThemes",
        "sceneThemeDefault",
        "shellTheme",
        "themes", // legacy shell theme host chrome
    ] {
        if let Some(value) = ops.get(key) {
            if !value.is_null() {
                out.insert(key.to_string(), value.clone());
            }
        }
    }
    if out.is_empty() {
        return Ok(());
    }
    let scene_count = out
        .get("sceneThemes")
        .and_then(|v| v.as_object())
        .map(|m| m.len())
        .unwrap_or(0);
    let payload = serde_json::Value::Object(out);
    fs::write(
        staging.join("workspace-ops.json"),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    resources.push(ResourceEntry {
        id: "workspace.scene-ops".into(),
        app_id: "*".into(),
        kind: "workspace-ops".into(),
        state: ResourceState::Bundled,
        target_path: "workspace-ops.json".into(),
        required_for: Some("theme".into()),
        severity: ResourceSeverity::Blocking,
        sha256: None,
        bytes: None,
        hint: Some(format!(
            "必要：已打包工作区 sceneThemes（{scene_count} 套）供 Viewer 装配 cockpit 外观"
        )),
        recovery: None,
    });
    Ok(())
}

/// Bundle workspace `stock/gis/**` (primarily `tiles/*.mbtiles`) into the portable zip.
fn pack_workspace_gis(
    workspace: &Path,
    staging: &Path,
    resources: &mut Vec<ResourceEntry>,
) -> anyhow::Result<()> {
    let gis_root = workspace.join("stock").join("gis");
    if !gis_root.is_dir() {
        return Ok(());
    }
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;
    for entry in WalkDir::new(&gis_root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(workspace)?
            .to_string_lossy()
            .replace('\\', "/");
        let dest = staging.join(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(path, &dest)?;
        let bytes = fs::metadata(path).ok().map(|m| m.len());
        total_bytes += bytes.unwrap_or(0);
        file_count += 1;
        let is_mbtiles = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("mbtiles"))
            .unwrap_or(false);
        if is_mbtiles {
            resources.push(ResourceEntry {
                id: format!("workspace.gis.{}", rel.replace('/', ".")),
                app_id: "*".into(),
                kind: "gis".into(),
                state: ResourceState::Bundled,
                target_path: rel,
                required_for: Some("basemap".into()),
                severity: ResourceSeverity::Info,
                sha256: sha256_file(path).ok(),
                bytes,
                hint: Some("已打入 portable 包；导入后由 Host managed_martin 自动托管".into()),
                recovery: None,
            });
        }
    }
    if file_count > 0 {
        resources.push(ResourceEntry {
            id: "workspace.gis.stock".into(),
            app_id: "*".into(),
            kind: "gis".into(),
            state: ResourceState::Bundled,
            target_path: "stock/gis".into(),
            required_for: Some("basemap".into()),
            severity: ResourceSeverity::Info,
            sha256: None,
            bytes: Some(total_bytes),
            hint: Some(format!(
                "已打包工作区 stock/gis（{file_count} 个文件，约 {} MiB）",
                total_bytes / (1024 * 1024)
            )),
            recovery: None,
        });
    }
    Ok(())
}

fn read_stock_revision_hint(package_root: &Path) -> Option<String> {
    let components = package_root.join("stock").join("components");
    if !components.is_dir() {
        return None;
    }
    let mut hasher = Sha256::new();
    let mut paths: Vec<String> = WalkDir::new(&components)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            e.path()
                .strip_prefix(&components)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    paths.sort();
    for p in &paths {
        hasher.update(p.as_bytes());
        hasher.update(b"\n");
    }
    Some(format!("stock-components-{:x}", hasher.finalize()))
}

fn build_readme(app_ids: &[String], resources: &ResourcesDocument) -> String {
    let mut out = String::new();
    out.push_str("Mei Portable Snapshot (v2)\n");
    out.push_str("==========================\n\n");
    out.push_str(&format!("Apps: {}\n\n", app_ids.join(", ")));
    out.push_str("This archive is a portable demo package (not source).\n");
    out.push_str("Import with Mei Viewer. Host must NOT re-prebuild/compile sealed apps.\n\n");
    out.push_str("Necessary (blocking):\n");
    out.push_str("  - exchange/*.meibundle  — scene / metric graph\n");
    out.push_str("  - data-snapshots/*.parquet — table metrics (sealed eval)\n");
    out.push_str("  - store-content/** — theme / eval slots / panel skins (no prebuild)\n");
    out.push_str("  - registry/** — mcg/mrg for view-revision assemble (no prebuild)\n");
    out.push_str("  - workspace-ops.json — sceneThemes / sceneThemeDefault (cockpit look)\n");
    out.push_str("  - runtime/app.toml — portable config (basemap style / layout / fonts)\n\n");
    out.push_str("Optional (degrade / info):\n");
    out.push_str("  - upload videos/PDF — only if packed with --include-media\n");
    out.push_str("  - original xlsx — not needed when parquet is present\n");
    out.push_str("  - stock/gis tiles — map basemap; other features still work\n\n");
    let external: Vec<_> = resources
        .resources
        .iter()
        .filter(|r| r.state == ResourceState::External || r.state == ResourceState::Missing)
        .collect();
    if !external.is_empty() {
        out.push_str("External / missing resources:\n");
        for r in external {
            out.push_str(&format!("- [{}] {} → {}\n", r.kind, r.id, r.target_path));
            if let Some(hint) = &r.hint {
                out.push_str(&format!("  {hint}\n"));
            }
        }
        out.push_str(
            "\nUse Viewer 「待补齐资源」 to import optional files; do not edit env/ by hand.\n",
        );
    }
    out
}

fn dir_has_extension(dir: &Path, ext: &str) -> bool {
    if !dir.is_dir() {
        return false;
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case(ext))
        })
}

/// Pack the full sealed `build/store/content` tree (all artifact kinds).
/// A whitelist missed `manifest_index` / `navigation` / … and left Viewer
/// with partial digests after import; copy everything present instead.
fn pack_sealed_store_content(env_root: &Path, app_stage: &Path) -> anyhow::Result<usize> {
    let content_root = env_root.join("build").join("store").join("content");
    if !content_root.is_dir() {
        return Ok(0);
    }
    let dest_root = app_stage.join("store-content");
    copy_dir_contents(&content_root, &dest_root)?;
    let packed = WalkDir::new(&dest_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();
    Ok(packed)
}

const SEALED_REGISTRY_FILES: &[&str] = &[
    "mcg-registry.json",
    "mrg-registry.json",
    "bridge.json",
    "admin-registry.json",
];

fn pack_sealed_registry(env_root: &Path, app_stage: &Path) -> anyhow::Result<usize> {
    let registry_root = env_root.join("build").join("registry");
    if !registry_root.is_dir() {
        return Ok(0);
    }
    let dest_root = app_stage.join("registry");
    fs::create_dir_all(&dest_root)?;
    let mut packed = 0usize;
    for name in SEALED_REGISTRY_FILES {
        let src = registry_root.join(name);
        if !src.is_file() {
            continue;
        }
        fs::copy(&src, dest_root.join(name))?;
        packed += 1;
    }
    Ok(packed)
}

fn tempfile_dir(out: &Path) -> anyhow::Result<PathBuf> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = format!(".mei-snapshot-staging-{}", std::process::id());
    let dir = parent.join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn copy_dir_contents(src: &Path, dst: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path == src {
            continue;
        }
        let rel = path.strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &target)?;
        }
    }
    Ok(())
}

fn collect_file_entries(root: &Path) -> anyhow::Result<Vec<ManifestFileEntry>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "manifest.json" {
            continue;
        }
        let bytes = fs::metadata(path)?.len();
        let sha256 = sha256_file(path)?;
        out.push(ManifestFileEntry {
            path: rel,
            sha256: Some(sha256),
            bytes: Some(bytes),
        });
    }
    Ok(out)
}

pub(crate) fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_zip(staging: &Path, out: &Path) -> anyhow::Result<()> {
    let file = File::create(out)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let manifest = staging.join("manifest.json");
    if manifest.is_file() {
        zip.start_file("manifest.json", options)?;
        let mut f = File::open(&manifest)?;
        std::io::copy(&mut f, &mut zip)?;
    }

    for entry in WalkDir::new(staging).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(staging)?
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "manifest.json" {
            continue;
        }
        zip.start_file(&rel, options)?;
        let mut f = File::open(path)?;
        std::io::copy(&mut f, &mut zip)?;
    }
    zip.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unpack::unpack_snapshot;

    #[test]
    fn pack_and_unpack_roundtrip_v1() {
        let tmp = std::env::temp_dir().join(format!("mei-snap-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let ws = tmp.join("ws");
        let app = "demo";
        let env = ws.join("apps").join(app).join("env").join("WS-1");
        let exchange = env.join("build").join("exchange");
        fs::create_dir_all(&exchange).unwrap();
        fs::write(exchange.join("demo.meibundle"), b"fake-bundle").unwrap();
        fs::create_dir_all(env.join("var").join("data-snapshots")).unwrap();
        fs::write(
            env.join("var").join("data-snapshots").join("a.parquet"),
            b"pq",
        )
        .unwrap();

        let out = tmp.join("demo.mei-snapshot.zip");
        let manifest = pack_snapshot(&PackOptions {
            workspace: ws,
            app_id: app.into(),
            out: out.clone(),
            include_data: true,
            include_cache: false,
            default_scene: Some("home".into()),
            compiler_version: Some("test".into()),
        })
        .unwrap();
        assert_eq!(manifest.app_id, "demo");
        assert_eq!(manifest.format_version, FORMAT_VERSION_V1);
        assert_eq!(manifest.data_mode_hint, DataModeHint::Eval);

        let dest = tmp.join("unpacked");
        let result = unpack_snapshot(&out, &dest).unwrap();
        assert_eq!(result.manifest.app_id, "demo");
        assert!(result.bundle_path.is_file());
        assert!(dest.join("data-snapshots").join("a.parquet").is_file());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn pack_portable_multi_app_v2() {
        let tmp = std::env::temp_dir().join(format!("mei-snap-v2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let ws = tmp.join("ws");
        for app in ["a", "b"] {
            let app_root = ws.join("apps").join(app);
            let env = app_root.join("env").join("WS-1");
            let exchange = env.join("build").join("exchange");
            fs::create_dir_all(&exchange).unwrap();
            fs::write(exchange.join(format!("{app}.meibundle")), b"bundle").unwrap();
            fs::create_dir_all(env.join("var").join("data-snapshots")).unwrap();
            fs::write(
                env.join("var")
                    .join("data-snapshots")
                    .join("import-manifest.json"),
                br#"{"schema_version":"mei-dataset-import-manifest-v1","entries":[]}"#,
            )
            .unwrap();
            fs::write(
                env.join("var").join("data-snapshots").join("x.parquet"),
                b"pq",
            )
            .unwrap();
            fs::create_dir_all(app_root.join("upload")).unwrap();
            fs::write(app_root.join("upload").join("t.csv"), b"c\n1\n").unwrap();
            fs::create_dir_all(app_root.join("assets")).unwrap();
            fs::write(app_root.join("assets").join("bg.png"), b"png").unwrap();
            fs::write(
                app_root.join("app.toml"),
                format!(
                    r#"
title = "{app}"
app_id = "{app}"
default_stage = "home"

[ops.sources.table]
kind = "csv"
path = "upload/t.csv"
"#
                ),
            )
            .unwrap();
        }

        // Workspace GIS tiles should be default-bundled into portable packs.
        let tiles = ws.join("stock").join("gis").join("tiles");
        fs::create_dir_all(&tiles).unwrap();
        fs::write(tiles.join("demo.mbtiles"), b"mbtiles-bytes").unwrap();

        let out = tmp.join("multi.mei-snapshot.zip");
        let manifest = pack_portable_snapshot(&PortablePackOptions {
            workspace: ws,
            app_ids: vec!["a".into(), "b".into()],
            out: out.clone(),
            default_scene: Some("home".into()),
            compiler_version: Some("test".into()),
            workspace_label: Some("demo".into()),
            package_root: None,
            include_media: false,
        })
        .unwrap();
        assert_eq!(manifest.format_version, FORMAT_VERSION_V2);
        assert_eq!(manifest.apps.len(), 2);

        let dest = tmp.join("unpacked");
        let result = unpack_snapshot(&out, &dest).unwrap();
        assert!(result.manifest.is_v2());
        assert!(dest.join("apps/a/runtime/app.toml").is_file());
        assert!(dest.join("apps/a/portable-data/upload/t.csv").is_file());
        assert!(dest.join("apps/a/assets/bg.png").is_file());
        assert!(dest.join("resources.json").is_file());
        assert!(dest.join("stock/gis/tiles/demo.mbtiles").is_file());
        assert_eq!(result.app_bundle_paths.len(), 2);
        let _ = fs::remove_dir_all(&tmp);
    }
}
