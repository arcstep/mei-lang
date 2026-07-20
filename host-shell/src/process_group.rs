//! Unix process-group helpers for managed child processes.

use std::process::Command as StdCommand;

use tokio::process::Command;

/// Put the child in its own process group (pgid == child pid) so host teardown can
/// signal the group without signalling the host itself.
pub fn configure_managed_child_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        // Tokio Command exposes process_group as an inherent Unix helper.
        command.process_group(0);
    }
    #[cfg(not(unix))]
    {
        let _ = command;
    }
}

/// Best-effort SIGKILL against a process group. `pgid` is typically the child pid
/// when spawned with [`configure_managed_child_process_group`].
pub fn kill_process_group_if_alive(pgid: u32) {
    if pgid == 0 {
        return;
    }
    #[cfg(unix)]
    {
        // kill -0: existence check
        let alive = StdCommand::new("kill")
            .args(["-0", &pgid.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !alive {
            return;
        }
        // Negative pid => process group (kill(1) -- -<pgid>).
        let _ = StdCommand::new("kill")
            .args(["-KILL", &format!("-{pgid}")])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = pgid;
    }
}
