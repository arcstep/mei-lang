//! Mei snapshot pack/unpack library (`mei-snapshot` format v1).
//!
//! See docs/mei-lang-v2/05-host/0541-desktop-viewer-implementation-plan.md.

mod manifest;
mod pack;
mod paths;
mod unpack;

pub use manifest::{DataModeHint, ManifestFileEntry, SnapshotManifest, FORMAT_NAME, FORMAT_VERSION};
pub use pack::{pack_snapshot, PackOptions};
pub use paths::{resolve_app_env_root, resolve_bundle_path};
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
