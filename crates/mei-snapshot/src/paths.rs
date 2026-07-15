use std::fs;
use std::path::{Path, PathBuf};

/// Resolve `apps/{app}/env/current` when present, else newest `env/WS-*` directory.
pub fn resolve_app_env_root(workspace: &Path, app_id: &str) -> anyhow::Result<PathBuf> {
    let env_dir = workspace.join("apps").join(app_id).join("env");
    if !env_dir.is_dir() {
        anyhow::bail!("app env dir missing: {}", env_dir.display());
    }
    let current = env_dir.join("current");
    if current.exists() {
        let resolved = fs::canonicalize(&current).unwrap_or(current);
        return Ok(resolved);
    }
    let mut gens: Vec<PathBuf> = fs::read_dir(&env_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("WS-"))
        })
        .collect();
    gens.sort();
    gens.pop()
        .ok_or_else(|| anyhow::anyhow!("no env generation under {}", env_dir.display()))
}

/// Prefer `build/exchange/{app}.meibundle`, else any `*.meibundle` under exchange.
pub fn resolve_bundle_path(env_root: &Path, app_id: &str) -> anyhow::Result<PathBuf> {
    let exchange = env_root.join("build").join("exchange");
    let preferred = exchange.join(format!("{app_id}.meibundle"));
    if preferred.is_file() {
        return Ok(preferred);
    }
    let mut found: Vec<PathBuf> = fs::read_dir(&exchange)
        .map_err(|e| anyhow::anyhow!("exchange dir {}: {e}", exchange.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e == "meibundle")
        })
        .collect();
    found.sort();
    found.pop().ok_or_else(|| {
        anyhow::anyhow!(
            "no .meibundle under {}; run compile/prebuild first",
            exchange.display()
        )
    })
}
