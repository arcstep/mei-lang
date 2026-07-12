//! Deprecated Host-managed `mei-plug-ds` sidecars.
//!
//! Prefer `mei-app-runtime` (embedded DS). Spawn only for apps **not** covered by an
//! active LaunchManifest Running route; see [`crate::legacy_compat::apps_needing_managed_plug_ds`].

use std::collections::{BTreeMap, BTreeSet};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

const MANAGED_PLUG_DS_HOST: &str = "127.0.0.1";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(200);

pub struct ManagedPlugDs {
    pub endpoint: String,
    child: Child,
}

impl ManagedPlugDs {
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        if let Err(error) = self.child.start_kill() {
            if error.kind() != std::io::ErrorKind::InvalidInput {
                return Err(anyhow::anyhow!("stop managed plug-ds: {error}"));
            }
        }
        let _ = timeout(Duration::from_secs(3), self.child.wait()).await;
        Ok(())
    }
}

pub struct ManagedPlugDsPool {
    pub endpoints: BTreeMap<String, String>,
    sidecars: BTreeMap<String, ManagedPlugDs>,
}

impl ManagedPlugDsPool {
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        for (app_id, sidecar) in self.sidecars.iter_mut() {
            if let Err(error) = sidecar.shutdown().await {
                tracing::warn!(app_id = %app_id, detail = %error, "managed plug-ds shutdown failed");
            }
        }
        Ok(())
    }
}

/// Spawn managed plug-ds only for apps not covered by app-runtime intent.
///
/// When `covered_by_runtime` includes every `app_id`, returns an empty pool without spawning.
pub async fn spawn_managed_plug_ds_pool(
    workspace_root: &std::path::Path,
    app_ids: &[String],
    covered_by_runtime: &BTreeSet<String>,
) -> anyhow::Result<ManagedPlugDsPool> {
    let needing = crate::legacy_compat::apps_needing_managed_plug_ds(app_ids, covered_by_runtime);
    if needing.is_empty() {
        if !app_ids.is_empty() {
            tracing::info!(
                skipped = app_ids.len(),
                "skipping managed plug-ds; all target apps covered by app-runtime routes"
            );
        }
        return Ok(ManagedPlugDsPool {
            endpoints: BTreeMap::new(),
            sidecars: BTreeMap::new(),
        });
    }
    if !covered_by_runtime.is_empty() {
        tracing::info!(
            spawn = needing.len(),
            skipped = covered_by_runtime.len(),
            "managed plug-ds spawn limited to apps without app-runtime coverage"
        );
    }
    let mut endpoints = BTreeMap::new();
    let mut sidecars = BTreeMap::new();
    for app_id in &needing {
        match spawn_managed_plug_ds_for_app(workspace_root, app_id.as_str()).await {
            Ok(sidecar) => {
                tracing::warn!(
                    app_id = %app_id,
                    endpoint = %sidecar.endpoint,
                    "managed plug-ds started (deprecated migration path; prefer mei-app-runtime)"
                );
                endpoints.insert(app_id.clone(), sidecar.endpoint.clone());
                sidecars.insert(app_id.clone(), sidecar);
            }
            Err(error) => {
                tracing::warn!(
                    app_id = %app_id,
                    error = %error,
                    "managed plug-ds failed to start; app data APIs may be unavailable"
                );
            }
        }
    }
    if endpoints.is_empty() && !needing.is_empty() {
        anyhow::bail!("failed to start managed plug-ds for any app");
    }
    Ok(ManagedPlugDsPool {
        endpoints,
        sidecars,
    })
}

async fn spawn_managed_plug_ds_for_app(
    workspace_root: &std::path::Path,
    app_id: &str,
) -> anyhow::Result<ManagedPlugDs> {
    let port = reserve_loopback_port()?;
    let endpoint = format!("http://{MANAGED_PLUG_DS_HOST}:{port}");
    let binary = crate::tool_exec::resolve_mei_plug_ds(Some(workspace_root))?;
    let mut child = Command::new(&binary)
        .arg("serve")
        .arg("--workspace")
        .arg(workspace_root)
        .arg("--app")
        .arg(app_id)
        .arg("--host")
        .arg(MANAGED_PLUG_DS_HOST)
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| anyhow::anyhow!("spawn {}: {error}", binary.display()))?;

    if let Err(error) = wait_for_health(endpoint.as_str(), &mut child).await {
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(error);
    }

    Ok(ManagedPlugDs { endpoint, child })
}

fn reserve_loopback_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| anyhow::anyhow!("reserve managed plug-ds port: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| anyhow::anyhow!("read managed plug-ds port: {error}"))?
        .port();
    drop(listener);
    Ok(port)
}

async fn wait_for_health(endpoint: &str, child: &mut Child) -> anyhow::Result<()> {
    let health_url = format!("{endpoint}/api/plug-ds/health");
    let client = reqwest::Client::builder()
        .build()
        .map_err(|error| anyhow::anyhow!("build managed plug-ds health client: {error}"))?;
    let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("managed plug-ds exited during startup (status={status})");
        }
        match client.get(health_url.as_str()).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(_) | Err(_) => {}
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("managed plug-ds health check timed out at {health_url}");
        }
        sleep(HEALTH_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_compat::apps_needing_managed_plug_ds;

    #[test]
    fn skip_set_filters_spawn_targets() {
        let covered = BTreeSet::from(["mini-data".to_string(), "a".to_string()]);
        let needing =
            apps_needing_managed_plug_ds(&["mini-data".into(), "a".into(), "b".into()], &covered);
        assert_eq!(needing, vec!["b".to_string()]);
    }

    #[tokio::test]
    async fn spawn_pool_skips_all_when_fully_covered() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let covered = BTreeSet::from(["mini-data".to_string()]);
        let pool = spawn_managed_plug_ds_pool(tmp.path(), &["mini-data".into()], &covered)
            .await
            .expect("empty pool");
        assert!(pool.endpoints.is_empty());
        assert!(pool.sidecars.is_empty());
    }
}
