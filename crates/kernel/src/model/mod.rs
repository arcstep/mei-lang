mod abi_project;
mod build_node;
mod build_view_index;
mod compile_out;
mod content_capability_abi;
mod contract;
mod dataset;
mod diagnostic;
mod layout;
mod narration_abi;
mod object_catalog;
mod page_program;
mod presentation_map_schema;
mod profile_layout_policy;
mod resource;
mod review_modes;
mod scene_slot_abi;
mod stage_mdx_apply;
mod stage_program;
mod stage_registry;
mod ui;
mod ui_layout_index;
mod ui_node;
mod workspace;
mod world;
mod world_semantic;

pub use abi_project::{
    bind_programs_to_abi, compute_narration_digest, compute_structure_digest,
    diagnose_slot_missing, project_abi, validate_abi_against_programs, AbiProjection,
    AbiProjectionInput,
};
pub use build_node::{
    resolve_build_view_query, tab_visible_for_node, tabs_for_node_kind, BuildExecScope,
    BuildNodeId, BuildNodeKind, BuildViewTab, LegacyBuildQuery, ProvenanceAnchor,
    ResolvedBuildViewQuery,
};
pub use build_view_index::{
    BuildExperienceIndex, BuildT2PageIndex, BuildTemplateIndex, ExperienceNodeManifest,
    MountChainEntry, ReachabilityTreeNodeSnapshot, ReachabilityTreeRootSnapshot, T2PageFileEntry,
    T2PageSlotEntry, TemplateCatalogEntry, TemplateConsumerAnchor,
};
pub use compile_out::{CompiledApp, CompiledSceneRoute};
pub use content_capability_abi::{ContentCapability, ContentCapabilityId, ContentCapabilityKind};
pub use contract::SceneContract;
pub use dataset::{
    AnalysisEdge, AnalysisGraph, AnalysisNode, ColumnSchema, DataRef, DataTransform,
    DatasetSourceRef, DatasetView, DimensionBinding, FilterIntent, FilterIntentSource,
    FilterOperator, MetricContract, MetricPackContract, MetricRef, MetricShape, QueryState,
    QueryTimeRange, SemanticEdgeKind, SemanticNodeKind, WorldMetricLedgerEntry,
};
pub use diagnostic::{Diagnostic, Severity};
pub use layout::{AppDecl, FrameDecl, LayoutDecl};
pub use narration_abi::{NarrationCatalog, NarrationCue, NarrationCueTarget, NarrationTrack};
pub use object_catalog::{
    derive_object_field_links, DefaultObjectAssembly, InteractionBinding, InteractionEvent,
    InteractionIntent, InteractionSubject, ObjectCatalog, ObjectCatalogAuthoringMode,
    ObjectCatalogDiagnostic, ObjectDescriptor, ObjectFieldLinkKeyMode, ObjectFieldLinkResolve,
    ObjectFieldLinkTarget, ObjectFocus, ObjectFocusCardinality, ObjectIdentityContract,
    ObjectIdentityMaterialization, ObjectIndexEntry, ObjectIntent, ObjectLocator,
    ObjectMaterializationError, ObjectProjectionRef, ObjectRecipeContract,
    ObjectRecipeInteractionContract, ObjectRecipeProjectionAssembly,
    ObjectRecipeProjectionContract, ObjectRecipeProjectionState, ObjectRecipeResponderContract,
    ObjectRecipeSlotContract, ObjectRecipeSlotRequirement, ObjectResolver, ObjectSet,
    ObjectTypeContract, Responder, RuntimeObjectIndex, RuntimeObjectIndexEntry,
    DEFAULT_OBJECT_ASSEMBLY_KIND, INTERACTION_PROTOCOL_SCHEMA_VERSION,
    OBJECT_CATALOG_SCHEMA_VERSION, OBJECT_INDEX_ENTRY_KIND, OBJECT_RECIPE_SCHEMA_VERSION,
};
pub use page_program::{AdminPageProgram, PageProgram};
pub use presentation_map_schema::{
    accept_presentation_map, presentation_map_is_absent, presentation_map_schema_ok,
    presentation_map_schema_version, PRESENTATION_MAP_SCHEMA_VERSION,
};
pub use profile_layout_policy::{
    profile_layout_policy_digest, FillDownPolicy, ProfileLayoutPolicy, ProfileSpacingTokens,
    ScrollOwnership, SizeAxisPolicy,
};
pub use resource::{LoadedResource, ResourceDecl, SourceDecl};
pub use review_modes::{
    ui_role_depth_rank, ui_role_within_max_depth, DataMode, DataModeCeiling, ReviewProjection,
    SurfacePreviewPolicy,
};
pub use scene_slot_abi::{SceneSlotModule, SceneSlotModuleId, SemanticSlotDecl, SlotCardinality};
pub use stage_mdx_apply::{
    apply_cockpit_stage_decl, CockpitFillDecl, CockpitStageDecl, CockpitStepDecl,
};
pub use stage_program::{
    StageProgram, StageProgramIndex, StageProgramSummary, StageSlideInput, StageSurface, StageUnit,
    StageUnitKind,
};
pub use stage_registry::{
    is_stage_registry_candidate, StageDescriptor, StageId, StageProfile, StageRegistry, StageRoute,
};
pub use ui::{
    deserialize_ui_node_value, BlockDecl, ComponentExportDecl, FrameExportDecl, PanelExportDecl,
    PanelRefEmbedDecl, SceneExportDecl, ThemeDecl, UiTreeNode,
};
pub use ui_layout_index::{
    LayoutBudgetManifest, LayoutBudgetManifestEntry, UiBudgetSummary, UiLayoutIndex, UiScopeNode,
    UiScopeRole, UiSourceAnchor,
};
pub use ui_node::{PanelSlotDecl, UiNodeDecl};
pub use workspace::{ComponentAsset, WorkspaceAppMeta, WorkspaceNode};
pub use world::{
    EntityDecl, FlowDecl, RuleClickDecl, RuleEffectDecl, RuleOutcomeDecl, RuleRequireDecl,
    RuleStartDecl, RuleSubjectTimerDecl, RuleTimerDecl, WorldCellDecl, WorldDecl, WorldGridDecl,
};
pub use world_semantic::{
    WorldSemanticDataset, WorldSemanticExplainBlock, WorldSemanticFileIndex, WorldSemanticMetric,
};

pub use ui::SceneDecl;
