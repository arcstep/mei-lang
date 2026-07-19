//! Phase 2 additive StageProgram IR (0104 §14 / 0105 §7).
//!
//! Legacy scene routes and deck.mdx adapt into a unified product IR without
//! replacing SceneContract / presentation_map or requiring author file changes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::stage_registry::{StageDescriptor, StageId, StageProfile, StageRegistry};
use super::{AdminPageProgram, PageProgram};

/// Stage Surface (viewport / paged / document; Access kept as wire-compat alias).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StageSurface {
    /// Legacy alias — treat as viewport for cockpit programs.
    #[default]
    Access,
    /// Cockpit continuous viewport.
    Viewport,
    /// Slides paged deck surface.
    Paged,
    /// Page document flow (width constrained; block intrinsic; stage scrolls).
    Document,
}

impl StageSurface {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Access | Self::Viewport => "viewport",
            Self::Paged => "paged",
            Self::Document => "document",
        }
    }

    pub fn from_profile(profile: StageProfile) -> Self {
        match profile {
            StageProfile::Cockpit => Self::Viewport,
            StageProfile::Slides => Self::Paged,
            StageProfile::Page => Self::Document,
        }
    }
}

/// Unit kind inside a StageProgram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageUnitKind {
    /// Cockpit: reference to a spatial Scene module.
    SceneRef,
    /// Slides: one ordered deck slide.
    Slide,
}

impl StageUnitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SceneRef => "scene_ref",
            Self::Slide => "slide",
        }
    }
}

/// One ordered unit in a StageProgram.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageUnit {
    pub id: String,
    pub kind: StageUnitKind,
    pub order: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub source_anchor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide_id: Option<String>,
}

/// Slide list input for Slides StageProgram adapter (from presentation_map / Deck).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageSlideInput {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub order: usize,
}

/// Unified Stage product IR (Phase 2 minimum contract + Phase 3 ABI refs/digests).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageProgram {
    pub stage_id: StageId,
    pub profile: StageProfile,
    #[serde(default)]
    pub surface: StageSurface,
    #[serde(default)]
    pub units: Vec<StageUnit>,
    pub state_namespace: String,
    pub source_anchor: String,
    /// Legacy scene_id alias (same string as stage_id). Phase 9: not written to new artifacts.
    #[serde(default, skip_serializing)]
    pub legacy_scene_id: String,
    /// Phase 3: NarrationCatalog key (`narration:{stage_id}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narration_ref: Option<String>,
    /// Phase 3: capability set digest key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_ref: Option<String>,
    /// Phase 3: SceneSlotModule id (`scene:{stage_id}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_module_ref: Option<String>,
    /// Phase 3: structure digest (slots + capabilities; excludes narration text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_digest: Option<String>,
    /// Phase 3: narration digest (caption/notes/timing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narration_digest: Option<String>,
}

impl StageProgram {
    pub fn state_namespace_for(stage_id: &str) -> String {
        format!("stage:{stage_id}")
    }

    /// Adapt a Cockpit StageDescriptor → StageProgram with a single SceneRef unit.
    pub fn from_cockpit(desc: &StageDescriptor) -> Self {
        Self::from_scene_ref_profile(desc, StageProfile::Cockpit)
    }

    /// Adapt a Page StageDescriptor → StageProgram with a single SceneRef unit.
    pub fn from_page(desc: &StageDescriptor) -> Self {
        Self::from_scene_ref_profile(desc, StageProfile::Page)
    }

    fn from_scene_ref_profile(desc: &StageDescriptor, profile: StageProfile) -> Self {
        let stage_id = desc.id.as_str();
        let unit = StageUnit {
            id: stage_id.to_string(),
            kind: StageUnitKind::SceneRef,
            order: 0,
            title: desc.title.clone(),
            source_anchor: desc.source_anchor.clone(),
            scene_ref: Some(stage_id.to_string()),
            slide_id: None,
        };
        Self {
            stage_id: desc.id.clone(),
            profile,
            surface: StageSurface::from_profile(profile),
            units: vec![unit],
            state_namespace: Self::state_namespace_for(stage_id),
            source_anchor: desc.source_anchor.clone(),
            legacy_scene_id: desc.legacy_scene_id.clone(),
            narration_ref: None,
            capability_ref: None,
            slot_module_ref: None,
            structure_digest: None,
            narration_digest: None,
        }
    }

    /// Adapt a Slides StageDescriptor → StageProgram with ordered slide units.
    ///
    /// When `slides` is empty, emits a single deck-level Slide unit so the Program
    /// is still present before presentation_map enrichment.
    pub fn from_slides(desc: &StageDescriptor, slides: &[StageSlideInput]) -> Self {
        let stage_id = desc.id.as_str();
        let units = if slides.is_empty() {
            vec![StageUnit {
                id: stage_id.to_string(),
                kind: StageUnitKind::Slide,
                order: 0,
                title: desc.title.clone(),
                source_anchor: desc.source_anchor.clone(),
                scene_ref: None,
                slide_id: None,
            }]
        } else {
            let mut ordered: Vec<&StageSlideInput> = slides.iter().collect();
            ordered.sort_by_key(|s| s.order);
            ordered
                .into_iter()
                .map(|slide| StageUnit {
                    id: slide.id.clone(),
                    kind: StageUnitKind::Slide,
                    order: slide.order,
                    title: slide.title.clone(),
                    source_anchor: desc.source_anchor.clone(),
                    scene_ref: None,
                    slide_id: Some(slide.id.clone()),
                })
                .collect()
        };
        Self {
            stage_id: desc.id.clone(),
            profile: StageProfile::Slides,
            surface: StageSurface::from_profile(StageProfile::Slides),
            units,
            state_namespace: Self::state_namespace_for(stage_id),
            source_anchor: desc.source_anchor.clone(),
            legacy_scene_id: desc.legacy_scene_id.clone(),
            narration_ref: None,
            capability_ref: None,
            slot_module_ref: None,
            structure_digest: None,
            narration_digest: None,
        }
    }

    pub fn from_descriptor(
        desc: &StageDescriptor,
        slides_by_stage: &BTreeMap<String, Vec<StageSlideInput>>,
    ) -> Self {
        match desc.profile {
            StageProfile::Cockpit => Self::from_cockpit(desc),
            StageProfile::Page => Self::from_page(desc),
            StageProfile::Slides => {
                let slides = slides_by_stage
                    .get(desc.id.as_str())
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                Self::from_slides(desc, slides)
            }
        }
    }

    pub fn unit_ids(&self) -> Vec<&str> {
        self.units.iter().map(|u| u.id.as_str()).collect()
    }

    /// Project a page-profile StageProgram into the generic document PageProgram IR.
    ///
    /// The projection keeps the existing StageProgram wire shape and call sites
    /// intact while making PageProgram the typed wrapper for page consumers.
    pub fn page_program(&self) -> Option<PageProgram> {
        if self.profile != StageProfile::Page {
            return None;
        }

        let root = self
            .units
            .iter()
            .find(|unit| unit.kind == StageUnitKind::SceneRef)?;
        let scene_ref = root.scene_ref.as_deref()?;

        Some(PageProgram::from_scene_ref(
            self.stage_id.as_str(),
            root.title.clone(),
            self.source_anchor.clone(),
            scene_ref,
        ))
    }

    /// Wrap this page-profile stage for an admin resource route.
    pub fn admin_page_program(&self, resource_id: impl Into<String>) -> Option<AdminPageProgram> {
        self.page_program()
            .map(|page| AdminPageProgram::new(resource_id, page))
    }
}

/// App-level index of adapted StagePrograms (keyed by stage_id string).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StageProgramIndex {
    #[serde(default)]
    pub programs: BTreeMap<String, StageProgram>,
}

impl StageProgramIndex {
    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }

    pub fn get(&self, stage_id: &str) -> Option<&StageProgram> {
        self.programs.get(stage_id)
    }

    pub fn contains(&self, stage_id: &str) -> bool {
        self.programs.contains_key(stage_id)
    }

    pub fn stage_ids(&self) -> Vec<&str> {
        self.programs.keys().map(String::as_str).collect()
    }

    /// Build one Program per Registry Stage (T2 never appears in Registry).
    pub fn from_registry(
        registry: &StageRegistry,
        slides_by_stage: &BTreeMap<String, Vec<StageSlideInput>>,
    ) -> Self {
        let mut programs = BTreeMap::new();
        for desc in &registry.stages {
            let program = StageProgram::from_descriptor(desc, slides_by_stage);
            programs.insert(desc.id.as_str().to_string(), program);
        }
        Self { programs }
    }

    /// Stable diagnostic / baseline summary rows (sorted by stage_id).
    pub fn summary_rows(&self) -> Vec<StageProgramSummary> {
        self.programs
            .values()
            .map(|program| StageProgramSummary {
                stage_id: program.stage_id.as_str().to_string(),
                profile: program.profile.as_str().to_string(),
                surface: program.surface.as_str().to_string(),
                source_anchor: program.source_anchor.replace('\\', "/"),
                unit_count: program.units.len(),
                unit_ids: program.units.iter().map(|u| u.id.clone()).collect(),
                state_namespace: program.state_namespace.clone(),
                legacy_scene_id: program.legacy_scene_id.clone(),
            })
            .collect()
    }
}

/// Normalized summary for fixtures / Inspector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageProgramSummary {
    pub stage_id: String,
    pub profile: String,
    pub surface: String,
    pub source_anchor: String,
    pub unit_count: usize,
    pub unit_ids: Vec<String>,
    pub state_namespace: String,
    #[serde(default, skip_serializing)]
    pub legacy_scene_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::stage_registry::StageDescriptor;

    fn cockpit_desc() -> StageDescriptor {
        StageDescriptor {
            id: StageId::new("home"),
            profile: StageProfile::Cockpit,
            title: Some("Home".to_string()),
            source_anchor: "src/scene/home.mei".to_string(),
            is_default: true,
            legacy_scene_id: "home".to_string(),
        }
    }

    fn slides_desc() -> StageDescriptor {
        StageDescriptor {
            id: StageId::new("intro"),
            profile: StageProfile::Slides,
            title: None,
            source_anchor: "src/presentation/intro/intro.deck.mdx".to_string(),
            is_default: true,
            legacy_scene_id: "intro".to_string(),
        }
    }

    fn page_desc() -> StageDescriptor {
        StageDescriptor {
            id: StageId::new("report"),
            profile: StageProfile::Page,
            title: Some("Report".to_string()),
            source_anchor: "src/page/report.mei".to_string(),
            is_default: true,
            legacy_scene_id: "report".to_string(),
        }
    }

    #[test]
    fn cockpit_program_has_single_scene_ref_unit() {
        let program = StageProgram::from_cockpit(&cockpit_desc());
        assert_eq!(program.profile, StageProfile::Cockpit);
        assert_eq!(program.surface, StageSurface::Viewport);
        assert_eq!(program.surface.as_str(), "viewport");
        assert_eq!(program.unit_ids(), vec!["home"]);
        assert_eq!(program.units[0].kind, StageUnitKind::SceneRef);
        assert_eq!(program.units[0].scene_ref.as_deref(), Some("home"));
        assert_eq!(program.state_namespace, "stage:home");
    }

    #[test]
    fn slides_program_preserves_slide_order() {
        let slides = vec![
            StageSlideInput {
                id: "slide-02".to_string(),
                title: Some("Why".to_string()),
                order: 1,
            },
            StageSlideInput {
                id: "slide-01".to_string(),
                title: Some("Cover".to_string()),
                order: 0,
            },
        ];
        let program = StageProgram::from_slides(&slides_desc(), &slides);
        assert_eq!(program.profile, StageProfile::Slides);
        assert_eq!(program.surface, StageSurface::Paged);
        assert_eq!(program.surface.as_str(), "paged");
        assert_eq!(program.unit_ids(), vec!["slide-01", "slide-02"]);
        assert_eq!(program.units[0].slide_id.as_deref(), Some("slide-01"));
        assert_eq!(program.units[0].order, 0);
    }

    #[test]
    fn page_profile_projects_to_page_program_without_changing_stage_wire_shape() {
        let program = StageProgram::from_page(&page_desc());
        let page = program.page_program().expect("page program wrapper");

        assert_eq!(page.page_id, "report");
        assert_eq!(page.title.as_deref(), Some("Report"));
        assert_eq!(page.source_anchor, "src/page/report.mei");
        assert_eq!(page.surface.as_str(), "document");
        assert_eq!(page.root.scene_ref(), "report");

        let stage_value = serde_json::to_value(&program).expect("serialize stage program");
        assert!(
            stage_value.get("page_program").is_none(),
            "PageProgram projection must not alter the existing StageProgram envelope"
        );

        let decoded: StageProgram =
            serde_json::from_value(stage_value).expect("deserialize stage program");
        assert_eq!(
            decoded
                .page_program()
                .expect("decoded page wrapper")
                .root
                .scene_ref(),
            "report"
        );
    }

    #[test]
    fn index_from_registry_covers_each_stage() {
        let registry = StageRegistry {
            stages: vec![cockpit_desc(), slides_desc()],
            default_stage_id: Some(StageId::new("home")),
        };
        let mut slides = BTreeMap::new();
        slides.insert(
            "intro".to_string(),
            vec![StageSlideInput {
                id: "s1".to_string(),
                title: None,
                order: 0,
            }],
        );
        let index = StageProgramIndex::from_registry(&registry, &slides);
        assert!(index.contains("home"));
        assert!(index.contains("intro"));
        assert!(!index.contains("warnings_analytics_page"));
        assert_eq!(index.get("intro").map(|p| p.unit_ids()), Some(vec!["s1"]));
    }
}
