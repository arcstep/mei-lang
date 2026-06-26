mod prelude;
mod types;
mod materialize;
mod doctor;
mod migrate;
mod manifest;
mod profile;

#[cfg(test)]
mod tests;

pub(crate) use materialize::*;
pub(crate) use manifest::*;

pub use types::*;
pub use materialize::{
    ensure_workspace_stock_materialized, materialize_workspace_stock, sync_workspace_stock,
};
pub use doctor::{doctor_workspace_stock, ensure_stock_catalog_app_synced};
pub use migrate::migrate_workspace_stock_paths;
pub use manifest::workspace_stock_revision;
pub use profile::{create_app_skeleton, init_workspace_profile};
