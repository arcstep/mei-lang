use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use mei_snapshot::readiness;

use crate::paths;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostReadinessDto {
    pub host_ready: bool,
    pub control_ready: bool,
    pub access_ready: bool,
    pub warmup_ready: bool,
    pub startup_phase: Option<String>,
    pub startup_detail: Option<String>,
    pub startup_error: Option<String>,
    pub progress_percent: u8,
    pub progress_label: String,
}

pub struct HostHandle {
    child: Option<Child>,
    port: Option<u16>,
    workspace: Option<PathBuf>,
    log_path: Option<PathBuf>,
    ready: bool,
}

impl Default for HostHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl HostHandle {
    pub fn new() -> Self {
        Self {
            child: None,
            port: None,
            workspace: None,
            log_path: None,
            ready: false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn port(&self) -> Option<u16> {
        self.port
    }

    pub fn workspace(&self) -> Option<&Path> {
        self.workspace.as_deref()
    }

    pub fn log_path(&self) -> Option<&Path> {
        self.log_path.as_deref()
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.port = None;
        self.workspace = None;
        self.ready = false;
        // Keep last log_path so UI can still read it after stop.
        Ok(())
    }

    /// Start host-shell for a workspace.
    ///
    /// `launch_all`: pass `--launch` so discovered apps autostart (Viewer default).
    /// `gis_env`: optional `(MEI_GIS_PROXY_UPSTREAM, MEI_TILES_JSON_PATH)` when Martin is ready.
    pub fn start_workspace(
        &mut self,
        workspace: &Path,
        app: Option<String>,
        data_mode_ceiling: Option<String>,
        launch_all: bool,
        gis_env: Option<(String, String)>,
    ) -> anyhow::Result<()> {
        self.stop()?;
        let bin = paths::resolve_host_shell_bin()?;
        let port = pick_free_port()?;
        let log_path = paths::host_log_file()?;
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Truncate previous session log for this Viewer instance.
        let log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)?;
        let log_err = log_file.try_clone()?;

        let mut cmd = Command::new(&bin);
        cmd.arg("serve")
            .arg("--workspace")
            .arg(workspace)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string());
        if launch_all && app.is_none() {
            cmd.arg("--launch");
        }
        if let Some(app_id) = app.as_ref() {
            cmd.arg("--app").arg(app_id);
        }
        if let Some(ceiling) = data_mode_ceiling.as_ref() {
            cmd.arg("--data-mode-ceiling").arg(ceiling);
        }
        apply_sidecar_env(&mut cmd)?;
        if workspace_has_portable_snapshot(workspace) {
            cmd.env("MEI_SNAPSHOT_SEALED_DATA", "1");
        }
        if let Some((upstream, tiles_json)) = gis_env {
            cmd.env("MEI_GIS_PROXY_UPSTREAM", &upstream);
            cmd.env("MEI_TILES_JSON_PATH", &tiles_json);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_err));

        {
            use std::io::Write;
            let mut header = OpenOptions::new().append(true).open(&log_path)?;
            let pkg = paths::sidecar_package_root()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(unset)".into());
            writeln!(
                header,
                "==== mei-viewer spawn {} port={} workspace={} bin={} MEI_PACKAGE_ROOT={} ====",
                chrono_like_now(),
                port,
                workspace.display(),
                bin.display(),
                pkg
            )?;
        }

        let child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("spawn {}: {e}", bin.display()))?;
        self.child = Some(child);
        self.port = Some(port);
        self.workspace = Some(workspace.to_path_buf());
        self.log_path = Some(log_path);
        self.ready = false;
        Ok(())
    }

    /// Poll readiness until control plane is up or timeout (for auto-open / UI progress).
    pub fn wait_for_control_ready(&mut self, timeout: Duration) -> anyhow::Result<()> {
        let port = self.port.ok_or_else(|| anyhow::anyhow!("no port"))?;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(child) = self.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    let tail = self
                        .log_path
                        .as_ref()
                        .and_then(|p| read_log_tail(p, 8 * 1024).ok())
                        .unwrap_or_default();
                    anyhow::bail!("mei-host-shell exited early: {status}\n--- host log tail ---\n{tail}");
                }
            }
            if let Ok(dto) = fetch_readiness(port) {
                if dto.control_ready && dto.host_ready {
                    self.ready = true;
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        let tail = self
            .log_path
            .as_ref()
            .and_then(|p| read_log_tail(p, 8 * 1024).ok())
            .unwrap_or_default();
        anyhow::bail!("timeout waiting for host control readiness\n--- host log tail ---\n{tail}")
    }

    pub fn poll_readiness(&mut self) -> anyhow::Result<HostReadinessDto> {
        let port = self.port.ok_or_else(|| anyhow::anyhow!("host is not running"))?;
        let dto = fetch_readiness(port)?;
        if dto.host_ready && dto.control_ready {
            self.ready = true;
        }
        Ok(dto)
    }

    pub fn import_bundle(
        &self,
        workspace: &Path,
        app: &str,
        bundle: &Path,
    ) -> anyhow::Result<()> {
        let bin = paths::resolve_host_shell_bin()?;
        let mut cmd = Command::new(&bin);
        cmd.arg("import")
            .arg("--workspace")
            .arg(workspace)
            .arg("--app")
            .arg(app)
            .arg("--bundle")
            .arg(bundle);
        apply_sidecar_env(&mut cmd)?;
        let output = cmd.output().map_err(|e| {
            anyhow::anyhow!("spawn {}: {e}", bin.display())
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = [stderr.trim(), stdout.trim()]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or("(no output)");
            anyhow::bail!(
                "import failed with status {}: {}",
                output.status,
                detail
            );
        }
        Ok(())
    }

    /// Author-workspace prepare: compile + import + data snapshots + warmup + finalize.
    /// Used before portable snapshot export so the pack validates sealed products, not stale files.
    pub fn prebuild_app(&self, workspace: &Path, app: &str) -> anyhow::Result<()> {
        let bin = paths::resolve_host_shell_bin()?;
        let mut cmd = Command::new(&bin);
        cmd.arg("prebuild")
            .arg("--workspace")
            .arg(workspace)
            .arg("--app")
            .arg(app)
            .arg("--policy")
            .arg("home");
        apply_sidecar_env(&mut cmd)?;
        let output = cmd
            .output()
            .map_err(|e| anyhow::anyhow!("spawn {}: {e}", bin.display()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = [stderr.trim(), stdout.trim()]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or("(no output)");
            anyhow::bail!(
                "prebuild failed for `{app}` (status {}): {}",
                output.status,
                detail
            );
        }
        Ok(())
    }
}

/// True when this app was materialized from a portable snapshot (must not re-prebuild).
pub fn app_is_sealed_portable(workspace: &Path, app_id: &str) -> bool {
    let app_root = workspace.join("apps").join(app_id);
    app_root
        .join(mei_snapshot::PORTABLE_SNAPSHOT_MARKER)
        .is_file()
        || app_root
            .join("env")
            .join("current")
            .join("var")
            .join("data-snapshots")
            .join(mei_snapshot::PORTABLE_SNAPSHOT_MARKER)
            .is_file()
}

fn workspace_has_portable_snapshot(workspace: &Path) -> bool {
    let apps = workspace.join("apps");
    let Ok(entries) = std::fs::read_dir(&apps) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .join(mei_snapshot::PORTABLE_SNAPSHOT_MARKER)
            .is_file()
        {
            return true;
        }
    }
    workspace.join("resources.json").is_file()
}

fn apply_sidecar_env(cmd: &mut Command) -> anyhow::Result<()> {
    if let Ok(bin_dir) = paths::sidecar_bin_dir() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths_os = std::env::split_paths(&path).collect::<Vec<_>>();
        paths_os.insert(0, bin_dir.clone());
        cmd.env("PATH", std::env::join_paths(paths_os)?);
        cmd.env("MEI_DESKTOP_BIN", &bin_dir);
        if let Some(pkg) = paths::sidecar_package_root() {
            cmd.env("MEI_PACKAGE_ROOT", &pkg);
        }
        let compiler = bin_dir.join(if cfg!(windows) {
            "mei-compiler.exe"
        } else {
            "mei-compiler"
        });
        if compiler.is_file() {
            cmd.env("MEI_COMPILER_BIN", &compiler);
        }
        let runtime = bin_dir.join(if cfg!(windows) {
            "mei-app-runtime.exe"
        } else {
            "mei-app-runtime"
        });
        if runtime.is_file() {
            cmd.env("MEI_APP_RUNTIME_BIN", &runtime);
        }
        let plug = bin_dir.join(if cfg!(windows) {
            "mei-plug-ds.exe"
        } else {
            "mei-plug-ds"
        });
        if plug.is_file() {
            cmd.env("MEI_PLUG_DS_BIN", &plug);
        }
    }
    if std::env::var_os("RUST_LOG").is_none() {
        cmd.env("RUST_LOG", "info");
    }
    Ok(())
}

fn fetch_readiness(port: u16) -> anyhow::Result<HostReadinessDto> {
    let url = format!("http://127.0.0.1:{port}{}", readiness::PATH);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;
    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("readiness HTTP {}", resp.status());
    }
    let v: serde_json::Value = resp.json()?;
    let host_ready = v.get("hostReady").and_then(|x| x.as_bool()) == Some(true);
    let control_ready = v.get("controlReady").and_then(|x| x.as_bool()) == Some(true);
    let access_ready = v.get("accessReady").and_then(|x| x.as_bool()) == Some(true);
    let warmup_ready = v.get("warmupReady").and_then(|x| x.as_bool()) == Some(true);
    let startup_phase = v
        .get("startupPhase")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let startup_detail = v
        .get("startupDetail")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let startup_error = v
        .get("startupError")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let (progress_percent, progress_label) =
        readiness_progress(host_ready, control_ready, access_ready, warmup_ready, &startup_phase, &startup_detail);
    Ok(HostReadinessDto {
        host_ready,
        control_ready,
        access_ready,
        warmup_ready,
        startup_phase,
        startup_detail,
        startup_error,
        progress_percent,
        progress_label,
    })
}

fn readiness_progress(
    host_ready: bool,
    control_ready: bool,
    access_ready: bool,
    warmup_ready: bool,
    startup_phase: &Option<String>,
    startup_detail: &Option<String>,
) -> (u8, String) {
    let detail = startup_detail
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("正在启动…");
    if !host_ready {
        return (8, "正在拉起 mei-host-shell…".into());
    }
    if !control_ready {
        return (22, "初始化控制面…".into());
    }
    let phase = startup_phase.as_deref().unwrap_or("");
    if phase == "unconfigured" {
        return (38, format!("{detail}（可在宿主 /runtime 应用配置档）"));
    }
    if !access_ready {
        return (58, format!("{detail}"));
    }
    if !warmup_ready {
        return (78, "预热视图与数据面…".into());
    }
    (92, "控制面已就绪".into())
}

impl Drop for HostHandle {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn pick_free_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

pub fn read_log_tail(path: &Path, max_bytes: u64) -> anyhow::Result<String> {
    if !path.is_file() {
        return Ok(String::new());
    }
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len > max_bytes {
        file.seek(SeekFrom::Start(len - max_bytes))?;
    }
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    // If we started mid-line, drop the partial first line.
    if len > max_bytes {
        if let Some(idx) = buf.find('\n') {
            buf = buf[idx + 1..].to_string();
        }
    }
    Ok(buf)
}
