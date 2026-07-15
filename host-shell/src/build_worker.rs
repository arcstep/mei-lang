//! Build Worker CLI and Host-side spawn helper.
//!
//! Worker process boundary: compile → import → snapshot → seal BUILD.json.
//! Does **not** warmup and does **not** cut routes / write LaunchManifest routes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use mei_host_core::{
    BuildAppArtifact, BuildPhaseReport, BuildRequest, BuildResult, SCHEMA_BUILD_REQUEST_V1,
};
use mei_lang_kernel::{
    attach_build_generation, finish_prebuild_generation, prepare_dev_build_generation_with_hint,
    resolve_app_root, resolve_toolchain_version_with_hint, resolve_workspace_version,
    PrebuildGeneration,
};
use sha2::{Digest, Sha256};

use crate::build_ops::{canonical_workspace, import_with_options, toolchain_hint};
use crate::cli::{BuildWorkerCommand, BuildWorkerRunArgs};

const IN_PROCESS_ENV: &str = "MEI_BUILD_WORKER_IN_PROCESS";
const WORKER_BIN_ENV: &str = "MEI_BUILD_WORKER_BIN";

#[cfg(test)]
pub(crate) static BUILD_WORKER_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn dispatch_build_worker(command: BuildWorkerCommand) -> anyhow::Result<()> {
    match command {
        BuildWorkerCommand::Run(args) => run_cli(args),
    }
}

fn run_cli(args: BuildWorkerRunArgs) -> anyhow::Result<()> {
    let workspace = canonical_workspace(args.workspace.as_path());
    let raw = fs::read_to_string(args.request.as_path()).map_err(|error| {
        anyhow::anyhow!("read BuildRequest {}: {error}", args.request.display())
    })?;
    let request: BuildRequest = serde_json::from_str(raw.as_str())
        .map_err(|error| anyhow::anyhow!("parse BuildRequest: {error}"))?;
    let result = execute_build_pipeline(workspace.as_path(), &request);
    let result = match result {
        Ok(ok) => ok,
        Err(error) => BuildResult::failure(error.to_string(), Vec::new()),
    };
    let json = serde_json::to_string_pretty(&result)?;
    if let Some(output) = args.output.as_ref() {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, format!("{json}\n"))?;
    } else {
        println!("{json}");
    }
    if result.ok {
        Ok(())
    } else {
        anyhow::bail!(result
            .error
            .unwrap_or_else(|| "build worker failed".to_string()))
    }
}

/// Host entry: run Build Worker out-of-process by default.
///
/// Set `MEI_BUILD_WORKER_IN_PROCESS=1` to execute in the current address space (tests).
/// Set `MEI_BUILD_WORKER_BIN` to override the worker binary (tests / alternate install).
pub fn run_build_request(workspace: &Path, request: &BuildRequest) -> anyhow::Result<BuildResult> {
    request
        .validate()
        .map_err(|error| anyhow::anyhow!("invalid BuildRequest: {error}"))?;
    if in_process_enabled() {
        return execute_build_pipeline(workspace, request);
    }
    spawn_build_worker(workspace, request)
}

fn in_process_enabled() -> bool {
    std::env::var(IN_PROCESS_ENV)
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn spawn_build_worker(workspace: &Path, request: &BuildRequest) -> anyhow::Result<BuildResult> {
    let workspace = canonical_workspace(workspace);
    let temp_dir = tempfile_dir()?;
    let request_path = temp_dir.join("build-request.json");
    let result_path = temp_dir.join("build-result.json");
    fs::write(
        &request_path,
        serde_json::to_vec_pretty(request)
            .map_err(|error| anyhow::anyhow!("serialize BuildRequest: {error}"))?,
    )?;

    let binary = resolve_build_worker_binary(Some(workspace.as_path()))?;
    let output = Command::new(&binary)
        .arg("build-worker")
        .arg("run")
        .arg("--workspace")
        .arg(workspace.as_path())
        .arg("--request")
        .arg(&request_path)
        .arg("--output")
        .arg(&result_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| anyhow::anyhow!("spawn build-worker {}: {error}", binary.display()))?;

    let result = if result_path.is_file() {
        let raw = fs::read_to_string(&result_path)?;
        serde_json::from_str::<BuildResult>(raw.as_str()).map_err(|error| {
            anyhow::anyhow!("parse BuildResult from {}: {error}", result_path.display())
        })?
    } else if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow::anyhow!(
            "build-worker exited {} without result file: stdout={stdout} stderr={stderr}",
            output.status.code().unwrap_or(-1)
        ));
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str::<BuildResult>(stdout.as_ref())
            .map_err(|error| anyhow::anyhow!("parse BuildResult from stdout: {error}"))?
    };

    let _ = fs::remove_dir_all(&temp_dir);
    if result.ok {
        Ok(result)
    } else {
        Err(anyhow::anyhow!(result.error.clone().unwrap_or_else(|| {
            "build worker reported failure".to_string()
        })))
    }
}

pub fn resolve_build_worker_binary(workspace: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Ok(path) = std::env::var(WORKER_BIN_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        anyhow::bail!("{WORKER_BIN_ENV}={} is not a file", path.display());
    }
    if let Ok(path) = std::env::var("MEI_HOST_SHELL_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        anyhow::bail!("MEI_HOST_SHELL_BIN={} is not a file", path.display());
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if current_exe.is_file() {
            return Ok(current_exe);
        }
    }
    if let Some(workspace) = workspace {
        let deploy_bin = workspace.join("deploy/bin/mei-host-shell");
        if deploy_bin.is_file() {
            return Ok(deploy_bin);
        }
    }
    anyhow::bail!(
        "mei-host-shell not found for build-worker; set {WORKER_BIN_ENV} or MEI_HOST_SHELL_BIN"
    )
}

fn tempfile_dir() -> anyhow::Result<PathBuf> {
    let base = std::env::temp_dir().join(format!(
        "mei-build-worker-{}-{}",
        std::process::id(),
        crate::state::current_time_ms()
    ));
    fs::create_dir_all(&base)?;
    Ok(base)
}

/// Core pipeline executed inside the Build Worker process (or in-process test mode).
pub fn execute_build_pipeline(
    workspace: &Path,
    request: &BuildRequest,
) -> anyhow::Result<BuildResult> {
    request
        .validate()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let workspace = canonical_workspace(workspace);
    let config_path = workspace.join(request.profile_file.as_str());
    if !config_path.is_file() {
        anyhow::bail!("profile file not found: {}", config_path.display());
    }

    let mut phases = Vec::new();
    let generation = timed_phase(&mut phases, "preparing", || {
        prepare_generation(workspace.as_path(), request)
    })?;

    timed_phase(&mut phases, "compiling", || {
        for app_id in &request.apps {
            crate::tool_exec::run_mei_compiler_compile_with_config(
                workspace.as_path(),
                app_id.as_str(),
                Some(config_path.as_path()),
            )?;
        }
        Ok(())
    })?;

    timed_phase(&mut phases, "importing", || {
        for app_id in &request.apps {
            let _ = import_with_options(workspace.as_path(), app_id.as_str(), None)?;
        }
        Ok(())
    })?;

    timed_phase(&mut phases, "snapshotting", || {
        for app_id in &request.apps {
            let _ =
                mei_host_graph::publish_app_data_snapshots(workspace.as_path(), app_id.as_str())?;
        }
        Ok(())
    })?;

    timed_phase(&mut phases, "sealing", || {
        finish_prebuild_generation(
            workspace.as_path(),
            &generation,
            request.apps.as_slice(),
            None,
            None,
        )?;
        Ok(())
    })?;

    let apps = request
        .apps
        .iter()
        .map(|app_id| build_app_artifact(workspace.as_path(), app_id.as_str(), &generation))
        .collect::<Vec<_>>();

    let mut result = BuildResult::success(generation.env_version.clone(), apps);
    result.phases = phases;
    Ok(result)
}

fn prepare_generation(
    workspace: &Path,
    request: &BuildRequest,
) -> anyhow::Result<PrebuildGeneration> {
    let hint = request
        .toolchain_hint
        .as_deref()
        .unwrap_or_else(|| toolchain_hint());
    if let Some(desired) = request
        .desired_generation
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        attach_build_generation(workspace, request.apps.as_slice(), desired)?;
        let mut generation = PrebuildGeneration {
            env_version: desired.to_string(),
            build_generation: desired.to_string(),
            toolchain_version: resolve_toolchain_version_with_hint(workspace, Some(hint)),
            workspace_version: resolve_workspace_version(workspace),
            config_digest: Some(request.profile_revision.clone()),
            store_dirs: request
                .apps
                .iter()
                .map(|app_id| {
                    let app_root = resolve_app_root(workspace, app_id.as_str());
                    (
                        app_id.clone(),
                        mei_lang_kernel::app_env_build_dir(app_root.as_path(), desired),
                    )
                })
                .collect(),
        };
        generation.config_digest = Some(request.profile_revision.clone());
        return Ok(generation);
    }

    let mut generation =
        prepare_dev_build_generation_with_hint(workspace, request.apps.as_slice(), Some(hint))?;
    generation.config_digest = Some(request.profile_revision.clone());
    Ok(generation)
}

fn build_app_artifact(
    workspace: &Path,
    app_id: &str,
    generation: &PrebuildGeneration,
) -> BuildAppArtifact {
    let bundle_path = format!("apps/{app_id}/env/{}", generation.env_version);
    let meibundle = resolve_app_root(workspace, app_id)
        .join("env")
        .join(generation.env_version.as_str())
        .join("build/exchange")
        .join(format!("{app_id}.meibundle"));
    let digest = file_sha256(meibundle.as_path());
    BuildAppArtifact {
        app_id: app_id.to_string(),
        bundle_path,
        digest,
        config_digest: generation.config_digest.clone(),
    }
}

fn file_sha256(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Some(format!("{:x}", hasher.finalize()))
}

fn timed_phase<T>(
    phases: &mut Vec<BuildPhaseReport>,
    name: &str,
    f: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let started = Instant::now();
    match f() {
        Ok(value) => {
            phases.push(BuildPhaseReport {
                name: name.to_string(),
                ok: true,
                ms: started.elapsed().as_millis() as u64,
                message: None,
            });
            Ok(value)
        }
        Err(error) => {
            phases.push(BuildPhaseReport {
                name: name.to_string(),
                ok: false,
                ms: started.elapsed().as_millis() as u64,
                message: Some(error.to_string()),
            });
            Err(error)
        }
    }
}

pub fn build_request_from_profile(
    profile_id: &str,
    profile_revision: &str,
    profile_file: &str,
    apps: &[String],
) -> BuildRequest {
    let mut request = BuildRequest::new(profile_id, profile_revision, profile_file, apps.to_vec());
    request.schema_version = SCHEMA_BUILD_REQUEST_V1.to_string();
    request.toolchain_hint = Some(toolchain_hint().to_string());
    request
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_host_core::SCHEMA_BUILD_RESULT_V1;

    #[test]
    fn build_request_helper_sets_toolchain_hint() {
        let request = build_request_from_profile(
            "local",
            "r1",
            "configs/local.json",
            &["mini-data".to_string()],
        );
        assert_eq!(request.schema_version, SCHEMA_BUILD_REQUEST_V1);
        assert!(request.toolchain_hint.is_some());
        assert_eq!(request.apps, vec!["mini-data".to_string()]);
    }

    #[test]
    fn execute_pipeline_rejects_bad_schema_without_touching_workspace() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let request = BuildRequest {
            schema_version: "wrong".to_string(),
            profile_id: "local".to_string(),
            profile_revision: "r1".to_string(),
            profile_file: "missing.json".to_string(),
            apps: vec!["mini-data".to_string()],
            toolchain_hint: None,
            compile_scope: None,
            desired_generation: None,
        };
        let err = execute_build_pipeline(tmp.path(), &request).expect_err("schema");
        assert!(err.to_string().contains("schemaVersion"));
    }

    #[test]
    fn spawn_path_uses_stub_worker_binary() {
        let _env_guard = BUILD_WORKER_ENV_LOCK.lock().expect("worker env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path().join("ws");
        fs::create_dir_all(&workspace).expect("ws");
        let stub = tmp.path().join("fake-worker");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = r#"#!/bin/sh
# Parse --output path
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
cat > "$out" <<'EOF'
{
  "schemaVersion": "mei-build-result-v1",
  "ok": true,
  "generation": "WS-stub.1",
  "apps": [{"appId":"mini-data","bundlePath":"apps/mini-data/env/WS-stub.1","digest":null,"configDigest":"r1"}],
  "error": null,
  "phases": [{"name":"compiling","ok":true,"ms":1}]
}
EOF
"#;
            fs::write(&stub, script).expect("write stub");
            let mut perms = fs::metadata(&stub).expect("meta").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&stub, perms).expect("chmod");
        }
        std::env::set_var(WORKER_BIN_ENV, &stub);
        std::env::remove_var(IN_PROCESS_ENV);
        let request = BuildRequest::new(
            "local",
            "r1",
            "configs/local.json",
            vec!["mini-data".into()],
        );
        let result = run_build_request(workspace.as_path(), &request).expect("stub build");
        std::env::remove_var(WORKER_BIN_ENV);
        assert!(result.ok);
        assert_eq!(result.schema_version, SCHEMA_BUILD_RESULT_V1);
        assert_eq!(result.generation.as_deref(), Some("WS-stub.1"));
        assert_eq!(result.apps[0].app_id, "mini-data");
    }
}
