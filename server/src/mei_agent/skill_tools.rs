//! 只读访问 meilang-author skill 目录（与 workspace `read_file` 分离：skill 常位于配置根下）。

use std::path::Path;

use serde_json::{json, Value};

use crate::mei_agent::llm::sanitize_relative_path;
use crate::opencode::runtime::resolve_meilang_skill_home;

const MAX_SKILL_READ_BYTES: usize = 200_000;

fn list_markdown_under_skill_home(home: &Path) -> Vec<String> {
    if !home.exists() {
        return Vec::new();
    }
    let mut files = walkdir::WalkDir::new(home)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(home)
                .ok()
                .and_then(|value| value.to_str())
                .map(|value| value.replace('\\', "/"))
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

/// JSON：`skill_home`, `files`（相对 skill 根的 `.md` 路径，含 `SKILL.md`）。
pub(crate) fn execute_skill_list(package_root: &Path) -> String {
    let Some(home) = resolve_meilang_skill_home(package_root) else {
        return "error: mei-lang author skill not found (sync skill or ensure guides/claude-skills exists)"
            .to_string();
    };
    let files = list_markdown_under_skill_home(&home);
    json!({
        "skill_home": home.display().to_string(),
        "files": files,
    })
    .to_string()
}

/// 读取 skill 根下的 UTF-8 文本（`rel` 为相对路径，禁止 `..`）。
pub(crate) fn execute_skill_read(package_root: &Path, arguments_json: &str) -> String {
    let args: Value = match serde_json::from_str(arguments_json) {
        Ok(v) => v,
        Err(e) => return format!("error: invalid tool arguments JSON: {e}"),
    };
    let raw = args.get("path").and_then(Value::as_str).unwrap_or("");
    let Some(rel) = sanitize_relative_path(raw) else {
        return "error: path must be a relative path without '..'".to_string();
    };
    let Some(home) = resolve_meilang_skill_home(package_root) else {
        return "error: mei-lang author skill not found".to_string();
    };
    let full = home.join(&rel);
    let Ok(canonical_home) = home.canonicalize() else {
        return "error: cannot canonicalize skill home".to_string();
    };
    let Ok(canonical_file) = full.canonicalize() else {
        return format!("error: file not found: {}", full.display());
    };
    if !canonical_file.starts_with(&canonical_home) {
        return "error: path escapes skill home".to_string();
    }
    match std::fs::read_to_string(&canonical_file) {
        Ok(s) if s.len() > MAX_SKILL_READ_BYTES => {
            format!("error: file too large ({} bytes)", s.len())
        }
        Ok(s) => s,
        Err(e) => format!("error: {e}"),
    }
}

pub(crate) fn skill_list_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "skill_list",
            "description": "List Markdown files under the managed MeiLang author skill directory (SKILL.md and companions). Read-only.",
            "parameters": {
                "type": "object",
                "properties": {}
            }
        }
    })
}

pub(crate) fn skill_read_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "skill_read",
            "description": "Read a UTF-8 Markdown file under the managed MeiLang author skill root. Path is relative to skill root, forward slashes, no '..'.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "e.g. syntax-rules.md or SKILL.md" }
                },
                "required": ["path"]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn skill_read_rejects_escape_from_skill_home() {
        let pkg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let out = execute_skill_read(&pkg, r#"{"path":"../Cargo.toml"}"#);
        assert!(out.starts_with("error:"), "{out}");
    }

    #[test]
    fn skill_list_finds_skill_md_when_repo_present() {
        let pkg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let json = execute_skill_list(&pkg);
        assert!(
            json.contains("SKILL.md"),
            "expected SKILL.md in listing: {json}"
        );
    }
}
