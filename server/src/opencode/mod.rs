pub(crate) mod bridge;
pub(crate) mod events;
pub(crate) mod runtime;

use serde::{Deserialize, Serialize};

pub(crate) const MANAGED_OPENCODE_PROVIDER_ID: &str = "qwen-openai";
pub(crate) const MANAGED_OPENCODE_PROVIDER_NAME: &str = "Qwen (DashScope OpenAI-compatible)";
pub(crate) const MANAGED_OPENCODE_READONLY_AGENT: &str = "mei_readonly";
pub(crate) const MANAGED_OPENCODE_REQUIRED_ENV: &[&str] =
    &["QWEN_BASE_URL", "QWEN_API_KEY", "QWEN_COMPLETION_MODEL"];

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ManagedCompletionModelChoice {
    pub provider_id: String,
    pub model_id: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ManagedOpencodeConfigSummary {
    pub agent_backend: &'static str,
    pub preferred_mode: String,
    pub preferred_server_url: Option<String>,
    pub auto_start_managed: bool,
    pub managed_start_available: bool,
    pub runtime_env_ready: bool,
    pub api_key_configured: bool,
    pub config_content_ready: bool,
    pub config_root: Option<String>,
    pub dotenv_path: Option<String>,
    pub project_config_present: bool,
    pub provider_id: &'static str,
    pub provider_name: &'static str,
    pub project_config_path: Option<String>,
    pub base_url: Option<String>,
    pub completion_model: Option<String>,
    /// 与 `OPENAI_IMITATORS` 及 `*_COMPLETION_MODEL` 顺序一致，供作者面板下拉框使用。
    pub completion_model_choices: Vec<ManagedCompletionModelChoice>,
    pub embedding_model: Option<String>,
    pub default_model: Option<String>,
    pub missing_env: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ManagedOpencodeSkillStatus {
    pub source_dir: String,
    pub install_dir: String,
    pub entry_file: String,
    pub source_present: bool,
    pub installed: bool,
    pub stale: bool,
    pub source_updated_at_ms: Option<u128>,
    pub install_updated_at_ms: Option<u128>,
    pub file_count: usize,
    pub revision: Option<String>,
}

/// 安装态或源码态 skill 目录元信息（不读取 `SKILL.md` 正文，避免塞进 system）。
#[derive(Debug, Clone)]
pub(crate) struct ManagedOpencodeSkillMeta {
    pub skill_home: String,
    pub source_kind: String,
    pub companion_files: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ManagedOpencodeRuntime {
    pub last_exit: Option<ManagedOpencodeExit>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ManagedOpencodeExit {
    pub kind: &'static str,
    pub success: bool,
    pub code: Option<i32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ManagedOpencodeRuntimeStatus {
    pub configured: ManagedOpencodeConfigSummary,
    pub running: bool,
    pub managed_running: bool,
    pub managed_by_mei: bool,
    pub connection_source: String,
    pub pid: Option<u32>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub server_url: Option<String>,
    pub started_at_ms: Option<u128>,
    pub working_directory: Option<String>,
    pub last_exit: Option<ManagedOpencodeExit>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct StartManagedOpencodeRequest {}
