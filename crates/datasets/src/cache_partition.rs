//! Cache partition wire format shared with `mei-host-core::CachePartitionKey`.
//!
//! Kept local so `mei-lang-datasets` does not depend on host-core. Format must stay:
//! `part:{app_id}/{generation}/{config_digest}|{inner}`

/// Prefix for process-global cache maps so dual instances do not share entries.
pub fn partition_prefix(app_id: &str, generation: &str, config_digest: &str) -> String {
    format!(
        "part:{}/{}/{}|",
        app_id.trim(),
        generation.trim(),
        config_digest.trim()
    )
}

pub fn partition_cache_key(
    app_id: &str,
    generation: &str,
    config_digest: &str,
    inner: &str,
) -> String {
    format!(
        "{}{inner}",
        partition_prefix(app_id, generation, config_digest)
    )
}

pub fn partition_matches_key(
    app_id: &str,
    generation: &str,
    config_digest: &str,
    key: &str,
) -> bool {
    key.starts_with(partition_prefix(app_id, generation, config_digest).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_instance_partition_keys_differ() {
        let a = partition_cache_key("mini-data", "WS-1", "cfg-scoped", "metric|x");
        let b = partition_cache_key("mini-data", "WS-1", "cfg-full", "metric|x");
        assert_ne!(a, b);
        assert!(partition_matches_key(
            "mini-data",
            "WS-1",
            "cfg-scoped",
            a.as_str()
        ));
        assert!(!partition_matches_key(
            "mini-data",
            "WS-1",
            "cfg-scoped",
            b.as_str()
        ));
    }
}
