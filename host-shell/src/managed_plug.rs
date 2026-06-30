use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::process::Stdio;
use std::time::Duration;

use mei_host_core::HostContext;
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

pub async fn spawn_managed_plug_ds(ctx: &HostContext) -> anyhow::Result<ManagedPlugDs> {
    let port = reserve_loopback_port()?;
    let endpoint = format!("http://{MANAGED_PLUG_DS_HOST}:{port}");
    let binary = crate::tool_exec::resolve_mei_plug_ds(Some(ctx.workspace_root.as_path()))?;
    let mut child = Command::new(&binary)
        .arg("serve")
        .arg("--workspace")
        .arg(&ctx.workspace_root)
        .arg("--app")
        .arg(&ctx.app_id)
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
