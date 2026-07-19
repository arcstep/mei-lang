//! Generic document/page UI Program IR.
//!
//! This module intentionally depends only on the kernel model layer so admin
//! resource discovery can wrap a page without introducing a `mei_config`
//! dependency cycle.

use serde::{Deserialize, Serialize};

use crate::mei_config::ProviderBinding;

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
    SceneRef {
        scene_ref: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_anchor: Option<String>,
    },
}

impl PageRoot {
    pub fn scene_ref(&self) -> &str {
        match self {
            Self::SceneRef { scene_ref, .. } => scene_ref,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PageVisibleBody {
    pub markdown: String,
    pub html: String,
    pub source_anchor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageFill {
    pub slot: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub source_anchor: String,
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
    #[serde(default)]
    pub visible_body: PageVisibleBody,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fills: Vec<PageFill>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_bindings: Vec<ProviderBinding>,
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
                source_anchor: None,
            },
            visible_body: PageVisibleBody::default(),
            fills: Vec::new(),
            provider_bindings: Vec::new(),
        }
    }

    pub fn from_admin_entry(
        page_id: impl Into<String>,
        title: Option<String>,
        entry_source_anchor: impl Into<String>,
        scene_ref: impl Into<String>,
        scene_source_anchor: impl Into<String>,
        visible_markdown: impl Into<String>,
        visible_html: impl Into<String>,
        fills: Vec<PageFill>,
        provider_bindings: Vec<ProviderBinding>,
    ) -> Self {
        let source_anchor = entry_source_anchor.into();
        Self {
            page_id: page_id.into(),
            title,
            source_anchor: source_anchor.clone(),
            surface: PageSurface::Document,
            root: PageRoot::SceneRef {
                scene_ref: scene_ref.into(),
                source_anchor: Some(scene_source_anchor.into()),
            },
            visible_body: PageVisibleBody {
                markdown: visible_markdown.into(),
                html: visible_html.into(),
                source_anchor,
            },
            fills,
            provider_bindings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_program_has_stable_admin_content_envelope() {
        let page = PageProgram::from_admin_entry(
            "warnings",
            Some("Warnings".to_string()),
            "src/admin/runtime/warnings.mdx",
            "admin.runtime.warnings",
            "src/scene/admin/runtime/warnings.mei",
            "Runtime warnings.",
            "<p>Runtime warnings.</p>",
            vec![],
            vec![],
        );

        let value = serde_json::to_value(&page).expect("serialize page program");
        assert_eq!(value["page_id"], json!("warnings"));
        assert_eq!(
            value["root"],
            json!({
                "kind": "scene_ref",
                "scene_ref": "admin.runtime.warnings",
                "source_anchor": "src/scene/admin/runtime/warnings.mei"
            })
        );
        assert_eq!(
            value["visible_body"],
            json!({
                "markdown": "Runtime warnings.",
                "html": "<p>Runtime warnings.</p>",
                "source_anchor": "src/admin/runtime/warnings.mdx"
            })
        );
        let decoded: PageProgram = serde_json::from_value(value).expect("deserialize page program");
        assert_eq!(decoded, page);
    }
}
