/// Graph HTTP observability APIs (always on in 1.3.0+).
pub fn graph_registry_enabled() -> bool {
    true
}

const GRAPH_REGISTRY_DEDUP_ENV: &str = "MEI_GRAPH_REGISTRY";

/// MCG/MRG registry read/write for compile/eval dedup (always on in 1.3.0+).
pub fn graph_registry_dedup_enabled() -> bool {
    true
}

pub fn graph_registry_dedup_env_override_detected() -> Option<String> {
    std::env::var(GRAPH_REGISTRY_DEDUP_ENV)
        .ok()
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
                Some(format!("{GRAPH_REGISTRY_DEDUP_ENV}={trimmed}"))
            } else {
                None
            }
        })
}
