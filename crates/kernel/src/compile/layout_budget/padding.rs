/// Fixed section title bar height (titled_shell / section_shell).
pub const TITLE_BAR_HEIGHT_PX: f64 = 54.0;

pub fn padding_profile_vertical_px(profile: &str) -> Option<(f64, f64)> {
    let (top, bottom) = match profile {
        "dense_strip_100" => (8.0, 2.0),
        "compact_ai" => (8.0, 3.0),
        "compact" => (8.0, 6.0),
        "dense" => (8.0, 4.0),
        "none" => (0.0, 0.0),
        _ => return None,
    };
    Some((top, bottom))
}

pub fn padding_profile_css(profile: &str) -> Option<&'static str> {
    match profile {
        "dense_strip_100" => Some("8px 4px 2px 4px"),
        "compact_ai" => Some("8px 6px 3px 6px"),
        "compact" => Some("8px 6px 6px 6px"),
        "dense" => Some("8px 4px 4px 4px"),
        "none" => Some("0"),
        _ => None,
    }
}