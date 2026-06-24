use std::path::{Path, PathBuf};

/// Per-app graph registry root: `{workspace}/.mei/graphs/{app_id}/`.
pub fn resolve_graph_root(source_root: &Path, app_id: &str) -> PathBuf {
    source_root
        .join(".mei")
        .join("graphs")
        .join(app_id.trim())
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

pub fn scene_payload_artifact_dir(app_root: &Path) -> PathBuf {
    app_root.join(".mei").join("graph").join("payloads").join("scene")
}

pub fn panel_contract_artifact_dir(app_root: &Path) -> PathBuf {
    app_root
        .join(".mei")
        .join("graph")
        .join("payloads")
        .join("panel")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_graph_root_layout() {
        let root = Path::new("/ws");
        assert_eq!(
            resolve_graph_root(root, "zhifa"),
            PathBuf::from("/ws/.mei/graphs/zhifa")
        );
    }
}
