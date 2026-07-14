use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use mei_host_core::path_for_log;
use mei_lang_kernel::{
    data_snapshot_import_manifest_path, load_mei_config_for_app, ops_source_entry_to_decl,
    publish_xlsx_data_snapshots_for_paths, resolve_app_root,
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
    if !matches!(kind.as_str(), "xlsx" | "xls") {
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
        match publish_xlsx_data_snapshots_for_paths(app_root.as_path(), &refs) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_xlsx_sources_from_ws_demo_v2_config() {
        let workspace =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
        if !workspace.join("apps/data-demo/app.config.json").is_file() {
            return;
        }
        let sources = collect_app_xlsx_sources(workspace.as_path(), "data-demo").expect("collect");
        assert!(
            sources.iter().any(|(path, _, _)| path.contains("预警清单")),
            "expected alert_tracking xlsx in sources: {sources:?}"
        );
    }
}
