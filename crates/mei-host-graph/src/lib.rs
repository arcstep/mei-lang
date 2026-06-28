//! Graph registry, CAS, import-bundle, and v2 assemble for mei-host-shell.

mod projection_normalize;
mod assemble;
mod data_snapshot;
mod v2_lower;
mod v2_bundle_constants;
mod v2_metric_lower;
mod bridge;
mod content_store;
mod import;
mod io;
mod mcg;
mod metric_hydrate;
mod mrg;
mod panel_constants;
mod paths;
mod types;

pub use assemble::{assemble_scope_from_registry, list_scope_routes, AssembleOutcome, ScopeRoute};
pub use data_snapshot::{
    collect_app_xlsx_sources, publish_app_data_snapshots, PublishDataSnapshotsReport,
};
pub use import::{import_bundle, load_block_artifact, ImportOptions};
pub use mcg::registry::{McgRegistry, McgRegistryWriter};
pub use mrg::client_bootstrap::{
    bootstrap_embed_allowed, clear_client_bootstrap_for_scope, clear_client_bootstraps_for_stale_scopes,
    read_client_bootstrap, write_client_bootstrap, ClientBootstrapManifest,
};
pub use mrg::frontier::{
    collect_eval_frontier, collect_eval_frontier_with_hops, record_navigation_edges_for_scope,
    FrontierMetric,
};
pub use mrg::registry::{MrgRegistry, MrgRegistryWriter};
pub use mrg::slots::{
    default_metric_response_descriptor, mark_slots_stale_for_bundles, record_slot_failed,
    record_slot_from_descriptor, record_slots_from_descriptors, MRG_REGISTRY_SCHEMA_V3,
};
pub use mrg::telemetry::{flush_telemetry_to_registry, mrg_status_json, record_access, MrgAccessKind};
pub use mrg::tier::{compute_client_revision, WarmupTier};
pub use mrg::warmup::{record_navigation_edge, warm_frontier_slots, WarmupFrontierOutcome};
pub use paths::{bridge_path, mcg_registry_path, mrg_registry_path, resolve_graph_root};
pub use types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};
