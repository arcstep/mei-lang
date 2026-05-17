use std::{
    fs,
    path::{Path as FsPath, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use serde_json::json;
use walkdir::WalkDir;

use super::{
    ManagedCompletionModelChoice, ManagedOpencodeConfigSummary, ManagedOpencodeRuntimeStatus,
    ManagedOpencodeSkillMeta, ManagedOpencodeSkillStatus, StartManagedOpencodeRequest,
    MANAGED_OPENCODE_PROVIDER_ID, MANAGED_OPENCODE_PROVIDER_NAME, MANAGED_OPENCODE_READONLY_AGENT,
    MANAGED_OPENCODE_REQUIRED_ENV,
};
use crate::{mei_agent::llm_config, AppState};

const MANAGED_SKILL_SOURCE_REL: &str = "guides/claude-skills";
const MANAGED_SKILL_INSTALL_REL: &str = ".mei/skills/meilang-author";
const MANAGED_SKILL_ALLOW_PATH_GLOB: &str = "*/.mei/skills/meilang-author";
const MANAGED_SKILL_ALLOW_FILE_GLOB: &str = "*/.mei/skills/meilang-author/*";

pub(crate) fn preferred_opencode_mode() -> String {
    match std::env::var("MEI_OPENCODE_MODE")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("managed") => "managed".to_string(),
        _ => "external".to_string(),
    }
}

pub(crate) fn preferred_opencode_server_url() -> String {
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
        .find(|root| root.join(".env").exists() || root.join("opencode.json").exists())
        .unwrap_or_else(|| package_root.to_path_buf())
}

fn repo_dotenv_path(package_root: &FsPath) -> PathBuf {
    config_root(package_root).join(".env")
}

fn opencode_project_config_path(package_root: &FsPath) -> PathBuf {
    config_root(package_root).join("opencode.json")
}

fn managed_skill_source_dir(package_root: &FsPath) -> PathBuf {
    package_root.join(MANAGED_SKILL_SOURCE_REL)
}

fn managed_skill_install_dir(source_root: &FsPath) -> PathBuf {
    source_root.join(MANAGED_SKILL_INSTALL_REL)
}

fn unix_timestamp_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis())
}

fn directory_latest_modified_ms(path: &FsPath) -> Option<u128> {
    if !path.exists() {
        return None;
    }
    let mut latest = fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(unix_timestamp_ms);
    for entry in WalkDir::new(path).into_iter().flatten() {
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(unix_timestamp_ms);
        if modified > latest {
            latest = modified;
        }
    }
    latest
}

fn markdown_file_count(path: &FsPath) -> usize {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count()
}

fn markdown_files(path: &FsPath) -> Vec<String> {
    if !path.exists() {
        return Vec::new();
    }
    let mut files = WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(path)
                .ok()
                .and_then(|value| value.to_str())
                .map(|value| value.replace('\\', "/"))
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn git_revision_short(package_root: &FsPath) -> Option<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(package_root)
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let revision = String::from_utf8(output.stdout).ok()?;
    let revision = revision.trim();
    if revision.is_empty() {
        None
    } else {
        Some(revision.to_string())
    }
}

fn copy_skill_tree(source_dir: &FsPath, install_dir: &FsPath) -> anyhow::Result<()> {
    if install_dir.exists() {
        fs::remove_dir_all(install_dir).with_context(|| {
            format!(
                "failed to reset installed skill directory {}",
                install_dir.display()
            )
        })?;
    }
    fs::create_dir_all(install_dir).with_context(|| {
        format!(
            "failed to create installed skill directory {}",
            install_dir.display()
        )
    })?;
    for entry in WalkDir::new(source_dir).into_iter().flatten() {
        let source_path = entry.path();
        let Some(relative) = source_path.strip_prefix(source_dir).ok() else {
            continue;
        };
        let target_path = install_dir.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target_path)
                .with_context(|| format!("failed to create {}", target_path.display()))?;
            continue;
        }
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(source_path, &target_path).with_context(|| {
            format!(
                "failed to copy skill file {} -> {}",
                source_path.display(),
                target_path.display()
            )
        })?;
    }
    Ok(())
}

fn build_skill_status(package_root: &FsPath, source_root: &FsPath) -> ManagedOpencodeSkillStatus {
    let source_dir = managed_skill_source_dir(package_root);
    let install_dir = managed_skill_install_dir(source_root);
    let entry_file = install_dir.join("SKILL.md");
    let source_present = source_dir.join("SKILL.md").exists();
    let installed = entry_file.exists();
    let source_updated_at_ms = directory_latest_modified_ms(&source_dir);
    let install_updated_at_ms = directory_latest_modified_ms(&install_dir);
    let stale = source_present
        && installed
        && source_updated_at_ms
            .zip(install_updated_at_ms)
            .is_some_and(|(source_ms, install_ms)| source_ms > install_ms);
    ManagedOpencodeSkillStatus {
        source_dir: source_dir.display().to_string(),
        install_dir: install_dir.display().to_string(),
        entry_file: entry_file.display().to_string(),
        source_present,
        installed,
        stale,
        source_updated_at_ms,
        install_updated_at_ms,
        file_count: markdown_file_count(if installed { &install_dir } else { &source_dir }),
        revision: git_revision_short(package_root),
    }
}

pub(crate) fn managed_opencode_skill_status_for_root(
    package_root: &FsPath,
    source_root: &FsPath,
) -> ManagedOpencodeSkillStatus {
    build_skill_status(package_root, source_root)
}

pub(crate) fn managed_opencode_skill_status(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    Ok(build_skill_status(&state.package_root, &state.source_root))
}

pub(crate) fn sync_managed_opencode_skill_for_root(
    package_root: &FsPath,
    source_root: &FsPath,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    let source_dir = managed_skill_source_dir(package_root);
    let source_entry = source_dir.join("SKILL.md");
    if !source_entry.exists() {
        anyhow::bail!(
            "MeiLang skill source is missing: {}",
            source_entry.display()
        );
    }
    let install_dir = managed_skill_install_dir(source_root);
    copy_skill_tree(&source_dir, &install_dir)?;
    Ok(build_skill_status(package_root, source_root))
}

pub(crate) fn sync_managed_opencode_skill(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    sync_managed_opencode_skill_for_root(&state.package_root, &state.source_root)
}

pub(crate) fn ensure_managed_opencode_skill_synced(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeSkillStatus> {
    let status = build_skill_status(&state.package_root, &state.source_root);
    if !status.source_present {
        return Ok(status);
    }
    if status.installed && !status.stale {
        return Ok(status);
    }
    sync_managed_opencode_skill_for_root(&state.package_root, &state.source_root)
}

/// 解析 meilang-author skill 根目录（已安装优先，否则源码目录）。
/// 默认安装路径为 `{source_root}/.mei/skills/meilang-author`。
pub(crate) fn resolve_meilang_skill_home_for_source_root(
    package_root: &FsPath,
    source_root: &FsPath,
) -> Option<PathBuf> {
    let status = build_skill_status(package_root, source_root);
    if status.installed {
        Some(PathBuf::from(&status.install_dir))
    } else if status.source_present {
        Some(PathBuf::from(&status.source_dir))
    } else {
        None
    }
}

pub(crate) fn load_managed_opencode_skill_meta(
    state: &AppState,
) -> anyhow::Result<Option<ManagedOpencodeSkillMeta>> {
    let Some(home) =
        resolve_meilang_skill_home_for_source_root(&state.package_root, &state.source_root)
    else {
        return Ok(None);
    };
    let status = build_skill_status(&state.package_root, &state.source_root);
    let source_kind = if status.installed {
        "installed"
    } else {
        "source"
    };
    let companion_files = markdown_files(&home)
        .into_iter()
        .filter(|file| file != "SKILL.md")
        .collect::<Vec<_>>();
    Ok(Some(ManagedOpencodeSkillMeta {
        skill_home: home.display().to_string(),
        source_kind: source_kind.to_string(),
        companion_files,
    }))
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

fn managed_opencode_default_model(completion_model: &str) -> String {
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

fn render_managed_opencode_runtime_config_content(
    base_url: &str,
    api_key: &str,
    completion_model: &str,
) -> String {
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

pub(crate) fn managed_opencode_config_summary(state: &AppState) -> ManagedOpencodeConfigSummary {
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
                if !render_managed_opencode_runtime_config_content(base_url, "placeholder", completion_model).is_empty()
        );
    let project_config_path = opencode_project_config_path(&state.package_root);
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
    let default_model = completion_model
        .as_deref()
        .map(managed_opencode_default_model);
    let preferred_mode = state.opencode_preferred_mode.as_ref().clone();
    let preferred_server_url = (preferred_mode == "external")
        .then(|| state.opencode_preferred_server_url.as_ref().clone());

    ManagedOpencodeConfigSummary {
        agent_backend: "native",
        preferred_mode,
        preferred_server_url,
        auto_start_managed: state.opencode_auto_start,
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

pub(crate) fn managed_opencode_runtime_status(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    let configured = managed_opencode_config_summary(state);
    let last_exit = state
        .opencode_runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("opencode runtime lock poisoned"))?
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

pub(crate) async fn start_managed_opencode(
    state: &AppState,
    _request: StartManagedOpencodeRequest,
) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    managed_opencode_runtime_status(state)
}

pub(crate) fn stop_managed_opencode(
    state: &AppState,
) -> anyhow::Result<ManagedOpencodeRuntimeStatus> {
    managed_opencode_runtime_status(state)
}
