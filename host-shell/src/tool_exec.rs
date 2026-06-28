use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolve `mei-compiler` binary.
///
/// Order: `MEI_COMPILER_BIN` → sibling of current exe → `deploy/bin` under workspace → PATH.
pub fn resolve_mei_compiler(workspace: Option<&Path>) -> anyhow::Result<PathBuf> {
    resolve_tool_binary("mei-compiler", "MEI_COMPILER_BIN", workspace)
}

/// Resolve `mei-plug-ds` binary.
pub fn resolve_mei_plug_ds(workspace: Option<&Path>) -> anyhow::Result<PathBuf> {
    resolve_tool_binary("mei-plug-ds", "MEI_PLUG_DS_BIN", workspace)
}

fn resolve_tool_binary(
    name: &str,
    env_var: &str,
    workspace: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    if let Ok(path) = std::env::var(env_var) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        anyhow::bail!("{env_var}={} is not a file", path.display());
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling = parent.join(name);
            if sibling.is_file() {
                return Ok(sibling);
            }
        }
    }

    if let Some(workspace) = workspace {
        let deploy_bin = workspace.join("deploy/bin").join(name);
        if deploy_bin.is_file() {
            return Ok(deploy_bin);
        }
    }

    if let Some(path) = find_on_path(name) {
        return Ok(path);
    }

    anyhow::bail!(
        "{name} not found; set {env_var}, install to deploy/bin, or add to PATH"
    )
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn run_mei_compiler_compile(workspace: &Path, app: &str) -> anyhow::Result<()> {
    let compiler = resolve_mei_compiler(Some(workspace))?;
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let status = Command::new(&compiler)
        .arg("compile")
        .arg("--workspace")
        .arg(&workspace)
        .arg("--app")
        .arg(app)
        .status()
        .map_err(|e| anyhow::anyhow!("spawn {}: {e}", compiler.display()))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "mei-compiler compile failed (exit={})",
            status.code().unwrap_or(-1)
        )
    }
}

pub fn run_mei_plug_ds_warmup(
    workspace: &Path,
    app: &str,
    policy: &str,
    tier: &str,
) -> anyhow::Result<()> {
    let plug_ds = resolve_mei_plug_ds(Some(workspace))?;
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let status = Command::new(&plug_ds)
        .arg("warmup")
        .arg("--workspace")
        .arg(&workspace)
        .arg("--app")
        .arg(app)
        .arg("--policy")
        .arg(policy)
        .arg("--tier")
        .arg(tier)
        .status()
        .map_err(|e| anyhow::anyhow!("spawn {}: {e}", plug_ds.display()))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "mei-plug-ds warmup failed (exit={})",
            status.code().unwrap_or(-1)
        )
    }
}
