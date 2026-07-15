mod host;
mod recent;
mod paths;

use host::HostHandle;
use recent::RecentStore;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

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
    pub auto_opened: bool,
    pub log_path: Option<String>,
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
    }
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
    let out = PathBuf::from(&out_path);
    if out.as_os_str().is_empty() {
        return Err("输出路径不能为空".into());
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
        let mut host = state.host.lock().map_err(|e| e.to_string())?;
        // Viewer default: --launch so discovered apps autostart (not bare control plane).
        host.start_workspace(&workspace, None, None, true)
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
        let mut host = state.host.lock().map_err(|e| e.to_string())?;
        // Prefer explicit import before serve so registry exists for --app autostart.
        host.import_bundle(&ws, &unpacked.manifest.app_id, &unpacked.bundle_path)
            .map_err(|e| e.to_string())?;
        host.start_workspace(
            &ws,
            Some(unpacked.manifest.app_id.clone()),
            Some(data_ceiling),
            false, // --app already selects the snapshot app
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
    let exchange = app_root
        .join("env")
        .join("current")
        .join("build")
        .join("exchange");
    std::fs::create_dir_all(&exchange)?;
    // On Windows, `env/current` as a real directory avoids symlink requirements for Viewer slots.
    let bundle_name = format!("{app_id}.meibundle");
    std::fs::copy(&unpacked.bundle_path, exchange.join(&bundle_name))?;
    if unpacked.dest.join("data-snapshots").is_dir() {
        let var_ds = app_root
            .join("env")
            .join("current")
            .join("var")
            .join("data-snapshots");
        copy_dir(&unpacked.dest.join("data-snapshots"), &var_ds)?;
    }
    if !ws.join("workspace.json").exists() {
        std::fs::write(
            ws.join("workspace.json"),
            serde_json::json!({
                "id": format!("snapshot-{app_id}"),
                "label": format!("Snapshot {app_id}"),
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

fn open_host_window(app: &AppHandle, port: u16) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/");
    if let Some(w) = app.get_webview_window("host") {
        let _ = w.eval(&format!("window.location.replace({url:?})"));
        let _ = w.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(app, "host", WebviewUrl::External(url.parse().unwrap()))
        .title("mei-host")
        .inner_size(1280.0, 860.0)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_host_ui(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let host = state.host.lock().map_err(|e| e.to_string())?;
    let port = host
        .port()
        .ok_or_else(|| "host is not running".to_string())?;
    if !host.is_ready() {
        return Err("host is not ready yet".into());
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
    MenuBuilder::new(app).item(&view).build()
}

fn try_auto_open_workspace(app: &AppHandle, state: &AppState) -> anyhow::Result<bool> {
    let Some(ws) = paths::launch_workspace_candidate() else {
        return Ok(false);
    };
    {
        let mut host = state.host.lock().expect("host lock");
        host.start_workspace(&ws, None, None, true)?;
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
            if event.id() == "show_launcher" {
                let _ = show_launcher(app.clone());
            }
        })
        .invoke_handler(tauri::generate_handler![
            host_status,
            list_recent,
            list_workspace_apps,
            export_snapshot,
            start_workspace,
            import_snapshot,
            stop_host,
            open_host_ui,
            host_log_tail,
            reveal_host_log,
            show_launcher
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mei Viewer");
}
