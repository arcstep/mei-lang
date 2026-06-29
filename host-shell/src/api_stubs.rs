use axum::response::{IntoResponse, Json};
use serde_json::{json, Value};

fn agent_config_summary() -> Value {
    json!({
        "agent_backend": "native",
        "preferred_mode": "managed",
        "preferred_server_url": null,
        "auto_start_managed": false,
        "managed_start_available": false,
        "runtime_env_ready": false,
        "api_key_configured": false,
        "config_content_ready": false,
        "config_root": null,
        "dotenv_path": null,
        "project_config_present": false,
        "provider_id": "qwen-openai",
        "provider_name": "Qwen (DashScope OpenAI-compatible)",
        "project_config_path": null,
        "base_url": null,
        "completion_model": null,
        "completion_model_choices": [],
        "embedding_model": null,
        "default_model": null,
        "missing_env": ["QWEN_BASE_URL", "QWEN_API_KEY", "QWEN_COMPLETION_MODEL"],
    })
}

pub async fn api_agent_config_stub() -> impl IntoResponse {
    Json(agent_config_summary())
}

pub async fn api_agent_runtime_stub() -> impl IntoResponse {
    Json(json!({
        "configured": agent_config_summary(),
        "running": false,
        "managed_running": false,
        "managed_by_mei": false,
        "connection_source": "disabled",
        "pid": null,
        "host": null,
        "port": null,
        "server_url": null,
        "started_at_ms": null,
        "working_directory": null,
        "last_exit": null,
    }))
}

pub async fn api_agent_skill_stub() -> impl IntoResponse {
    Json(json!({
        "source_dir": "",
        "install_dir": "",
        "entry_file": "",
        "source_present": false,
        "installed": false,
        "stale": false,
        "source_updated_at_ms": null,
        "install_updated_at_ms": null,
        "file_count": 0,
        "revision": null,
    }))
}

pub async fn api_agent_sessions_stub() -> impl IntoResponse {
    Json(json!([]))
}

pub async fn api_agent_context_preview_stub() -> impl IntoResponse {
    Json(json!({
        "available": false,
        "reason": "agent context preview is not available in mei-host-shell",
        "sections": [],
    }))
}
