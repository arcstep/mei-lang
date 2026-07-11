use std::fs;
use std::path::Path;

use mei_lang_kernel::resolve_app_root;

/// Remove legacy on-disk page-render-cache directories (abolished; one-time hygiene).
pub fn clear_legacy_page_render_cache_for_app(workspace_root: &Path, app_id: &str) -> usize {
    crate::thin_shell_page_cache::clear_for_app(app_id);
    let app_root = resolve_app_root(workspace_root, app_id);
    let disk_dir =
        mei_lang_kernel::resolve_app_var_root(app_root.as_path()).join("page-render-cache");
    if !disk_dir.is_dir() {
        return 0;
    }
    let cleared = fs::read_dir(&disk_dir)
        .map(|entries| entries.flatten().count())
        .unwrap_or(0);
    let _ = fs::remove_dir_all(&disk_dir);
    cleared
}

pub fn clear_legacy_page_render_cache_for_apps(workspace_root: &Path, app_ids: &[String]) -> usize {
    let mut cleared = 0usize;
    for app_id in app_ids {
        cleared += clear_legacy_page_render_cache_for_app(workspace_root, app_id.as_str());
    }
    cleared
}
