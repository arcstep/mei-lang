mod app_skeleton;
mod artifact_expand;
mod deck;
mod expand;
mod lower;
mod registry;
mod stage_closure;
mod stage_mdx;
mod workspace;
mod world_expand;

pub use artifact_expand::{
    collect_template_imports, expand_artifact_value, json_to_expr, try_expand_artifact_macro_call,
};
pub use deck::{deck_to_v2, DeckBuildError};
pub use expand::{expand_artifact_expr, expand_v2_file, ExpandError};
pub use lower::{lower_v2_file, GraphBlock, GraphOutcome, LowerGraphError};
pub use registry::{MacroRegistry, TemplateRoots};
pub use app_skeleton::synthesize_app_skeleton;
pub use stage_closure::synthesize_stage_access_navigation;
pub use stage_mdx::{cockpit_stage_to_graph, compile_cockpit_stage_file};
pub use workspace::{compile_app, resolve_workspace_config_path, CompileAppError, CompileOutcome};
pub use world_expand::{expand_world_v2_file, WorldContextCatalog, WorldExpandError};
