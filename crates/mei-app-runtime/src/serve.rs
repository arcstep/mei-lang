use mei_host_core::HostContext;

use crate::cli::ServeArgs;
use crate::lifecycle::bootstrap_runtime;
use crate::state::{resolve_instance_spec, AppRuntimeServeState};

/// Bind loopback only; port `0` asks the OS for an ephemeral port.
///
/// Contract with Host supervisor:
/// 1. Bind + print `MEI_APP_RUNTIME_LISTEN=...` **before** hot warmup
/// 2. Serve `/health` immediately so Host can discover the port
/// 3. Run bootstrap/warmup concurrently; `/ready` flips when Acceptable
/// 4. Non-ready Access/API returns 503 quickly (does not block Host workers)
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

    let addr = format!("{host}:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let local = listener.local_addr()?;
    // Supervisor contract: exact stdout line — must appear before long warmup.
    println!("MEI_APP_RUNTIME_LISTEN={}:{}", local.ip(), local.port());
    tracing::info!(
        app = %state.app_id(),
        generation = %state.generation(),
        instance = %state.instance_id(),
        listen = %local,
        "mei-app-runtime listening (bootstrap may still be running)"
    );

    let boot_state = state.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(error) = bootstrap_runtime(boot_state.as_ref()) {
            boot_state.set_failed(error.to_string());
            tracing::error!(error = %error, "app-runtime bootstrap failed");
        } else {
            tracing::info!(
                app = %boot_state.app_id(),
                generation = %boot_state.generation(),
                instance = %boot_state.instance_id(),
                "mei-app-runtime ready"
            );
        }
    });

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
