mod host;
mod paths;
mod recent;

use host::{HostHandle, HostReadinessDto};
use recent::RecentStore;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

fn open_host_window(app: &AppHandle, port: u16) -> Result<(), String> {
    // Open in the system browser — same URL renders correctly in Safari/Chrome, but
    // Tauri's WKWebView consistently fails host chrome (Shoelace <sl-dropdown> never
    // upgrades → vertical "mini/其他" bars crush the main pane).
    let url = format!("http://127.0.0.1:{port}/home");
    if let Some(w) = app.get_webview_window("host") {
        let _ = w.close();
    }
    open_system_browser(&url)?;
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(())
}

fn open_system_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("open browser: {e}"))?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|e| format!("open browser: {e}"))?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("open browser: {e}"))?;
        Ok(())
    }
}

pub struct AppState {
    pub host: Mutex<HostHandle>,
    pub recent: Mutex<RecentStore>,
    /// True when launch cwd/argv was a workspace and we skipped the launcher UI.
    pub auto_opened: Mutex<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDto {
    pub running: bool,
    pub ready: bool,
    pub port: Option<u16>,
    pub workspace: Option<String>,
    pub home_workspace: Option<String>,
    pub auto_opened: bool,
    pub log_path: Option<String>,
    pub viewer_version: String,
}

fn viewer_build_version() -> String {
    option_env!("MEI_VIEWER_BUILD_VERSION")
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string()
}

#[tauri::command]
fn host_status(state: State<'_, AppState>) -> StatusDto {
    let host = state.host.lock().expect("host lock");
    let auto_opened = *state.auto_opened.lock().expect("auto_opened lock");
    let log_path = host
        .log_path()
        .map(|p| p.display().to_string())
        .or_else(|| paths::host_log_file().ok().map(|p| p.display().to_string()));
    StatusDto {
        running: host.is_running(),
        ready: host.is_ready(),
        port: host.port(),
        workspace: host.workspace().map(|p| p.display().to_string()),
        home_workspace: paths::home_workspace_dir()
            .ok()
            .map(|p| p.display().to_string()),
        auto_opened,
        log_path,
        viewer_version: viewer_build_version(),
    }
}

#[tauri::command]
fn viewer_version() -> String {
    viewer_build_version()
}

#[tauri::command]
fn host_readiness(state: State<'_, AppState>) -> Result<HostReadinessDto, String> {
    let mut host = state.host.lock().map_err(|e| e.to_string())?;
    host.poll_readiness().map_err(|e| e.to_string())
}

#[tauri::command]
fn wait_host_ready(state: State<'_, AppState>) -> Result<(), String> {
    let mut host = state.host.lock().map_err(|e| e.to_string())?;
    host.wait_for_control_ready(std::time::Duration::from_secs(240))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_recent(state: State<'_, AppState>) -> Vec<String> {
    state
        .recent
        .lock()
        .expect("recent lock")
        .list()
        .into_iter()
        .map(|p| p.display().to_string())
        .collect()
}

/// Scan `workspace/apps/*/app.toml` (or directory name) for exportable apps.
#[tauri::command]
fn list_workspace_apps(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let workspace = host
        .workspace()
        .ok_or_else(|| "未打开工作区，无法列出 app".to_string())?
        .to_path_buf();
    drop(host);
    list_apps_in_workspace(&workspace).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_snapshot(
    app_ids: Vec<String>,
    out_path: String,
    include_data: bool,
    include_media: bool,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let workspace = host
        .workspace()
        .ok_or_else(|| "未打开工作区，无法导出快照".to_string())?
        .to_path_buf();
    drop(host);

    let mut ids: Vec<String> = app_ids
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Err("请至少选择一个 app".into());
    }
    let mut out = PathBuf::from(&out_path);
    if out.as_os_str().is_empty() {
        return Err("输出路径不能为空".into());
    }
    let name = out
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    if !name.ends_with(".mei-snapshot.zip") && !name.ends_with(".zip") {
        out.set_file_name(format!("{name}.mei-snapshot.zip"));
    } else if name.ends_with(".zip") && !name.ends_with(".mei-snapshot.zip") {
        let stem = name.trim_end_matches(".zip");
        out.set_file_name(format!("{stem}.mei-snapshot.zip"));
    }

    for app_id in &ids {
        match mei_snapshot::resolve_app_env_root(&workspace, app_id)
            .and_then(|env| mei_snapshot::resolve_bundle_path(&env, app_id))
        {
            Ok(_) => {}
            Err(err) => {
                return Err(format!(
                    "无法导出 {app_id}：{err}（请先 compile，或在宿主 /runtime 执行 reload）"
                ));
            }
        }
    }

    let package_root = std::env::var_os("MEI_PACKAGE_ROOT").map(PathBuf::from);

    // Prefer portable v2 whenever possible (multi-app or single with data closure).
    let use_portable = ids.len() > 1 || include_data;
    let manifest = if use_portable {
        mei_snapshot::pack_portable_snapshot(&mei_snapshot::PortablePackOptions {
            workspace,
            app_ids: ids.clone(),
            out: out.clone(),
            default_scene: None,
            compiler_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            workspace_label: None,
            package_root,
            include_media,
        })
        .map_err(|e| e.to_string())?
    } else {
        mei_snapshot::pack_snapshot(&mei_snapshot::PackOptions {
            workspace,
            app_id: ids[0].clone(),
            out: out.clone(),
            include_data: false,
            include_cache: false,
            default_scene: None,
            compiler_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        })
        .map_err(|e| e.to_string())?
    };

    Ok(format!(
        "已导出 v{} [{}] → {}（files={}, dataHint={}）",
        manifest.format_version,
        ids.join(", "),
        out.display(),
        manifest.files.len(),
        manifest.data_mode_hint.as_str()
    ))
}

fn list_apps_in_workspace(workspace: &std::path::Path) -> anyhow::Result<Vec<String>> {
    let apps_dir = workspace.join("apps");
    if !apps_dir.is_dir() {
        anyhow::bail!("工作区无 apps/ 目录: {}", apps_dir.display());
    }
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(&apps_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        // Prefer dirs that look like apps (app.toml or any content).
        if path.join("app.toml").is_file() || path.join("env").is_dir() {
            ids.push(name);
        }
    }
    ids.sort();
    Ok(ids)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceProbe {
    path: String,
    is_workspace: bool,
}

#[tauri::command]
fn probe_workspace(path: String) -> Result<WorkspaceProbe, String> {
    let workspace = PathBuf::from(&path);
    if !workspace.is_dir() {
        return Err(format!("不是目录: {path}"));
    }
    Ok(WorkspaceProbe {
        path: workspace.display().to_string(),
        is_workspace: paths::is_workspace_dir(&workspace),
    })
}

/// Create a Mei workspace in an empty/non-workspace folder (confirm in UI first).
#[tauri::command]
fn init_workspace(path: String) -> Result<String, String> {
    let workspace = PathBuf::from(&path);
    if !workspace.is_dir() {
        return Err(format!("不是目录: {path}"));
    }
    if paths::is_workspace_dir(&workspace) {
        return Ok(format!("已是 Mei 工作区: {}", workspace.display()));
    }
    paths::init_workspace_dir(&workspace, None).map_err(|e| e.to_string())?;
    Ok(format!("已创建 Mei 工作区: {}", workspace.display()))
}

#[tauri::command]
fn start_workspace(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let workspace = PathBuf::from(&path);
    if !workspace.is_dir() {
        return Err(format!("workspace is not a directory: {path}"));
    }
    if !paths::is_workspace_dir(&workspace) {
        return Err(format!(
            "not_a_mei_workspace:{path}"
        ));
    }
    {
        let mut host = state.host.lock().map_err(|e| e.to_string())?;
        // Viewer default: --launch so discovered apps autostart (not bare control plane).
        // GIS: Host managed_martin + sidecars/bin/martin (do not inject MEI_GIS_*).
        host.start_workspace(&workspace, None, None, true, None)
            .map_err(|e| e.to_string())?;
    }
    let mut recent = state.recent.lock().map_err(|e| e.to_string())?;
    recent.push(workspace).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn home_workspace_path() -> Result<String, String> {
    paths::ensure_home_workspace()
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn start_home_workspace(state: State<'_, AppState>) -> Result<(), String> {
    let ws = paths::ensure_home_workspace().map_err(|e| e.to_string())?;
    start_workspace(ws.display().to_string(), state)
}

/// Merge a `.mei-snapshot.zip` into the **default workspace** and start host on it.
///
/// Does **not** require an already-open host workspace: when none is running,
/// `ensure_home_workspace` creates/uses `MeiViewer/workspace`, then import + serve.
#[tauri::command]
fn import_snapshot(archive: String, state: State<'_, AppState>) -> Result<(), String> {
    let archive_path = PathBuf::from(&archive);
    if !archive_path.is_file() {
        return Err(format!("archive not found: {archive}"));
    }
    let ws = paths::ensure_home_workspace().map_err(|e| e.to_string())?;
    let slot = paths::snapshot_slot_dir().map_err(|e| e.to_string())?;
    let dest = slot.join(format!(
        "snap-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let unpacked =
        mei_snapshot::unpack_snapshot(&archive_path, &dest).map_err(|e| e.to_string())?;

    materialize_snapshot_workspace(&ws, &unpacked).map_err(|e| e.to_string())?;
    // Ensure platform stock/templates exist before serve (idempotent). Control-plane
    // also seeds stock, but doing it here keeps re-import resilient if Host is old.
    if let Err(error) = paths::init_workspace_dir(&ws, Some("Mei Viewer 默认工作区")) {
        eprintln!("post-import workspace stock seed failed: {error}");
    }

    let data_ceiling = unpacked.manifest.data_mode_hint.as_str().to_string();
    {
        let mut host = state.host.lock().map_err(|e| e.to_string())?;
        if host.is_running() {
            host.stop().map_err(|e| e.to_string())?;
        }
        for (app_id, bundle_path) in &unpacked.app_bundle_paths {
            host.import_bundle(&ws, app_id, bundle_path)
                .map_err(|e| e.to_string())?;
            // Import reads the bundle but must not leave exchange empty — reassert
            // the necessary meibundle under env/current (Host start must not prebuild).
            ensure_exchange_bundle(&ws, app_id, bundle_path).map_err(|e| e.to_string())?;
        }
        // Viewer default: always --launch so discovered apps are admitted (same as
        // open-home). Consistent path for re-import into the default workspace.
        host.start_workspace(&ws, None, Some(data_ceiling), true, None)
            .map_err(|e| e.to_string())?;
    }
    let mut recent = state.recent.lock().map_err(|e| e.to_string())?;
    recent.push(ws).map_err(|e| e.to_string())?;
    Ok(())
}

fn materialize_snapshot_workspace(
    ws: &PathBuf,
    unpacked: &mei_snapshot::UnpackResult,
) -> anyhow::Result<()> {
    // Merge into home (or target) workspace: overwrite listed apps only; never wipe siblings.
    let is_v2 = unpacked.manifest.is_v2();
    let generation = snapshot_generation_id();

    if is_v2 {
        for (app_id, bundle_path) in &unpacked.app_bundle_paths {
            let app_pack = unpacked.dest.join("apps").join(app_id);
            let app_root = ws.join("apps").join(app_id);
            if app_root.exists() {
                std::fs::remove_dir_all(&app_root)?;
            }
            let env_root = app_root.join("env");
            let gen_dir = env_root.join(&generation);
            let exchange = gen_dir.join("build").join("exchange");
            std::fs::create_dir_all(&exchange)?;
            let bundle_name = format!("{app_id}.meibundle");
            std::fs::copy(bundle_path, exchange.join(&bundle_name))?;

            let ds_src = app_pack.join("data-snapshots");
            if ds_src.is_dir() {
                let var_ds = gen_dir.join("var").join("data-snapshots");
                copy_dir(&ds_src, &var_ds)?;
                // Sealed marker next to parquet store
                std::fs::write(
                    var_ds.join(mei_snapshot::PORTABLE_SNAPSHOT_MARKER),
                    b"1\n",
                )?;
            }
            link_env_current(&env_root, &generation)?;

            // Portable runtime config
            let runtime_toml = app_pack.join("runtime").join("app.toml");
            if runtime_toml.is_file() {
                std::fs::create_dir_all(&app_root)?;
                std::fs::copy(&runtime_toml, app_root.join("app.toml"))?;
            } else if !app_root.join("app.toml").exists() {
                std::fs::create_dir_all(&app_root)?;
                std::fs::write(
                    app_root.join("app.toml"),
                    format!("id = \"{app_id}\"\nlabel = \"{app_id}\"\n"),
                )?;
            }
            std::fs::write(
                app_root.join(mei_snapshot::PORTABLE_SNAPSHOT_MARKER),
                b"1\n",
            )?;

            // Assets
            let assets_src = app_pack.join("assets");
            if assets_src.is_dir() {
                copy_dir(&assets_src, &app_root.join("assets"))?;
            }
            let proto_src = app_pack.join("prototype");
            if proto_src.is_dir() {
                copy_dir(&proto_src, &app_root.join("prototype"))?;
            }
            // Structured / optional media data → upload/
            let portable_data = app_pack.join("portable-data");
            if portable_data.is_dir() {
                copy_dir(&portable_data, &app_root)?;
            }

            // Sealed store (theme / eval slots / panel skins) — under env generation
            let store_src = app_pack.join("store-content");
            if store_src.is_dir() {
                let store_dest = gen_dir.join("build").join("store").join("content");
                copy_dir(&store_src, &store_dest)?;
            }
            // Sealed registries for view-revision assemble (map + scene)
            let registry_src = app_pack.join("registry");
            if registry_src.is_dir() {
                let registry_dest = gen_dir.join("build").join("registry");
                copy_dir(&registry_src, &registry_dest)?;
            }
        }

        // Bundled workspace stock/gis (mbtiles) — merge overwrite
        let packed_gis = unpacked.dest.join("stock").join("gis");
        if packed_gis.is_dir() {
            copy_dir(&packed_gis, &ws.join("stock").join("gis"))?;
        }
        // Stock overlay (components/templates diffs)
        let overlay = unpacked.dest.join("stock-overlay");
        if overlay.is_dir() {
            copy_dir(&overlay, &ws.join("stock"))?;
        }

        // resources.json: merge by id (overwrite same id, keep others)
        let resources_src = unpacked.dest.join("resources.json");
        if resources_src.is_file() {
            merge_resources_json(&ws.join("resources.json"), &resources_src)?;
        }
        // Workspace scene theme library (ops.sceneThemes …)
        let workspace_ops = unpacked.dest.join("workspace-ops.json");
        if workspace_ops.is_file() {
            merge_workspace_scene_ops(&ws.join("workspace.json"), &workspace_ops)?;
        }
        let readme = unpacked.dest.join("README.txt");
        if readme.is_file() {
            let _ = std::fs::copy(&readme, ws.join("SNAPSHOT-README.txt"));
        }
    } else {
        // v1 legacy layout — overwrite only this app
        let app_id = &unpacked.manifest.app_id;
        let app_root = ws.join("apps").join(app_id);
        if app_root.exists() {
            std::fs::remove_dir_all(&app_root)?;
        }
        let env_root = app_root.join("env");
        let gen_dir = env_root.join(&generation);
        let exchange = gen_dir.join("build").join("exchange");
        std::fs::create_dir_all(&exchange)?;
        let bundle_name = format!("{app_id}.meibundle");
        std::fs::copy(&unpacked.bundle_path, exchange.join(&bundle_name))?;
        if unpacked.dest.join("data-snapshots").is_dir() {
            let var_ds = gen_dir.join("var").join("data-snapshots");
            copy_dir(&unpacked.dest.join("data-snapshots"), &var_ds)?;
        }
        link_env_current(&env_root, &generation)?;
        if !app_root.join("app.toml").exists() {
            std::fs::create_dir_all(&app_root)?;
            std::fs::write(
                app_root.join("app.toml"),
                format!("id = \"{app_id}\"\nlabel = \"{app_id}\"\n"),
            )?;
        }
    }

    // Preserve existing home workspace.json; only create if missing, and set defaultApp gently.
    let ws_json = ws.join("workspace.json");
    if !ws_json.is_file() {
        let label = unpacked
            .manifest
            .workspace_label
            .clone()
            .unwrap_or_else(|| format!("Mei Viewer 默认工作区"));
        std::fs::write(
            &ws_json,
            serde_json::json!({
                "id": "mei-viewer-home",
                "label": label,
                "schemaVersion": 2,
                "workspace": {
                    "defaultApp": unpacked.manifest.app_id,
                }
            })
            .to_string(),
        )?;
    } else if let Ok(text) = std::fs::read_to_string(&ws_json) {
        if let Ok(mut doc) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(obj) = doc.get_mut("workspace").and_then(|v| v.as_object_mut()) {
                obj.insert(
                    "defaultApp".into(),
                    serde_json::Value::String(unpacked.manifest.app_id.clone()),
                );
                let _ = std::fs::write(&ws_json, serde_json::to_string_pretty(&doc)?);
            }
        }
    }
    Ok(())
}

fn merge_resources_json(dest: &std::path::Path, incoming: &std::path::Path) -> anyhow::Result<()> {
    let incoming_text = std::fs::read_to_string(incoming)?;
    let incoming_doc: mei_snapshot::ResourcesDocument = serde_json::from_str(&incoming_text)?;
    if !dest.is_file() {
        std::fs::write(dest, serde_json::to_string_pretty(&incoming_doc)?)?;
        return Ok(());
    }
    let existing_text = std::fs::read_to_string(dest)?;
    let mut existing: mei_snapshot::ResourcesDocument = serde_json::from_str(&existing_text)?;
    for entry in incoming_doc.resources {
        if let Some(slot) = existing.resources.iter_mut().find(|r| r.id == entry.id) {
            *slot = entry;
        } else {
            existing.resources.push(entry);
        }
    }
    std::fs::write(dest, serde_json::to_string_pretty(&existing)?)?;
    Ok(())
}

/// Merge packed `workspace-ops.json` (sceneThemes …) into target workspace.json.
fn merge_workspace_scene_ops(
    dest_workspace_json: &std::path::Path,
    incoming_ops_path: &std::path::Path,
) -> anyhow::Result<()> {
    let incoming: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(incoming_ops_path)?)?;
    let Some(incoming_ops) = incoming.as_object() else {
        return Ok(());
    };
    if incoming_ops.is_empty() {
        return Ok(());
    }

    let mut doc = if dest_workspace_json.is_file() {
        serde_json::from_str(&std::fs::read_to_string(dest_workspace_json)?)?
    } else {
        serde_json::json!({
            "id": "mei-viewer-home",
            "label": "Mei Viewer 默认工作区",
            "schemaVersion": 2,
            "workspace": {}
        })
    };

    let root = doc
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("workspace.json root must be an object"))?;
    let ops = root
        .entry("ops")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("workspace.json ops must be an object"))?;

    for (key, value) in incoming_ops {
        if key == "sceneThemes" {
            let dest_themes = ops
                .entry("sceneThemes")
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("ops.sceneThemes must be an object"))?;
            if let Some(themes) = value.as_object() {
                for (theme_id, theme_val) in themes {
                    dest_themes.insert(theme_id.clone(), theme_val.clone());
                }
            }
            continue;
        }
        ops.insert(key.clone(), value.clone());
    }

    std::fs::write(
        dest_workspace_json,
        serde_json::to_string_pretty(&doc)?,
    )?;
    Ok(())
}

fn snapshot_generation_id() -> String {
    // Valid WS-yyyymmdd.fixver tag (UTC calendar date).
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86400) as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(days);
    format!("WS-{y:04}{m:02}{d:02}.0")
}

/// Howard Hinnant civil_from_days (UTC), same algorithm as mei-lang-kernel.
fn civil_from_days(mut days: i64) -> (i64, i64, i64) {
    days += 719468;
    let era = if days >= 0 {
        days
    } else {
        days - 146096
    } / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m, d)
}

fn link_env_current(env_root: &std::path::Path, generation: &str) -> anyhow::Result<()> {
    let current = env_root.join("current");
    if current.exists() || current.is_symlink() {
        if current.is_dir() && !current.is_symlink() {
            std::fs::remove_dir_all(&current)?;
        } else {
            std::fs::remove_file(&current)?;
        }
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(generation, &current)?;
    }
    #[cfg(not(unix))]
    {
        // Real directory + marker (no Developer Mode / junction required).
        std::fs::create_dir_all(&current)?;
        std::fs::write(current.join(".mei-build-target"), generation)?;
    }
    Ok(())
}

/// Ensure `apps/{id}/env/current/build/exchange/{id}.meibundle` exists after import.
fn ensure_exchange_bundle(
    ws: &PathBuf,
    app_id: &str,
    bundle_src: &std::path::Path,
) -> anyhow::Result<()> {
    let exchange = ws
        .join("apps")
        .join(app_id)
        .join("env")
        .join("current")
        .join("build")
        .join("exchange");
    std::fs::create_dir_all(&exchange)?;
    let dest = exchange.join(format!("{app_id}.meibundle"));
    if !dest.is_file() {
        std::fs::copy(bundle_src, &dest)?;
    }
    Ok(())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir_simple(src)? {
        let rel = entry.strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&entry, &target)?;
        }
    }
    Ok(())
}

fn walkdir_simple(root: &std::path::Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            out.push(path.clone());
            if path.is_dir() {
                walk(&path, out)?;
            }
        }
        Ok(())
    }
    walk(root, &mut out)?;
    Ok(out)
}

#[tauri::command]
fn stop_host(state: State<'_, AppState>) -> Result<(), String> {
    let mut host = state.host.lock().map_err(|e| e.to_string())?;
    host.stop().map_err(|e| e.to_string())
}

#[tauri::command]
fn host_log_tail(state: State<'_, AppState>, max_bytes: Option<u64>) -> Result<String, String> {
    let max = max_bytes.unwrap_or(64 * 1024);
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let path = host
        .log_path()
        .map(|p| p.to_path_buf())
        .or_else(|| paths::host_log_file().ok())
        .ok_or_else(|| "no log path".to_string())?;
    drop(host);
    host::read_log_tail(&path, max).map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal_host_log(state: State<'_, AppState>) -> Result<String, String> {
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let path = host
        .log_path()
        .map(|p| p.to_path_buf())
        .or_else(|| paths::host_log_file().ok())
        .ok_or_else(|| "no log path".to_string())?;
    drop(host);
    if !path.is_file() {
        // Ensure file exists so Finder can select it.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path);
    }
    reveal_in_file_manager(&path)
}

/// Reveal a workspace / log path in the OS file manager (Finder / Explorer).
#[tauri::command]
fn reveal_path(path: String) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "—" {
        return Err("路径为空".into());
    }
    let p = PathBuf::from(trimmed);
    if !p.exists() {
        return Err(format!("路径不存在：{}", p.display()));
    }
    reveal_in_file_manager(&p)
}

fn reveal_in_file_manager(path: &std::path::Path) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if path.is_dir() {
            cmd.arg(path);
        } else {
            cmd.arg("-R").arg(path);
        }
        let status = cmd.status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("open failed: {status}"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let status = if path.is_dir() {
            std::process::Command::new("explorer")
                .arg(path)
                .status()
                .map_err(|e| e.to_string())?
        } else {
            std::process::Command::new("explorer")
                .arg("/select,")
                .arg(path)
                .status()
                .map_err(|e| e.to_string())?
        };
        if !status.success() {
            return Err(format!("explorer failed: {status}"));
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        return Err("reveal path not supported on this OS".into());
    }
    Ok(path.display().to_string())
}

#[tauri::command]
fn show_launcher(app: AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
        return Ok(());
    }
    Err("launcher window missing".into())
}

#[tauri::command]
fn list_snapshot_resources(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let workspace = host
        .workspace()
        .ok_or_else(|| "未打开工作区".to_string())?
        .to_path_buf();
    drop(host);
    let path = workspace.join("resources.json");
    if !path.is_file() {
        return Ok(serde_json::json!({
            "schemaVersion": "mei-snapshot-resources-v1",
            "resources": [],
            "workspace": workspace.display().to_string(),
        }));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut doc: mei_snapshot::ResourcesDocument =
        serde_json::from_str(&text).map_err(|e| e.to_string())?;
    for entry in &mut doc.resources {
        let target = workspace.join(&entry.target_path);
        if target.is_file() || target.is_dir() {
            if entry.state == mei_snapshot::ResourceState::External
                || entry.state == mei_snapshot::ResourceState::Missing
            {
                entry.state = mei_snapshot::ResourceState::Bundled;
                entry.hint = Some("已在工作区就位".into());
            }
        }
    }
    Ok(serde_json::json!({
        "schemaVersion": doc.schema_version,
        "resources": doc.resources,
        "workspace": workspace.display().to_string(),
    }))
}

#[tauri::command]
fn replenish_snapshot_resource(
    resource_id: String,
    source_file: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let workspace = host
        .workspace()
        .ok_or_else(|| "未打开工作区".to_string())?
        .to_path_buf();
    drop(host);
    let resources_path = workspace.join("resources.json");
    if !resources_path.is_file() {
        return Err("当前工作区没有 resources.json（非 portable snapshot）".into());
    }
    let text = std::fs::read_to_string(&resources_path).map_err(|e| e.to_string())?;
    let mut doc: mei_snapshot::ResourcesDocument =
        serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let entry = doc
        .resources
        .iter_mut()
        .find(|r| r.id == resource_id)
        .ok_or_else(|| format!("未知资源 id: {resource_id}"))?;
    let src = PathBuf::from(&source_file);
    if !src.is_file() {
        return Err(format!("源文件不存在: {source_file}"));
    }
    let mut hash_note = String::new();
    if let Some(expected) = entry.sha256.as_ref() {
        if let Ok(actual) = sha256_path(&src) {
            if &actual != expected {
                hash_note = format!("（校验与导出时不一致，仍已放入）");
            }
        }
    }
    let dest = workspace.join(&entry.target_path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    entry.state = mei_snapshot::ResourceState::Bundled;
    entry.bytes = std::fs::metadata(&dest).ok().map(|m| m.len());
    entry.sha256 = sha256_path(&dest).ok();
    entry.hint = Some(format!("已通过 Viewer 补齐{hash_note}"));
    std::fs::write(
        &resources_path,
        serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(format!(
        "已补齐 {} → {}{hash_note}",
        resource_id,
        dest.display()
    ))
}

#[tauri::command]
fn reveal_snapshot_resource_dir(
    resource_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let workspace = host
        .workspace()
        .ok_or_else(|| "未打开工作区".to_string())?
        .to_path_buf();
    drop(host);
    let resources_path = workspace.join("resources.json");
    let text = std::fs::read_to_string(&resources_path).map_err(|e| e.to_string())?;
    let doc: mei_snapshot::ResourcesDocument =
        serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let entry = doc
        .resources
        .iter()
        .find(|r| r.id == resource_id)
        .ok_or_else(|| format!("未知资源 id: {resource_id}"))?;
    let dest = workspace.join(&entry.target_path);
    let dir = dest.parent().unwrap_or(workspace.as_path()).to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(dir.display().to_string())
}

fn sha256_path(path: &std::path::Path) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
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

#[tauri::command]
fn open_host_ui(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut host = state.host.lock().map_err(|e| e.to_string())?;
    let port = host
        .port()
        .ok_or_else(|| "host is not running".to_string())?;
    if !host.is_ready() {
        // Refresh from live readiness — UI waitReady may have seen HTTP ready
        // before the Rust ready flag was set.
        match host.poll_readiness() {
            Ok(dto) if dto.host_ready && dto.control_ready => {}
            Ok(_) => return Err("host is not ready yet".into()),
            Err(e) => return Err(format!("host is not ready yet ({e})")),
        }
    }
    drop(host);
    open_host_window(&app, port)
}

fn build_app_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
    let show = MenuItemBuilder::with_id("show_launcher", "显示启动器")
        .accelerator("CmdOrCtrl+L")
        .build(app)?;
    let view = SubmenuBuilder::new(app, "查看").item(&show).build()?;
    MenuBuilder::new(app).item(&view).build()
}

fn try_auto_open_workspace(app: &AppHandle, state: &AppState) -> anyhow::Result<bool> {
    let Some(ws) = paths::launch_workspace_candidate() else {
        return Ok(false);
    };
    {
        let mut host = state.host.lock().expect("host lock");
        host.start_workspace(&ws, None, None, true, None)?;
        host.wait_for_control_ready(std::time::Duration::from_secs(240))?;
        let port = host.port().ok_or_else(|| anyhow::anyhow!("no port after start"))?;
        drop(host);
        open_host_window(app, port).map_err(|e| anyhow::anyhow!(e))?;
    }
    {
        let mut recent = state.recent.lock().expect("recent lock");
        let _ = recent.push(ws);
    }
    *state.auto_opened.lock().expect("auto_opened lock") = true;
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    Ok(true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let recent = RecentStore::load().unwrap_or_else(|_| RecentStore::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            host: Mutex::new(HostHandle::new()),
            recent: Mutex::new(recent),
            auto_opened: Mutex::new(false),
        })
        .setup(|app| {
            let ver = viewer_build_version();
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.set_title(&format!("mei-viewer {ver}"));
            }
            // Menu: when host UI is foreground after auto-open, user can still reveal logs.
            if let Ok(menu) = build_app_menu(app.handle()) {
                let _ = app.set_menu(menu);
            }
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            match try_auto_open_workspace(&handle, &*state) {
                Ok(true) => {}
                Ok(false) => {}
                Err(err) => {
                    eprintln!("Mei Viewer auto-open workspace failed: {err:#}");
                    *state.auto_opened.lock().expect("auto_opened lock") = false;
                    if let Some(main) = handle.get_webview_window("main") {
                        let _ = main.show();
                        let _ = main.set_focus();
                    }
                }
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "show_launcher" => {
                    let _ = show_launcher(app.clone());
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            host_status,
            viewer_version,
            host_readiness,
            wait_host_ready,
            list_recent,
            list_workspace_apps,
            export_snapshot,
            start_workspace,
            probe_workspace,
            init_workspace,
            import_snapshot,
            list_snapshot_resources,
            replenish_snapshot_resource,
            reveal_snapshot_resource_dir,
            stop_host,
            open_host_ui,
            host_log_tail,
            reveal_host_log,
            reveal_path,
            show_launcher,
            home_workspace_path,
            start_home_workspace
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mei Viewer");
}
