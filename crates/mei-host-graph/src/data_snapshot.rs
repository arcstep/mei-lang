use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use mei_host_core::path_for_log;
use mei_lang_kernel::{
    cell_normalize_rules_from_schema, data_snapshot_import_manifest_path, load_mei_config_for_app,
    merge_cell_normalize_rules, ops_source_entry_to_decl, parquet_snapshot_path,
    publish_xlsx_data_snapshots_for_paths_with_cell_normalizes, resolve_app_root,
    resolve_data_snapshot_import_entry,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PublishDataSnapshotsReport {
    pub app_id: String,
    pub discovered_sources: Vec<String>,
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    pub manifest_path: String,
    pub total_written_bytes: u64,
}

/// Collect xlsx/xls sources from `app.toml` / `app.config.json` ops.sources and metric bundles.
pub fn collect_app_xlsx_sources(
    source_root: &Path,
    app_id: &str,
) -> Result<Vec<(String, Option<String>, usize)>> {
    let app_root = resolve_app_root(source_root, app_id);
    let mut out = BTreeSet::new();

    let config = load_mei_config_for_app(app_root.as_path(), Some(source_root));
    for entry in config.ops.sources.values() {
        push_xlsx_source(&mut out, &ops_source_entry_to_decl(entry));
    }

    let registry = crate::mcg::registry::McgRegistryWriter::load(source_root, app_id);
    let resources =
        crate::metric_hydrate::load_metric_resources_hydrated(app_root.as_path(), &registry)?;
    for resource in resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        push_xlsx_source(&mut out, &dataset.source);
    }

    Ok(out.into_iter().collect())
}

fn push_xlsx_source(
    out: &mut BTreeSet<(String, Option<String>, usize)>,
    source: &mei_lang_kernel::SourceDecl,
) {
    let kind = source.kind.trim().to_ascii_lowercase();
    if !matches!(kind.as_str(), "xlsx" | "xls" | "csv" | "json") {
        return;
    }
    let path = source.path.trim();
    if path.is_empty() {
        return;
    }
    out.insert((
        path.to_string(),
        source
            .sheet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        source.header_row.unwrap_or(1).max(1) as usize,
    ));
}

/// Generate `.mei/data-snapshots` parquet sidecars for configured xlsx sources.
pub fn publish_app_data_snapshots(
    source_root: &Path,
    app_id: &str,
) -> Result<PublishDataSnapshotsReport> {
    let app_root = resolve_app_root(source_root, app_id);
    let required = collect_app_xlsx_sources(source_root, app_id)?;
    let cell_normalizes_by_source = collect_app_cell_normalize_rules(source_root, app_id)?;
    let discovered_sources = required
        .iter()
        .map(|(path, sheet, header_row)| {
            format!(
                "{}|sheet={}|header_row={}",
                path,
                sheet.as_deref().unwrap_or(""),
                header_row
            )
        })
        .collect::<Vec<_>>();

    let mut written = Vec::new();
    let mut skipped = Vec::new();
    let mut total_written_bytes = 0u64;
    for (path, sheet, header_row) in required {
        let refs = [(path.as_str(), sheet.as_deref(), header_row)];
        match publish_xlsx_data_snapshots_for_paths_with_cell_normalizes(
            app_root.as_path(),
            &refs,
            &cell_normalizes_by_source,
        ) {
            Ok(paths) => {
                for snapshot_path in paths {
                    if snapshot_path.is_file() {
                        if let Ok(metadata) = snapshot_path.metadata() {
                            total_written_bytes += metadata.len();
                        }
                    }
                    written.push(path_for_log(source_root, snapshot_path.as_path()));
                }
            }
            Err(error) => {
                skipped.push(format!("{path}: {error:#}"));
            }
        }
    }

    Ok(PublishDataSnapshotsReport {
        app_id: app_id.to_string(),
        discovered_sources,
        written,
        skipped,
        manifest_path: path_for_log(
            source_root,
            data_snapshot_import_manifest_path(app_root.as_path()).as_path(),
        ),
        total_written_bytes,
    })
}

fn collect_app_cell_normalize_rules(
    source_root: &Path,
    app_id: &str,
) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let app_root = resolve_app_root(source_root, app_id);
    let registry = crate::mcg::registry::McgRegistryWriter::load(source_root, app_id);
    let resources =
        crate::metric_hydrate::load_metric_resources_hydrated(app_root.as_path(), &registry)?;
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for resource in resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        let rules = cell_normalize_rules_from_schema(&dataset.schema);
        if rules.is_empty() {
            continue;
        }
        let kind = dataset.source.kind.trim().to_ascii_lowercase();
        if !matches!(kind.as_str(), "xlsx" | "xls" | "csv" | "json") {
            continue;
        }
        let path = dataset.source.path.trim();
        if path.is_empty() {
            continue;
        }
        let sheet = dataset
            .source
            .sheet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("");
        let header_row = dataset.source.header_row.unwrap_or(1).max(1) as usize;
        let key = format!("{path}|sheet={sheet}|header_row={header_row}");
        let entry = out.entry(key).or_default();
        merge_cell_normalize_rules(entry, &rules);
    }
    Ok(out)
}

/// After parquet sidecars exist: require authored `schema.source` to match snapshot columns.
///
/// Non-optional missing physical columns fail the prebuild; optional missing columns warn.
pub fn verify_app_dataset_schema_physical_sources(
    source_root: &Path,
    app_id: &str,
) -> Result<()> {
    let app_root = resolve_app_root(source_root, app_id);
    let registry = crate::mcg::registry::McgRegistryWriter::load(source_root, app_id);
    let resources =
        crate::metric_hydrate::load_metric_resources_hydrated(app_root.as_path(), &registry)?;
    for resource in resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset.schema.is_empty() {
            continue;
        }
        let kind = dataset.source.kind.trim().to_ascii_lowercase();
        if !matches!(kind.as_str(), "xlsx" | "xls" | "csv" | "json") {
            continue;
        }
        let path = dataset.source.path.trim();
        if path.is_empty() {
            continue;
        }
        let header_row = dataset.source.header_row.unwrap_or(1).max(1) as usize;
        let sheet = dataset
            .source
            .sheet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(entry) =
            resolve_data_snapshot_import_entry(app_root.as_path(), path, sheet, header_row)
        else {
            // Existence is covered by verify_required_xlsx_sources; skip here.
            continue;
        };
        if entry.columns.is_empty() {
            continue;
        }
        mei_lang_datasets::ensure_schema_physical_sources(
            dataset.id.as_str(),
            &dataset.schema,
            &entry.columns,
            Some(path),
        )?;
    }
    Ok(())
}

/// Ensure a hot reload has all parquet imports required by sealed Access traffic.
///
/// The common path is metadata-only. XLSX files are republished only when the
/// active generation has no matching manifest entry or parquet artifact.
pub fn ensure_app_data_snapshots(
    source_root: &Path,
    app_id: &str,
) -> Result<Option<PublishDataSnapshotsReport>> {
    let app_root = resolve_app_root(source_root, app_id);
    let required = collect_app_xlsx_sources(source_root, app_id)?;
    let ready = required.iter().all(|(path, sheet, header_row)| {
        resolve_data_snapshot_import_entry(
            app_root.as_path(),
            path.as_str(),
            sheet.as_deref(),
            *header_row,
        )
        .is_some()
            && parquet_snapshot_path(
                app_root.as_path(),
                path.as_str(),
                sheet.as_deref(),
                *header_row,
            )
            .is_some_and(|artifact| artifact.is_file())
    });
    if ready {
        return Ok(None);
    }
    publish_app_data_snapshots(source_root, app_id).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn optional_external_workspace() -> Option<std::path::PathBuf> {
        let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
        let path = std::path::PathBuf::from(raw.trim());
        if path.as_os_str().is_empty() || !path.is_dir() {
            return None;
        }
        Some(path.canonicalize().unwrap_or(path))
    }

    #[test]
    fn collect_xlsx_sources_from_ws_demo_v2_config() {
        let Some(workspace) = optional_external_workspace() else {
            eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
            return;
        };
        if !workspace.join("apps/data-demo/app.config.json").is_file() {
            eprintln!("skip: apps/data-demo missing under MEI_TEST_WORKSPACE");
            return;
        }
        let sources = collect_app_xlsx_sources(workspace.as_path(), "data-demo").expect("collect");
        assert!(
            sources.iter().any(|(path, _, _)| path.contains("预警清单")),
            "expected alert_tracking xlsx in sources: {sources:?}"
        );
    }
}
