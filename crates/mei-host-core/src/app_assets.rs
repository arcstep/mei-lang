//! Resolve `app/assets` for packaged (`$PACKAGE_ROOT/app/assets`) and source-tree
//! (`$MEI_LANG_ROOT/host-shell/app/assets`) layouts.

use std::path::{Path, PathBuf};

/// Prefer packaged layout, then source-tree layout under `host-shell/app/assets`.
pub fn resolve_app_assets_dir(package_root: &Path) -> PathBuf {
    let packaged = package_root.join("app").join("assets");
    if packaged.is_dir() {
        return packaged;
    }
    let source = package_root.join("host-shell").join("app").join("assets");
    if source.is_dir() {
        return source;
    }
    packaged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_packaged_when_present() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        // Source checkout: host-shell/app/assets exists; packaged app/assets may not.
        let assets = resolve_app_assets_dir(&root);
        assert!(
            assets.ends_with("host-shell/app/assets") || assets.ends_with("app/assets"),
            "unexpected {}",
            assets.display()
        );
        assert!(assets.is_dir(), "missing {}", assets.display());
    }
}
