//! Build route axis presets: `/apps/build/{app}/eval|region|section|content|compile|eval/content`.

use crate::pages::AppQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildAxisPreset {
    pub data_mode: Option<String>,
    pub review_projection: Option<String>,
    pub tree_max_ui_role: Option<String>,
    /// `structure` (ui_structure) or `compile` (mcg/scenes/routes/…).
    pub tree_mode: Option<String>,
}

impl BuildAxisPreset {
    pub fn apply_eval(&mut self) {
        self.tree_mode = Some("structure".to_string());
        self.data_mode = Some("eval".to_string());
        if self.review_projection.is_none() {
            self.review_projection = Some("live_full".to_string());
        }
    }

    pub fn apply_region(&mut self) {
        self.tree_mode = Some("structure".to_string());
        if self.data_mode.is_none() {
            self.data_mode = Some("static".to_string());
        }
        self.review_projection = Some("plane_region".to_string());
    }

    pub fn apply_section(&mut self) {
        self.tree_mode = Some("structure".to_string());
        if self.data_mode.is_none() {
            self.data_mode = Some("static".to_string());
        }
        self.review_projection = Some("plane_region_section".to_string());
    }

    /// Content 审阅面（`static` + `static_full`）。
    pub fn apply_content_preset(&mut self) {
        self.tree_mode = Some("structure".to_string());
        self.data_mode = Some("static".to_string());
        self.review_projection = Some("static_full".to_string());
    }

    /// 在 eval 轴上追加 content 树深（`/eval/content`）。
    pub fn apply_content_tree_depth(&mut self) {
        self.tree_max_ui_role = Some("content".to_string());
        if self.review_projection.is_none() {
            self.review_projection = Some("static_full".to_string());
        }
        if self.data_mode.is_none() {
            self.data_mode = Some("static".to_string());
        }
    }

    pub fn apply_compile(&mut self) {
        self.tree_mode = Some("compile".to_string());
    }

    #[cfg(test)]
    pub fn path_suffix(&self) -> String {
        let mut segments = Vec::new();
        if self.tree_mode.as_deref() == Some("compile") {
            segments.push("compile");
        } else {
            if self.data_mode.as_deref() == Some("eval") {
                segments.push("eval");
            }
            if self.review_projection.as_deref() == Some("plane_region") {
                segments.push("region");
            }
            if self.review_projection.as_deref() == Some("plane_region_section") {
                segments.push("section");
            }
            if self.review_projection.as_deref() == Some("static_full")
                && self.data_mode.as_deref() == Some("static")
                && self.tree_max_ui_role.is_none()
            {
                segments.push("content");
            }
            if self.tree_max_ui_role.as_deref() == Some("content") {
                if !segments.contains(&"content") {
                    segments.push("content");
                }
            }
        }
        if segments.is_empty() {
            String::new()
        } else {
            format!("/{}", segments.join("/"))
        }
    }
}

/// Parse `/apps/build/{app-id}/[eval|region|section|content|compile|eval/content]` tail.
pub fn parse_build_app_tail(tail: &str) -> (String, BuildAxisPreset) {
    let parts: Vec<&str> = tail.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return (String::new(), BuildAxisPreset::default());
    }
    let app_id = parts[0].to_string();
    let mut preset = BuildAxisPreset::default();
    let mut idx = 1usize;
    while idx < parts.len() {
        match parts[idx] {
            "eval" => {
                preset.apply_eval();
                idx += 1;
            }
            "region" => {
                preset.apply_region();
                idx += 1;
            }
            "section" => {
                preset.apply_section();
                idx += 1;
            }
            "content" => {
                if preset.data_mode.as_deref() == Some("eval") {
                    preset.apply_content_tree_depth();
                } else {
                    preset.apply_content_preset();
                }
                idx += 1;
            }
            "compile" => {
                preset.apply_compile();
                idx += 1;
            }
            _ => break,
        }
    }
    (app_id, preset)
}

/// Path preset fills query defaults; explicit query params win.
pub fn merge_build_preset_into_query(query: &mut AppQuery, preset: &BuildAxisPreset) {
    if query.data_mode.is_none() {
        query.data_mode = preset.data_mode.clone();
    }
    if query.review_projection.is_none() {
        query.review_projection = preset.review_projection.clone();
    }
    if query.tree_max.is_none() {
        query.tree_max = preset.tree_max_ui_role.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_eval_content_axis() {
        let (app_id, preset) = parse_build_app_tail("pretty-panels/eval/content");
        assert_eq!(app_id, "pretty-panels");
        assert_eq!(preset.data_mode.as_deref(), Some("eval"));
        assert_eq!(preset.tree_max_ui_role.as_deref(), Some("content"));
        assert_eq!(preset.path_suffix(), "/eval/content");
    }

    #[test]
    fn parse_region_axis() {
        let (_, preset) = parse_build_app_tail("demo/region");
        assert_eq!(preset.review_projection.as_deref(), Some("plane_region"));
        assert_eq!(preset.path_suffix(), "/region");
    }

    #[test]
    fn parse_section_axis() {
        let (_, preset) = parse_build_app_tail("demo/section");
        assert_eq!(preset.review_projection.as_deref(), Some("plane_region_section"));
        assert_eq!(preset.path_suffix(), "/section");
    }

    #[test]
    fn parse_content_preset_axis() {
        let (_, preset) = parse_build_app_tail("demo/content");
        assert_eq!(preset.data_mode.as_deref(), Some("static"));
        assert_eq!(preset.review_projection.as_deref(), Some("static_full"));
    }

    #[test]
    fn parse_compile_axis() {
        let (_, preset) = parse_build_app_tail("demo/compile");
        assert_eq!(preset.tree_mode.as_deref(), Some("compile"));
        assert_eq!(preset.path_suffix(), "/compile");
    }

    #[test]
    fn query_overrides_preset_merge() {
        let mut query = AppQuery {
            data_mode: Some("static".to_string()),
            ..Default::default()
        };
        let mut preset = BuildAxisPreset::default();
        preset.apply_eval();
        merge_build_preset_into_query(&mut query, &preset);
        assert_eq!(query.data_mode.as_deref(), Some("static"));
        assert_eq!(query.review_projection.as_deref(), Some("live_full"));
    }
}
