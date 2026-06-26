use super::prelude::*;
use super::*;

pub fn workspace_runtime_version_descriptor() -> WorkspaceRuntimeVersionDescriptor {
    WorkspaceRuntimeVersionDescriptor {
        schema_version: WORKSPACE_RUNTIME_VERSION_SCHEMA_VERSION.to_string(),
        toolchain_version: TOOLCHAIN_VERSION.to_string(),
        source_revision: runtime_source_revision(),
        compatibility: RuntimeCompatibilityDescriptor {
            line: COMPATIBILITY_LINE.to_string(),
            bundle_schema: RUNTIME_BUNDLE_SCHEMA_VERSION.to_string(),
            catalog_schema: CAPABILITY_CATALOG_SCHEMA_VERSION.to_string(),
        },
        installed_runtime: InstalledRuntimeDescriptor {
            runtime_id: runtime_bundle_id(),
            target_triple: TARGET_TRIPLE.to_string(),
        },
        generated_at: now_timestamp_utc(),
    }
}

pub fn workspace_runtime_manifest_for_package_root(
    package_root: &Path,
) -> WorkspaceRuntimeManifest {
    WorkspaceRuntimeManifest {
        schema_version: WORKSPACE_RUNTIME_MANIFEST_SCHEMA_VERSION.to_string(),
        bundle_id: runtime_bundle_id(),
        toolchain_version: TOOLCHAIN_VERSION.to_string(),
        source_revision: runtime_source_revision(),
        compatibility_line: COMPATIBILITY_LINE.to_string(),
        bundle_schema: RUNTIME_BUNDLE_SCHEMA_VERSION.to_string(),
        target_triple: TARGET_TRIPLE.to_string(),
        artifacts: RuntimeManifestArtifactDescriptor {
            mei_toolchain: "bin/mei-toolchain".to_string(),
            mei_lsp: "bin/mei-lsp".to_string(),
            mei_host_web: "bin/mei-host-web".to_string(),
            author_mcp_adapter: "bin/author-mcp-adapter".to_string(),
            access_mcp_adapter: "bin/access-mcp-adapter".to_string(),
        },
        content: RuntimeManifestContentDescriptor {
            capability_catalog: "share/mei/catalog/capability-catalog.json".to_string(),
            author_surface: "share/mei/catalog/author-surface.json".to_string(),
            access_surface: "share/mei/catalog/access-surface.json".to_string(),
            knowledge_path: "share/mei/knowledge/author".to_string(),
            platform_assets_path: "share/mei/platform-assets/stock".to_string(),
            tooling_templates_path: "share/mei/tooling-templates".to_string(),
        },
        provenance: RuntimeManifestProvenance {
            built_at: BUILD_TIMESTAMP_UTC.to_string(),
            built_from: "source-tree-bootstrap".to_string(),
            package_root: package_root_hint(package_root),
        },
    }
}

pub fn editor_runtime_descriptor_for_package_root(package_root: &Path) -> EditorRuntimeDescriptor {
    let editor_knowledge_bundle =
        knowledge_bundle_descriptor_for_package_root(package_root, "author")
            .expect("author bundle");
    EditorRuntimeDescriptor {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        package_root: package_root_hint(package_root),
        declared_layout: declared_layout(),
        current_source_layout: current_source_layout(),
        package_root_resolution: vec![
            "Prefer MEI_PACKAGE_ROOT when explicitly provided.".to_string(),
            "Otherwise infer from the current executable and prefer a sibling share/mei layout when present.".to_string(),
            "Fallback to source-tree package root for local development builds.".to_string(),
        ],
        standalone_flow: vec![
            "Run `mei-toolchain workspace init --standalone --source-root <dir>` to create a source workspace skeleton.".to_string(),
            "Run `mei-toolchain workspace bootstrap --source-root <dir>` to create a workspace (stock is copied automatically).".to_string(),
            "Run `mei-toolchain workspace runtime install --source-root <dir>` to install workspace-local .mei runtime assets and `./start.sh`.".to_string(),
            "Run `./start.sh` from the workspace root to launch the MeiLang host.".to_string(),
            "Run `mei-toolchain editor-runtime scaffold --target-root <dir> --tool <tool>` to write tool glue files only.".to_string(),
            "Run `mei-toolchain knowledge --surface author --include-content --json` to export packaged authoring docs/examples.".to_string(),
            "Use `mei-lsp` for IDE semantics and `node scripts/mcp/mei-author-stdio-adapter.mjs` for agent-side tools.".to_string(),
        ],
        tooling_templates: tooling_templates(),
        editor_knowledge_bundle,
    }
}

pub fn doctor_editor_runtime_for_package_root(package_root: &Path) -> EditorRuntimeDoctorReport {
    let EditorRuntimeDescriptor {
        current_source_layout,
        editor_knowledge_bundle,
        ..
    } = editor_runtime_descriptor_for_package_root(package_root);
    let mut checks = current_source_layout
        .into_iter()
        .map(|item| {
            let path = package_root.join(&item.rel_path);
            EditorRuntimeCheck {
                id: item.id,
                ok: path.exists(),
                path: path.display().to_string(),
                message: if path.exists() {
                    format!("source-backed runtime asset present: {}", item.purpose)
                } else {
                    format!("missing source-backed runtime asset: {}", item.purpose)
                },
            }
        })
        .collect::<Vec<_>>();
    checks.extend(editor_knowledge_bundle.assets.into_iter().map(|asset| {
        let path = package_root.join(&asset.relative_path);
        EditorRuntimeCheck {
            id: format!("knowledge_asset:{}", asset.id),
            ok: path.exists(),
            path: path.display().to_string(),
            message: if path.exists() {
                format!(
                    "packaged knowledge asset present for topic `{}`",
                    asset.topic
                )
            } else {
                format!(
                    "missing packaged knowledge asset for topic `{}`",
                    asset.topic
                )
            },
        }
    }));
    let ok = checks.iter().all(|check| check.ok);
    EditorRuntimeDoctorReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        ok,
        package_root: package_root.display().to_string(),
        workspace_root: None,
        checks,
    }
}
