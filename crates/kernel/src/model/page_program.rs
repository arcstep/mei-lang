//! Generic document/page UI Program IR.
//!
//! This module intentionally depends only on the kernel model layer so admin
//! resource discovery can wrap a page without introducing a `mei_config`
//! dependency cycle.

use serde::{Deserialize, Serialize};

/// A page always renders as a document-flow surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PageSurface {
    #[default]
    Document,
}

impl PageSurface {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
        }
    }
}

/// Root content reference for a page.
///
/// The MVP reuses an existing scene module. Additional root kinds can be
/// introduced additively without changing the `PageProgram` envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PageRoot {
    SceneRef { scene_ref: String },
}

impl PageRoot {
    pub fn scene_ref(&self) -> &str {
        match self {
            Self::SceneRef { scene_ref } => scene_ref,
        }
    }
}

/// Stable document/page UI Program envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageProgram {
    pub page_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub source_anchor: String,
    #[serde(default)]
    pub surface: PageSurface,
    pub root: PageRoot,
}

impl PageProgram {
    pub fn from_scene_ref(
        page_id: impl Into<String>,
        title: Option<String>,
        source_anchor: impl Into<String>,
        scene_ref: impl Into<String>,
    ) -> Self {
        Self {
            page_id: page_id.into(),
            title,
            source_anchor: source_anchor.into(),
            surface: PageSurface::Document,
            root: PageRoot::SceneRef {
                scene_ref: scene_ref.into(),
            },
        }
    }
}

/// Thin admin-resource wrapper around the generic page IR.
///
/// `source_anchor` remains owned by `PageProgram`; the admin layer only adds
/// the resource identity needed for routing and lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminPageProgram {
    pub resource_id: String,
    pub page: PageProgram,
}

impl AdminPageProgram {
    pub fn new(resource_id: impl Into<String>, page: PageProgram) -> Self {
        Self {
            resource_id: resource_id.into(),
            page,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn admin_page_program_has_stable_serde_envelope() {
        let wrapper = AdminPageProgram::new(
            "warnings",
            PageProgram::from_scene_ref(
                "warnings",
                Some("Warnings".to_string()),
                "src/admin/warnings.mei",
                "warnings",
            ),
        );

        let value = serde_json::to_value(&wrapper).expect("serialize admin page program");
        assert_eq!(
            value,
            json!({
                "resource_id": "warnings",
                "page": {
                    "page_id": "warnings",
                    "title": "Warnings",
                    "source_anchor": "src/admin/warnings.mei",
                    "surface": "document",
                    "root": {
                        "kind": "scene_ref",
                        "scene_ref": "warnings"
                    }
                }
            })
        );

        let decoded: AdminPageProgram =
            serde_json::from_value(value).expect("deserialize admin page program");
        assert_eq!(decoded, wrapper);
    }
}
