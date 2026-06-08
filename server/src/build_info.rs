use serde_json::{json, Value};

pub const MAJOR_VERSION: &str = env!("MEI_MAJOR_VERSION");
pub const INTERNAL_VERSION: &str = env!("MEI_INTERNAL_VERSION");
pub const BUILD_VERSION: &str = env!("MEI_BUILD_VERSION");
pub const GIT_COMMIT_SHORT: &str = env!("MEI_GIT_COMMIT_SHORT");
pub const GIT_COMMIT_FULL: &str = env!("MEI_GIT_COMMIT_FULL");
pub const GIT_BRANCH: &str = env!("MEI_GIT_BRANCH");
pub const GIT_DIRTY: &str = env!("MEI_GIT_DIRTY");
pub const BUILD_TARGET_TAG: &str = env!("MEI_BUILD_TARGET_TAG");
pub const BUILD_TIMESTAMP_UTC: &str = env!("MEI_BUILD_TIMESTAMP_UTC");
pub const CARGO_PACKAGE_VERSION: &str = env!("MEI_CARGO_PACKAGE_VERSION");

pub fn version_label() -> String {
    format!("Mei {CARGO_PACKAGE_VERSION} · {INTERNAL_VERSION}")
}

pub fn version_label_with_target() -> String {
    if BUILD_TARGET_TAG == "unknown" {
        version_label()
    } else {
        format!("Mei {CARGO_PACKAGE_VERSION} · {INTERNAL_VERSION} ({BUILD_TARGET_TAG})")
    }
}

pub fn descriptor() -> Value {
    json!({
        "build_version": BUILD_VERSION,
        "cargo_package_version": CARGO_PACKAGE_VERSION,
        "major_version": MAJOR_VERSION,
        "internal_version": INTERNAL_VERSION,
        "git": {
            "commit_short": GIT_COMMIT_SHORT,
            "commit_full": GIT_COMMIT_FULL,
            "branch": GIT_BRANCH,
            "dirty": GIT_DIRTY == "true",
        },
        "build_target_tag": BUILD_TARGET_TAG,
        "build_timestamp_utc": BUILD_TIMESTAMP_UTC,
    })
}
