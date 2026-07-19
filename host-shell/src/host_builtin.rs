//! Host-injected admin builtins (0548 Phase C).
//!
//! Not authored via `admin.toml`; merged into Registry for every discovered app.

use mei_lang_kernel::{
    AdminPageProgram, AdminProviderKind, AdminRegistryProjection, AdminResourceProjection,
    AdminResourceSpec, AdminTemplate, AdminUiSurface, PageProgram, ADMIN_RESOURCE_API_VERSION,
};

pub const HOST_BUILTIN_OPS_CONFIG: &str = "ops_config";
pub const HOST_BUILTIN_UPLOAD_FILES: &str = "upload_files";

fn stub_spec(
    resource_id: &str,
    title: &str,
    description: &str,
    template: AdminTemplate,
    provider: AdminProviderKind,
) -> AdminResourceSpec {
    AdminResourceSpec {
        resource_id: resource_id.to_string(),
        title: title.to_string(),
        description: Some(description.to_string()),
        namespace: None,
        template,
        provider,
        record_path: None,
        config_path: None,
        required_capabilities: vec!["config_upload".to_string()],
        scope: None,
        audit: None,
        danger_level: None,
        revision_policy: None,
        validation: None,
        idempotency: None,
        dirty_policy: None,
        apply_policy: None,
        navigation: None,
        sections: Vec::new(),
        columns: Vec::new(),
        allowed_views: Vec::new(),
        upload: None,
        actions: Vec::new(),
        query: None,
        get: None,
        mutation: None,
    }
}

fn project_builtin(
    app_id: &str,
    resource_id: &str,
    title: &str,
    description: &str,
    template: AdminTemplate,
    provider: AdminProviderKind,
    ui_surface: AdminUiSurface,
) -> AdminResourceProjection {
    let spec = stub_spec(resource_id, title, description, template, provider);
    AdminResourceProjection {
        resource_key: format!("app:{app_id}.{resource_id}"),
        resource_id: resource_id.to_string(),
        app_id: app_id.to_string(),
        title: title.to_string(),
        description: Some(description.to_string()),
        template,
        provider,
        required_capabilities: vec!["config_upload".to_string()],
        record_path: None,
        config_path: None,
        href: format!("/admin/apps/{app_id}/{resource_id}"),
        page_program: AdminPageProgram::new(
            resource_id,
            PageProgram::from_scene_ref(
                resource_id,
                Some(title.to_string()),
                format!("host://admin/{resource_id}"),
                format!("admin/{resource_id}"),
            ),
        ),
        ui_surface,
        spec,
    }
}

/// Host builtins for one app (ops_config + upload_files).
pub fn host_builtin_resources(app_id: &str) -> Vec<AdminResourceProjection> {
    vec![
        project_builtin(
            app_id,
            HOST_BUILTIN_OPS_CONFIG,
            "运维配置",
            "Host 内建：编辑 .mei-config.json ops 白名单（manage-ops-panel）",
            AdminTemplate::SingletonForm,
            AdminProviderKind::ConfigRecord,
            AdminUiSurface::OpsEmbed,
        ),
        project_builtin(
            app_id,
            HOST_BUILTIN_UPLOAD_FILES,
            "上传物料",
            "Host 内建：应用 upload 目录物料管理（upload-panel）",
            AdminTemplate::AssetSlotCollection,
            AdminProviderKind::AssetSlot,
            AdminUiSurface::UploadEmbed,
        ),
    ]
}

/// Merge manifest projection (if any) with Host builtins.
/// Manifest resources win on `resource_id` collision; builtins are prepended when absent.
pub fn merge_host_builtins(
    app_id: &str,
    manifest: Option<AdminRegistryProjection>,
) -> AdminRegistryProjection {
    let builtins = host_builtin_resources(app_id);
    let Some(mut proj) = manifest else {
        return AdminRegistryProjection {
            app_id: app_id.to_string(),
            api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
            manifest_digest: "host-builtin".to_string(),
            resources: builtins,
        };
    };
    let existing: std::collections::HashSet<String> = proj
        .resources
        .iter()
        .map(|r| r.resource_id.clone())
        .collect();
    let mut merged = Vec::with_capacity(builtins.len() + proj.resources.len());
    for b in builtins {
        if !existing.contains(&b.resource_id) {
            merged.push(b);
        }
    }
    merged.append(&mut proj.resources);
    proj.resources = merged;
    proj
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_without_manifest() {
        let proj = merge_host_builtins("demo", None);
        assert_eq!(proj.resources.len(), 2);
        assert_eq!(proj.resources[0].resource_id, HOST_BUILTIN_OPS_CONFIG);
        assert_eq!(proj.resources[1].resource_id, HOST_BUILTIN_UPLOAD_FILES);
        assert_eq!(proj.resources[0].ui_surface, AdminUiSurface::OpsEmbed);
        assert_eq!(proj.resources[1].href, "/admin/apps/demo/upload_files");
    }

    #[test]
    fn manifest_wins_on_collision() {
        let mut manifest = merge_host_builtins("demo", None);
        // Simulate app declaring ops_config via manifest (FormCard).
        manifest.resources[0].ui_surface = AdminUiSurface::FormCard;
        manifest.resources[0].title = "自定义运维".into();
        let only_org = AdminRegistryProjection {
            app_id: "demo".into(),
            api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
            manifest_digest: "x".into(),
            resources: vec![manifest.resources[0].clone()],
        };
        let merged = merge_host_builtins("demo", Some(only_org));
        let ops = merged
            .resources
            .iter()
            .find(|r| r.resource_id == HOST_BUILTIN_OPS_CONFIG)
            .unwrap();
        assert_eq!(ops.title, "自定义运维");
        assert_eq!(ops.ui_surface, AdminUiSurface::FormCard);
        assert!(merged
            .resources
            .iter()
            .any(|r| r.resource_id == HOST_BUILTIN_UPLOAD_FILES));
    }
}
