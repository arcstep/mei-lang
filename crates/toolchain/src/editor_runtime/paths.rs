use super::prelude::*;

pub(crate) fn workspace_platform_dir(workspace_root: &Path) -> PathBuf {
    resolve_workspace_runtime_root(workspace_root).join("platform")
}

pub(crate) fn workspace_runtime_bin_dir(workspace_root: &Path) -> PathBuf {
    resolve_toolchain_root(workspace_root).join("bin")
}

pub(crate) fn workspace_store_bin_dir(workspace_root: &Path, toolchain_version: &str) -> PathBuf {
    toolchain_store_dir(workspace_root, toolchain_version).join("bin")
}

pub(crate) fn workspace_catalog_dir(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("catalog")
}

pub(crate) fn workspace_profiles_dir(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("profiles")
}

pub(crate) fn workspace_knowledge_dir(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("knowledge")
}

pub(crate) fn workspace_author_skill_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(crate::meilang_author_skill_package().install_dir_rel)
}

pub(crate) fn workspace_access_skill_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(crate::meilang_access_skill_package().install_dir_rel)
}

