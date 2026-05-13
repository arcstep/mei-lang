#[path = "compile_new.rs"]
mod compile;
mod eval;
mod model;
mod runtime;
mod workspace;

pub use compile::{compile_app, compile_app_from_root};
pub use eval::{describe_dsl, evaluate_mei_file, evaluate_mei_source};
pub use model::{
    BlockDecl, ColumnSchema, CompiledApp, ComponentAsset, DataRef, DataTransform, DatasetSourceRef,
    DatasetView, Diagnostic, EntryDecl, FlowDecl, FrameDecl, LayoutDecl, LoadedResource,
    MetricContract, MetricPackContract, MetricRef, MetricShape, PanelDecl, ResourceDecl,
    RuleClickDecl, RuleEffectDecl, RuleOutcomeDecl, RuleRequireDecl, RuleStartDecl,
    RuleSubjectTimerDecl, RuleTimerDecl, SceneContract, SceneDecl, Severity, SourceDecl, ThemeDecl,
    UiNodeDecl,
    WorkspaceAppMeta, WorkspaceNode, WorldCellDecl,
};
pub use runtime::{
    initial_runtime_state, project_runtime_view, render_runtime_html, runtime_step, RuntimeIntent,
    RuntimeSceneView, RuntimeState, RuntimeSubjectTimerState, RuntimeTraceItem,
};
pub use workspace::{discover_apps, load_component_assets, read_source_file, source_tree};
