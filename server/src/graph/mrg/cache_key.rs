use crate::graph::types::stable_hash;

/// Canonical MRG slot cache key (1.3.0): owner | scopeKey | slotRevision | metricSetFingerprint.
pub fn slot_cache_key(
    owner_resource_id: &str,
    scope_key: &str,
    slot_revision: &str,
    metric_set_fingerprint: &str,
) -> String {
    let body = format!(
        "owner={owner_resource_id}\nscope={scope_key}\nsr={slot_revision}\nmetrics={metric_set_fingerprint}"
    );
    format!("slot:{}", stable_hash(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_cache_key_stable() {
        let a = slot_cache_key("ds1", "default", "sr:abc", "mdb:xyz");
        let b = slot_cache_key("ds1", "default", "sr:abc", "mdb:xyz");
        assert_eq!(a, b);
        assert!(a.starts_with("slot:"));
    }
}
