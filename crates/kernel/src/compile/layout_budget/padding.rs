pub fn padding_profile_css(profile: &str) -> Option<&'static str> {
    match profile {
        "dense_strip_100" => Some("8px 4px 2px 4px"),
        "compact_ai" => Some("8px 6px 3px 6px"),
        "compact" => Some("8px 6px 6px 6px"),
        "dense" => Some("8px 4px 4px 4px"),
        // 0332 层级边距规范：单位「1」= 4px（section / area 内边距）
        "space_1" => Some("4px"),
        "none" => Some("0"),
        _ => None,
    }
}
