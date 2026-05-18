//! 与作者面板权限提示一致的策略（供 native agent 与 HTTP 层共用）。

use crate::agent_runtime::events::looks_like_meilang_skill_path;

pub(crate) fn classify_blocked_permission(
    permission: &str,
    patterns: &[String],
) -> (Option<String>, bool, String) {
    let path = patterns
        .iter()
        .map(String::as_str)
        .find(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());
    if permission == "external_directory" {
        let all_skill = !patterns.is_empty()
            && patterns
                .iter()
                .all(|pattern| looks_like_meilang_skill_path(pattern));
        if all_skill {
            return (
                path,
                true,
                "系统尝试读取 MeiLang skill 目录，但当前内置助手未授予该路径；请在权限提示中批准或联系管理员。"
                    .to_string(),
            );
        }
        return (
            path,
            true,
            "你尝试访问了未授权的文件夹。请检查任务路径是否正确；若这是系统预期目录，请在权限提示中批准或联系管理员。"
                .to_string(),
        );
    }
    if permission == "scope_denied" {
        let p = path.clone().unwrap_or_default();
        return (
            path,
            false,
            format!(
                "当前请求的业务 scope 不允许读取该路径（{}）。若确有需要，请在作者面板将「引用可见范围」扩大到「直接引用」或「场景可达」后重试。",
                if p.is_empty() { "路径未记录" } else { p.trim() }
            ),
        );
    }
    (
        path,
        true,
        format!("触发了未支持的运行时授权请求（permission={permission}）。请联系管理员检查策略。"),
    )
}

#[cfg(test)]
mod tests {
    use super::classify_blocked_permission;

    #[test]
    fn classify_scope_denied_suggests_widen_visibility() {
        let patterns = vec!["demo/other.mei".to_string()];
        let (_, requires_admin, msg) = classify_blocked_permission("scope_denied", &patterns);
        assert!(!requires_admin);
        assert!(msg.contains("引用可见范围"), "{msg}");
    }

    #[test]
    fn classify_skill_directory_mentions_builtin_assistant() {
        let patterns = vec!["/tmp/proj/.mei/skills/meilang-author/foo.md".to_string()];
        let (_, _, msg) = classify_blocked_permission("external_directory", &patterns);
        assert!(msg.contains("内置助手"), "{msg}");
    }
}
