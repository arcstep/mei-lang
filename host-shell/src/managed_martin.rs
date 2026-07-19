//! Host-managed MapLibre Martin sidecar for workspace `stock/gis/tiles`.
//!
//! One Martin process per `mei-host-shell serve` (not per-app). External
//! `MEI_GIS_PROXY_UPSTREAM` wins and skips spawn. Missing binary or empty tiles
//! directory skips with a warning so serve still starts.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

const MANAGED_MARTIN_HOST: &str = "127.0.0.1";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(200);

pub struct ManagedMartin {
    pub endpoint: String,
    child: Child,
}

impl ManagedMartin {
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        if let Err(error) = self.child.start_kill() {
            if error.kind() != std::io::ErrorKind::InvalidInput {
                return Err(anyhow::anyhow!("stop managed Martin: {error}"));
            }
        }
        let _ = timeout(Duration::from_secs(3), self.child.wait()).await;
        Ok(())
    }
}

/// Non-empty `MEI_GIS_PROXY_UPSTREAM` means an external Martin is already configured.
pub fn external_gis_upstream_configured() -> bool {
    configured_external_gis_upstream().is_some()
}

pub fn configured_external_gis_upstream() -> Option<String> {
    std::env::var("MEI_GIS_PROXY_UPSTREAM")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

pub fn stock_gis_tiles_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("stock/gis/tiles")
}

pub fn tiles_dir_has_mbtiles(tiles_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(tiles_dir) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mbtiles"))
    })
}

/// Spawn Martin for `{workspace}/stock/gis/tiles`, or `Ok(None)` when skipped.
pub async fn spawn_managed_martin(workspace_root: &Path) -> anyhow::Result<Option<ManagedMartin>> {
    if external_gis_upstream_configured() {
        tracing::info!(
            "skipping managed Martin; MEI_GIS_PROXY_UPSTREAM is set (external upstream)"
        );
        return Ok(None);
    }

    let tiles_dir = stock_gis_tiles_dir(workspace_root);
    if !tiles_dir.is_dir() {
        tracing::warn!(
            path = %tiles_dir.display(),
            "skipping managed Martin; stock/gis/tiles directory missing"
        );
        return Ok(None);
    }
    if !tiles_dir_has_mbtiles(&tiles_dir) {
        tracing::warn!(
            path = %tiles_dir.display(),
            "skipping managed Martin; no .mbtiles under stock/gis/tiles"
        );
        return Ok(None);
    }

    let binary = match crate::tool_exec::resolve_mei_martin(Some(workspace_root)) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "skipping managed Martin; binary not found (set MEI_MARTIN_BIN or ship sidecar)"
            );
            return Ok(None);
        }
    };

    let port = reserve_loopback_port()?;
    let listen = format!("{MANAGED_MARTIN_HOST}:{port}");
    let endpoint = format!("http://{listen}");
    let mut child = Command::new(&binary)
        .arg("--listen-addresses")
        .arg(&listen)
        .arg(tiles_dir.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| anyhow::anyhow!("spawn {}: {error}", binary.display()))?;

    if let Err(error) = wait_for_catalog(endpoint.as_str(), &mut child).await {
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(error);
    }

    tracing::info!(
        endpoint = %endpoint,
        tiles = %tiles_dir.display(),
        binary = %binary.display(),
        "managed Martin started for workspace stock/gis/tiles"
    );
    Ok(Some(ManagedMartin { endpoint, child }))
}

/// Best-effort attach into the HostHttpState slot (never fails serve).
pub async fn attach_managed_martin(
    workspace_root: &Path,
    slot: &Arc<Mutex<Option<ManagedMartin>>>,
) {
    match spawn_managed_martin(workspace_root).await {
        Ok(Some(martin)) => {
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(martin);
            }
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(
                error = %error,
                "managed Martin failed to start; /gis may return 503 until Martin is available"
            );
        }
    }
}

pub async fn shutdown_managed_martin_slot(slot: &Arc<Mutex<Option<ManagedMartin>>>) {
    let Some(mut martin) = slot.lock().ok().and_then(|mut guard| guard.take()) else {
        return;
    };
    if let Err(error) = martin.shutdown().await {
        tracing::warn!(detail = %error, "managed Martin shutdown failed");
    }
}

fn reserve_loopback_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| anyhow::anyhow!("reserve managed Martin port: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| anyhow::anyhow!("read managed Martin port: {error}"))?
        .port();
    drop(listener);
    Ok(port)
}

async fn wait_for_catalog(endpoint: &str, child: &mut Child) -> anyhow::Result<()> {
    let catalog_url = format!("{endpoint}/catalog");
    let client = reqwest::Client::builder()
        .build()
        .map_err(|error| anyhow::anyhow!("build managed Martin health client: {error}"))?;
    let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("managed Martin exited during startup (status={status})");
        }
        match client.get(catalog_url.as_str()).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(_) | Err(_) => {}
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("managed Martin catalog check timed out at {catalog_url}");
        }
        sleep(HEALTH_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn tiles_dir_detects_mbtiles() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let tiles = tmp.path().join("stock/gis/tiles");
        fs::create_dir_all(&tiles).expect("mkdir");
        assert!(!tiles_dir_has_mbtiles(&tiles));
        fs::write(tiles.join("demo.mbtiles"), b"x").expect("write");
        assert!(tiles_dir_has_mbtiles(&tiles));
    }

    #[test]
    fn external_upstream_env_detected() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("MEI_GIS_PROXY_UPSTREAM");
        assert!(!external_gis_upstream_configured());
        std::env::set_var("MEI_GIS_PROXY_UPSTREAM", "http://127.0.0.1:18080");
        assert!(external_gis_upstream_configured());
        assert_eq!(
            configured_external_gis_upstream().as_deref(),
            Some("http://127.0.0.1:18080")
        );
        std::env::remove_var("MEI_GIS_PROXY_UPSTREAM");
    }

    #[tokio::test]
    async fn spawn_skips_when_external_upstream_set() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let tiles = tmp.path().join("stock/gis/tiles");
        fs::create_dir_all(&tiles).expect("mkdir");
        fs::write(tiles.join("demo.mbtiles"), b"x").expect("write");
        std::env::set_var("MEI_GIS_PROXY_UPSTREAM", "http://127.0.0.1:18080");
        let result = spawn_managed_martin(tmp.path()).await.expect("ok");
        std::env::remove_var("MEI_GIS_PROXY_UPSTREAM");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn spawn_skips_when_tiles_missing() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("MEI_GIS_PROXY_UPSTREAM");
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = spawn_managed_martin(tmp.path()).await.expect("ok");
        assert!(result.is_none());
    }

    #[test]
    fn reserve_loopback_port_is_nonzero() {
        let port = reserve_loopback_port().expect("port");
        assert!(port > 0);
    }
}
