mod compile_out;
mod contract;
mod dataset;
mod diagnostic;
mod layout;
mod panel;
mod resource;
mod ui;
mod workspace;
mod world;

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
pub use ui::{
    deserialize_ui_node_value, BlockDecl, ComponentExportDecl, FrameExportDecl,
    PanelExportDecl, PanelRefEmbedDecl, SceneExportDecl, ThemeDecl, UiNodeDecl,
};
pub use workspace::{ComponentAsset, WorkspaceAppMeta, WorkspaceNode};
pub use world::{
    EntityDecl, FlowDecl, RuleClickDecl, RuleEffectDecl, RuleOutcomeDecl, RuleRequireDecl,
    RuleStartDecl, RuleSubjectTimerDecl, RuleTimerDecl, WorldCellDecl, WorldDecl, WorldGridDecl,
};

pub use ui::SceneDecl;
