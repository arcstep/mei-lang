use super::prelude::*;
use super::*;

pub(crate) fn copy_runtime_file(
    target_root: &Path,
    source_path: &Path,
    destination_path: &Path,
    force: bool,
) -> Result<EditorRuntimeScaffoldFile> {
    let content = fs::read_to_string(source_path)
        .with_context(|| format!("read runtime asset {}", source_path.display()))?;
    write_file(destination_path, content.as_str(), force).map(|mut file| {
        file.rel_path = destination_path.display().to_string();
        normalize_scaffold_files(target_root, vec![file])
            .into_iter()
            .next()
            .expect("normalized runtime file")
    })
}

pub(crate) fn copy_runtime_tree(
    target_root: &Path,
    source_dir: &Path,
    destination_dir: &Path,
    force: bool,
) -> Result<Vec<EditorRuntimeScaffoldFile>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(source_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let source_path = entry.path();
        let rel = source_path
            .strip_prefix(source_dir)
            .with_context(|| format!("strip runtime asset prefix {}", source_dir.display()))?;
        if rel.as_os_str().is_empty() || entry.file_type().is_dir() {
            continue;
        }
        files.push(copy_runtime_file(
            target_root,
            source_path,
            &destination_dir.join(rel),
            force,
        )?);
    }
    Ok(files)
}

pub(crate) fn write_runtime_projection_files(
    target_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<Vec<EditorRuntimeScaffoldFile>> {
    let mut files = Vec::new();
    for (_, source_path, destination_path) in
        build_runtime_binary_set_for_package_root(package_root, target_root)?
    {
        files.push(copy_runtime_binary(
            target_root,
            &source_path,
            &destination_path,
            force,
        )?);
    }
    let catalog_dir = workspace_catalog_dir(target_root);
    files.push(write_file(
        &catalog_dir.join("capability-catalog.json"),
        &render_workspace_catalog_json(target_root, package_root)?,
        force,
    )?);
    files.push(write_file(
        &catalog_dir.join("author-surface.json"),
        &render_workspace_surface_json(target_root, package_root, "author")?,
        force,
    )?);
    files.push(write_file(
        &catalog_dir.join("access-surface.json"),
        &render_workspace_surface_json(target_root, package_root, "access")?,
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("guides/author-profile.md"),
        &workspace_profiles_dir(target_root).join("author.md"),
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("guides/access-profile.md"),
        &workspace_profiles_dir(target_root).join("access.md"),
        force,
    )?);
    files.extend(copy_runtime_tree(
        target_root,
        &package_root.join("guides/author-skills"),
        &workspace_author_skill_dir(target_root),
        force,
    )?);
    files.extend(copy_runtime_tree(
        target_root,
        &package_root.join("guides/access-skills"),
        &workspace_access_skill_dir(target_root),
        force,
    )?);
    files.extend(copy_runtime_tree(
        target_root,
        &package_root.join("knowledge/editor-runtime"),
        &workspace_knowledge_dir(target_root).join("author"),
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("scripts/mcp/mei-author-stdio-adapter.mjs"),
        &workspace_store_bin_dir(target_root, TOOLCHAIN_VERSION).join("author-mcp-adapter"),
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("scripts/mcp/mei-access-stdio-adapter.mjs"),
        &workspace_store_bin_dir(target_root, TOOLCHAIN_VERSION).join("access-mcp-adapter"),
        force,
    )?);
    files.push(copy_runtime_file(
        target_root,
        &package_root.join("scripts/mcp/mcp-adapter-common.mjs"),
        &workspace_store_bin_dir(target_root, TOOLCHAIN_VERSION).join("mcp-adapter-common.mjs"),
        force,
    )?);
    finalize_toolchain_store_layout(target_root)?;
    Ok(files)
}

pub(crate) fn normalize_scaffold_files(
    target_root: &Path,
    files: Vec<EditorRuntimeScaffoldFile>,
) -> Vec<EditorRuntimeScaffoldFile> {
    files
        .into_iter()
        .map(|mut item| {
            if let Ok(rel) = PathBuf::from(&item.rel_path).strip_prefix(target_root) {
                item.rel_path = rel.to_string_lossy().replace('\\', "/");
            }
            item
        })
        .collect()
}

pub(crate) fn write_common_runtime_files(
    target_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<Vec<EditorRuntimeScaffoldFile>> {
    let mut files = Vec::new();
    let platform = workspace_platform_dir(target_root);
    fs::create_dir_all(&platform).ok();
    files.push(write_file(
        &platform.join("editor-runtime.json"),
        &render_common_runtime_json(package_root)?,
        force,
    )?);
    files.push(write_file(
        &platform.join("knowledge/author-runtime.json"),
        &serde_json::to_string_pretty(&crate::export_knowledge_bundle_for_package_root(
            package_root,
            "author",
            None,
            false,
        )?)?,
        force,
    )?);
    files.push(write_file(
        &platform.join("version.json"),
        &render_workspace_runtime_version_json()?,
        force,
    )?);
    let manifest_json = render_workspace_runtime_manifest_json(package_root)?;
    files.push(write_file(
        &toolchain_store_dir(target_root, TOOLCHAIN_VERSION).join("MANIFEST.json"),
        &manifest_json,
        force,
    )?);
    files.push(write_file(
        &resolve_toolchain_root(target_root).join("MANIFEST.json"),
        &manifest_json,
        force,
    )?);
    files.push(write_file(
        &target_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL),
        &render_workspace_runtime_warmup_manifest_json(target_root)?,
        force,
    )?);
    files.extend(write_runtime_projection_files(
        target_root,
        package_root,
        force,
    )?);
    fs::create_dir_all(target_root.join("deploy")).ok();
    files.push(write_executable_file(
        &target_root.join("deploy/start.sh"),
        render_workspace_start_script(),
        force,
    )?);
    Ok(files)
}

pub fn install_editor_runtime_support_files(
    target_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<EditorRuntimeInstallReport> {
    fs::create_dir_all(target_root)
        .with_context(|| format!("create target root {}", target_root.display()))?;
    crate::workspace_stock::ensure_workspace_stock_materialized(target_root, package_root)?;
    let files = normalize_scaffold_files(
        target_root,
        write_common_runtime_files(target_root, package_root, force)?,
    );
    Ok(EditorRuntimeInstallReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        target_root: target_root.display().to_string(),
        files,
    })
}

/// Bootstrap only the workspace-local author skill tree when missing.
/// Does not install binaries, MCP adapters, or other runtime projection files.
pub fn ensure_workspace_author_skill_package(
    workspace_root: &Path,
    package_root: &Path,
) -> Result<EnsureAuthorSkillReport> {
    let install_dir = workspace_author_skill_dir(workspace_root);
    let entry_file = install_dir.join("SKILL.md");
    if entry_file.is_file() {
        return Ok(EnsureAuthorSkillReport {
            installed: true,
            installed_now: false,
            install_dir: install_dir.display().to_string(),
            file_count: count_markdown_files(&install_dir),
        });
    }
    let source_dir = package_root.join("guides/author-skills");
    anyhow::ensure!(
        source_dir.is_dir(),
        "author skill source tree missing at {}",
        source_dir.display()
    );
    copy_runtime_tree(workspace_root, &source_dir, &install_dir, false)?;
    anyhow::ensure!(
        entry_file.is_file(),
        "author skill install incomplete at {}",
        entry_file.display()
    );
    Ok(EnsureAuthorSkillReport {
        installed: true,
        installed_now: true,
        install_dir: install_dir.display().to_string(),
        file_count: count_markdown_files(&install_dir),
    })
}

pub(crate) fn count_markdown_files(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count()
}

pub(crate) fn render_workspace_runtime_warmup_manifest_json(target_root: &Path) -> Result<String> {
    serde_json::to_string_pretty(&build_workspace_runtime_warmup_manifest(target_root)?)
        .context("serialize workspace warmup manifest")
}

pub(crate) fn build_workspace_runtime_warmup_manifest(target_root: &Path) -> Result<RuntimeWarmupManifest> {
    build_runtime_warmup_manifest(target_root)
}
