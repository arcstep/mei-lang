mod host;
mod martin;
mod paths;
mod recent;

use host::{HostHandle, HostReadinessDto};
use martin::{MartinHandle, MartinStatusDto};
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
    pub martin: Mutex<MartinHandle>,
    pub recent: Mutex<RecentStore>,
    /// True when launch cwd/argv was a workspace and we skipped the launcher UI.
    pub auto_opened: Mutex<bool>,
}

fn martin_gis_env(state: &AppState) -> Option<(String, String)> {
    let mut martin = state.martin.lock().ok()?;
    if !martin.is_ready() {
        return None;
    }
    let tiles = martin.tiles_json_path_for_host()?;
    Some((martin.gis_upstream_for_host(), tiles))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDto {
    pub running: bool,
    pub ready: bool,
    pub port: Option<u16>,
    pub workspace: Option<String>,
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
    app_id: String,
    out_path: String,
    include_data: bool,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let workspace = host
        .workspace()
        .ok_or_else(|| "未打开工作区，无法导出快照".to_string())?
        .to_path_buf();
    drop(host);

    let app_id = app_id.trim().to_string();
    if app_id.is_empty() {
        return Err("appId 不能为空".into());
    }
    let mut out = PathBuf::from(&out_path);
    if out.as_os_str().is_empty() {
        return Err("输出路径不能为空".into());
    }
    // Ensure recognizable snapshot extension (save dialog may omit it).
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

    // Preflight: clearer error when bundle missing.
    match mei_snapshot::resolve_app_env_root(&workspace, &app_id)
        .and_then(|env| mei_snapshot::resolve_bundle_path(&env, &app_id))
    {
        Ok(_) => {}
        Err(err) => {
            return Err(format!(
                "无法导出 {app_id}：{err}（请先 compile，或在宿主 /runtime 执行 reload）"
            ));
        }
    }

    let manifest = mei_snapshot::pack_snapshot(&mei_snapshot::PackOptions {
        workspace,
        app_id: app_id.clone(),
        out: out.clone(),
        include_data,
        include_cache: false,
        default_scene: None,
        compiler_version: None,
    })
    .map_err(|e| e.to_string())?;

    Ok(format!(
        "已导出 {} → {}（files={}, dataHint={}）",
        app_id,
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

#[tauri::command]
fn start_workspace(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let workspace = PathBuf::from(&path);
    if !workspace.is_dir() {
        return Err(format!("workspace is not a directory: {path}"));
    }
    if !paths::is_workspace_dir(&workspace) {
        return Err(format!(
            "not a Mei workspace (missing workspace.json): {path}"
        ));
    }
    {
        let gis = martin_gis_env(&state);
        let mut host = state.host.lock().map_err(|e| e.to_string())?;
        // Viewer default: --launch so discovered apps autostart (not bare control plane).
        host.start_workspace(&workspace, None, None, true, gis)
            .map_err(|e| e.to_string())?;
    }
    let mut recent = state.recent.lock().map_err(|e| e.to_string())?;
    recent.push(workspace).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn import_snapshot(archive: String, state: State<'_, AppState>) -> Result<(), String> {
    let archive_path = PathBuf::from(&archive);
    if !archive_path.is_file() {
        return Err(format!("archive not found: {archive}"));
    }
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

    // Materialize a minimal workspace pointing at the unpacked bundle via a staging ws.
    let ws = paths::snapshot_workspace_dir(&unpacked.manifest.app_id)
        .map_err(|e| e.to_string())?;
    materialize_snapshot_workspace(&ws, &unpacked).map_err(|e| e.to_string())?;

    let data_ceiling = unpacked.manifest.data_mode_hint.as_str().to_string();
    {
        let gis = martin_gis_env(&state);
        let mut host = state.host.lock().map_err(|e| e.to_string())?;
        // Prefer explicit import before serve so registry exists for --app autostart.
        host.import_bundle(&ws, &unpacked.manifest.app_id, &unpacked.bundle_path)
            .map_err(|e| e.to_string())?;
        host.start_workspace(
            &ws,
            Some(unpacked.manifest.app_id.clone()),
            Some(data_ceiling),
            false, // --app already selects the snapshot app
            gis,
        )
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
    let app_id = &unpacked.manifest.app_id;
    let app_root = ws.join("apps").join(app_id);
    let env_root = app_root.join("env");
    // Host expects env/current → WS-yyyymmdd.n (symlink on Unix; marker file on Windows).
    // Wipe prior env so re-import does not leave a real `current/` directory (Unix rejects that).
    if env_root.exists() {
        std::fs::remove_dir_all(&env_root)?;
    }
    let generation = snapshot_generation_id();
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

    if !ws.join("workspace.json").exists() {
        std::fs::write(
            ws.join("workspace.json"),
            serde_json::json!({
                "id": format!("snapshot-{app_id}"),
                "label": format!("Snapshot {app_id}"),
                "schemaVersion": 2,
            })
            .to_string(),
        )?;
    }
    if !app_root.join("app.toml").exists() {
        std::fs::write(
            app_root.join("app.toml"),
            format!("id = \"{app_id}\"\nlabel = \"{app_id}\"\n"),
        )?;
    }
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
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("open -R failed: {status}"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("explorer select failed: {status}"));
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = path;
        return Err("reveal log not supported on this OS".into());
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
fn martin_status(state: State<'_, AppState>) -> Result<MartinStatusDto, String> {
    let mut martin = state.martin.lock().map_err(|e| e.to_string())?;
    Ok(martin.status())
}

#[tauri::command]
fn martin_ensure_installed(state: State<'_, AppState>) -> Result<MartinStatusDto, String> {
    let mut martin = state.martin.lock().map_err(|e| e.to_string())?;
    martin.ensure_installed().map_err(|e| e.to_string())?;
    Ok(martin.status())
}

#[tauri::command]
fn martin_start(state: State<'_, AppState>) -> Result<MartinStatusDto, String> {
    let mut martin = state.martin.lock().map_err(|e| e.to_string())?;
    martin.start().map_err(|e| e.to_string())?;
    Ok(martin.status())
}

#[tauri::command]
fn martin_stop(state: State<'_, AppState>) -> Result<MartinStatusDto, String> {
    let mut martin = state.martin.lock().map_err(|e| e.to_string())?;
    martin.stop().map_err(|e| e.to_string())?;
    Ok(martin.status())
}

#[tauri::command]
fn martin_pick_mbtiles(path: String, state: State<'_, AppState>) -> Result<MartinStatusDto, String> {
    let mut martin = state.martin.lock().map_err(|e| e.to_string())?;
    martin
        .set_mbtiles_path(PathBuf::from(path))
        .map_err(|e| e.to_string())?;
    Ok(martin.status())
}

#[tauri::command]
fn martin_reveal(state: State<'_, AppState>) -> Result<String, String> {
    let dir = paths::martin_root().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg(&dir)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("open failed: {status}"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("explorer")
            .arg(&dir)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("explorer failed: {status}"));
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let status = std::process::Command::new("xdg-open")
            .arg(&dir)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("xdg-open failed: {status}"));
        }
    }
    let _ = state;
    Ok(dir.display().to_string())
}

#[tauri::command]
fn martin_open_catalog(state: State<'_, AppState>) -> Result<(), String> {
    let mut martin = state.martin.lock().map_err(|e| e.to_string())?;
    let status = martin.status();
    if !status.running {
        return Err("Martin 未在运行；请先启动瓦片服务".into());
    }
    open_system_browser(&status.catalog_url)
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
    let show = MenuItemBuilder::with_id("show_launcher", "显示启动器与运行日志")
        .accelerator("CmdOrCtrl+L")
        .build(app)?;
    let view = SubmenuBuilder::new(app, "查看").item(&show).build()?;

    let download = MenuItemBuilder::with_id("martin_download", "下载或更新 Martin").build(app)?;
    let start = MenuItemBuilder::with_id("martin_start", "启动瓦片服务").build(app)?;
    let stop = MenuItemBuilder::with_id("martin_stop", "停止瓦片服务").build(app)?;
    let reveal = MenuItemBuilder::with_id("martin_reveal", "在访达中显示 Martin 目录").build(app)?;
    let catalog = MenuItemBuilder::with_id("martin_catalog", "打开 catalog").build(app)?;
    let tiles = SubmenuBuilder::new(app, "地图瓦片")
        .item(&download)
        .item(&start)
        .item(&stop)
        .separator()
        .item(&reveal)
        .item(&catalog)
        .build()?;

    MenuBuilder::new(app).item(&view).item(&tiles).build()
}

fn try_auto_open_workspace(app: &AppHandle, state: &AppState) -> anyhow::Result<bool> {
    let Some(ws) = paths::launch_workspace_candidate() else {
        return Ok(false);
    };
    {
        let gis = martin_gis_env(state);
        let mut host = state.host.lock().expect("host lock");
        host.start_workspace(&ws, None, None, true, gis)?;
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
            martin: Mutex::new(MartinHandle::new()),
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
                "martin_download" => {
                    let state = app.state::<AppState>();
                    let mut martin = state.martin.lock().expect("martin lock");
                    if let Err(e) = martin.ensure_installed() {
                        eprintln!("Martin download failed: {e:#}");
                    } else {
                        let _ = show_launcher(app.clone());
                    }
                }
                "martin_start" => {
                    let state = app.state::<AppState>();
                    let mut martin = state.martin.lock().expect("martin lock");
                    if let Err(e) = martin.start() {
                        eprintln!("Martin start failed: {e:#}");
                        let _ = show_launcher(app.clone());
                    }
                }
                "martin_stop" => {
                    let state = app.state::<AppState>();
                    let mut martin = state.martin.lock().expect("martin lock");
                    let _ = martin.stop();
                }
                "martin_reveal" => {
                    let state = app.state::<AppState>();
                    match paths::martin_root() {
                        Ok(dir) => {
                            #[cfg(target_os = "macos")]
                            let _ = std::process::Command::new("open").arg(&dir).status();
                            #[cfg(target_os = "windows")]
                            let _ = std::process::Command::new("explorer").arg(&dir).status();
                            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                            let _ = std::process::Command::new("xdg-open").arg(&dir).status();
                            let _ = state;
                        }
                        Err(e) => eprintln!("Martin reveal failed: {e:#}"),
                    }
                }
                "martin_catalog" => {
                    let state = app.state::<AppState>();
                    let mut martin = state.martin.lock().expect("martin lock");
                    let status = martin.status();
                    if status.running {
                        let _ = open_system_browser(&status.catalog_url);
                    } else {
                        eprintln!("Martin is not running");
                        let _ = show_launcher(app.clone());
                    }
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
            import_snapshot,
            stop_host,
            open_host_ui,
            host_log_tail,
            reveal_host_log,
            show_launcher,
            martin_status,
            martin_ensure_installed,
            martin_start,
            martin_stop,
            martin_pick_mbtiles,
            martin_reveal,
            martin_open_catalog
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mei Viewer");
}
