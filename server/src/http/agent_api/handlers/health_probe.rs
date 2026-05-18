use std::path::Path as FsPath;
use std::time::Instant;

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    agent_runtime::bridge::{BridgeHealthResponse, BridgeModelRef},
    mei_agent::{
        agent_health, agent_project_worktree, agent_vcs_summary, llm_config,
        resolve_agent_conn,
    },
    AppState,
};

use crate::http::error_response;

#[derive(Debug, Deserialize)]
pub struct OpencodeModelProbeQuery {
    #[serde(default, alias = "providerID")]
    pub provider_id: Option<String>,
    #[serde(default, alias = "modelID")]
    pub model_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpencodeModelProbeResponse {
    pub reachable: bool,
    pub provider_id: String,
    pub model_id: String,
    pub base_url: String,
    #[serde(default)]
    pub latency_ms: Option<u128>,
    #[serde(default)]
    pub status_code: Option<u16>,
    #[serde(default)]
    pub error: Option<String>,
}

pub async fn api_agent_health(State(state): State<AppState>) -> Response {
    fn normalize_path(value: &str) -> String {
        value.trim().trim_end_matches('/').to_string()
    }

    fn worktree_matches_expected(project_worktree: &str, expected_worktree: &str) -> bool {
        let project = normalize_path(project_worktree);
        let expected = normalize_path(expected_worktree);
        if project == expected {
            return true;
        }

        let project_path = FsPath::new(&project);
        let expected_path = FsPath::new(&expected);
        // OpenCode 可能返回更外层的 project/worktree，Mei `source_root` 更具体
        if expected_path.starts_with(project_path) {
            return true;
        }
        // OpenCode cwd 在子目录，Mei 绑定整个 `workspaces`
        if project_path.starts_with(expected_path) {
            return true;
        }
        false
    }

    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(_) => {
            return Json(BridgeHealthResponse {
                server_url: String::new(),
                healthy: false,
                version: String::new(),
                expected_worktree: Some(state.source_root.display().to_string()),
                project_worktree: None,
                vcs_detected: false,
                vcs_branch: None,
                history_available: false,
                history_reason: Some(
                    "内置助手未初始化；Undo/Redo 与自动刷新依赖正确的 worktree 和 Git/VCS 视角。"
                        .to_string(),
                ),
            })
            .into_response();
        }
    };
    match agent_health(&state, &conn).await {
        Ok(mut status) => {
            let expected_worktree = state.source_root.display().to_string();
            status.expected_worktree = Some(expected_worktree.clone());
            match agent_project_worktree(&state, &conn).await {
                Ok(project_worktree) => status.project_worktree = project_worktree,
                Err(error) => {
                    status.history_available = false;
                    status.history_reason = Some(format!("无法读取当前 worktree：{error}"));
                    return Json(status).into_response();
                }
            }
            match agent_vcs_summary(&state, &conn).await {
                Ok((vcs_detected, vcs_branch)) => {
                    status.vcs_detected = vcs_detected;
                    status.vcs_branch = vcs_branch;
                }
                Err(error) => {
                    status.history_available = false;
                    status.history_reason = Some(format!("无法读取 VCS 状态：{error}"));
                    return Json(status).into_response();
                }
            }
            let project_matches = status
                .project_worktree
                .as_deref()
                .is_some_and(|value| worktree_matches_expected(value, &expected_worktree));
            if !status.healthy {
                status.history_available = false;
                if status.history_reason.is_none() {
                    status.history_reason =
                        Some("内置助手未就绪；Undo/Redo 与自动刷新当前不可用。".to_string());
                }
            } else if !project_matches {
                status.history_available = false;
                status.history_reason = Some(format!(
                    "当前 worktree 为 {}，而 MeiLang 预期工作区为 {}；Undo/Redo 与自动刷新不可用。",
                    status.project_worktree.as_deref().unwrap_or("(unknown)"),
                    expected_worktree
                ));
            } else if !status.vcs_detected {
                status.history_available = false;
                status.history_reason = Some(
                    "当前 worktree 未检测到 Git/VCS；Undo/Redo 与自动刷新不可用。".to_string(),
                );
            } else {
                status.history_available = true;
                status.history_reason = None;
            }
            Json(status).into_response()
        }
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_model_probe(Query(query): Query<OpencodeModelProbeQuery>) -> Response {
    let provider_id = query
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let model_id = query
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let model_ref = if provider_id.is_some() || model_id.is_some() {
        Some(BridgeModelRef {
            provider_id: provider_id.clone().unwrap_or_default(),
            model_id: model_id.clone().unwrap_or_default(),
        })
    } else {
        None
    };
    let conn = match llm_config::resolve_llm(model_ref.as_ref()) {
        Ok(value) => value,
        Err(error) => {
            return Json(OpencodeModelProbeResponse {
                reachable: false,
                provider_id: provider_id.unwrap_or_else(llm_config::default_provider_id_for_ui),
                model_id: model_id.unwrap_or_default(),
                base_url: String::new(),
                latency_ms: None,
                status_code: None,
                error: Some(error.to_string()),
            })
            .into_response();
        }
    };

    let models_url = format!("{}/models", conn.base_url.trim_end_matches('/'));
    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build();
    let Ok(client) = client else {
        return Json(OpencodeModelProbeResponse {
            reachable: false,
            provider_id: provider_id.unwrap_or_else(llm_config::default_provider_id_for_ui),
            model_id: conn.model.clone(),
            base_url: conn.base_url.clone(),
            latency_ms: None,
            status_code: None,
            error: Some("failed to initialize probe client".to_string()),
        })
        .into_response();
    };

    let result = client
        .get(&models_url)
        .header("Authorization", format!("Bearer {}", conn.api_key))
        .header("Content-Type", "application/json")
        .send()
        .await;
    match result {
        Ok(response) => {
            let status = response.status();
            Json(OpencodeModelProbeResponse {
                reachable: status.is_success(),
                provider_id: provider_id.unwrap_or_else(llm_config::default_provider_id_for_ui),
                model_id: conn.model.clone(),
                base_url: conn.base_url.clone(),
                latency_ms: Some(start.elapsed().as_millis()),
                status_code: Some(status.as_u16()),
                error: (!status.is_success()).then(|| format!("probe status {}", status.as_u16())),
            })
            .into_response()
        }
        Err(error) => Json(OpencodeModelProbeResponse {
            reachable: false,
            provider_id: provider_id.unwrap_or_else(llm_config::default_provider_id_for_ui),
            model_id: conn.model.clone(),
            base_url: conn.base_url.clone(),
            latency_ms: Some(start.elapsed().as_millis()),
            status_code: None,
            error: Some(error.to_string()),
        })
        .into_response(),
    }
}
