use std::path::Path;

use chrono::{DateTime, Local};
use mei_lang_app::SourcePanelMeta;
use mei_lang_kernel::{
    compile_app_with_options, CompileOptions, WorkspaceAppMeta,
};
use std::fs;

pub(crate) fn source_panel_meta(source_path: &Path, source: &str) -> SourcePanelMeta {
    let line_count = if source.is_empty() {
        0
    } else {
        source.split('\n').count()
    };
    let char_count = source.chars().count();
    let last_modified_label = fs::metadata(source_path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|modified| {
            let modified: DateTime<Local> = modified.into();
            modified.format("%Y-%m-%d %H:%M:%S").to_string()
        });
    SourcePanelMeta {
        line_count,
        char_count,
        last_modified_label,
    }
}

pub(crate) fn choose_default_app<'a>(
    source_root: &Path,
    apps: &'a [WorkspaceAppMeta],
) -> Option<&'a WorkspaceAppMeta> {
    for app in apps {
        if compile_app_with_options(source_root, &app.id, CompileOptions::default()).is_ok() {
            return Some(app);
        }
        tracing::warn!(app_id = %app.id, "skip broken app as default landing target");
    }
    None
}
