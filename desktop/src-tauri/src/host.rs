use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use mei_snapshot::readiness;

use crate::paths;

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
    pub fn start_workspace(
        &mut self,
        workspace: &Path,
        app: Option<String>,
        data_mode_ceiling: Option<String>,
        launch_all: bool,
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
        // Ensure sidecar bins (incl. mei-compiler / app-runtime / plug-ds) are on PATH.
        if let Ok(bin_dir) = paths::sidecar_bin_dir() {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut paths_os = std::env::split_paths(&path).collect::<Vec<_>>();
            paths_os.insert(0, bin_dir.clone());
            cmd.env("PATH", std::env::join_paths(paths_os)?);
            cmd.env("MEI_DESKTOP_BIN", &bin_dir);
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
        cmd.stdin(Stdio::null())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_err));

        {
            use std::io::Write;
            let mut header = OpenOptions::new().append(true).open(&log_path)?;
            writeln!(
                header,
                "==== mei-viewer spawn {} port={} workspace={} bin={} ====",
                chrono_like_now(),
                port,
                workspace.display(),
                bin.display()
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
        match self.wait_ready(Duration::from_secs(90)) {
            Ok(()) => Ok(()),
            Err(err) => {
                let tail = self
                    .log_path
                    .as_ref()
                    .and_then(|p| read_log_tail(p, 8 * 1024).ok())
                    .unwrap_or_default();
                anyhow::bail!("{err}\n--- host log tail ---\n{tail}")
            }
        }
    }

    pub fn import_bundle(
        &self,
        workspace: &Path,
        app: &str,
        bundle: &Path,
    ) -> anyhow::Result<()> {
        let bin = paths::resolve_host_shell_bin()?;
        let status = Command::new(&bin)
            .arg("import")
            .arg("--workspace")
            .arg(workspace)
            .arg("--app")
            .arg(app)
            .arg("--bundle")
            .arg(bundle)
            .status()?;
        if !status.success() {
            anyhow::bail!("import failed with status {status}");
        }
        Ok(())
    }

    fn wait_ready(&mut self, timeout: Duration) -> anyhow::Result<()> {
        let port = self.port.ok_or_else(|| anyhow::anyhow!("no port"))?;
        let url = format!("http://127.0.0.1:{port}{}", readiness::PATH);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(child) = self.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    anyhow::bail!("mei-host-shell exited early: {status}");
                }
            }
            if let Ok(resp) = client.get(&url).send() {
                if resp.status().is_success() {
                    if let Ok(v) = resp.json::<serde_json::Value>() {
                        let ok = readiness::REQUIRED_TRUE_FIELDS
                            .iter()
                            .all(|k| v.get(*k).and_then(|x| x.as_bool()) == Some(true));
                        if ok {
                            self.ready = true;
                            return Ok(());
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        anyhow::bail!("timeout waiting for {url}");
    }
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
