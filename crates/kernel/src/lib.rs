mod compile;
mod eval;
mod geojson;
mod model;
mod runtime;
mod runtime_resource_index;
mod typed_refs;
mod workspace;

pub use compile::{
    compile_app, compile_app_from_root, compile_app_from_root_with_options,
    compile_app_with_options, dataset_materialize_cache_epoch,
    evaluate_runtime_metric_defs, panel_resolved_has_head, scene_payload_cache_epoch,
    CompileOptions,
};

pub use geojson::{parse_geojson_rows, rows_from_geojson_value};
pub use eval::{describe_dsl, evaluate_mei_file, evaluate_mei_source};
pub use model::{
    BlockDecl, ColumnSchema, CompiledApp, CompiledSceneRoute, ComponentAsset, DataRef,
    DataTransform, DatasetSourceRef, DatasetView, Diagnostic, FlowDecl, FrameDecl, LayoutDecl,
    LoadedResource, MetricContract, MetricPackContract, MetricRef, MetricShape, PanelDecl,
    ResourceDecl, RuleClickDecl, RuleEffectDecl, RuleOutcomeDecl, RuleRequireDecl, RuleStartDecl,
    RuleSubjectTimerDecl, RuleTimerDecl, SceneContract, SceneDecl, Severity, SourceDecl, ThemeDecl,
    PanelRefEmbedDecl, UiNodeDecl, WorldMetricLedgerEntry, WorkspaceAppMeta, WorkspaceNode,
    WorldCellDecl,
};
pub use runtime::{
    initial_runtime_state, project_runtime_view, render_runtime_html, runtime_step, RuntimeIntent,
    RuntimeSceneView, RuntimeState, RuntimeSubjectTimerState, RuntimeTraceItem,
};
pub use typed_refs::{
    decode_binding_value, decode_ref_value, ref_to_json, BindingValue, RefExpr, RefKind,
    SceneLocator, SceneRegistry,
};
pub use runtime_resource_index::{
    build_runtime_resource_index, build_runtime_resource_map, is_forbidden_legacy_resource_id,
    locate_dataset_resource, resolve_dataset_resource_id, resolve_dataset_selector_value,
    RuntimeResourceIndex, RuntimeResourceResolveError,
};
pub use workspace::{discover_apps, load_component_assets, read_source_file, source_tree};
