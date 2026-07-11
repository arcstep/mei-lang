use super::prelude::*;
use super::*;

pub(crate) fn runtime_source_revision() -> RuntimeSourceRevision {
    RuntimeSourceRevision {
        git_commit: GIT_COMMIT_FULL.to_string(),
        git_commit_short: GIT_COMMIT_SHORT.to_string(),
        dirty: GIT_DIRTY == "true",
    }
}

pub(crate) fn runtime_bundle_id() -> String {
    format!("mei-lang-{BUILD_VERSION}")
}

pub(crate) fn now_timestamp_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(crate) fn binary_file_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

pub(crate) fn current_exe_candidates(base: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        let file_name = binary_file_name(base);
        let current_name = current_exe
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if current_name == file_name {
            candidates.push(current_exe.clone());
        }
        if let Some(bin_dir) = current_exe.parent() {
            candidates.push(bin_dir.join(&file_name));
            if bin_dir.file_name().and_then(|value| value.to_str()) == Some("deps") {
                if let Some(parent) = bin_dir.parent() {
                    candidates.push(parent.join(&file_name));
                }
            }
        }
    }
    candidates
}

pub(crate) fn package_root_binary_candidates(package_root: &Path, base: &str) -> Vec<PathBuf> {
    let file_name = binary_file_name(base);
    let mut candidates = Vec::new();
    if package_root.ends_with(Path::new("share/mei")) {
        if let Some(prefix) = package_root.parent().and_then(|path| path.parent()) {
            candidates.push(prefix.join("bin").join(&file_name));
        }
    }
    candidates.push(package_root.join("target/debug").join(&file_name));
    candidates.push(package_root.join("target/release").join(&file_name));
    candidates
}

pub(crate) fn try_resolve_runtime_binary(
    package_root: &Path,
    env_key: &str,
    base: &str,
) -> Option<PathBuf> {
    if let Ok(raw) = std::env::var(env_key) {
        let candidate = PathBuf::from(raw);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    current_exe_candidates(base)
        .into_iter()
        .chain(package_root_binary_candidates(package_root, base))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn build_runtime_binary_set_for_package_root(
    package_root: &Path,
    target_root: &Path,
) -> Result<Vec<(&'static str, PathBuf, PathBuf)>> {
    let version = TOOLCHAIN_VERSION;
    let store_bin = workspace_store_bin_dir(target_root, version);
    let mut binaries = vec![
        (
            "mei-toolchain",
            PathBuf::new(),
            store_bin.join(binary_file_name("mei-toolchain")),
        ),
        (
            "mei-lsp",
            PathBuf::new(),
            store_bin.join(binary_file_name("mei-lsp")),
        ),
        (
            "mei-host-web",
            PathBuf::new(),
            store_bin.join(binary_file_name("mei-host-web")),
        ),
    ];
    let env_keys = [
        ("MEI_TOOLCHAIN_BIN", "mei-toolchain"),
        ("MEI_LSP_BIN", "mei-lsp"),
        ("MEI_HOST_WEB_BIN", "mei-host-web"),
    ];
    let missing = env_keys
        .iter()
        .filter_map(|(env_key, base)| {
            try_resolve_runtime_binary(package_root, env_key, base)
                .map(|path| ((*base).to_string(), path))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if missing.len() != env_keys.len() && package_root.join("Cargo.toml").is_file() {
        let status = Command::new("cargo")
            .arg("build")
            .arg("-p")
            .arg("mei-lang-server")
            .arg("-p")
            .arg("mei-lang-lsp")
            .arg("--bin")
            .arg("mei-toolchain")
            .arg("--bin")
            .arg("mei-host-web")
            .arg("--bin")
            .arg("mei-lsp")
            .current_dir(package_root)
            .status()
            .with_context(|| format!("build runtime binaries under {}", package_root.display()))?;
        if !status.success() {
            anyhow::bail!(
                "failed to build workspace-local runtime binaries from {}",
                package_root.display()
            );
        }
    }
    for (name, source, destination) in &mut binaries {
        let env_key = match *name {
            "mei-toolchain" => "MEI_TOOLCHAIN_BIN",
            "mei-lsp" => "MEI_LSP_BIN",
            "mei-host-web" => "MEI_HOST_WEB_BIN",
            _ => unreachable!(),
        };
        *source = try_resolve_runtime_binary(package_root, env_key, name).ok_or_else(|| {
            anyhow::anyhow!(
                "cannot locate required runtime binary `{}`; checked current executable siblings, {} and {}",
                name,
                package_root.join("target/debug").display(),
                package_root.join("target/release").display()
            )
        })?;
        *destination = store_bin.join(binary_file_name(name));
    }
    Ok(binaries)
}

pub(crate) fn finalize_toolchain_store_layout(target_root: &Path) -> Result<()> {
    apply_toolchain_store_symlinks(target_root, TOOLCHAIN_VERSION)?;
    record_toolchain_install_links(target_root, TOOLCHAIN_VERSION)?;
    Ok(())
}
