use std::env;
use std::path::{Path, PathBuf};

pub fn app_support_dir() -> anyhow::Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("no data_dir"))?;
    let dir = base.join("MeiViewer");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn recent_file() -> anyhow::Result<PathBuf> {
    Ok(app_support_dir()?.join("recent.json"))
}

pub fn logs_dir() -> anyhow::Result<PathBuf> {
    let dir = app_support_dir()?.join("logs");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn host_log_file() -> anyhow::Result<PathBuf> {
    Ok(logs_dir()?.join("host-shell.log"))
}

/// Default home workspace for non-technical users (`MeiViewer/workspace`).
pub fn home_workspace_dir() -> anyhow::Result<PathBuf> {
    let dir = app_support_dir()?.join("workspace");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Create a minimal home workspace skeleton if missing.
pub fn ensure_home_workspace() -> anyhow::Result<PathBuf> {
    let ws = home_workspace_dir()?;
    let marker = ws.join("workspace.json");
    if !marker.is_file() {
        let doc = serde_json::json!({
            "id": "mei-viewer-home",
            "label": "Mei Viewer 家工作区",
            "schemaVersion": 2,
            "workspace": {
                "defaultApp": ""
            }
        });
        std::fs::write(&marker, serde_json::to_string_pretty(&doc)?)?;
    }
    std::fs::create_dir_all(ws.join("apps"))?;
    std::fs::create_dir_all(ws.join("stock").join("gis").join("tiles"))?;
    Ok(ws)
}

pub fn snapshot_slot_dir() -> anyhow::Result<PathBuf> {
    let dir = app_support_dir()?.join("snapshots");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// V2 workspace marker: `workspace.json` (legacy `.mei-workspace.json` also accepted).
pub fn is_workspace_dir(path: &Path) -> bool {
    path.is_dir()
        && (path.join("workspace.json").is_file() || path.join(".mei-workspace.json").is_file())
}

/// Resolve a workspace to open at launch:
/// 1) first CLI arg if it is a workspace directory
/// 2) else `--workspace <path>`
/// 3) else current working directory if it is a workspace
pub fn launch_workspace_candidate() -> Option<PathBuf> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--workspace" {
            if let Some(ws) = args.get(i + 1) {
                let p = PathBuf::from(ws);
                if is_workspace_dir(&p) {
                    return std::fs::canonicalize(&p).ok().or(Some(p));
                }
            }
            i += 2;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        let p = PathBuf::from(arg);
        if is_workspace_dir(&p) {
            return std::fs::canonicalize(&p).ok().or(Some(p));
        }
        i += 1;
    }
    if let Ok(cwd) = env::current_dir() {
        if is_workspace_dir(&cwd) {
            return std::fs::canonicalize(&cwd).ok().or(Some(cwd));
        }
    }
    None
}

pub fn sidecar_package_root() -> Option<PathBuf> {
    let bin_dir = sidecar_bin_dir().ok()?;
    let sidecars = bin_dir.parent()?;
    let app_assets = sidecars.join("app").join("assets");
    if app_assets.is_dir() {
        return std::fs::canonicalize(sidecars).ok().or(Some(sidecars.to_path_buf()));
    }
    None
}

pub fn sidecar_bin_dir() -> anyhow::Result<PathBuf> {
    if let Ok(v) = env::var("MEI_DESKTOP_BIN") {
        let p = PathBuf::from(v);
        if p.is_dir() {
            return Ok(p);
        }
    }
    // Dev: mei-lang/desktop/sidecars/bin relative to CARGO_MANIFEST_DIR
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sidecars/bin");
    if let Ok(canon) = std::fs::canonicalize(&dev) {
        if canon.is_dir() {
            return Ok(canon);
        }
    }
    // Bundled resource next to executable
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidates = [
                dir.join("sidecars").join("bin"),
                dir.join("bin"),
                dir.join("../Resources/sidecars/bin"),
                dir.join("../Resources"),
            ];
            for c in candidates {
                if let Ok(canon) = std::fs::canonicalize(&c) {
                    if canon.is_dir() {
                        return Ok(canon);
                    }
                }
            }
        }
    }
    anyhow::bail!(
        "sidecar bin dir not found; run scripts/desktop/collect-desktop-sidecars.sh or set MEI_DESKTOP_BIN"
    )
}

pub fn resolve_host_shell_bin() -> anyhow::Result<PathBuf> {
    let name = if cfg!(windows) {
        "mei-host-shell.exe"
    } else {
        "mei-host-shell"
    };
    let bin_dir = sidecar_bin_dir()?;
    let candidate = bin_dir.join(name);
    if candidate.is_file() {
        return Ok(candidate);
    }
    // Fall back to PATH / mei-lang target
    if let Ok(path) = which_in_path(name) {
        return Ok(path);
    }
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(name);
    if target.is_file() {
        return Ok(target);
    }
    let target_rel = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/release")
        .join(name);
    if target_rel.is_file() {
        return Ok(target_rel);
    }
    anyhow::bail!("mei-host-shell not found (checked sidecars, PATH, target/)")
}

fn which_in_path(name: &str) -> anyhow::Result<PathBuf> {
    let path = env::var_os("PATH").ok_or_else(|| anyhow::anyhow!("no PATH"))?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("{name} not on PATH")
}
