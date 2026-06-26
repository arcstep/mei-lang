/// Graph HTTP observability APIs (default on unless `MEI_GRAPH_REGISTRY=0`).
pub fn graph_registry_enabled() -> bool {
    graph_registry_dedup_enabled()
}

/// MCG/MRG registry read/write for compile/eval dedup (default on unless explicitly disabled).
pub fn graph_registry_dedup_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| match std::env::var("MEI_GRAPH_REGISTRY") {
        Ok(value) => {
            let trimmed = value.trim();
            !(trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
        }
        Err(_) => true,
    })
}
