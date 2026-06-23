//! OpenCode 风格的独立快照 Git：`GIT_DIR` 在 `.mei/local/agent/snapshot/git`，`GIT_WORK_TREE` 为宿主工作区。
//! 用于 `write-tree` 记录整树哈希、`read-tree` + `checkout-index` 恢复工作区文件（不触碰主仓库 `.git`）。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::{
    LEGACY_WORKSPACE_SNAPSHOT_DIR_REL, WORKSPACE_SNAPSHOT_DIR_REL, WORKSPACE_SNAPSHOT_GIT_REL,
};

/// 会话创建时写入的基线锚点（无 assistant 树快照时的 revert 目标）。
pub(crate) const SESSION_BASELINE_ANCHOR: &str = "__session_baseline__";

pub(crate) struct WorkspaceSnapshotGit {
    worktree: PathBuf,
    git_dir: PathBuf,
}

impl WorkspaceSnapshotGit {
    pub fn new(worktree: PathBuf) -> Result<Self> {
        migrate_legacy_snapshot_dir(&worktree)?;
        let git_dir = worktree.join(WORKSPACE_SNAPSHOT_GIT_REL);
        Ok(Self { worktree, git_dir })
    }

    fn git_dir_os(&self) -> std::ffi::OsString {
        self.git_dir.as_os_str().to_owned()
    }

    fn worktree_os(&self) -> std::ffi::OsString {
        self.worktree.as_os_str().to_owned()
    }

    /// 避免快照库把自身 `.mei/local/agent/snapshot/` 纳入索引。
    fn ensure_info_exclude(&self) -> Result<()> {
        let info = self.git_dir.join("info");
        std::fs::create_dir_all(&info).with_context(|| format!("create {}", info.display()))?;
        let exclude = info.join("exclude");
        let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
        for line in [".mei/local/agent/snapshot/\n", ".mei/snapshot/\n"] {
            if existing.contains(line.trim_end()) {
                continue;
            }
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&exclude)
                .with_context(|| format!("open {}", exclude.display()))?;
            f.write_all(line.as_bytes())
                .with_context(|| format!("write {}", exclude.display()))?;
        }
        Ok(())
    }

    pub fn ensure_initialized(&self) -> Result<()> {
        std::fs::create_dir_all(&self.git_dir)
            .with_context(|| format!("create {}", self.git_dir.display()))?;
        let refs = self.git_dir.join("refs").join("heads");
        if !refs.exists() {
            let wt = self
                .worktree
                .as_os_str()
                .to_str()
                .ok_or_else(|| anyhow!("worktree path is not valid UTF-8"))?;
            let gd = self
                .git_dir
                .as_os_str()
                .to_str()
                .ok_or_else(|| anyhow!("GIT_DIR path is not valid UTF-8"))?;
            let st = Command::new("git")
                .current_dir(wt)
                .env("GIT_DIR", gd)
                .env("GIT_WORK_TREE", wt)
                .args(["init", "--quiet"])
                .status()
                .context("spawn git init")?;
            if !st.success() {
                return Err(anyhow!("git init failed (exit {:?})", st.code()));
            }
            let _ = Command::new("git")
                .env("GIT_DIR", gd)
                .args(["config", "core.autocrlf", "false"])
                .status();
        }
        self.ensure_info_exclude()
    }

    /// 将当前工作区索引为树并返回 `write-tree` 哈希（40 位十六进制）。
    pub fn track(&self) -> Result<String> {
        self.ensure_initialized()?;
        let wt = self.worktree_os();
        let gd = self.git_dir_os();
        let add = Command::new("git")
            .current_dir(&self.worktree)
            .env("GIT_DIR", &gd)
            .env("GIT_WORK_TREE", &wt)
            .args(["add", "-A"])
            .status()
            .context("spawn git add")?;
        if !add.success() {
            return Err(anyhow!("git add -A failed (exit {:?})", add.code()));
        }
        let out = Command::new("git")
            .current_dir(&self.worktree)
            .env("GIT_DIR", &gd)
            .env("GIT_WORK_TREE", &wt)
            .args(["write-tree"])
            .output()
            .context("spawn git write-tree")?;
        if !out.status.success() {
            return Err(anyhow!(
                "git write-tree failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !is_valid_object_name(&hash) {
            return Err(anyhow!("unexpected write-tree output: {:?}", hash));
        }
        Ok(hash)
    }

    /// 将工作区文件恢复为给定树（`read-tree` + `checkout-index -a -f`）。
    #[allow(dead_code)] // authoring revert 已下线；保留供外部 Agent 桥接。
    pub fn restore_worktree(&self, tree_hash: &str) -> Result<()> {
        let th = tree_hash.trim();
        if th.is_empty() {
            return Err(anyhow!("empty tree hash"));
        }
        if !is_valid_object_name(th) {
            return Err(anyhow!("invalid tree hash: {:?}", th));
        }
        self.ensure_initialized()?;
        let wt = self.worktree_os();
        let gd = self.git_dir_os();
        let r1 = Command::new("git")
            .current_dir(&self.worktree)
            .env("GIT_DIR", &gd)
            .env("GIT_WORK_TREE", &wt)
            .args(["read-tree", th])
            .status()
            .context("spawn git read-tree")?;
        if !r1.success() {
            return Err(anyhow!("git read-tree failed (exit {:?})", r1.code()));
        }
        let r2 = Command::new("git")
            .current_dir(&self.worktree)
            .env("GIT_DIR", &gd)
            .env("GIT_WORK_TREE", &wt)
            .args(["checkout-index", "-a", "-f"])
            .status()
            .context("spawn git checkout-index")?;
        if !r2.success() {
            return Err(anyhow!("git checkout-index failed (exit {:?})", r2.code()));
        }
        Ok(())
    }
}

fn is_valid_object_name(s: &str) -> bool {
    let b = s.as_bytes();
    (4..=64).contains(&b.len()) && b.iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

fn migrate_legacy_snapshot_dir(worktree: &Path) -> Result<()> {
    let modern = worktree.join(WORKSPACE_SNAPSHOT_DIR_REL);
    if modern.exists() {
        return Ok(());
    }
    let legacy = worktree.join(LEGACY_WORKSPACE_SNAPSHOT_DIR_REL);
    if !legacy.exists() {
        return Ok(());
    }
    if let Some(parent) = modern.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::rename(&legacy, &modern).with_context(|| {
        format!(
            "move legacy snapshot dir {} -> {}",
            legacy.display(),
            modern.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::LEGACY_WORKSPACE_SNAPSHOT_GIT_REL;

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let unique = format!(
                "{prefix}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            let path = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn new_migrates_legacy_snapshot_directory() {
        let temp = TempDirGuard::new("mei-snapshot");
        let legacy_git_dir = temp.path.join(LEGACY_WORKSPACE_SNAPSHOT_GIT_REL);
        std::fs::create_dir_all(&legacy_git_dir).expect("create legacy snapshot git dir");
        std::fs::write(legacy_git_dir.join("HEAD"), "ref: refs/heads/main\n")
            .expect("write legacy head");

        let snapshot = WorkspaceSnapshotGit::new(temp.path.clone()).expect("create snapshot git");

        assert_eq!(snapshot.git_dir, temp.path.join(WORKSPACE_SNAPSHOT_GIT_REL));
        assert!(temp.path.join(WORKSPACE_SNAPSHOT_GIT_REL).is_dir());
        assert!(!temp.path.join(LEGACY_WORKSPACE_SNAPSHOT_DIR_REL).exists());
    }
}
