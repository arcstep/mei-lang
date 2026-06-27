//! Layer/block toolchain orchestration — SSOT for compile, verify, eval, inspect.

mod compile;
mod eval;
mod hints;
mod id;
mod inspect;
mod layer;
mod list;
mod orchestrator;
mod types;
mod verify;

pub use id::{parse_block_id, parse_material_states};
pub use types::BlockLayer;

pub use eval::{block_eval, materialize_worksets, BlockEvalRequest};
pub use hints::{
    block_compile_hint, block_eval_hint, block_list_hint, collect_prebuild_failed_block_hints,
    fast_loop_hints, layer_verify_hint, prebuild_warning_hint,
};
pub use layer::{layer_compile, layer_inspect, layer_status, LayerCompileOptions};
pub use list::block_list;
pub use orchestrator::BlockOrchestrator;
pub use verify::layer_verify;
