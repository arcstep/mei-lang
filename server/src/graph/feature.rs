/// `MEI_GRAPH_REGISTRY=1` enables MCG/MRG registry read/write (default off).
pub fn graph_registry_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MEI_GRAPH_REGISTRY")
            .map(|value| {
                let trimmed = value.trim();
                trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
            })
            .unwrap_or(false)
    })
}
