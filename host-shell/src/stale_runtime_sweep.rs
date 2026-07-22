//! Sweep leftover `mei-app-runtime` processes for a workspace.
//!
//! Used on host bootstrap and after `shutdown_all` so disk "Stopped" and the OS
//! process table stay aligned (covers orphans from prior kill -9 / IDE Stop).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Kill `mei-app-runtime serve --workspace <workspace>` processes that are not
/// in `keep_pids` (currently managed children).
///
/// Returns the number of processes signalled.
pub fn sweep_stale_app_runtimes(workspace: &Path, keep_pids: &BTreeSet<u32>) -> usize {
    let Ok(workspace_abs) = workspace.canonicalize() else {
        tracing::warn!(
            path = %workspace.display(),
            "stale runtime sweep skipped: workspace canonicalize failed"
        );
        return 0;
    };
    let workspace_key = workspace_abs.to_string_lossy().to_string();

    // Drop stale runtime.pid files whose process is already gone (no kill noise).
    let mut pruned_dead = 0usize;
    for pid in pids_from_runtime_pid_files(&workspace_abs) {
        if keep_pids.contains(&pid) {
            continue;
        }
        if !pid_alive(pid) {
            clear_runtime_pid_files_for_pid(&workspace_abs, pid);
            pruned_dead += 1;
        }
    }
    if pruned_dead > 0 {
        tracing::info!(
            pruned_dead,
            workspace = %workspace_key,
            "pruned stale runtime.pid files (process already gone)"
        );
    }

    let mut targets: BTreeSet<u32> = BTreeSet::new();
    for pid in pids_from_runtime_pid_files(&workspace_abs) {
        if !keep_pids.contains(&pid) && process_matches_workspace(pid, workspace_key.as_str()) {
            targets.insert(pid);
        }
    }
    for pid in pids_from_process_table(workspace_key.as_str()) {
        if !keep_pids.contains(&pid) && pid_alive(pid) {
            targets.insert(pid);
        }
    }

    let self_pid = std::process::id();
    targets.remove(&self_pid);
    // Re-check liveness right before signalling (shell sweep may have raced us).
    targets.retain(|pid| pid_alive(*pid));

    if targets.is_empty() {
        return 0;
    }

    tracing::warn!(
        count = targets.len(),
        workspace = %workspace_key,
        pids = ?targets,
        "sweeping stale mei-app-runtime processes for workspace"
    );

    for pid in &targets {
        if pid_alive(*pid) {
            signal_pid(*pid, "TERM");
        }
    }
    // Keep this short: shutdown_all may call us on the async runtime thread.
    std::thread::sleep(Duration::from_millis(200));
    for pid in &targets {
        if pid_alive(*pid) {
            signal_pid(*pid, "KILL");
        }
    }
    for pid in &targets {
        clear_runtime_pid_files_for_pid(&workspace_abs, *pid);
    }
    targets.len()
}

/// Pure matching helper for tests: given `ps`-like lines, return PIDs whose
/// command contains `mei-app-runtime` + `--workspace` + the workspace path.
pub fn match_stale_runtime_pids_from_ps_lines(
    lines: &[String],
    workspace_abs: &str,
) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(pid_str) = parts.next() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let command = trimmed[pid_str.len()..].trim_start();
        if command_matches_workspace(command, workspace_abs) {
            out.insert(pid);
        }
    }
    out
}

fn command_matches_workspace(command: &str, workspace_abs: &str) -> bool {
    if !command.contains("mei-app-runtime") || !command.contains("serve") {
        return false;
    }
    if !command.contains("--workspace") {
        return false;
    }
    command.contains(workspace_abs)
}

fn pids_from_process_table(workspace_abs: &str) -> BTreeSet<u32> {
    let output = Command::new("ps").args(["-axo", "pid=,command="]).output();
    let Ok(output) = output else {
        return BTreeSet::new();
    };
    if !output.status.success() {
        return BTreeSet::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    match_stale_runtime_pids_from_ps_lines(&lines, workspace_abs)
}

fn pids_from_runtime_pid_files(workspace_abs: &Path) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    let apps_root = workspace_abs.join("deploy/runtime/apps");
    let Ok(apps) = std::fs::read_dir(&apps_root) else {
        return out;
    };
    for app_entry in apps.flatten() {
        let instances = app_entry.path().join("instances");
        let Ok(instance_dirs) = std::fs::read_dir(&instances) else {
            continue;
        };
        for instance_entry in instance_dirs.flatten() {
            let pid_path = instance_entry.path().join("var").join("runtime.pid");
            if let Some(pid) = read_pid_file(&pid_path) {
                out.insert(pid);
            }
        }
    }
    out
}

fn clear_runtime_pid_files_for_pid(workspace_abs: &Path, pid: u32) {
    let apps_root = workspace_abs.join("deploy/runtime/apps");
    let Ok(apps) = std::fs::read_dir(apps_root) else {
        return;
    };
    for app_entry in apps.flatten() {
        let instances = app_entry.path().join("instances");
        let Ok(instance_dirs) = std::fs::read_dir(instances) else {
            continue;
        };
        for instance_entry in instance_dirs.flatten() {
            let pid_path = instance_entry.path().join("var").join("runtime.pid");
            if read_pid_file(&pid_path) == Some(pid) {
                let _ = std::fs::remove_file(&pid_path);
            }
        }
    }
}

pub fn write_runtime_pid_file(
    workspace: &Path,
    app_id: &str,
    instance_id: &str,
    pid: u32,
) -> anyhow::Result<PathBuf> {
    let var_dir = mei_host_core::instance_var_dir(workspace, app_id, instance_id);
    std::fs::create_dir_all(&var_dir)?;
    let path = var_dir.join("runtime.pid");
    std::fs::write(&path, format!("{pid}\n"))?;
    Ok(path)
}

pub fn clear_runtime_pid_file(workspace: &Path, app_id: &str, instance_id: &str) {
    let path = mei_host_core::instance_var_dir(workspace, app_id, instance_id).join("runtime.pid");
    let _ = std::fs::remove_file(path);
}

fn read_pid_file(path: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(path).ok()?;
    text.trim().parse::<u32>().ok()
}

fn process_matches_workspace(pid: u32, workspace_abs: &str) -> bool {
    if !pid_alive(pid) {
        return false;
    }
    // Require cmdline match so recycled PIDs from stale runtime.pid files are not killed.
    pids_from_process_table(workspace_abs).contains(&pid)
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn signal_pid(pid: u32, signal: &str) {
    let _ = Command::new("kill")
        .args([format!("-{signal}"), pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_workspace_runtime_lines_only() {
        let ws = "/tmp/mei-workspace-probe";
        let lines = vec![
            format!("  123 mei-app-runtime serve --workspace {ws} --app zhifa --instance-id a"),
            "  456 mei-app-runtime serve --workspace /other/ws --app zhifa".to_string(),
            "  789 /usr/bin/vim notes.txt".to_string(),
            format!("  notapid mei-app-runtime serve --workspace {ws}"),
        ];
        let matched = match_stale_runtime_pids_from_ps_lines(&lines, ws);
        assert_eq!(matched, BTreeSet::from([123]));
    }
}
