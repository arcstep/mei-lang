use std::path::{Path as FsPath, PathBuf};

use serde_json::json;

use super::super::{
    ManagedCompletionModelChoice, ManagedOpencodeConfigSummary, ManagedOpencodeRuntimeStatus,
    StartManagedOpencodeRequest, MANAGED_OPENCODE_PROVIDER_ID, MANAGED_OPENCODE_PROVIDER_NAME,
    MANAGED_OPENCODE_READONLY_AGENT, MANAGED_OPENCODE_REQUIRED_ENV,
};
use crate::{mei_agent::llm_config, AppState};

const MANAGED_SKILL_ALLOW_PATH_GLOB: &str = "*/.mei/skills/meilang-author";
const MANAGED_SKILL_ALLOW_FILE_GLOB: &str = "*/.mei/skills/meilang-author/*";
pub(crate) fn preferred_agent_mode() -> String {
    match std::env::var("MEI_OPENCODE_MODE")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("managed") => "managed".to_string(),
        _ => "external".to_string(),
    }
}

pub(crate) fn preferred_agent_server_url() -> String {
    std::env::var("MEI_OPENCODE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:4099".to_string())
}

fn candidate_config_roots(package_root: &FsPath) -> Vec<PathBuf> {
    let mut roots = vec![package_root.to_path_buf()];
    if let Some(workspace_root) = package_root.parent() {
        let workspace_root = workspace_root.to_path_buf();
        if !roots.iter().any(|item| item == &workspace_root) {
            roots.push(workspace_root);
        }
    }
    roots
}

fn config_root(package_root: &FsPath) -> PathBuf {
    candidate_config_roots(package_root)
        .into_iter()
        .find(|root| root.join(".env").exists() || root.join("agent.json").exists())
        .unwrap_or_else(|| package_root.to_path_buf())
}

fn repo_dotenv_path(package_root: &FsPath) -> PathBuf {
    config_root(package_root).join(".env")
}

fn agent_project_config_path(package_root: &FsPath) -> PathBuf {
    config_root(package_root).join("agent.json")
}
pub(crate) fn load_repo_dotenv(package_root: &FsPath) {
    for root in candidate_config_roots(package_root) {
        let dotenv_path = root.join(".env");
        if !dotenv_path.exists() {
            continue;
        }
        if let Err(error) = dotenvy::from_path(&dotenv_path) {
            tracing::warn!(
                path = %dotenv_path.display(),
                %error,
                "failed to load repo .env"
            );
        }
        break;
    }
}

fn managed_env_value(name: &'static str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn managed_agent_default_model(completion_model: &str) -> String {
    format!("{MANAGED_OPENCODE_PROVIDER_ID}/{completion_model}")
}

fn managed_external_directory_permissions() -> serde_json::Value {
    json!({
        "external_directory": {
            MANAGED_SKILL_ALLOW_PATH_GLOB: "allow",
            MANAGED_SKILL_ALLOW_FILE_GLOB: "allow",
        }
    })
}

fn render_managed_agent_runtime_config_content(
    base_url: &str,
    api_key: &str,
    completion_model: &str,
) -> String {
    let default_model = managed_agent_default_model(completion_model);
    json!({
        "$schema": "https://mei-lang.dev/agent-config.json",
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
        "permission": managed_external_directory_permissions(),
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

pub(crate) fn managed_agent_config_summary(state: &AppState) -> ManagedOpencodeConfigSummary {
    let base_url = managed_env_value("QWEN_BASE_URL");
    let qwen_completion_raw = managed_env_value("QWEN_COMPLETION_MODEL");
    let qwen_completion_first = qwen_completion_raw.as_deref().and_then(|s| {
        s.split(',')
            .next()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
    });
    let embedding_model = managed_env_value("QWEN_EMBEDDING_MODEL");
    let api_key_configured = managed_env_value("QWEN_API_KEY").is_some();
    let missing_env = MANAGED_OPENCODE_REQUIRED_ENV
        .iter()
        .copied()
        .filter(|name| managed_env_value(name).is_none())
        .collect::<Vec<_>>();
    let config_content_ready = api_key_configured
        && matches!(
            (base_url.as_deref(), qwen_completion_first),
            (Some(base_url), Some(completion_model))
                if !render_managed_agent_runtime_config_content(base_url, "placeholder", completion_model).is_empty()
        );
    let project_config_path = agent_project_config_path(&state.package_root);
    let config_root = config_root(&state.package_root);
    let dotenv_path = repo_dotenv_path(&state.package_root);
    let project_config_present = project_config_path.exists();
    let completion_model_choices: Vec<ManagedCompletionModelChoice> =
        llm_config::enumerate_completion_choices()
            .into_iter()
            .map(|c| ManagedCompletionModelChoice {
                provider_id: c.provider_id,
                model_id: c.model_id.clone(),
                label: c.label,
            })
            .collect();
    let completion_model = completion_model_choices
        .first()
        .map(|c| c.model_id.clone())
        .or_else(|| qwen_completion_first.map(|s| s.to_string()));
    let default_model = completion_model.as_deref().map(managed_agent_default_model);
    let preferred_mode = state.agent_preferred_mode.as_ref().clone();
    let preferred_server_url =
        (preferred_mode == "external").then(|| state.agent_preferred_server_url.as_ref().clone());

    ManagedOpencodeConfigSummary {
        agent_backend: "native",
        preferred_mode,
        preferred_server_url,
        auto_start_managed: state.agent_auto_start,
        managed_start_available: missing_env.is_empty() && config_content_ready,
        runtime_env_ready: missing_env.is_empty(),
        api_key_configured,
        config_content_ready,
        config_root: Some(config_root.display().to_string()),
        dotenv_path: dotenv_path
            .exists()
            .then(|| dotenv_path.display().to_string()),
        project_config_present,
        provider_id: MANAGED_OPENCODE_PROVIDER_ID,
        provider_name: MANAGED_OPENCODE_PROVIDER_NAME,
        project_config_path: Some(project_config_path.display().to_string()),
        base_url,
        completion_model,
        completion_model_choices,
        embedding_model,
        default_model,
        missing_env,
    }
}

pub(crate) fn managed_agent_runtime_status(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    let configured = managed_agent_config_summary(state);
    let last_exit = state
        .agent_runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("agent runtime lock poisoned"))?
        .last_exit
        .clone();
    Ok(ManagedOpencodeRuntimeStatus {
        configured,
        running: true,
        managed_running: false,
        managed_by_mei: false,
        connection_source: "native".to_string(),
        pid: None,
        host: None,
        port: None,
        server_url: Some("mei://native-agent".to_string()),
        started_at_ms: None,
        working_directory: Some(state.source_root.display().to_string()),
        last_exit,
    })
}

pub(crate) async fn start_managed_agent(
    state: &AppState,
    _request: StartManagedOpencodeRequest,
) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    managed_agent_runtime_status(state)
}

pub(crate) fn stop_managed_agent(state: &AppState) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    managed_agent_runtime_status(state)
}
