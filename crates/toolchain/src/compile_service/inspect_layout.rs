use std::path::Path;

use mei_lang_kernel::{resolve_app_root, resolve_templates_root as kernel_resolve_templates_root};
use serde::Serialize;

use super::resolve_components_root;

#[derive(Debug, Clone, Serialize)]
pub struct LayoutCheck {
    pub id: String,
    pub level: String,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceLayoutRoots {
    pub source_root: String,
    pub app_root: String,
    pub components_root: String,
    pub components_resolution: String,
    pub templates_root: String,
    pub vendor_root: String,
    pub upload_root: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceLayoutInspection {
    pub app_id: String,
    pub roots: SourceLayoutRoots,
    pub checks: Vec<LayoutCheck>,
    pub ok: bool,
}

pub fn inspect_source_layout(source_root: &Path, app_id: &str) -> SourceLayoutInspection {
    let app_id = app_id.trim();
    let app_root = resolve_app_root(source_root, app_id);
    let components_root = resolve_components_root(source_root);
    let templates_root = kernel_resolve_templates_root(source_root);
    let vendor_root = components_root.join("vendor");
    let upload_root = app_root.join("upload");
    let components_resolution = components_root
        .strip_prefix(source_root)
        .map(|rel| format!("source_root/{}", rel.to_string_lossy().replace('\\', "/")))
        .unwrap_or_else(|_| components_root.display().to_string());

    let mut checks: Vec<LayoutCheck> = Vec::new();
    push_layout_check(
        &mut checks,
        "app_root_exists",
        app_root.is_dir(),
        "error",
        format!("app root `{}` does not exist", app_root.display()),
        Some(format!(
            "create `{}` and place `main.mei` under it, or update --app/--source-root",
            app_root.display()
        )),
    );
    push_layout_check(
        &mut checks,
        "app_main_exists",
        app_root.join("main.mei").is_file(),
        "error",
        format!("`{}` is missing", app_root.join("main.mei").display()),
        Some("ensure entry.main resolves to main.mei or provide a valid app root".to_string()),
    );
    push_layout_check(
        &mut checks,
        "components_root_exists",
        components_root.is_dir(),
        "error",
        format!(
            "components root `{}` does not exist",
            components_root.display()
        ),
        Some(
            "run `mei workspace materialize` or set paths.components in `.mei-workspace.json`"
                .to_string(),
        ),
    );
    push_layout_check(
        &mut checks,
        "vendor_root_exists",
        vendor_root.is_dir(),
        "warning",
        format!("vendor root `{}` does not exist", vendor_root.display()),
        Some(
            "run `npm run assets:build` in mei-lang to refresh shared vendor bundles when chart/map components are used"
                .to_string(),
        ),
    );
    push_layout_check(
        &mut checks,
        "templates_root_exists",
        templates_root.is_dir(),
        "warning",
        format!("templates root `{}` does not exist", templates_root.display()),
        Some(
            "run `mei workspace materialize` or set paths.templates; scenes should reference `../.stock/templates/...`"
                .to_string(),
        ),
    );
    push_layout_check(
        &mut checks,
        "upload_root_exists",
        upload_root.is_dir(),
        "info",
        format!("upload root `{}` does not exist", upload_root.display()),
        Some("optional: create when the app uses upload-backed data sources".to_string()),
    );

    let ok = !checks.iter().any(|item| item.level == "error");
    SourceLayoutInspection {
        app_id: app_id.to_string(),
        roots: SourceLayoutRoots {
            source_root: source_root.display().to_string(),
            app_root: app_root.display().to_string(),
            components_root: components_root.display().to_string(),
            components_resolution,
            templates_root: templates_root.display().to_string(),
            vendor_root: vendor_root.display().to_string(),
            upload_root: upload_root.display().to_string(),
        },
        checks,
        ok,
    }
}

fn push_layout_check(
    checks: &mut Vec<LayoutCheck>,
    id: &str,
    passed: bool,
    level: &str,
    message: String,
    hint: Option<String>,
) {
    if passed {
        return;
    }
    checks.push(LayoutCheck {
        id: id.to_string(),
        level: level.to_string(),
        message,
        hint,
    });
}
