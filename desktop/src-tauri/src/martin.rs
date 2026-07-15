//! Download / supervise MapLibre Martin (MBTiles tile server) without Docker.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::paths;

pub const MARTIN_VERSION: &str = "1.10.1";
pub const MARTIN_PORT: u16 = 8080;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MartinState {
    pub version: String,
    pub mbtiles_path: Option<String>,
    pub port: u16,
}

impl Default for MartinState {
    fn default() -> Self {
        Self {
            version: MARTIN_VERSION.to_string(),
            mbtiles_path: None,
            port: MARTIN_PORT,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MartinStatusDto {
    pub installed: bool,
    pub version: Option<String>,
    pub running: bool,
    pub ready: bool,
    pub port: u16,
    pub mbtiles_path: Option<String>,
    pub source_id: Option<String>,
    pub catalog_url: String,
    pub bin_path: Option<String>,
    pub log_path: Option<String>,
}

pub struct MartinHandle {
    child: Option<Child>,
    ready: bool,
    state: MartinState,
}

impl Default for MartinHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl MartinHandle {
    pub fn new() -> Self {
        let state = load_state().unwrap_or_default();
        Self {
            child: None,
            ready: false,
            state,
        }
    }

    pub fn status(&mut self) -> MartinStatusDto {
        self.reap_if_exited();
        let bin = paths::martin_bin().ok();
        let installed = bin.as_ref().map(|p| p.is_file()).unwrap_or(false);
        let mbtiles = self
            .state
            .mbtiles_path
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| p.is_file());
        let source_id = mbtiles
            .as_ref()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()));
        let running = self.child.is_some();
        if running && !self.ready {
            self.ready = catalog_ready(self.state.port);
        }
        MartinStatusDto {
            installed,
            version: if installed {
                Some(self.state.version.clone())
            } else {
                None
            },
            running,
            ready: running && self.ready,
            port: self.state.port,
            mbtiles_path: mbtiles.map(|p| p.display().to_string()),
            source_id,
            catalog_url: format!("http://127.0.0.1:{}/catalog", self.state.port),
            bin_path: bin.map(|p| p.display().to_string()),
            log_path: paths::martin_log_file()
                .ok()
                .map(|p| p.display().to_string()),
        }
    }

    pub fn is_ready(&mut self) -> bool {
        self.reap_if_exited();
        if self.child.is_none() {
            self.ready = false;
            return false;
        }
        if !self.ready {
            self.ready = catalog_ready(self.state.port);
        }
        self.ready
    }

    pub fn tiles_json_path_for_host(&self) -> Option<String> {
        let path = self.state.mbtiles_path.as_ref()?;
        let p = Path::new(path);
        if !p.is_file() {
            return None;
        }
        let stem = p.file_stem()?.to_string_lossy();
        Some(format!("/{stem}"))
    }

    pub fn gis_upstream_for_host(&self) -> String {
        format!("http://127.0.0.1:{}", self.state.port)
    }

    pub fn set_mbtiles_path(&mut self, path: PathBuf) -> anyhow::Result<()> {
        if !path.is_file() {
            anyhow::bail!("MBTiles 文件不存在: {}", path.display());
        }
        let canon = std::fs::canonicalize(&path).unwrap_or(path);
        self.state.mbtiles_path = Some(canon.display().to_string());
        save_state(&self.state)?;
        Ok(())
    }

    pub fn resolve_mbtiles(&mut self) -> anyhow::Result<PathBuf> {
        if let Some(ref s) = self.state.mbtiles_path {
            let p = PathBuf::from(s);
            if p.is_file() {
                return Ok(p);
            }
        }
        if let Some(dev) = paths::default_shapingba_mbtiles() {
            self.set_mbtiles_path(dev.clone())?;
            return Ok(dev);
        }
        anyhow::bail!("请先选择 .mbtiles 文件（地图瓦片 → 选择 MBTiles）")
    }

    pub fn ensure_installed(&mut self) -> anyhow::Result<()> {
        let bin = paths::martin_bin()?;
        if bin.is_file() && self.state.version == MARTIN_VERSION {
            return Ok(());
        }
        download_and_install(MARTIN_VERSION)?;
        self.state.version = MARTIN_VERSION.to_string();
        save_state(&self.state)?;
        Ok(())
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        self.ensure_installed()?;
        let mbtiles = self.resolve_mbtiles()?;
        self.stop()?;

        let bin = paths::martin_bin()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&bin)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&bin, perms)?;
            adhoc_codesign_macos(&bin);
        }

        let log_path = paths::martin_log_file()?;
        let log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)?;
        let log_err = log_file.try_clone()?;

        let listen = format!("127.0.0.1:{}", self.state.port);
        {
            let mut header = OpenOptions::new().append(true).open(&log_path)?;
            writeln!(
                header,
                "==== mei-viewer martin {} listen={} tiles={} bin={} ====",
                MARTIN_VERSION,
                listen,
                mbtiles.display(),
                bin.display()
            )?;
        }

        let mut cmd = Command::new(&bin);
        cmd.arg("--listen-addresses")
            .arg(&listen)
            .arg(&mbtiles)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_err));

        let child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("spawn {}: {e}", bin.display()))?;
        self.child = Some(child);
        self.ready = false;

        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Some(child) = self.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    let tail = read_log_tail(&log_path, 4 * 1024).unwrap_or_default();
                    anyhow::bail!("martin exited early: {status}\n--- log ---\n{tail}");
                }
            }
            if catalog_ready(self.state.port) {
                self.ready = true;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let tail = read_log_tail(&log_path, 4 * 1024).unwrap_or_default();
        anyhow::bail!("timeout waiting for martin /catalog\n--- log ---\n{tail}")
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.ready = false;
        Ok(())
    }

    fn reap_if_exited(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(_)) = child.try_wait() {
                self.child = None;
                self.ready = false;
            }
        }
    }
}

impl Drop for MartinHandle {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn load_state() -> anyhow::Result<MartinState> {
    let path = paths::martin_state_path()?;
    if !path.is_file() {
        return Ok(MartinState::default());
    }
    let text = std::fs::read_to_string(&path)?;
    let mut state: MartinState = serde_json::from_str(&text)?;
    if state.port == 0 {
        state.port = MARTIN_PORT;
    }
    Ok(state)
}

fn save_state(state: &MartinState) -> anyhow::Result<()> {
    let path = paths::martin_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(state)?;
    std::fs::write(path, text)?;
    Ok(())
}

fn catalog_ready(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/catalog");
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    match client.get(&url).send() {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

fn read_log_tail(path: &Path, max_bytes: u64) -> anyhow::Result<String> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    use std::io::Seek;
    file.seek(std::io::SeekFrom::Start(start))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

struct PlatformAsset {
    target: &'static str,
    asset: &'static str,
    is_zip: bool,
}

fn platform_asset() -> anyhow::Result<PlatformAsset> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Ok(PlatformAsset {
            target: "macos-arm64",
            asset: "martin-aarch64-apple-darwin.tar.gz",
            is_zip: false,
        })
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Ok(PlatformAsset {
            target: "macos-x64",
            asset: "martin-x86_64-apple-darwin.tar.gz",
            is_zip: false,
        })
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Ok(PlatformAsset {
            target: "linux-x64",
            asset: "martin-x86_64-unknown-linux-musl.tar.gz",
            is_zip: false,
        })
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Ok(PlatformAsset {
            target: "linux-arm64",
            asset: "martin-aarch64-unknown-linux-musl.tar.gz",
            is_zip: false,
        })
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Ok(PlatformAsset {
            target: "windows-x64",
            asset: "martin-x86_64-pc-windows-msvc.zip",
            is_zip: true,
        })
    } else {
        anyhow::bail!("unsupported platform for Martin download")
    }
}

fn download_and_install(version: &str) -> anyhow::Result<()> {
    let asset = platform_asset()?;
    let url = format!(
        "https://github.com/maplibre/martin/releases/download/martin-v{version}/{}",
        asset.asset
    );
    let cache = paths::martin_cache_dir()?.join(format!("{version}-{}", asset.asset));
    if !cache.is_file() {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;
        let mut resp = client
            .get(&url)
            .send()
            .map_err(|e| anyhow::anyhow!("download martin: {e}"))?;
        if !resp.status().is_success() {
            anyhow::bail!("download martin HTTP {} from {url}", resp.status());
        }
        let partial = cache.with_extension("partial");
        let mut file = File::create(&partial)?;
        std::io::copy(&mut resp, &mut file)?;
        std::fs::rename(&partial, &cache)?;
    }

    let extract_dir = paths::martin_cache_dir()?.join(format!("extract-{}-{}", asset.target, version));
    if extract_dir.exists() {
        std::fs::remove_dir_all(&extract_dir)?;
    }
    std::fs::create_dir_all(&extract_dir)?;

    if asset.is_zip {
        extract_zip(&cache, &extract_dir)?;
    } else {
        extract_tar_gz(&cache, &extract_dir)?;
    }

    let found = find_martin_binary(&extract_dir)?;
    let dest = paths::martin_bin()?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&found, &dest)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
        adhoc_codesign_macos(&dest);
    }
    let _ = std::fs::remove_dir_all(&extract_dir);
    Ok(())
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest)?;
    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let out_path = match entry.enclosed_name() {
            Some(p) => dest.join(p),
            None => continue,
        };
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

fn find_martin_binary(root: &Path) -> anyhow::Result<PathBuf> {
    let names = if cfg!(windows) {
        vec!["martin.exe"]
    } else {
        vec!["martin"]
    };
    for name in &names {
        let direct = root.join(name);
        if direct.is_file() {
            return Ok(direct);
        }
    }
    for entry in walkdir_files(root)? {
        let Some(name) = entry.file_name() else {
            continue;
        };
        let name = name.to_string_lossy();
        if names.iter().any(|n| *n == name) {
            return Ok(entry);
        }
    }
    anyhow::bail!("解压后未找到 martin 二进制 ({})", root.display())
}

fn walkdir_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out)?;
            } else if path.is_file() {
                out.push(path);
            }
        }
        Ok(())
    }
    walk(root, &mut out)?;
    Ok(out)
}

#[cfg(target_os = "macos")]
fn adhoc_codesign_macos(bin: &Path) {
    let _ = Command::new("codesign")
        .args(["--force", "-s", "-", bin.to_str().unwrap_or("")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("xattr")
        .args(["-d", "com.apple.quarantine", bin.to_str().unwrap_or("")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(target_os = "macos"))]
fn adhoc_codesign_macos(_bin: &Path) {}
