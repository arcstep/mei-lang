//! Phase 8.5 ScopeTarget ABI: unique structure addressing + temporary Stage routes.
//!
//! Canonical debug route: `/apps/{app}/~/{scope_or_node}` (`~` = temporary Stage).
//! `node_id` is the sole unambiguous identity. `preview_scope` / `panel_id` are
//! assistive; duplicate local ids must surface as diagnostics, never silent picks.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::view_artifact::{StructureFullDocument, StructureFullNode};

/// Addressable UI roles for scoped routes (kernel UiScopeRole subset).
pub const SCOPED_ROUTE_ROLES: &[&str] = &["plane", "region", "section", "slot", "content"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ScopeTarget {
    pub stage_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane_id: Option<String>,
    /// `plane | region | section | slot | content`
    pub ui_role: String,
    /// Unique true identity.
    pub node_id: String,
    /// Readable prefix-match assistive identity.
    pub preview_scope: String,
    /// Parse hint only — never substitutes for `node_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeTargetHint {
    /// Tier / plane root under the stage (`t0` / `t1` / `T0` …).
    Tier(String),
    /// Role + local id (`region` + `r-right-rail`).
    RoleLocal { role: String, local_id: String },
    /// Exact structure node id.
    NodeId(String),
    /// Panel / preview_scope assistive id (must resolve uniquely).
    PanelId(String),
    /// Preview scope path (must resolve uniquely).
    PreviewScope(String),
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ScopeTargetResolveError {
    #[error("scope target not found: {0}")]
    NotFound(String),
    #[error("ambiguous scope target `{hint}` matches {count} nodes: {candidates}")]
    Ambiguous {
        hint: String,
        count: usize,
        candidates: String,
    },
    #[error("invalid scope target role `{0}` (expected plane|region|section|slot|content)")]
    InvalidRole(String),
}

impl ScopeTarget {
    pub fn scope_digest(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.stage_id.trim(),
            self.node_id.trim(),
            self.preview_scope.trim(),
            self.ui_role.trim().to_ascii_lowercase()
        )
    }

    /// Canonical temporary-Stage Access path: `/apps/{app}/~/{scope_or_node}`.
    pub fn canonical_path(&self, app_id: &str) -> String {
        canonical_temp_stage_path(app_id, self)
    }
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .trim_matches('/')
        .to_ascii_lowercase()
        .replace('_', "-")
}

fn normalize_tier(value: &str) -> String {
    let raw = value.trim().trim_matches('/');
    if raw.is_empty() {
        return String::new();
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with('t') && lower.len() <= 3 {
        return lower;
    }
    lower
}

fn local_id_of(node: &StructureFullNode) -> String {
    let scope = node.preview_scope.trim().trim_matches('/');
    let tail = scope.rsplit('/').next().unwrap_or(scope);
    if !tail.is_empty() {
        return tail.to_string();
    }
    node.label.trim().to_string()
}

fn plane_of(node: &StructureFullNode) -> Option<String> {
    node.plane
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .or_else(|| {
            let scope = node.preview_scope.trim().trim_matches('/');
            for part in scope.split('/') {
                let n = normalize_tier(part);
                if matches!(n.as_str(), "t0" | "t1" | "t2" | "p") {
                    return Some(part.to_string());
                }
            }
            None
        })
}

fn role_allowed(role: &str) -> bool {
    let r = role.trim().to_ascii_lowercase();
    SCOPED_ROUTE_ROLES.iter().any(|allowed| *allowed == r)
}

fn target_from_node(stage_id: &str, node: &StructureFullNode) -> ScopeTarget {
    ScopeTarget {
        stage_id: stage_id.to_string(),
        plane_id: plane_of(node),
        ui_role: node.ui_role.trim().to_ascii_lowercase(),
        node_id: node.node_id.clone(),
        preview_scope: node.preview_scope.clone(),
        panel_id: node
            .panel_id
            .clone()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                if node.preview_scope.trim().is_empty() {
                    None
                } else {
                    Some(node.preview_scope.clone())
                }
            }),
    }
}

fn finish_unique(
    hint: &str,
    stage_id: &str,
    matches: Vec<&StructureFullNode>,
) -> Result<ScopeTarget, ScopeTargetResolveError> {
    match matches.as_slice() {
        [] => Err(ScopeTargetResolveError::NotFound(hint.to_string())),
        [only] => Ok(target_from_node(stage_id, only)),
        many => {
            let candidates = many
                .iter()
                .take(8)
                .map(|node| {
                    format!(
                        "{} ({}/{})",
                        node.node_id,
                        node.ui_role,
                        node.preview_scope
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            Err(ScopeTargetResolveError::Ambiguous {
                hint: hint.to_string(),
                count: many.len(),
                candidates,
            })
        }
    }
}

/// Resolve a unique ScopeTarget from structure.full + hint.
pub fn resolve_scope_target(
    document: &StructureFullDocument,
    hint: ScopeTargetHint,
) -> Result<ScopeTarget, ScopeTargetResolveError> {
    let stage_id = document.scene_id.as_str();
    match hint {
        ScopeTargetHint::NodeId(node_id) => {
            let id = node_id.trim();
            let matches: Vec<_> = document
                .nodes
                .iter()
                .filter(|node| node.node_id == id)
                .collect();
            finish_unique(id, stage_id, matches)
        }
        ScopeTargetHint::PreviewScope(scope) => {
            let needle = normalize_token(&scope);
            let matches: Vec<_> = document
                .nodes
                .iter()
                .filter(|node| normalize_token(&node.preview_scope) == needle)
                .collect();
            finish_unique(scope.trim(), stage_id, matches)
        }
        ScopeTargetHint::PanelId(panel_id) => {
            let needle = normalize_token(&panel_id);
            let matches: Vec<_> = document
                .nodes
                .iter()
                .filter(|node| {
                    node.panel_id
                        .as_deref()
                        .map(|v| normalize_token(v) == needle)
                        .unwrap_or(false)
                        || normalize_token(&node.preview_scope) == needle
                        || normalize_token(&local_id_of(node)) == needle
                })
                .collect();
            finish_unique(panel_id.trim(), stage_id, matches)
        }
        ScopeTargetHint::Tier(tier) => {
            let want = normalize_tier(&tier);
            let matches: Vec<_> = document
                .nodes
                .iter()
                .filter(|node| {
                    let role = node.ui_role.trim().to_ascii_lowercase();
                    if role != "plane" {
                        return false;
                    }
                    plane_of(node)
                        .map(|p| normalize_tier(&p) == want)
                        .unwrap_or(false)
                        || normalize_token(&local_id_of(node)) == want
                        || normalize_token(&node.preview_scope)
                            .split('/')
                            .any(|part| part == want)
                })
                .collect();
            finish_unique(tier.trim(), stage_id, matches)
        }
        ScopeTargetHint::RoleLocal { role, local_id } => {
            let role_n = role.trim().to_ascii_lowercase();
            if !role_allowed(&role_n) {
                return Err(ScopeTargetResolveError::InvalidRole(role));
            }
            let local_n = normalize_token(&local_id);
            let matches: Vec<_> = document
                .nodes
                .iter()
                .filter(|node| {
                    node.ui_role.trim().to_ascii_lowercase() == role_n
                        && (normalize_token(&local_id_of(node)) == local_n
                            || normalize_token(&node.preview_scope)
                                .rsplit('/')
                                .next()
                                .map(|tail| tail == local_n)
                                .unwrap_or(false)
                            || node
                                .panel_id
                                .as_deref()
                                .map(|v| normalize_token(v) == local_n)
                                .unwrap_or(false))
                })
                .collect();
            finish_unique(
                &format!("{role_n}/{local_id}"),
                stage_id,
                matches,
            )
        }
    }
}

/// Canonical temporary-Stage Access path (no query).
///
/// Prefers readable `preview_scope`; falls back to `node/{node_id}`.
pub fn canonical_temp_stage_path(app_id: &str, target: &ScopeTarget) -> String {
    let app = app_id.trim();
    let scope = target.preview_scope.trim().trim_matches('/');
    if !scope.is_empty() {
        return format!("/apps/{app}/~/{scope}");
    }
    let node = target.node_id.trim().trim_start_matches('/');
    format!("/apps/{app}/~/node/{node}")
}

fn encode_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => ch.to_string(),
            _ => format!("%{:02X}", ch as u32),
        })
        .collect()
}

/// Product Access Stage URL: `/apps/{app_id}/{stage_id}` (default stage `home`).
///
/// Shared by Host redirects, MRG navigation sync, and `list_scope_routes`.
/// Does not generate legacy `/apps/app/.../scene/...` or `/layout?...` deep URLs.
pub fn canonical_access_stage_url(app_id: &str, stage_id: &str) -> String {
    let app = encode_path_segment(app_id.trim());
    let stage = stage_id.trim();
    let stage = if stage.is_empty() { "home" } else { stage };
    format!("/apps/{app}/{}", encode_path_segment(stage))
}

/// Compatibility alias — same as [`canonical_temp_stage_path`].
pub fn canonical_scoped_path(app_id: &str, target: &ScopeTarget) -> String {
    canonical_temp_stage_path(app_id, target)
}

/// Parse `/apps/{app}/~/{…}` target segment into a resolve hint.
pub fn parse_temp_stage_target(target: &str) -> Option<ScopeTargetHint> {
    let trimmed = target.trim().trim_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    if parts[0].eq_ignore_ascii_case("node") && parts.len() >= 2 {
        return Some(ScopeTargetHint::NodeId(parts[1..].join("/")));
    }
    // Prefer exact preview_scope / panel id; Tier only when a single tN token.
    if parts.len() == 1 && matches!(normalize_tier(parts[0]).as_str(), "t0" | "t1" | "t2" | "p") {
        return Some(ScopeTargetHint::Tier(parts[0].to_string()));
    }
    Some(ScopeTargetHint::PreviewScope(trimmed.to_string()))
}

/// Infer source stage id from a temp-stage target token (`home/T1/...` → `home`).
pub fn infer_stage_from_temp_target(target: &str) -> String {
    let trimmed = target.trim().trim_matches('/');
    if trimmed.is_empty() {
        return "home".to_string();
    }
    let parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    if parts
        .first()
        .map(|p| p.eq_ignore_ascii_case("node"))
        .unwrap_or(false)
    {
        return "home".to_string();
    }
    let head = parts[0];
    if matches!(normalize_tier(head).as_str(), "t0" | "t1" | "t2" | "p") {
        return "home".to_string();
    }
    head.to_string()
}

/// T2 page route (page scene ≠ plane id).
pub fn canonical_t2_page_path(app_id: &str, stage_id: &str, page_scene_id: &str) -> String {
    format!(
        "/apps/{}/{}/t2/page/{}",
        app_id.trim(),
        stage_id.trim(),
        page_scene_id.trim().trim_start_matches('/')
    )
}

/// Parse path segments after `/apps/{app}/{stage}` into a hint.
///
/// Supported:
/// - `{tier}`
/// - `{tier}/{role}/{local}`
/// - `{tier}/node/{…node_id}`
/// - `{tier}/panel/{panel_id}` (unique-only; caller must redirect to canonical)
/// - `t2/page/{page_scene_id}` → returns `None` (handled as page scene, not structure)
pub fn parse_scoped_route_tail(tail: &str) -> Option<ScopedRouteParse> {
    let parts: Vec<&str> = tail
        .trim()
        .trim_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    if parts.len() >= 3 && normalize_tier(parts[0]) == "t2" && parts[1].eq_ignore_ascii_case("page")
    {
        return Some(ScopedRouteParse::T2Page {
            page_scene_id: parts[2..].join("/"),
        });
    }
    let tier = parts[0].to_string();
    if parts.len() == 1 {
        return Some(ScopedRouteParse::Structure {
            hint: ScopeTargetHint::Tier(tier),
        });
    }
    if parts.len() >= 3 && parts[1].eq_ignore_ascii_case("node") {
        return Some(ScopedRouteParse::Structure {
            hint: ScopeTargetHint::NodeId(parts[2..].join("/")),
        });
    }
    if parts.len() >= 3 && parts[1].eq_ignore_ascii_case("panel") {
        return Some(ScopedRouteParse::Structure {
            hint: ScopeTargetHint::PanelId(parts[2..].join("/")),
        });
    }
    if parts.len() >= 3 && role_allowed(parts[1]) {
        return Some(ScopedRouteParse::Structure {
            hint: ScopeTargetHint::RoleLocal {
                role: parts[1].to_ascii_lowercase(),
                local_id: parts[2..].join("/"),
            },
        });
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopedRouteParse {
    Structure { hint: ScopeTargetHint },
    T2Page { page_scene_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_artifact::StructureFullDocument;

    fn sample_doc() -> StructureFullDocument {
        StructureFullDocument {
            schema_version: "structure-full-v1".to_string(),
            app_id: "mini-data".to_string(),
            scene_id: "home".to_string(),
            semantic_revision: "r1".to_string(),
            scene_roots: vec!["ui-scope:home".to_string()],
            nodes: vec![
                StructureFullNode {
                    node_id: "ui-scope:home/T1".to_string(),
                    ui_role: "plane".to_string(),
                    preview_scope: "home/T1".to_string(),
                    label: "T1".to_string(),
                    parent_id: Some("ui-scope:home".to_string()),
                    children: vec!["ui-scope:home/T1/r-right-rail".to_string()],
                    plane: Some("T1".to_string()),
                    content_kind: None,
                    panel_id: Some("home/T1".to_string()),
                    use_keys: vec![],
                    frame_viewport: None,
                },
                StructureFullNode {
                    node_id: "ui-scope:home/T1/r-right-rail".to_string(),
                    ui_role: "region".to_string(),
                    preview_scope: "home/T1/r-right-rail".to_string(),
                    label: "right".to_string(),
                    parent_id: Some("ui-scope:home/T1".to_string()),
                    children: vec!["ui-scope:home/T1/r-right-rail/s-warning".to_string()],
                    plane: Some("T1".to_string()),
                    content_kind: None,
                    panel_id: Some("home/T1/r-right-rail".to_string()),
                    use_keys: vec![],
                    frame_viewport: None,
                },
                StructureFullNode {
                    node_id: "ui-scope:home/T1/r-right-rail/s-warning".to_string(),
                    ui_role: "section".to_string(),
                    preview_scope: "home/T1/r-right-rail/s-warning".to_string(),
                    label: "warning".to_string(),
                    parent_id: Some("ui-scope:home/T1/r-right-rail".to_string()),
                    children: vec![],
                    plane: Some("T1".to_string()),
                    content_kind: None,
                    panel_id: Some("home/T1/r-right-rail/s-warning".to_string()),
                    use_keys: vec![],
                    frame_viewport: None,
                },
                StructureFullNode {
                    node_id: "ui-scope:home/T1/r-left/s-warning".to_string(),
                    ui_role: "section".to_string(),
                    preview_scope: "home/T1/r-left/s-warning".to_string(),
                    label: "warning".to_string(),
                    parent_id: Some("ui-scope:home/T1/r-left".to_string()),
                    children: vec![],
                    plane: Some("T1".to_string()),
                    content_kind: None,
                    panel_id: Some("home/T1/r-left/s-warning".to_string()),
                    use_keys: vec![],
                    frame_viewport: None,
                },
            ],
            frame_viewport: None,
        }
    }

    #[test]
    fn resolves_tier_and_role_local() {
        let doc = sample_doc();
        let plane = resolve_scope_target(&doc, ScopeTargetHint::Tier("t1".into())).unwrap();
        assert_eq!(plane.node_id, "ui-scope:home/T1");
        assert_eq!(
            plane.canonical_path("mini-data"),
            "/apps/mini-data/~/home/T1"
        );

        let section = resolve_scope_target(
            &doc,
            ScopeTargetHint::RoleLocal {
                role: "section".into(),
                local_id: "s-warning".into(),
            },
        );
        assert!(matches!(
            section,
            Err(ScopeTargetResolveError::Ambiguous { count: 2, .. })
        ));

        let by_node = resolve_scope_target(
            &doc,
            ScopeTargetHint::NodeId("ui-scope:home/T1/r-right-rail/s-warning".into()),
        )
        .unwrap();
        assert_eq!(
            by_node.canonical_path("mini-data"),
            "/apps/mini-data/~/home/T1/r-right-rail/s-warning"
        );
    }

    #[test]
    fn parse_temp_stage_and_legacy_tail() {
        assert_eq!(
            parse_temp_stage_target("home/T1/r-right-rail"),
            Some(ScopeTargetHint::PreviewScope(
                "home/T1/r-right-rail".into()
            ))
        );
        assert_eq!(
            parse_temp_stage_target("node/ui-scope:home/T1"),
            Some(ScopeTargetHint::NodeId("ui-scope:home/T1".into()))
        );
        assert_eq!(
            parse_scoped_route_tail("t1"),
            Some(ScopedRouteParse::Structure {
                hint: ScopeTargetHint::Tier("t1".into())
            })
        );
        assert_eq!(
            parse_scoped_route_tail("t1/section/s-warning"),
            Some(ScopedRouteParse::Structure {
                hint: ScopeTargetHint::RoleLocal {
                    role: "section".into(),
                    local_id: "s-warning".into()
                }
            })
        );
        assert_eq!(
            parse_scoped_route_tail("t2/page/park_point_1_page"),
            Some(ScopedRouteParse::T2Page {
                page_scene_id: "park_point_1_page".into()
            })
        );
        assert_eq!(
            canonical_t2_page_path("mini-park", "home", "park_point_1_page"),
            "/apps/mini-park/home/t2/page/park_point_1_page"
        );
    }
}
