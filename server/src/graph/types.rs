//! Shared graph identity types — owned by `mei-host-graph`, re-exported for server.

pub use mei_host_graph::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};

pub fn stable_hash(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
