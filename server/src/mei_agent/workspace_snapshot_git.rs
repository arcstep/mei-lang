//! OpenCode 风格的独立快照 Git：`GIT_DIR` 在 `.mei/snapshot/git`，`GIT_WORK_TREE` 为宿主工作区。
//! 用于 `write-tree` 记录整树哈希、`read-tree` + `checkout-index` 恢复工作区文件（不触碰主仓库 `.git`）。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

const SNAPSHOT_GIT_REL: &str = ".mei/snapshot/git";
/// 会话创建时写入的基线锚点（无 assistant 树快照时的 revert 目标）。
pub(crate) const SESSION_BASELINE_ANCHOR: &str = "__session_baseline__";

pub(crate) struct WorkspaceSnapshotGit {
    worktree: PathBuf,
    git_dir: PathBuf,
}

impl WorkspaceSnapshotGit {
    pub fn new(worktree: PathBuf) -> Self {
        let git_dir = worktree.join(SNAPSHOT_GIT_REL);
        Self { worktree, git_dir }
    }

    fn git_dir_os(&self) -> std::ffi::OsString {
        self.git_dir.as_os_str().to_owned()
    }

    fn worktree_os(&self) -> std::ffi::OsString {
        self.worktree.as_os_str().to_owned()
    }

    /// 避免快照库把自身 `.mei/snapshot/` 纳入索引。
    fn ensure_info_exclude(&self) -> Result<()> {
        let info = self.git_dir.join("info");
        std::fs::create_dir_all(&info).with_context(|| format!("create {}", info.display()))?;
        let exclude = info.join("exclude");
        let line = ".mei/snapshot/\n";
        let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
        if existing.contains(".mei/snapshot/") {
            return Ok(());
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&exclude)
            .with_context(|| format!("open {}", exclude.display()))?;
        f.write_all(line.as_bytes())
            .with_context(|| format!("write {}", exclude.display()))?;
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
