//! Exit when the supervising host process disappears (Erlang-like link).
//!
//! Covers host `kill -9` / IDE hard-stop where the parent cannot run teardown.
//! Disable with `MEI_RUNTIME_EXIT_ON_PARENT_DEATH=0`.

use std::time::Duration;

use tokio::sync::oneshot;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub fn parent_death_watch_enabled() -> bool {
    match std::env::var("MEI_RUNTIME_EXIT_ON_PARENT_DEATH") {
        Ok(value) => {
            let trimmed = value.trim();
            !(trimmed == "0"
                || trimmed.eq_ignore_ascii_case("false")
                || trimmed.eq_ignore_ascii_case("off"))
        }
        Err(_) => true,
    }
}

/// Spawns a background task that completes `shutdown_tx` when the recorded parent dies.
pub fn spawn_parent_death_watcher(shutdown_tx: oneshot::Sender<()>) {
    if !parent_death_watch_enabled() {
        tracing::info!("parent-death watch disabled (MEI_RUNTIME_EXIT_ON_PARENT_DEATH=0)");
        return;
    }
    #[cfg(unix)]
    {
        let parent_pid = std::os::unix::process::parent_id();
        if parent_pid == 0 || parent_pid == 1 {
            tracing::warn!(
                parent_pid,
                "parent-death watch skipped: unusual parent pid at startup"
            );
            return;
        }
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(POLL_INTERVAL).await;
                if parent_is_gone(parent_pid) {
                    tracing::warn!(
                        parent_pid,
                        "supervising host gone; shutting down mei-app-runtime"
                    );
                    let _ = shutdown_tx.send(());
                    return;
                }
            }
        });
    }
    #[cfg(not(unix))]
    {
        let _ = shutdown_tx;
        tracing::info!("parent-death watch not supported on this platform");
    }
}

#[cfg(unix)]
fn parent_is_gone(original_parent: u32) -> bool {
    let current_ppid = std::os::unix::process::parent_id();
    if current_ppid == 1 || current_ppid != original_parent {
        return true;
    }
    // kill -0: parent pid no longer exists (or not signalable).
    !std::process::Command::new("kill")
        .args(["-0", &original_parent.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
