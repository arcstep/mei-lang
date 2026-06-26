use super::prelude::*;

/// Stable revision string for promote gates (`stockRevision` in links state).
pub fn workspace_stock_revision(source_root: &Path) -> Option<String> {
    let manifest_path = resolve_stock_root(source_root).join(STOCK_MANIFEST_FILENAME);
    let raw = fs::read_to_string(manifest_path).ok()?;
    let manifest: StockManifest = serde_json::from_str(&raw).ok()?;
    let mut hasher = DefaultHasher::new();
    manifest.components.paths_hash.hash(&mut hasher);
    manifest.templates.paths_hash.hash(&mut hasher);
    manifest.authoring.paths_hash.hash(&mut hasher);
    Some(format!(
        "stock-v{STOCK_MANIFEST_SCHEMA_VERSION}-{:016x}",
        hasher.finish()
    ))
}

pub(crate) fn write_stock_manifest(
    source_root: &Path,
    package_root: &Path,
    components_root: &Path,
    templates_root: &Path,
    authoring_root: &Path,
) -> Result<()> {
    let stock_root = resolve_stock_root(source_root);
    fs::create_dir_all(&stock_root).context("create stock root for manifest")?;
    let manifest = StockManifest {
        schema_version: STOCK_MANIFEST_SCHEMA_VERSION,
        materialized_at: Utc::now().to_rfc3339(),
        bootstrap_source: BOOTSTRAP_SOURCE_PLATFORM_DEFAULT.to_string(),
        package_root: package_root.display().to_string(),
        components: fingerprint_tree(components_root)?,
        templates: fingerprint_tree(templates_root)?,
        authoring: fingerprint_tree(authoring_root)?,
    };
    let manifest_path = stock_root.join(STOCK_MANIFEST_FILENAME);
    let json = serde_json::to_string_pretty(&manifest).context("serialize STOCK.json")?;
    fs::write(&manifest_path, format!("{json}\n"))
        .with_context(|| format!("write {}", manifest_path.display()))?;
    Ok(())
}

pub(crate) fn fingerprint_tree(root: &Path) -> Result<StockTreeFingerprint> {
    if !root.is_dir() {
        return Ok(StockTreeFingerprint {
            file_count: 0,
            paths_hash: None,
        });
    }
    let mut rel_paths = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .context("strip fingerprint prefix")?
            .to_string_lossy()
            .replace('\\', "/");
        rel_paths.push(rel);
    }
    rel_paths.sort();
    let file_count = rel_paths.len();
    let paths_hash = if file_count == 0 {
        None
    } else {
        let mut hasher = DefaultHasher::new();
        rel_paths.join("\n").hash(&mut hasher);
        Some(format!("{:016x}", hasher.finish()))
    };
    Ok(StockTreeFingerprint {
        file_count,
        paths_hash,
    })
}
