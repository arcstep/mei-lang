use std::{
    collections::BTreeMap,
    path::{Path as FsPath, PathBuf},
    process::{Command as ProcessCommand, ExitStatus},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use tokio::time::{sleep, Duration};

use super::bridge::health as bridge_health;
use super::{
    ManagedOpencodeConfigSummary, ManagedOpencodeExit, ManagedOpencodeProcess,
    ManagedOpencodeRuntime, ManagedOpencodeRuntimeStatus, StartManagedOpencodeRequest,
    MANAGED_OPENCODE_PROVIDER_ID, MANAGED_OPENCODE_PROVIDER_NAME, MANAGED_OPENCODE_READONLY_AGENT,
    MANAGED_OPENCODE_REQUIRED_ENV,
};
use crate::AppState;

fn repo_root(package_root: &FsPath) -> Option<PathBuf> {
    Some(package_root.to_path_buf())
}

fn repo_dotenv_path(package_root: &FsPath) -> Option<PathBuf> {
    repo_root(package_root).map(|root| root.join(".env"))
}

fn opencode_project_config_path(package_root: &FsPath) -> Option<PathBuf> {
    repo_root(package_root).map(|root| root.join("opencode.json"))
}

pub(crate) fn load_repo_dotenv(package_root: &FsPath) {
    let Some(dotenv_path) = repo_dotenv_path(package_root).filter(|path| path.exists()) else {
        return;
    };
    if let Err(error) = dotenvy::from_path(&dotenv_path) {
        tracing::warn!(
            path = %dotenv_path.display(),
            %error,
            "failed to load repo .env"
        );
    }
}

fn managed_env_value(name: &'static str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn managed_opencode_default_model(completion_model: &str) -> String {
    format!("{MANAGED_OPENCODE_PROVIDER_ID}/{completion_model}")
}

fn current_unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn render_managed_opencode_runtime_config_content(base_url: &str, api_key: &str, completion_model: &str) -> String {
    let default_model = managed_opencode_default_model(completion_model);
    json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": {
            MANAGED_OPENCODE_PROVIDER_ID: {
                "npm": "@ai-sdk/openai-compatible",
                "name": MANAGED_OPENCODE_PROVIDER_NAME,
                "options": {
                    "baseURL": base_url,
                    "apiKey": api_key
                },
                "models": {
                    completion_model: {
                        "name": completion_model
                    }
                }
            }
        },
        "model": default_model,
        "small_model": default_model,
        "agent": {
            MANAGED_OPENCODE_READONLY_AGENT: {
                "description": "Read-only MeiLang assistant for guides apps.",
                "permission": {
                    "edit": "deny",
                    "write": "deny",
                    "patch": "deny"
                }
            }
        },
        "enabled_providers": [MANAGED_OPENCODE_PROVIDER_ID]
    })
    .to_string()
}

fn render_managed_opencode_launch_env(
    base_url: &str,
    api_key: &str,
    completion_model: &str,
    embedding_model: Option<&str>,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::from([
        ("QWEN_BASE_URL".to_string(), base_url.to_string()),
        ("QWEN_API_KEY".to_string(), api_key.to_string()),
        (
            "QWEN_COMPLETION_MODEL".to_string(),
            completion_model.to_string(),
        ),
        (
            "OPENCODE_CONFIG_CONTENT".to_string(),
            render_managed_opencode_runtime_config_content(base_url, api_key, completion_model),
        ),
    ]);
    if let Some(embedding_model) = embedding_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        env.insert(
            "QWEN_EMBEDDING_MODEL".to_string(),
            embedding_model.to_string(),
        );
    }
    env
}

pub(crate) fn managed_opencode_config_summary(state: &AppState) -> ManagedOpencodeConfigSummary {
    let base_url = managed_env_value("QWEN_BASE_URL");
    let completion_model = managed_env_value("QWEN_COMPLETION_MODEL");
    let embedding_model = managed_env_value("QWEN_EMBEDDING_MODEL");
    let api_key_configured = managed_env_value("QWEN_API_KEY").is_some();
    let missing_env = MANAGED_OPENCODE_REQUIRED_ENV
        .iter()
        .copied()
        .filter(|name| managed_env_value(name).is_none())
        .collect::<Vec<_>>();
    let config_content_ready = api_key_configured
        && matches!(
            (base_url.as_deref(), completion_model.as_deref()),
            (Some(base_url), Some(completion_model))
                if !render_managed_opencode_runtime_config_content(base_url, "placeholder", completion_model).is_empty()
        );
    let project_config_path =
        opencode_project_config_path(&state.package_root).map(|path| path.display().to_string());
    let project_config_present = project_config_path
        .as_ref()
        .map(|path| FsPath::new(path).exists())
        .unwrap_or(false);
    let default_model = completion_model
        .as_deref()
        .map(managed_opencode_default_model);

    ManagedOpencodeConfigSummary {
        runtime_env_ready: missing_env.is_empty(),
        api_key_configured,
        config_content_ready,
        project_config_present,
        provider_id: MANAGED_OPENCODE_PROVIDER_ID,
        provider_name: MANAGED_OPENCODE_PROVIDER_NAME,
        project_config_path,
        base_url,
        completion_model,
        embedding_model,
        default_model,
        missing_env,
    }
}

fn managed_opencode_exit(status: ExitStatus, kind: &'static str) -> ManagedOpencodeExit {
    ManagedOpencodeExit {
        kind,
        success: status.success(),
        code: status.code(),
    }
}

fn refresh_managed_opencode_runtime(runtime: &mut ManagedOpencodeRuntime) -> anyhow::Result<()> {
    let Some(process) = runtime.process.as_mut() else {
        return Ok(());
    };
    if let Some(status) = process.child.try_wait()? {
        runtime.last_exit = Some(managed_opencode_exit(status, "exited"));
        runtime.process = None;
    }
    Ok(())
}

pub(crate) fn managed_opencode_runtime_status(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    let configured = managed_opencode_config_summary(state);
    let mut runtime = state
        .opencode_runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("opencode runtime lock poisoned"))?;
    refresh_managed_opencode_runtime(&mut runtime)?;

    let (running, pid, host, port, server_url, started_at_ms, working_directory) =
        if let Some(process) = runtime.process.as_ref() {
            (
                true,
                Some(process.child.id()),
                Some(process.host.clone()),
                Some(process.port),
                Some(format!("http://{}:{}", process.host, process.port)),
                Some(process.started_at_ms),
                Some(process.working_directory.display().to_string()),
            )
        } else {
            (false, None, None, None, None, None, None)
        };

    Ok(ManagedOpencodeRuntimeStatus {
        configured,
        running,
        pid,
        host,
        port,
        server_url,
        started_at_ms,
        working_directory,
        last_exit: runtime.last_exit.clone(),
    })
}

async fn wait_for_managed_opencode_ready(
    state: &AppState,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    let server_url = format!("http://{}:{}", host, port);
    let deadline = Instant::now() + Duration::from_secs(10);
    let last_error = loop {
        match bridge_health(&state.opencode_http, &server_url).await {
            Ok(_) => return Ok(()),
            Err(error) => {
                let error_text = error.to_string();
                if Instant::now() >= deadline {
                    break error_text;
                }

                {
                    let mut runtime = state
                        .opencode_runtime
                        .lock()
                        .map_err(|_| anyhow::anyhow!("opencode runtime lock poisoned"))?;
                    refresh_managed_opencode_runtime(&mut runtime)?;
                    if runtime.process.is_none() {
                        let exit = runtime
                            .last_exit
                            .as_ref()
                            .map(|value| format!("{value:?}"))
                            .unwrap_or_else(|| "unknown exit state".to_string());
                        anyhow::bail!("managed opencode exited before ready: {exit}");
                    }
                }

                sleep(Duration::from_millis(250)).await;
            }
        }
    };

    anyhow::bail!(
        "managed opencode did not become ready within 10s: {}",
        last_error
    )
}

pub(crate) async fn start_managed_opencode(
    state: &AppState,
    request: StartManagedOpencodeRequest,
) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    let summary = managed_opencode_config_summary(state);
    let base_url = summary
        .base_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing QWEN_BASE_URL"))?;
    let api_key =
        managed_env_value("QWEN_API_KEY").ok_or_else(|| anyhow::anyhow!("missing QWEN_API_KEY"))?;
    let completion_model = summary
        .completion_model
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing QWEN_COMPLETION_MODEL"))?;
    let launch_env = render_managed_opencode_launch_env(
        base_url,
        &api_key,
        completion_model,
        summary.embedding_model.as_deref(),
    );
    let host = request
        .host
        .unwrap_or_else(|| "127.0.0.1".to_string())
        .trim()
        .to_string();
    if host.is_empty() {
        anyhow::bail!("opencode host cannot be empty");
    }
    let port = request.port.unwrap_or(4099);
    let working_directory = repo_root(&state.package_root)
        .ok_or_else(|| anyhow::anyhow!("failed to resolve repo root for managed opencode"))?;

    {
        let mut runtime = state
            .opencode_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("opencode runtime lock poisoned"))?;
        refresh_managed_opencode_runtime(&mut runtime)?;
        if runtime.process.is_some() {
            return managed_opencode_runtime_status(state);
        }

        let mut command = ProcessCommand::new("opencode");
        command
            .current_dir(&working_directory)
            .arg("serve")
            .arg("--hostname")
            .arg(&host)
            .arg("--port")
            .arg(port.to_string());
        for (name, value) in launch_env {
            command.env(name, value);
        }
        let mut child = command.spawn()?;
        if let Some(status) = child.try_wait()? {
            runtime.last_exit = Some(managed_opencode_exit(status, "failed_to_start"));
            anyhow::bail!("managed opencode exited immediately during startup");
        }

        let pid = child.id();
        runtime.last_exit = None;
        runtime.process = Some(ManagedOpencodeProcess {
            child,
            host: host.clone(),
            port,
            started_at_ms: current_unix_timestamp_ms(),
            working_directory,
        });
        tracing::info!(pid, %host, port, "started managed opencode");
    }

    if let Err(error) = wait_for_managed_opencode_ready(state, &host, port).await {
        let _ = stop_managed_opencode(state);
        return Err(error);
    }

    managed_opencode_runtime_status(state)
}

pub(crate) fn stop_managed_opencode(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    let mut runtime = state
        .opencode_runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("opencode runtime lock poisoned"))?;
    refresh_managed_opencode_runtime(&mut runtime)?;

    let Some(mut process) = runtime.process.take() else {
        drop(runtime);
        return managed_opencode_runtime_status(state);
    };
    let pid = process.child.id();
    process.child.kill()?;
    let status = process.child.wait()?;
    runtime.last_exit = Some(managed_opencode_exit(status, "stopped"));
    tracing::info!(pid, "stopped managed opencode");
    drop(runtime);
    managed_opencode_runtime_status(state)
}

pub(crate) fn managed_opencode_server_url(state: &AppState) -> anyhow::Result<String> {
    let status = managed_opencode_runtime_status(state)?;
    if !status.running {
        anyhow::bail!("managed opencode server is not running");
    }
    status
        .server_url
        .ok_or_else(|| anyhow::anyhow!("managed opencode server URL is unavailable"))
}
