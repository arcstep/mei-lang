//! 解析 LLM 连接信息：多供应商由 `BridgeModelRef.provider_id` + 环境变量选择。
//!
//! 若设置 `OPENAI_IMITATORS`（逗号分隔前缀，如 `QWEN,PUMPK,OMLX`），则每个前缀对应一组
//! `{PREFIX}_BASE_URL` / `{PREFIX}_API_KEY` / `{PREFIX}_COMPLETION_MODEL`（及可选的
//! `{PREFIX}_EMBEDDING_MODEL`、`{PREFIX}_IMAGE_MODEL`）。`PREFIX` 在读取 env 时统一为大写。
//! 未设置 `OPENAI_IMITATORS` 时，仍支持历史上的 `QWEN_*` 与 `MEI_LLM_OPENAI_*` 分支。

use crate::opencode::bridge::BridgeModelRef;

#[derive(Debug, Clone)]
pub(crate) struct LlmConnection {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// 供作者面板等展示：顺序与 `.env` 中 `OPENAI_IMITATORS` 及各 `*_COMPLETION_MODEL` 逗号顺序一致。
#[derive(Debug, Clone)]
pub(crate) struct LlmCompletionChoice {
    pub provider_id: String,
    pub model_id: String,
    pub label: String,
}

pub(crate) fn enumerate_completion_choices() -> Vec<LlmCompletionChoice> {
    let imitators = openai_imitator_prefixes_upper();
    if !imitators.is_empty() {
        let mut out = Vec::new();
        for upper in imitators {
            let cats = completion_model_catalog(&upper);
            if cats.is_empty() {
                continue;
            }
            let slug = upper.to_ascii_lowercase();
            for mid in cats {
                out.push(LlmCompletionChoice {
                    provider_id: slug.clone(),
                    model_id: mid.clone(),
                    label: format!("{slug} · {mid}"),
                });
            }
        }
        return out;
    }
    let mut out = Vec::new();
    if let Some(raw) = trim_env("QWEN_COMPLETION_MODEL") {
        let slug = "qwen".to_string();
        for mid in split_csv_line(&raw) {
            out.push(LlmCompletionChoice {
                provider_id: slug.clone(),
                model_id: mid.clone(),
                label: format!("{slug} · {mid}"),
            });
        }
    }
    if let Some(raw) = trim_env("MEI_LLM_OPENAI_MODEL") {
        let slug = "openai".to_string();
        for mid in split_csv_line(&raw) {
            out.push(LlmCompletionChoice {
                provider_id: slug.clone(),
                model_id: mid.clone(),
                label: format!("{slug} · {mid}"),
            });
        }
    }
    out
}

fn trim_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `OPENAI_IMITATORS` 解析出的前缀列表（大写，用于拼 env 名）。
pub(crate) fn openai_imitator_prefixes_upper() -> Vec<String> {
    std::env::var("OPENAI_IMITATORS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|t| t.trim().to_uppercase())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn split_csv_line(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// 某前缀下 `*_COMPLETION_MODEL` 中声明的模型清单（逗号分隔）。
pub(crate) fn completion_model_catalog(prefix_upper: &str) -> Vec<String> {
    let key = format!("{prefix_upper}_COMPLETION_MODEL");
    trim_env(&key)
        .map(|s| split_csv_line(&s))
        .unwrap_or_default()
}

/// 某前缀下 `*_EMBEDDING_MODEL` 中声明的嵌入模型清单（逗号分隔）。
#[allow(dead_code)] // 供后续嵌入 / 路由使用，与补全解析规则一致
pub(crate) fn embedding_model_catalog(prefix_upper: &str) -> Vec<String> {
    let key = format!("{prefix_upper}_EMBEDDING_MODEL");
    trim_env(&key)
        .map(|s| split_csv_line(&s))
        .unwrap_or_default()
}

/// 某前缀下 `*_IMAGE_MODEL` 中声明的图片模型清单（逗号分隔；供后续能力接入）。
#[allow(dead_code)]
pub(crate) fn image_model_catalog(prefix_upper: &str) -> Vec<String> {
    let key = format!("{prefix_upper}_IMAGE_MODEL");
    trim_env(&key)
        .map(|s| split_csv_line(&s))
        .unwrap_or_default()
}

fn find_imitator_upper(provider_slug: &str, list: &[String]) -> Option<String> {
    let p = provider_slug.trim().to_ascii_lowercase();
    if p.is_empty() {
        return None;
    }
    list.iter().find(|im| im.to_ascii_lowercase() == p).cloned()
}

/// 未在请求中指定 `providerID` 时使用的标签（小写，与 `OPENAI_IMITATORS` 中 token 对应）。
pub(crate) fn default_provider_id_for_ui() -> String {
    if let Some(s) = trim_env("MEI_LLM_DEFAULT_PROVIDER") {
        return s.to_ascii_lowercase();
    }
    let list = openai_imitator_prefixes_upper();
    if let Some(first) = list.first() {
        return first.to_ascii_lowercase();
    }
    "qwen".to_string()
}

fn effective_provider_slug(model: Option<&BridgeModelRef>) -> String {
    model
        .map(|m| m.provider_id.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(default_provider_id_for_ui)
}

fn pick_completion_model(prefix_upper: &str, model_id: Option<String>) -> anyhow::Result<String> {
    let catalog = completion_model_catalog(prefix_upper);
    let mid = model_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(m) = mid {
        if catalog.is_empty() || catalog.iter().any(|x| x == &m) {
            return Ok(m);
        }
        // 清单非空且请求了不在清单内的 id：仍放行（兼容网关自定义模型名）
        return Ok(m);
    }
    catalog.first().cloned().ok_or_else(|| {
        anyhow::anyhow!(
            "missing {prefix_upper}_COMPLETION_MODEL (comma-separated) or model.modelID"
        )
    })
}

fn resolve_via_imitator_prefix(
    prefix_upper: &str,
    model_id: Option<String>,
) -> anyhow::Result<LlmConnection> {
    let base_url = trim_env(&format!("{prefix_upper}_BASE_URL"))
        .ok_or_else(|| anyhow::anyhow!("missing {prefix_upper}_BASE_URL"))?;
    let api_key = trim_env(&format!("{prefix_upper}_API_KEY"))
        .ok_or_else(|| anyhow::anyhow!("missing {prefix_upper}_API_KEY"))?;
    let model = pick_completion_model(prefix_upper, model_id)?;
    Ok(LlmConnection {
        base_url,
        api_key,
        model,
    })
}

fn resolve_with_openai_imitators(
    model: Option<&BridgeModelRef>,
    imitators: &[String],
) -> anyhow::Result<LlmConnection> {
    let slug = effective_provider_slug(model);
    let model_id = model
        .map(|m| m.model_id.trim().to_string())
        .filter(|s| !s.is_empty());
    let upper = find_imitator_upper(&slug, imitators).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown provider_id `{slug}` for OPENAI_IMITATORS={imitators:?}; set MEI_LLM_DEFAULT_PROVIDER or extend OPENAI_IMITATORS"
        )
    })?;
    resolve_via_imitator_prefix(&upper, model_id)
}

/// 未配置 `OPENAI_IMITATORS` 时的历史分支。
fn resolve_llm_legacy(model: Option<&BridgeModelRef>) -> anyhow::Result<LlmConnection> {
    let pid = effective_provider_slug(model);
    let model_id = model
        .map(|m| m.model_id.trim().to_string())
        .filter(|s| !s.is_empty());

    match pid.as_str() {
        "qwen" | "dashscope" | "" => {
            let base_url = trim_env("QWEN_BASE_URL")
                .ok_or_else(|| anyhow::anyhow!("missing QWEN_BASE_URL"))?;
            let api_key =
                trim_env("QWEN_API_KEY").ok_or_else(|| anyhow::anyhow!("missing QWEN_API_KEY"))?;
            let model = model_id
                .or_else(|| {
                    trim_env("QWEN_COMPLETION_MODEL").and_then(|s| {
                        let v = split_csv_line(&s);
                        v.first().cloned()
                    })
                })
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!("missing QWEN_COMPLETION_MODEL or model.model_id")
                })?;
            Ok(LlmConnection {
                base_url,
                api_key,
                model,
            })
        }
        "openai" | "mei_openai" | "openai_compatible" => {
            let base_url = trim_env("MEI_LLM_OPENAI_BASE_URL")
                .ok_or_else(|| anyhow::anyhow!("missing MEI_LLM_OPENAI_BASE_URL"))?;
            let api_key = trim_env("MEI_LLM_OPENAI_API_KEY")
                .ok_or_else(|| anyhow::anyhow!("missing MEI_LLM_OPENAI_API_KEY"))?;
            let model = model_id
                .or_else(|| trim_env("MEI_LLM_OPENAI_MODEL"))
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("missing MEI_LLM_OPENAI_MODEL or model.model_id"))?;
            Ok(LlmConnection {
                base_url,
                api_key,
                model,
            })
        }
        other => anyhow::bail!(
            "unsupported LLM provider_id `{other}`; set OPENAI_IMITATORS or use qwen/openai"
        ),
    }
}

pub(crate) fn resolve_llm(model: Option<&BridgeModelRef>) -> anyhow::Result<LlmConnection> {
    let imitators = openai_imitator_prefixes_upper();
    if !imitators.is_empty() {
        resolve_with_openai_imitators(model, &imitators)
    } else {
        resolve_llm_legacy(model)
    }
}

pub(crate) fn llm_env_ready(model: Option<&BridgeModelRef>) -> bool {
    resolve_llm(model).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// 环境变量为进程全局，串行化本模块内依赖 env 的用例。
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvScope {
        prev: Vec<(String, Option<String>)>,
    }

    impl EnvScope {
        fn set(pairs: &[(&str, &str)]) -> Self {
            let mut prev = Vec::new();
            for (k, v) in pairs {
                let key = (*k).to_string();
                let old = std::env::var(&key).ok();
                prev.push((key.clone(), old));
                std::env::set_var(k, v);
            }
            Self { prev }
        }
    }

    impl Drop for EnvScope {
        fn drop(&mut self) {
            for (k, old) in self.prev.iter().rev() {
                match old {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn resolve_unknown_provider_fails_legacy() {
        let _lock = ENV_LOCK.lock().unwrap();
        let saved = std::env::var("OPENAI_IMITATORS").ok();
        std::env::remove_var("OPENAI_IMITATORS");
        let m = BridgeModelRef {
            provider_id: "unknown_xyz".into(),
            model_id: "x".into(),
        };
        assert!(resolve_llm(Some(&m)).is_err());
        match saved {
            Some(s) => std::env::set_var("OPENAI_IMITATORS", s),
            None => std::env::remove_var("OPENAI_IMITATORS"),
        }
    }

    #[test]
    fn openai_imitators_first_completion_when_no_model_id() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _e = EnvScope::set(&[
            ("OPENAI_IMITATORS", "MEI_STUBX"),
            ("MEI_STUBX_BASE_URL", "http://localhost/v1"),
            ("MEI_STUBX_API_KEY", "k"),
            ("MEI_STUBX_COMPLETION_MODEL", "alpha, beta"),
        ]);
        assert_eq!(resolve_llm(None).unwrap().model, "alpha");
    }

    #[test]
    fn openai_imitators_respects_explicit_model_id() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _e = EnvScope::set(&[
            ("OPENAI_IMITATORS", "MEI_STUBY"),
            ("MEI_STUBY_BASE_URL", "http://localhost/v1"),
            ("MEI_STUBY_API_KEY", "k"),
            ("MEI_STUBY_COMPLETION_MODEL", "alpha, beta"),
        ]);
        let m = BridgeModelRef {
            provider_id: "mei_stuby".into(),
            model_id: "beta".into(),
        };
        assert_eq!(resolve_llm(Some(&m)).unwrap().model, "beta");
    }

    #[test]
    fn enumerate_imitators_order_matches_openai_imitators_line() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _e = EnvScope::set(&[
            ("OPENAI_IMITATORS", "MEI_A,MEI_B"),
            ("MEI_A_BASE_URL", "http://a"),
            ("MEI_A_API_KEY", "k"),
            ("MEI_A_COMPLETION_MODEL", "m1, m2"),
            ("MEI_B_BASE_URL", "http://b"),
            ("MEI_B_API_KEY", "k"),
            ("MEI_B_COMPLETION_MODEL", "x"),
        ]);
        let list = enumerate_completion_choices();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].model_id, "m1");
        assert_eq!(list[1].model_id, "m2");
        assert_eq!(list[2].model_id, "x");
        assert_eq!(list[2].provider_id, "mei_b");
    }
}
