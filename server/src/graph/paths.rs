use std::path::{Path, PathBuf};

use mei_lang_kernel::resolve_workspace_graph_root;

pub fn resolve_graph_root(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_workspace_graph_root(source_root, app_id)
}

pub fn mcg_registry_path(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_graph_root(source_root, app_id).join("mcg-registry.json")
}

pub fn mrg_registry_path(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_graph_root(source_root, app_id).join("mrg-registry.json")
}

pub fn bridge_path(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_graph_root(source_root, app_id).join("bridge.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_graph_root_layout() {
        let root = Path::new("/ws");
        assert_eq!(
            resolve_graph_root(root, "zhifa"),
            PathBuf::from("/ws/runtime/platform/graphs/zhifa")
        );
    }
}
