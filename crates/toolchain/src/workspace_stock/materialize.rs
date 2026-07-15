use super::prelude::*;
use super::*;

pub fn materialize_workspace_stock(
    source_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<MaterializeReport> {
    fs::create_dir_all(source_root.join(WORKSPACE_HOSTS_DIR_REL))
        .context("create workspace host-state dir")?;
    let components_dest = resolve_components_root(source_root);
    let templates_dest = resolve_templates_root(source_root);
    let authoring_dest = resolve_authoring_root(source_root);
    let components = materialize_tree(
        &stock_components_source(package_root),
        &components_dest,
        force,
    )?;
    let templates = materialize_tree(
        &stock_templates_source(package_root),
        &templates_dest,
        force,
    )?;
    let authoring = materialize_tree(
        &stock_authoring_source(package_root),
        &authoring_dest,
        force,
    )?;
    write_stock_manifest(
        source_root,
        package_root,
        &components_dest,
        &templates_dest,
        &authoring_dest,
    )?;
    Ok(MaterializeReport {
        source_root: source_root.display().to_string(),
        components,
        templates,
        authoring,
    })
}

/// Force-sync workspace stock trees from the platform package (alias for materialize).
pub fn sync_workspace_stock(
    source_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<MaterializeReport> {
    materialize_workspace_stock(source_root, package_root, force)
}

/// Idempotent stock bootstrap: copy platform `stock/*` into the workspace when trees are missing,
/// and on later runs refresh workspace copies when the platform source file is newer.
/// Called from workspace init, runtime install, prebuild, and host startup — not a standalone user workflow.
///
/// When `workspace.json` sets `stock.bootstrap.refresh` to `false` and stock trees already
/// exist, this is a no-op (workspace-owned stock; deleted files stay deleted).
pub fn ensure_workspace_stock_materialized(
    source_root: &Path,
    package_root: &Path,
) -> Result<Option<MaterializeReport>> {
    let needs_initial = workspace_stock_needs_materialize(source_root);
    if !needs_initial
        && !mei_lang_kernel::load_workspace_config(source_root)
            .stock
            .bootstrap
            .refresh
    {
        return Ok(None);
    }
    let report = materialize_workspace_stock(source_root, package_root, false)?;
    if !needs_initial
        && report.components.copied_files == 0
        && report.templates.copied_files == 0
        && report.authoring.copied_files == 0
    {
        return Ok(None);
    }
    Ok(Some(report))
}
pub(crate) fn workspace_stock_needs_materialize(source_root: &Path) -> bool {
    !stock_tree_ready(&resolve_authoring_root(source_root))
        || !stock_tree_ready(&resolve_components_root(source_root))
        || !stock_tree_ready(&resolve_templates_root(source_root))
}

pub(crate) fn stock_tree_ready(path: &Path) -> bool {
    path.is_dir()
        && fs::read_dir(path)
            .ok()
            .and_then(|mut entries| entries.next())
            .is_some()
}

pub(crate) fn materialize_tree(
    from: &Path,
    to: &Path,
    force: bool,
) -> Result<MaterializeDirReport> {
    if !from.is_dir() {
        anyhow::bail!(
            "stock source `{}` is missing; ensure mei-lang ships stock/components and stock/templates",
            from.display()
        );
    }
    let mut copied_files = 0usize;
    let mut skipped_files = 0usize;
    let mut overwritten_files = 0usize;
    for entry in WalkDir::new(from).into_iter().filter_map(Result::ok) {
        let src = entry.path();
        let rel = src
            .strip_prefix(from)
            .context("strip stock prefix")?
            .to_path_buf();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = to.join(&rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest).with_context(|| format!("create dir {}", dest.display()))?;
            continue;
        }
        if dest.exists() && !force {
            let should_refresh = match (fs::metadata(src), fs::metadata(&dest)) {
                (Ok(src_meta), Ok(dest_meta)) => {
                    match (src_meta.modified(), dest_meta.modified()) {
                        (Ok(src_mtime), Ok(dest_mtime)) => src_mtime > dest_mtime,
                        _ => false,
                    }
                }
                _ => false,
            };
            if !should_refresh {
                skipped_files += 1;
                continue;
            }
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent {}", parent.display()))?;
        }
        let existed = dest.exists();
        fs::copy(src, &dest)
            .with_context(|| format!("copy stock file {} -> {}", src.display(), dest.display()))?;
        copied_files += 1;
        if existed {
            overwritten_files += 1;
        }
    }
    Ok(MaterializeDirReport {
        from: from.display().to_string(),
        to: to.display().to_string(),
        copied_files,
        skipped_files,
        overwritten_files,
    })
}
