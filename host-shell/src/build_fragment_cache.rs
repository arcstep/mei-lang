//! Shared helpers for build node parsing (legacy node query → scene id).

use mei_lang_kernel::{BuildNodeId, BuildNodeKind};

pub fn scene_id_from_build_node(node_raw: &str) -> String {
    let Some(parsed) = BuildNodeId::parse(node_raw) else {
        return String::new();
    };
    match parsed.kind {
        BuildNodeKind::Scene | BuildNodeKind::Route => parsed.key,
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock | BuildNodeKind::Projection => parsed
            .key
            .split('/')
            .next()
            .unwrap_or(parsed.key.as_str())
            .to_string(),
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => parsed
            .key
            .split('#')
            .nth(1)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}
