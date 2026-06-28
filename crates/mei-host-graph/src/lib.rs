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
pub use mrg::registry::{MrgRegistry, MrgRegistryWriter};
pub use mrg::slots::{default_metric_response_descriptor, record_slot_from_descriptor, record_slots_from_descriptors};
pub use paths::{bridge_path, mcg_registry_path, mrg_registry_path, resolve_graph_root};
pub use types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};
