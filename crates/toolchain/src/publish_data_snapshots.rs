//! 发布期 xlsx → parquet 数据快照。

use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{publish_xlsx_data_snapshots_for_paths, resolve_app_root};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PublishDataSnapshotsReport {
    pub app_id: String,
    pub written: Vec<String>,
}

/// 为 app 内常用 xlsx 源生成 `.mei/data-snapshots` parquet 旁路。
pub fn publish_data_snapshots(
    source_root: &Path,
    app_id: &str,
    extra_sources: &[(&str, Option<&str>, usize)],
) -> Result<PublishDataSnapshotsReport> {
    let app_root = resolve_app_root(source_root, app_id);
    let mut sources: Vec<(&str, Option<&str>, usize)> = extra_sources.to_vec();
    if sources.is_empty() {
        sources.push(("upload/5.行政检查结果清单.xlsx", None, 1));
        sources.push(("upload/8.行政处罚结果清单.xlsx", None, 1));
    }
    let written = publish_xlsx_data_snapshots_for_paths(app_root.as_path(), sources.as_slice())?
        .into_iter()
        .map(|path| path.display().to_string())
        .collect();
    Ok(PublishDataSnapshotsReport {
        app_id: app_id.to_string(),
        written,
    })
}
