mod doctor;
mod manifest;
mod materialize;
mod migrate;
mod prelude;
mod profile;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use manifest::*;
pub(crate) use materialize::*;

pub use doctor::{doctor_workspace_stock, ensure_stock_catalog_app_synced};
pub use manifest::workspace_stock_revision;
pub use materialize::{
    ensure_workspace_stock_materialized, materialize_workspace_stock, sync_workspace_stock,
};
pub use migrate::migrate_workspace_stock_paths;
pub use profile::{create_app_skeleton, init_workspace_profile};
pub use types::*;
