//! Shared App Runtime instance contracts for Host control plane and runtimes.

mod build_protocol;
mod cache_partition;
mod ephemeral_overlay;
mod host_control;
mod instance_spec;
mod instance_store;
mod launch_manifest;
mod observed_instance;
mod paths;
mod runtime_state;

pub use build_protocol::{
    BuildAppArtifact, BuildPhaseReport, BuildRequest, BuildResult, SCHEMA_BUILD_REQUEST_V1,
    SCHEMA_BUILD_RESULT_V1,
};
pub use cache_partition::{partition_cache_key, CachePartitionKey};
pub use ephemeral_overlay::{
    clear_runtime_overlay, effective_runtime_plan, read_runtime_overlay, write_runtime_overlay,
    RuntimeOverlayError, RuntimeOverlayTarget, RuntimePolicyOverlay, SCHEMA_RUNTIME_OVERLAY_V1,
};
pub use host_control::{
    host_control_path, read_host_control_state, write_host_control_state,
    write_if_revision_matches, ActiveProfileRef, HostControlConflict, HostControlState,
    SCHEMA_HOST_CONTROL_V1, SCHEMA_HOST_CONTROL_V2,
};
pub use instance_spec::{BundleRef, ConfigSnapshot, InstanceSpec, SCHEMA_INSTANCE_SPEC_V1};
pub use instance_store::{
    clear_app_ephemeral_runtime, instance_spec_path, list_instance_runtime_ids, read_instance_spec,
    read_instance_spec_for_app, write_instance_spec,
};
pub use launch_manifest::{
    DesiredInstance, DesiredState, LastSuccessfulApply, LaunchManifest, RouteBinding,
    SCHEMA_LAUNCH_MANIFEST_V1,
};
pub use observed_instance::{
    InstanceHealth, InstancePhase, InstanceResource, InstanceRevisions, ObservedInstance,
};
pub use paths::{
    instance_bootstrap_dir, instance_eval_cache_dir, instance_logs_dir, instance_meta_dir,
    instance_mrg_disk_dir, instance_mrg_memory_dir, instance_runtime_root, instance_var_dir,
    legacy_instance_runtime_root, pinned_generation_root,
};
pub use runtime_state::{AppRuntimeState, RuntimeContext};

/// Internal Host → Runtime identity headers (loopback only; never browser cookies).
pub const HEADER_INSTANCE_TOKEN: &str = "x-mei-instance-token";
pub const HEADER_INSTANCE_ID: &str = "x-mei-instance-id";
pub const HEADER_APP_ID: &str = "x-mei-app-id";
pub const HEADER_GENERATION: &str = "x-mei-generation";
pub const HEADER_SPEC_DIGEST: &str = "x-mei-spec-digest";
pub const HEADER_PRINCIPAL: &str = "x-mei-principal";

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
