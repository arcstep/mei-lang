//! Mei snapshot pack/unpack library (`mei-snapshot` format v1 + portable v2).
//!
//! See docs/mei-lang-v2/05-host/0541-desktop-viewer-implementation-plan.md.

mod manifest;
mod pack;
mod paths;
mod portable_config;
mod resources;
mod unpack;

pub use manifest::{
    DataModeHint, ManifestFileEntry, SnapshotAppEntry, SnapshotManifest, FORMAT_NAME,
    FORMAT_VERSION, FORMAT_VERSION_V1, FORMAT_VERSION_V2,
};
pub use pack::{pack_portable_snapshot, pack_snapshot, PackOptions, PortablePackOptions};
pub use paths::{resolve_app_env_root, resolve_bundle_path};
pub use portable_config::{build_portable_app_toml, PortableConfigResult, PortableSource};
pub use resources::{
    ResourceEntry, ResourceSeverity, ResourceState, ResourcesDocument,
};
pub use unpack::{unpack_snapshot, UnpackResult};

/// Desktop Viewer readiness probe contract (control plane).
pub mod readiness {
    /// Path on the local host-shell HTTP server.
    pub const PATH: &str = "/api/host/readiness";

    /// JSON fields that must be true before the desktop WebView navigates.
    pub const REQUIRED_TRUE_FIELDS: &[&str] = &["hostReady", "controlReady"];

    /// Optional field indicating data plane / import readiness.
    pub const ACCESS_READY_FIELD: &str = "accessReady";
}

/// Marker filename written into portable app dirs and materialized workspaces.
pub const PORTABLE_SNAPSHOT_MARKER: &str = ".mei-portable-snapshot";
