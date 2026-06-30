use std::path::Path;

pub use mei_lang_app::load_topbar_menu_context;

/// Back-compat alias for server call sites.
pub(crate) fn load_segment_topbar_menus(source_root: &Path) -> mei_lang_app::TopbarMenuContext {
    load_topbar_menu_context(source_root)
}
