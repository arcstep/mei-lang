use mei_host_core::HostContext;

use crate::cli::ServeArgs;
use crate::lifecycle::bootstrap_runtime;
use crate::state::{resolve_instance_spec, AppRuntimeServeState};

/// Bind loopback only; port `0` asks the OS for an ephemeral port.
pub async fn run_serve(args: ServeArgs) -> anyhow::Result<()> {
    let workspace = args
        .workspace
        .canonicalize()
        .unwrap_or_else(|_| args.workspace.clone());
    if args.token.trim().is_empty() {
        anyhow::bail!("--token is required and must be non-empty");
    }
    let host = normalize_loopback_host(&args.host)?;
    let mut spec = resolve_instance_spec(
        workspace.as_path(),
        args.app.as_str(),
        args.instance_id.as_str(),
        args.generation.as_deref(),
        args.instance_spec.as_deref(),
    )?;
    if let Some(ceiling) = args
        .data_mode_ceiling
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        spec.data_mode_ceiling = Some(ceiling.to_string());
    }
    let host_ctx = HostContext::new(workspace, spec.app_id.as_str());
    let state = AppRuntimeServeState::new(host_ctx, spec, args.token.trim()).shared();

    if let Err(error) = bootstrap_runtime(state.as_ref()) {
        state.set_failed(error.to_string());
        tracing::error!(error = %error, "app-runtime bootstrap failed");
        // Still bind so supervisor can observe /ready=failed.
    }

    let addr = format!("{host}:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let local = listener.local_addr()?;
    // Supervisor contract: exact stdout line.
    println!("MEI_APP_RUNTIME_LISTEN={}:{}", local.ip(), local.port());
    tracing::info!(
        app = %state.app_id(),
        generation = %state.generation(),
        instance = %state.instance_id(),
        listen = %local,
        "mei-app-runtime serving"
    );

    let app = crate::http::router(state);
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

fn normalize_loopback_host(host: &str) -> anyhow::Result<String> {
    let trimmed = host.trim();
    match trimmed {
        "" | "127.0.0.1" | "localhost" | "::1" => Ok("127.0.0.1".to_string()),
        other => anyhow::bail!(
            "mei-app-runtime must bind loopback only (got `{other}`); use --host 127.0.0.1"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_loopback_host() {
        assert!(normalize_loopback_host("0.0.0.0").is_err());
        assert!(normalize_loopback_host("192.168.1.1").is_err());
        assert_eq!(normalize_loopback_host("127.0.0.1").unwrap(), "127.0.0.1");
        assert_eq!(normalize_loopback_host("localhost").unwrap(), "127.0.0.1");
    }
}
