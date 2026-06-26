use super::prelude::*;
use super::*;

pub(crate) fn json_value_matches(
    path: &Path,
    id: &str,
    message_prefix: &str,
    predicate: impl Fn(&Value) -> bool,
) -> EditorRuntimeCheck {
    match fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Value>(&contents) {
            Ok(value) => {
                let ok = predicate(&value);
                EditorRuntimeCheck {
                    id: id.to_string(),
                    ok,
                    path: path.display().to_string(),
                    message: if ok {
                        format!("{message_prefix}: metadata matches expected toolchain identity")
                    } else {
                        format!(
                            "{message_prefix}: metadata does not match expected toolchain identity"
                        )
                    },
                }
            }
            Err(error) => EditorRuntimeCheck {
                id: id.to_string(),
                ok: false,
                path: path.display().to_string(),
                message: format!("{message_prefix}: failed to parse json ({error})"),
            },
        },
        Err(error) => EditorRuntimeCheck {
            id: id.to_string(),
            ok: false,
            path: path.display().to_string(),
            message: format!("{message_prefix}: failed to read file ({error})"),
        },
    }
}

pub(crate) fn workspace_version_path(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("version.json")
}

pub(crate) fn workspace_manifest_path(workspace_root: &Path) -> PathBuf {
    resolve_toolchain_root(workspace_root).join("MANIFEST.json")
}

pub(crate) fn workspace_editor_runtime_path(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root).join("editor-runtime.json")
}

pub(crate) fn workspace_author_knowledge_path(workspace_root: &Path) -> PathBuf {
    workspace_platform_dir(workspace_root)
        .join("knowledge")
        .join("author-runtime.json")
}

pub fn doctor_editor_runtime_for_workspace_root(
    package_root: &Path,
    workspace_root: &Path,
) -> EditorRuntimeDoctorReport {
    let version_path = workspace_version_path(workspace_root);
    let manifest_path = workspace_manifest_path(workspace_root);
    let editor_runtime_path = workspace_editor_runtime_path(workspace_root);
    let knowledge_path = workspace_author_knowledge_path(workspace_root);
    let catalog_path = workspace_catalog_dir(workspace_root).join("capability-catalog.json");
    let author_surface_path = workspace_catalog_dir(workspace_root).join("author-surface.json");
    let access_surface_path = workspace_catalog_dir(workspace_root).join("access-surface.json");
    let author_profile_path = workspace_profiles_dir(workspace_root).join("author.md");
    let access_profile_path = workspace_profiles_dir(workspace_root).join("access.md");
    let author_skill_entry = workspace_author_skill_dir(workspace_root).join("SKILL.md");
    let access_skill_entry = workspace_access_skill_dir(workspace_root).join("SKILL.md");
    let toolchain_bin =
        workspace_runtime_bin_dir(workspace_root).join(binary_file_name("mei-toolchain"));
    let lsp_bin = workspace_runtime_bin_dir(workspace_root).join(binary_file_name("mei-lsp"));
    let host_web_bin =
        workspace_runtime_bin_dir(workspace_root).join(binary_file_name("mei-host-web"));
    let author_runtime_adapter =
        workspace_runtime_bin_dir(workspace_root).join("author-mcp-adapter");
    let access_runtime_adapter =
        workspace_runtime_bin_dir(workspace_root).join("access-mcp-adapter");
    let expected_version = workspace_runtime_version_descriptor();
    let expected_manifest = workspace_runtime_manifest_for_package_root(package_root);
    let checks = vec![
        EditorRuntimeCheck {
            id: "workspace_editor_runtime_descriptor".to_string(),
            ok: editor_runtime_path.is_file(),
            path: editor_runtime_path.display().to_string(),
            message: if editor_runtime_path.is_file() {
                "workspace runtime descriptor present".to_string()
            } else {
                "missing workspace runtime descriptor".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_knowledge_bundle".to_string(),
            ok: knowledge_path.is_file(),
            path: knowledge_path.display().to_string(),
            message: if knowledge_path.is_file() {
                "workspace author knowledge bundle present".to_string()
            } else {
                "missing workspace author knowledge bundle".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_capability_catalog".to_string(),
            ok: catalog_path.is_file(),
            path: catalog_path.display().to_string(),
            message: if catalog_path.is_file() {
                "workspace-local capability catalog present".to_string()
            } else {
                "missing workspace-local capability catalog".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_surface".to_string(),
            ok: author_surface_path.is_file(),
            path: author_surface_path.display().to_string(),
            message: if author_surface_path.is_file() {
                "workspace-local author MCP surface descriptor present".to_string()
            } else {
                "missing workspace-local author MCP surface descriptor".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_access_surface".to_string(),
            ok: access_surface_path.is_file(),
            path: access_surface_path.display().to_string(),
            message: if access_surface_path.is_file() {
                "workspace-local access MCP surface descriptor present".to_string()
            } else {
                "missing workspace-local access MCP surface descriptor".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_profile".to_string(),
            ok: author_profile_path.is_file(),
            path: author_profile_path.display().to_string(),
            message: if author_profile_path.is_file() {
                "workspace-local author profile present".to_string()
            } else {
                "missing workspace-local author profile".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_access_profile".to_string(),
            ok: access_profile_path.is_file(),
            path: access_profile_path.display().to_string(),
            message: if access_profile_path.is_file() {
                "workspace-local access profile present".to_string()
            } else {
                "missing workspace-local access profile".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_skill".to_string(),
            ok: author_skill_entry.is_file(),
            path: author_skill_entry.display().to_string(),
            message: if author_skill_entry.is_file() {
                "workspace-local author skill package present".to_string()
            } else {
                "missing workspace-local author skill package".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_access_skill".to_string(),
            ok: access_skill_entry.is_file(),
            path: access_skill_entry.display().to_string(),
            message: if access_skill_entry.is_file() {
                "workspace-local access skill package present".to_string()
            } else {
                "missing workspace-local access skill package".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_mei_toolchain_bin".to_string(),
            ok: toolchain_bin.is_file(),
            path: toolchain_bin.display().to_string(),
            message: if toolchain_bin.is_file() {
                "workspace-local mei-toolchain binary present".to_string()
            } else {
                "missing workspace-local mei-toolchain binary".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_mei_lsp_bin".to_string(),
            ok: lsp_bin.is_file(),
            path: lsp_bin.display().to_string(),
            message: if lsp_bin.is_file() {
                "workspace-local mei-lsp binary present".to_string()
            } else {
                "missing workspace-local mei-lsp binary".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_mei_host_web_bin".to_string(),
            ok: host_web_bin.is_file(),
            path: host_web_bin.display().to_string(),
            message: if host_web_bin.is_file() {
                "workspace-local mei-host-web binary present".to_string()
            } else {
                "missing workspace-local mei-host-web binary".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_author_mcp_adapter".to_string(),
            ok: author_runtime_adapter.is_file(),
            path: author_runtime_adapter.display().to_string(),
            message: if author_runtime_adapter.is_file() {
                "workspace-local author MCP adapter present".to_string()
            } else {
                "missing workspace-local author MCP adapter".to_string()
            },
        },
        EditorRuntimeCheck {
            id: "workspace_access_mcp_adapter".to_string(),
            ok: access_runtime_adapter.is_file(),
            path: access_runtime_adapter.display().to_string(),
            message: if access_runtime_adapter.is_file() {
                "workspace-local access MCP adapter present".to_string()
            } else {
                "missing workspace-local access MCP adapter".to_string()
            },
        },
        json_value_matches(
            &version_path,
            "workspace_version_descriptor",
            "workspace runtime version descriptor",
            |value| {
                value["schema_version"] == expected_version.schema_version
                    && value["toolchain_version"] == expected_version.toolchain_version
                    && value["compatibility"]["line"] == expected_version.compatibility.line
                    && value["installed_runtime"]["runtime_id"]
                        == expected_version.installed_runtime.runtime_id
                    && value["installed_runtime"]["target_triple"]
                        == expected_version.installed_runtime.target_triple
            },
        ),
        json_value_matches(
            &manifest_path,
            "workspace_runtime_manifest",
            "workspace runtime manifest",
            |value| {
                value["schema_version"] == expected_manifest.schema_version
                    && value["bundle_id"] == expected_manifest.bundle_id
                    && value["toolchain_version"] == expected_manifest.toolchain_version
                    && value["compatibility_line"] == expected_manifest.compatibility_line
                    && value["target_triple"] == expected_manifest.target_triple
                    && value["artifacts"]["mei_toolchain"]
                        == expected_manifest.artifacts.mei_toolchain
                    && value["artifacts"]["mei_lsp"] == expected_manifest.artifacts.mei_lsp
                    && value["artifacts"]["mei_host_web"]
                        == expected_manifest.artifacts.mei_host_web
            },
        ),
    ];
    let ok = checks.iter().all(|check| check.ok);
    EditorRuntimeDoctorReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        ok,
        package_root: package_root.display().to_string(),
        workspace_root: Some(workspace_root.display().to_string()),
        checks,
    }
}

pub fn workspace_runtime_status_for_workspace_root(
    package_root: &Path,
    workspace_root: &Path,
) -> WorkspaceRuntimeStatusReport {
    let doctor = doctor_editor_runtime_for_workspace_root(package_root, workspace_root);
    let version_path = workspace_version_path(workspace_root);
    let manifest_path = workspace_manifest_path(workspace_root);
    let catalog_path = workspace_catalog_dir(workspace_root).join("capability-catalog.json");
    let author_skill_dir = workspace_author_skill_dir(workspace_root);
    let access_skill_dir = workspace_access_skill_dir(workspace_root);
    let author_profile_path = workspace_profiles_dir(workspace_root).join("author.md");
    let runtime_bin_dir = workspace_runtime_bin_dir(workspace_root);
    let installed = version_path.is_file()
        && manifest_path.is_file()
        && catalog_path.is_file()
        && author_skill_dir.join("SKILL.md").is_file()
        && access_skill_dir.join("SKILL.md").is_file()
        && runtime_bin_dir
            .join(binary_file_name("mei-toolchain"))
            .is_file()
        && runtime_bin_dir.join(binary_file_name("mei-lsp")).is_file()
        && runtime_bin_dir
            .join(binary_file_name("mei-host-web"))
            .is_file()
        && runtime_bin_dir.join("author-mcp-adapter").is_file()
        && runtime_bin_dir.join("access-mcp-adapter").is_file();
    let fallback_to_source_tree = false;
    WorkspaceRuntimeStatusReport {
        schema_version: EDITOR_RUNTIME_SCHEMA_VERSION.to_string(),
        source_root: workspace_root.display().to_string(),
        runtime_root: resolve_workspace_runtime_root(workspace_root)
            .display()
            .to_string(),
        package_root: package_root.display().to_string(),
        installed,
        fallback_to_source_tree,
        version_path: version_path.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        catalog_path: catalog_path.display().to_string(),
        author_skill_dir: author_skill_dir.display().to_string(),
        author_profile_path: author_profile_path.display().to_string(),
        doctor,
    }
}
