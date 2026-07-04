mod build_node;
mod build_view_index;
mod ui_layout_index;
mod compile_out;
mod contract;
mod dataset;
mod diagnostic;
mod layout;
mod panel;
mod resource;
mod review_modes;
mod ui;
mod workspace;
mod world;
mod world_semantic;

pub use build_node::{
    resolve_build_view_query, tab_visible_for_node, tabs_for_node_kind, BuildExecScope,
    BuildNodeId, BuildNodeKind, BuildViewTab, LegacyBuildQuery, ProvenanceAnchor,
    ResolvedBuildViewQuery,
};
pub use build_view_index::{
    BoardFileEntry, BoardSlotEntry, BuildBoardIndex, BuildExperienceIndex, BuildTemplateIndex,
    ExperienceNodeManifest, MountChainEntry, ReachabilityTreeNodeSnapshot,
    ReachabilityTreeRootSnapshot, TemplateCatalogEntry, TemplateConsumerAnchor,
};
pub use ui_layout_index::{
    LayoutBudgetManifest, LayoutBudgetManifestEntry, UiBudgetSummary, UiLayoutIndex, UiScopeNode,
    UiScopeRole, UiSourceAnchor,
};
pub use compile_out::{CompiledApp, CompiledSceneRoute};
pub use contract::SceneContract;
pub use dataset::{
    AnalysisEdge, AnalysisGraph, AnalysisNode, ColumnSchema, DataRef, DataTransform,
    DatasetSourceRef, DatasetView, DimensionBinding, FilterIntent, FilterIntentSource,
    FilterOperator, MetricContract, MetricPackContract, MetricRef, MetricShape, QueryState,
    QueryTimeRange, SemanticEdgeKind, SemanticNodeKind, WorldMetricLedgerEntry,
};
pub use diagnostic::{Diagnostic, Severity};
pub use layout::{AppDecl, FrameDecl, LayoutDecl};
pub use panel::{PanelDecl, PanelSlotDecl};
pub use resource::{LoadedResource, ResourceDecl, SourceDecl};
pub use review_modes::{DataMode, DataModeCeiling, ReviewProjection};
pub use ui::{
    deserialize_ui_node_value, BlockDecl, ComponentExportDecl, FrameExportDecl, PanelExportDecl,
    PanelRefEmbedDecl, SceneExportDecl, ThemeDecl, UiNodeDecl,
};
pub use workspace::{ComponentAsset, WorkspaceAppMeta, WorkspaceNode};
pub use world::{
    EntityDecl, FlowDecl, RuleClickDecl, RuleEffectDecl, RuleOutcomeDecl, RuleRequireDecl,
    RuleStartDecl, RuleSubjectTimerDecl, RuleTimerDecl, WorldCellDecl, WorldDecl, WorldGridDecl,
};
pub use world_semantic::{
    WorldSemanticDataset, WorldSemanticExplainBlock, WorldSemanticFileIndex, WorldSemanticMetric,
};

pub use ui::SceneDecl;
