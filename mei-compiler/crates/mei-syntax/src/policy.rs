use std::path::Path;

use thiserror::Error;

const FORBIDDEN_TOKENS: &[&str] = &["for", "while", "lambda", "load", "import", "open"];
const WORLD_MEI_SUFFIX: &str = ".world.mei";
const WORLD_ALLOWED_TOKENS: &[&str] = &["for", "enum"];
const GRID_ONLY_POLICY_ROOTS: &[&str] = &[
    "/workspaces/ws-demo-v2/apps/pretty-panels/",
    "/workspaces/ws-demo-v2/stock/templates/cockpit/",
];
/// Author DSL patterns rejected on all paths (Phase 3 layout purge).
const GLOBAL_DEPRECATED_PATTERNS: &[(&str, &str)] = &[
    ("frame.add_panel(", "frame.add_panel(...)"),
    ("titled_shell(", "titled_shell(...)"),
    ("panel_slot(", "panel_slot(...)"),
    ("row_budgets", "row_budgets"),
    ("assembly_view(", "assembly_view(...)"),
    ("board_assembly(", "board_assembly(...)"),
    ("panel_contract(", "panel_contract(...)"),
];
const GRID_ONLY_DEPRECATED_PATTERNS: &[(&str, &str)] =
    &[("flex(", "flex(...)"), ("layout_policy", "layout_policy")];

fn sanitize_for_policy(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    let mut in_line_comment = false;
    for ch in source.chars() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                out.push('\n');
            } else {
                out.push(' ');
            }
            continue;
        }
        if let Some(quote) = in_string {
            if ch == '\n' {
                out.push('\n');
                escaped = false;
                continue;
            }
            out.push(' ');
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '#' => {
                in_line_comment = true;
                out.push(' ');
            }
            '"' | '\'' => {
                in_string = Some(ch);
                out.push(' ');
            }
            _ => out.push(ch),
        }
    }
    out
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct ForbiddenTokenError {
    pub token: String,
    pub message: String,
}

impl ForbiddenTokenError {
    fn forbidden_token(token: &str) -> Self {
        Self {
            token: token.to_string(),
            message: format!("authoring source contains forbidden token `{token}`"),
        }
    }

    fn deprecated_pattern(pattern: &str) -> Self {
        Self {
            token: pattern.to_string(),
            message: format!(
                "authoring source uses deleted layout authoring `{pattern}`; use `scene` + `section_shell` + `content_panel` / `page_instance` instead"
            ),
        }
    }
}

pub fn validate_authoring_policy(source: &str) -> Result<(), ForbiddenTokenError> {
    validate_authoring_policy_with_world_override(source, false)
}

pub fn validate_world_authoring_policy(source: &str) -> Result<(), ForbiddenTokenError> {
    validate_authoring_policy_with_world_override(source, true)
}

fn validate_authoring_policy_with_world_override(
    source: &str,
    allow_world_for_enum: bool,
) -> Result<(), ForbiddenTokenError> {
    let sanitized = sanitize_for_policy(source);
    for token in sanitized
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| !token.is_empty())
    {
        if FORBIDDEN_TOKENS.contains(&token) {
            if allow_world_for_enum && WORLD_ALLOWED_TOKENS.contains(&token) {
                continue;
            }
            return Err(ForbiddenTokenError::forbidden_token(token));
        }
    }
    Ok(())
}

pub fn validate_authoring_policy_for_path(
    path: &Path,
    source: &str,
) -> Result<(), ForbiddenTokenError> {
    let is_world_mei = path
        .to_string_lossy()
        .replace('\\', "/")
        .ends_with(WORLD_MEI_SUFFIX);
    validate_authoring_policy_with_world_override(source, is_world_mei)?;
    validate_region_layout_policy(path, source)?;
    validate_section_layout_policy(path, source)?;
    let sanitized = sanitize_for_policy(source);
    for (needle, label) in GLOBAL_DEPRECATED_PATTERNS {
        if sanitized.contains(needle) {
            return Err(ForbiddenTokenError::deprecated_pattern(label));
        }
    }
    if !should_enforce_grid_only(path) {
        return Ok(());
    }
    for (needle, label) in GRID_ONLY_DEPRECATED_PATTERNS {
        if sanitized.contains(needle) {
            return Err(ForbiddenTokenError::deprecated_pattern(label));
        }
    }
    Ok(())
}

fn validate_region_layout_policy(path: &Path, source: &str) -> Result<(), ForbiddenTokenError> {
    let raw = path.to_string_lossy().replace('\\', "/");
    if !raw.contains("/r-") || !raw.ends_with("/layout.mei") {
        return Ok(());
    }
    if !source.contains("region_layout(") {
        return Ok(());
    }
    let sanitized = sanitize_for_policy(source);
    if sanitized.contains("stage_anchor(") {
        return Err(ForbiddenTokenError::region_layout_violation(
            "stage_anchor",
            "region_layout must not use stage_anchor(...); use plane_layout grid + region area instead",
        ));
    }
    if region_layout_allows_empty_sections(source) {
        return Ok(());
    }
    if sanitized.contains("contents =") || sanitized.contains("contents=") {
        return Err(ForbiddenTokenError::region_layout_violation(
            "contents",
            "region_layout must use sections = [section_ref(...)]; direct content(...) is not allowed",
        ));
    }
    if sanitized.contains("blocks =") || sanitized.contains("blocks=") {
        return Err(ForbiddenTokenError::region_layout_violation(
            "blocks",
            "region_layout must use sections = [section_ref(...)]; direct blocks are not allowed",
        ));
    }
    if !sanitized.contains("sections =") && !sanitized.contains("sections=") {
        return Err(ForbiddenTokenError::region_layout_violation(
            "sections",
            "region_layout must declare non-empty sections = [section_ref(...)]",
        ));
    }
    Ok(())
}

fn region_layout_allows_empty_sections(source: &str) -> bool {
    source.contains("chrome_role = \"stage_aperture\"")
        || source.contains("chrome_role=\"stage_aperture\"")
        || source.contains("id = \"stage_aperture_frame\"")
        || source.contains("id=\"stage_aperture_frame\"")
}

fn validate_section_layout_policy(path: &Path, source: &str) -> Result<(), ForbiddenTokenError> {
    let raw = path.to_string_lossy().replace('\\', "/");
    if !raw.contains("/s-") || !raw.ends_with("/layout.mei") {
        return Ok(());
    }
    if !source.contains("section_layout(") {
        return Ok(());
    }
    if section_layout_uses_panel_ref_passthrough(source) {
        return Err(ForbiddenTokenError::section_layout_violation(
            "contents+panel_ref_passthrough",
            "section_layout must mount body via shell, not contents = [content(..., source = panel_ref(...))]. \
             The content wrapper hides map/viewport under build preview (PlaneRegionSection). \
             Use section_shell(title = \"...\", body = panel_ref(\"content/...\")) for titled panels, \
             or shell = content_panel(chrome = \"bare\", show_heading = False, blocks = [panel_ref(\"home:...\")]) \
             for bare stage/map pass-through. See r-left-rail/s-enforcement/layout.mei.",
        ));
    }
    Ok(())
}

fn section_layout_uses_panel_ref_passthrough(source: &str) -> bool {
    let has_contents = source.contains("contents =") || source.contains("contents=");
    if !has_contents || !source.contains("content(") {
        return false;
    }
    source.contains("source = panel_ref(")
        || source.contains("source=panel_ref(")
        || source.contains("source =panel_ref(")
        || source.contains("source= panel_ref(")
}

impl ForbiddenTokenError {
    fn section_layout_violation(token: &str, message: &str) -> Self {
        Self {
            token: token.to_string(),
            message: message.to_string(),
        }
    }

    fn region_layout_violation(token: &str, message: &str) -> Self {
        Self {
            token: token.to_string(),
            message: message.to_string(),
        }
    }
}

pub fn forbidden_authoring_tokens() -> &'static [&'static str] {
    FORBIDDEN_TOKENS
}

fn should_enforce_grid_only(path: &Path) -> bool {
    let raw = path.to_string_lossy().replace('\\', "/");
    GRID_ONLY_POLICY_ROOTS
        .iter()
        .any(|segment| raw.contains(segment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_only_policy_rejects_flex_in_cockpit_templates() {
        let path = Path::new("/tmp/workspaces/ws-demo-v2/stock/templates/cockpit/example.mei");
        let err = validate_authoring_policy_for_path(
            path,
            r#"frame(id = "home_frame", layout = flex(direction = "column"))"#,
        )
        .expect_err("flex should be rejected in cockpit templates");
        assert!(err.to_string().contains("content_panel") || err.to_string().contains("deleted"));
    }

    #[test]
    fn grid_only_policy_keeps_legacy_paths_compatible() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/stock/components/chart/echarts/previews/chart.bar.mei",
        );
        validate_authoring_policy_for_path(
            path,
            r#"frame(id = "home_frame", layout = flex(direction = "column"))"#,
        )
        .expect("legacy preview path remains compatible for flex");
    }

    #[test]
    fn global_policy_rejects_frame_add_panel() {
        let path = Path::new("/tmp/workspaces/ws-demo-v2/apps/x/src/scene/home.mei");
        let err = validate_authoring_policy_for_path(
            path,
            r#"frame.add_panel(id = "child_panel", area = "auto", blocks = [])"#,
        )
        .expect_err("frame.add_panel should be rejected");
        assert!(err.to_string().contains("frame.add_panel"));
    }

    #[test]
    fn global_policy_rejects_titled_shell_and_panel_slot() {
        let path = Path::new("/tmp/any.mei");
        assert!(
            validate_authoring_policy_for_path(path, "shell = titled_shell(title = \"x\")")
                .is_err()
        );
        assert!(
            validate_authoring_policy_for_path(path, "slot = panel_slot(kind = \"filter\")")
                .is_err()
        );
        assert!(validate_authoring_policy_for_path(path, "row_budgets = [70, 70]").is_err());
    }

    #[test]
    fn global_policy_rejects_deleted_constructors() {
        let path = Path::new("/tmp/any.mei");
        assert!(validate_authoring_policy_for_path(path, "board_assembly(key = \"x\")").is_err());
        assert!(validate_authoring_policy_for_path(path, "panel_contract(id = \"x\")").is_err());
        assert!(validate_authoring_policy_for_path(path, "assembly_view(key = \"x\")").is_err());
    }

    #[test]
    fn region_layout_rejects_direct_contents() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/apps/mini-park/src/scene/home/t1/r-header/layout.mei",
        );
        let err = validate_authoring_policy_for_path(
            path,
            r#"region_layout(
                id = "home_header",
                contents = [content(id = "x", source = panel_ref("home:home_header"))],
            )"#,
        )
        .expect_err("contents should be rejected");
        assert!(err.to_string().contains("sections"));
    }

    #[test]
    fn region_layout_rejects_direct_blocks() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/apps/pretty-panels/src/scene/home/t1/r-layout-debug-controller/layout.mei",
        );
        let err = validate_authoring_policy_for_path(
            path,
            r#"region_layout(
                id = "layout_debug_controller",
                blocks = [component("cockpit.layout-debug")],
            )"#,
        )
        .expect_err("blocks should be rejected");
        assert!(err.to_string().contains("sections"));
    }

    #[test]
    fn region_layout_rejects_stage_anchor() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/apps/pretty-panels/src/scene/home/t0/r-map-stage/layout.mei",
        );
        let err = validate_authoring_policy_for_path(
            path,
            r#"region_layout(
                id = "map_stage",
                placement = stage_anchor(top = "0", left = "0", width = "100%", height = "1080px"),
                sections = [section_ref("pretty-panels/home/t0/r-map-stage/s-map-stage")],
            )"#,
        )
        .expect_err("stage_anchor should be rejected");
        assert!(err.to_string().contains("stage_anchor"));
    }

    #[test]
    fn region_layout_allows_stage_aperture_frame_without_sections() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/apps/pretty-panels/src/scene/home/t1/r-stage-aperture-frame/layout.mei",
        );
        validate_authoring_policy_for_path(
            path,
            r#"region_layout(
                id = "stage_aperture_frame",
                chrome_role = "stage_aperture",
            )"#,
        )
        .expect("stage_aperture frame-only region should be allowed");
    }

    #[test]
    fn section_layout_rejects_panel_ref_passthrough_contents() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/apps/pretty-panels/src/scene/home/t0/r-map-stage/s-map-stage/layout.mei",
        );
        let err = validate_authoring_policy_for_path(
            path,
            r#"section_layout(
                id = "map_stage",
                contents = [
                    content(
                        id = "map_stage_content",
                        content_kind = "map_view",
                        source = panel_ref("home:map_stage"),
                    ),
                ],
            )"#,
        )
        .expect_err("panel_ref passthrough contents should be rejected");
        assert!(err.to_string().contains("shell"));
        assert!(err.to_string().contains("s-enforcement"));
    }

    #[test]
    fn section_layout_allows_shell_body_panel_ref() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/apps/pretty-panels/src/scene/home/t1/r-left-rail/s-enforcement/layout.mei",
        );
        validate_authoring_policy_for_path(
            path,
            r#"section_layout(
                id = "enforcement",
                shell = section_shell(
                    title = "执法要素",
                    body = panel_ref("content/enforcement-stats"),
                ),
            )"#,
        )
        .expect("section_shell body = panel_ref should be allowed");
    }

    #[test]
    fn section_layout_allows_bare_content_panel_shell() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/apps/pretty-panels/src/scene/home/t0/r-map-stage/s-map-stage/layout.mei",
        );
        validate_authoring_policy_for_path(
            path,
            r#"section_layout(
                id = "map_stage_body",
                shell = content_panel(
                    chrome = "bare",
                    show_heading = False,
                    blocks = [panel_ref("home:map_stage")],
                ),
            )"#,
        )
        .expect("bare content_panel shell should be allowed");
    }

    #[test]
    fn section_layout_allows_contents_with_inline_block() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/apps/pretty-panels/src/scene/home/t1/r-layout-debug-controller/s-layout-debug/layout.mei",
        );
        validate_authoring_policy_for_path(
            path,
            r#"section_layout(
                id = "layout_debug",
                contents = [
                    content(
                        id = "layout_debug_content",
                        block = component("cockpit.layout-debug"),
                    ),
                ],
            )"#,
        )
        .expect("content with inline block should remain allowed");
    }
}
