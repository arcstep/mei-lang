

pub(crate) const TOKEN_DEFINITION_ROOTS: &[&str] = &["tokens", "font"];

pub(crate) const COLOR_REF_KEYS: &[&str] = &["color"];
pub(crate) const FONT_REF_KEYS: &[&str] = &["font"];
pub(crate) const FONT_SIZE_FORBIDDEN_KEYS: &[&str] = &["font_size", "fontSize"];

pub(crate) const REQUIRED_COLOR_KEYS_PAGE: &[&str] = &[
    "text_primary",
    "text_muted",
    "text_body",
    "text_inverse",
    "surface_bg",
    "border_default",
];

pub(crate) const REQUIRED_SHELL_KEYS: &[&str] = &[
    "bg",
    "text",
    "stage",
    "stage_border",
    "chrome_top_bg",
    "chrome_bottom_bg",
    "chrome_border_top",
    "chrome_border_bottom",
    "family_ui",
];

pub(crate) const REQUIRED_SHELL_COLOR_KEYS: &[&str] = &[
    "text_primary",
    "text_muted",
    "text_body",
    "text_inverse",
    "panel_bg",
    "border_default",
];

pub(crate) const REQUIRED_SHELL_FONT_KEYS: &[&str] = &["1", "2", "3", "4"];

pub(crate) const REQUIRED_COLOR_KEYS_COCKPIT: &[&str] = &[
    "text_value",
    "text_unit",
    "text_accent",
    "panel_title",
    "section_border",
    "chart_1",
    "chart_2",
    "chart_3",
    "chart_4",
    "chart_5",
    "chart_6",
];

