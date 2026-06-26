mod revision;
mod singleflight;
mod access_slim;

mod prelude;
mod types;
mod store;
mod lookup;
mod load;
mod compile;
mod invalidate;

#[cfg(test)]
mod tests;

pub(crate) use revision::{compile_revision, components_revision, normalize_path};
pub(crate) use singleflight::{
    compile_singleflight_enabled, finish_compile_inflight,
    register_compile_inflight, wait_for_compile_inflight,
};

pub(crate) use store::*;
pub(crate) use lookup::*;
pub(crate) use load::*;
pub(crate) use invalidate::*;

pub use access_slim::{
    access_slim_artifacts_enabled, canonical_artifact_persist_enabled,
    locked_cache_env_overrides, should_persist_compiled_app_artifact,
    slim_compiled_app_for_access, strip_loaded_compiled_app_for_access,
};
pub use singleflight::env_flag_enabled;

pub use types::{
    CompileWithCacheFailure, CompileWithCacheOutcome, CompileWithCacheOutcomeShared,
    PeekCompileCacheHit, PeekCompileCacheHitShared,
};
pub use load::{probe_compiled_app_manifest_identity};
pub use compile::{
    compile_app_with_cache, compile_app_with_cache_shared,
    load_compile_artifact_only, load_compile_artifact_only_shared, recent_compile_failure,
};
pub use invalidate::{
    clear_compile_cache_for_app, clear_compiled_app_artifacts_for_app, compile_cache_key,
    is_compile_inflight, peek_compile_cache, peek_compile_cache_hit, peek_compile_cache_hit_shared,
    peek_compile_cache_shared, resolve_components_root,
};
