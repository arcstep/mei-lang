use std::path::{Path, PathBuf};

/// `{workspace}/deploy/runtime/training`
pub fn training_root(workspace: &Path) -> PathBuf {
    workspace.join("deploy/runtime/training")
}

/// `{workspace}/deploy/runtime/training/{app_id}/{username}`
pub fn training_learner_dir(workspace: &Path, app_id: &str, username: &str) -> PathBuf {
    training_root(workspace)
        .join(sanitize_path_segment(app_id))
        .join(sanitize_username(username))
}

pub fn sanitize_username(username: &str) -> String {
    sanitize_path_segment(username)
}

fn sanitize_path_segment(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "_anonymous".to_string();
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_user".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_username() {
        assert_eq!(sanitize_username("alice"), "alice");
        assert_eq!(sanitize_username("a/../b"), "a_.._b");
        assert_eq!(sanitize_username("  "), "_anonymous");
    }
}
