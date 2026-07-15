use std::path::Path;

use mei_lang_kernel::{
    resolve_app_main_path, resolve_app_root,
    resolve_templates_root as kernel_resolve_templates_root,
};
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
    let upload_root = app_root.join("assets/upload");
    let app_main = resolve_app_main_path(&app_root);
    let has_app_toml = app_root.join("app.toml").is_file();
    let app_entry_ok = has_app_toml || app_main.is_file();
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
            "create `{}` with `app.toml` + stage MDX, or update --app/--source-root",
            app_root.display()
        )),
    );
    push_layout_check(
        &mut checks,
        "app_main_exists",
        app_entry_ok,
        "error",
        if has_app_toml {
            format!("app.toml present at `{}`", app_root.join("app.toml").display())
        } else {
            format!("`{}` is missing", app_main.display())
        },
        Some(
            "ensure app.toml exists (graph-native) or entry.main resolves to a Mei entry file"
                .to_string(),
        ),
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
            "run `mei-toolchain workspace init` or start the host; stock/components is ensured automatically"
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
            "run `mei-toolchain workspace init` or start the host; stock/templates is ensured automatically"
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
