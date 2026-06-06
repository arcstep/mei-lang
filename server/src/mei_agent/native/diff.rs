#![allow(dead_code)] // authoring diff 过滤工具；HTTP 路由已下线，保留供外部桥接复用。

use std::process::Command;

pub(super) fn normalize_diff_rel_path(rel: &str) -> String {
    rel.replace('\\', "/").trim_start_matches("./").to_string()
}

fn paths_match_for_diff_filter(git_path: &str, rel: &str) -> bool {
    let g = normalize_diff_rel_path(git_path);
    let r = normalize_diff_rel_path(rel);
    if g.is_empty() || r.is_empty() {
        return false;
    }
    g == r || g.ends_with(&format!("/{r}")) || r.ends_with(&format!("/{g}"))
}

fn unified_diff_git_line_matches_path(git_line: &str, rel: &str) -> bool {
    let parts: Vec<&str> = git_line.split_whitespace().collect();
    if parts.len() < 4 || parts[0] != "diff" || parts[1] != "--git" {
        return false;
    }
    for token in parts.iter().skip(2) {
        let p = token
            .strip_prefix("a/")
            .or_else(|| token.strip_prefix("b/"))
            .unwrap_or(token);
        if paths_match_for_diff_filter(p, rel) {
            return true;
        }
    }
    false
}

/// 从整工作区 unified diff 中只保留与 `rel` 对应文件的 hunk（兼容旧版「全仓快照」）。
pub(super) fn filter_unified_diff_for_rel_path(diff: &str, rel: &str) -> String {
    let r = normalize_diff_rel_path(rel);
    if r.is_empty() {
        return diff.to_string();
    }
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut keep = false;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            if !current.is_empty() && keep {
                blocks.push(current.join("\n"));
            }
            current.clear();
            keep = unified_diff_git_line_matches_path(line, &r);
            current.push(line);
        } else if !current.is_empty() {
            current.push(line);
        }
    }
    if !current.is_empty() && keep {
        blocks.push(current.join("\n"));
    }
    blocks.join("\n")
}

pub(super) fn git_worktree_diff(root: &std::path::Path, rel: Option<&str>) -> (String, u64, u64) {
    let root_s = root.as_os_str().to_str().unwrap_or("");
    let mut cmd = Command::new("git");
    cmd.args(["-C", root_s, "diff", "--no-color"]);
    if let Some(p) = rel {
        if !p.is_empty() {
            cmd.arg("--");
            cmd.arg(p);
        }
    }
    match cmd.output() {
        Ok(o) if o.status.success() => {
            let diff = String::from_utf8_lossy(&o.stdout).to_string();
            let (a, d) = count_diff_lines(&diff);
            (diff, a, d)
        }
        _ => (String::new(), 0, 0),
    }
}

pub(super) fn count_diff_lines(diff: &str) -> (u64, u64) {
    let mut add = 0u64;
    let mut del = 0u64;
    for line in diff.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            add += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            del += 1;
        }
    }
    (add, del)
}
