//! 发布期 xlsx → parquet 数据快照。

use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{
    data_snapshot_import_manifest_path, publish_xlsx_data_snapshots_for_paths, resolve_app_root,
};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct PublishDataSnapshotsReport {
    pub app_id: String,
    pub discovered_sources: Vec<String>,
    pub written: Vec<String>,
    pub manifest_path: String,
}

/// 为 app 内常用 xlsx 源生成 `.mei/data-snapshots` parquet 旁路。
pub fn publish_data_snapshots(
    source_root: &Path,
    app_id: &str,
    extra_sources: &[(&str, Option<&str>, usize)],
) -> Result<PublishDataSnapshotsReport> {
    let app_root = resolve_app_root(source_root, app_id);
    let mut owned_sources = extra_sources
        .iter()
        .map(|(path, sheet, header_row)| (path.to_string(), sheet.map(str::to_string), *header_row))
        .collect::<Vec<_>>();
    if owned_sources.is_empty() {
        for source in discover_xlsx_sources(app_root.as_path()) {
            owned_sources.push((source, None, 1));
        }
    }
    let discovered_sources = owned_sources
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
    let sources = owned_sources
        .iter()
        .map(|(path, sheet, header_row)| (path.as_str(), sheet.as_deref(), *header_row))
        .collect::<Vec<_>>();
    let written = publish_xlsx_data_snapshots_for_paths(app_root.as_path(), sources.as_slice())?
        .into_iter()
        .map(|path| path.display().to_string())
        .collect();
    Ok(PublishDataSnapshotsReport {
        app_id: app_id.to_string(),
        discovered_sources,
        written,
        manifest_path: data_snapshot_import_manifest_path(app_root.as_path())
            .display()
            .to_string(),
    })
}

fn discover_xlsx_sources(app_root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in WalkDir::new(app_root)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !should_skip_dir(entry.path()))
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("xlsx")
            && !ext.eq_ignore_ascii_case("xls")
            && !ext.eq_ignore_ascii_case("csv")
        {
            continue;
        }
        let Ok(rel) = path.strip_prefix(app_root) else {
            continue;
        };
        out.push(rel.to_string_lossy().replace('\\', "/"));
    }
    out.sort();
    out.dedup();
    out
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | "node_modules" | "target" | ".mei" | "__pycache__" | "dist" | "build"
            )
        })
}
