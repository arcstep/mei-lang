use std::cell::RefCell;
use std::path::{Path, PathBuf};

thread_local! {
    static PREBUILD_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn set_prebuild_build_root_override(_app_root: &Path, store_dir: Option<&Path>) {
    PREBUILD_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = store_dir.map(|dir| dir.to_path_buf());
    });
}

pub fn clear_prebuild_build_root_override() {
    PREBUILD_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub(super) fn prebuild_build_root_override() -> Option<PathBuf> {
    PREBUILD_OVERRIDE.with(|cell| cell.borrow().clone())
}
