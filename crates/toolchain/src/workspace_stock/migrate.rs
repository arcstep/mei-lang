use super::prelude::*;

pub fn migrate_workspace_stock_paths(
    source_root: &Path,
) -> Result<MigrateWorkspaceStockPathsReport> {
    let legacy_stock = source_root.join(LEGACY_STOCK_DIR);
    let stock_root = resolve_stock_root(source_root);
    let mut renamed_legacy_stock = false;
    if legacy_stock.is_dir() {
        if stock_root.exists() {
            anyhow::bail!(
                "both `{}` and `{}` exist; merge or remove legacy `.stock` manually before migrate",
                legacy_stock.display(),
                stock_root.display()
            );
        }
        fs::rename(&legacy_stock, &stock_root).with_context(|| {
            format!(
                "rename legacy stock {} -> {}",
                legacy_stock.display(),
                stock_root.display()
            )
        })?;
        renamed_legacy_stock = true;
    }

    let examples_dir = resolve_authoring_root(source_root).join("examples");
    let mut updated_example_files = Vec::new();
    if examples_dir.is_dir() {
        for entry in fs::read_dir(&examples_dir)
            .with_context(|| format!("read authoring examples dir {}", examples_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("mei") {
                continue;
            }
            let original = fs::read_to_string(&path)
                .with_context(|| format!("read example mei {}", path.display()))?;
            let migrated = migrate_example_mei_stock_paths(&original);
            if migrated != original {
                fs::write(&path, migrated)
                    .with_context(|| format!("write migrated example {}", path.display()))?;
                updated_example_files.push(path.display().to_string());
            }
        }
    }

    Ok(MigrateWorkspaceStockPathsReport {
        renamed_legacy_stock,
        updated_example_files,
    })
}

pub(crate) fn migrate_example_mei_stock_paths(content: &str) -> String {
    content
        .replace("../.stock/", "../../stock/")
        .replace(".stock/", "stock/")
}
