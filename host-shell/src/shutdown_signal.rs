//! Graceful shutdown triggers for `mei-host-shell serve`.

use std::time::Duration;

/// How long to wait for connection drain + child teardown before hard-exiting.
const FORCE_EXIT_AFTER: Duration = Duration::from_secs(8);

/// Resolves when the process should stop accepting connections and tear down children.
///
/// Listens for Ctrl-C and (on Unix) SIGTERM so IDE Stop / `kill <pid>` reach the
/// existing `shutdown_all` path instead of leaving `mei-app-runtime` orphans.
///
/// After the first signal, a **second** Ctrl-C/SIGTERM (or an 8s timeout) forces
/// `process::exit` — otherwise axum's indefinite connection-drain can leave the
/// process ignoring further Ctrl-C (handler already consumed).
pub async fn shutdown_signal() {
    wait_for_shutdown_signal().await;
    tracing::info!("shutdown signal received; draining connections then tearing down children");

    // Second signal: hard exit (user mash Ctrl-C).
    tokio::spawn(async {
        wait_for_shutdown_signal().await;
        eprintln!("mei-host-shell: second shutdown signal — forcing exit");
        tracing::error!("second shutdown signal; forcing process exit");
        std::process::exit(130);
    });

    // Timeout: drain/teardown stuck (long-lived request, mutex, etc.).
    tokio::spawn(async {
        tokio::time::sleep(FORCE_EXIT_AFTER).await;
        eprintln!(
            "mei-host-shell: graceful shutdown timed out after {}s — forcing exit",
            FORCE_EXIT_AFTER.as_secs()
        );
        tracing::error!(
            timeout_secs = FORCE_EXIT_AFTER.as_secs(),
            "graceful shutdown timed out; forcing process exit"
        );
        std::process::exit(1);
    });
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "failed to install Ctrl-C handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    {
        let mut sigterm =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(signal) => signal,
                Err(error) => {
                    tracing::warn!(%error, "failed to install SIGTERM handler");
                    ctrl_c.await;
                    return;
                }
            };
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}
