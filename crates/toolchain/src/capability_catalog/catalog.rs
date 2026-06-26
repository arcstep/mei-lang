use super::prelude::*;
use super::*;

pub fn capability_catalog_descriptor() -> Value {
    json!(capability_catalog_descriptor_for_package_root(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .as_path()
    ))
}

fn capability_catalog_descriptor_for_roots(
    package_root: &Path,
    workspace_root: Option<&Path>,
) -> Value {
    let workspace_root_marker = workspace_root.map(|_| ".".to_string());
    json!({
        "schema_version": CAPABILITY_CATALOG_SCHEMA_VERSION,
        "toolchain_role": "canonical_truth",
        "workspace_root": workspace_root_marker,
        "principles": [
            "toolchain_is_canonical_truth",
            "host_is_canonical_consumer",
            "ai_capability_catalog_is_single_source",
            "platform_assets_are_first_class",
            "host_specific_capability_must_register_before_export"
        ],
        "ai_profiles": [
            author_profile_descriptor(),
            access_profile_descriptor()
        ],
        "platform_assets": match workspace_root {
            Some(source_root) => {
                platform_asset_catalog_descriptor_for_workspace_root(source_root)
            }
            None => platform_asset_catalog_descriptor_for_package_root(package_root),
        },
        "skill_packages": [
            meilang_author_skill_package(),
            meilang_access_skill_package()
        ],
        "knowledge_bundles": [
            knowledge_bundle_descriptor_for_package_root(package_root, "author")
                .expect("author knowledge bundle"),
            knowledge_bundle_descriptor_for_package_root(package_root, "access")
                .expect("access knowledge bundle")
        ],
        "host_extensions": host_extension_registry_descriptor(),
        "host_requirements": [
            host_requirements_descriptor("mei-host-web").expect("mei-host-web requirements")
        ],
        "mcp_surfaces": [
            mcp_surface_descriptor_for_roots("author", package_root, workspace_root)
                .expect("author surface"),
            mcp_surface_descriptor_for_roots("access", package_root, workspace_root)
                .expect("access surface")
        ]
    })
}

pub fn capability_catalog_descriptor_for_package_root(package_root: &Path) -> Value {
    capability_catalog_descriptor_for_roots(package_root, None)
}

pub fn capability_catalog_descriptor_for_workspace_root(
    workspace_root: &Path,
    package_root: &Path,
) -> Value {
    capability_catalog_descriptor_for_roots(package_root, Some(workspace_root))
}
