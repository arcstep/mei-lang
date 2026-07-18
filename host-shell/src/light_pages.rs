use axum::http::{header, HeaderValue};
use axum::response::{Html, IntoResponse, Response};
use mei_lang_app::{
    page_body_theme_style, render_config_page, render_upload_page, HostAccountView,
    SourcePanelMeta, TopbarMenuContext, UiRouteMode,
};
use mei_lang_kernel::{
    load_workspace_config, read_source_file, resolve_app_root, WorkspaceAppMeta,
};

use crate::build_info::fill_page_shell_placeholders;
use crate::upload_support::{list_upload_files, upload_rel_from_config};

const LIGHT_PAGE_CACHE_CONTROL: &str = "private, no-cache, no-store, must-revalidate";

pub(crate) fn light_page_response(html: String) -> Response {
    let mut response = Html(html).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(LIGHT_PAGE_CACHE_CONTROL),
    );
    response
}

pub(crate) struct LightPageContext<'a> {
    pub workspace_root: &'a std::path::Path,
    pub _package_root: &'a std::path::Path,
    pub route_mode: UiRouteMode,
    pub app_id: &'a str,
    pub apps: &'a [WorkspaceAppMeta],
    pub app_title: &'a str,
    pub topbar_menu: &'a TopbarMenuContext,
    pub lightweight_scene: Option<&'a str>,
    pub request_file: Option<&'a str>,
    pub auth_enabled: bool,
    pub account_view: Option<&'a HostAccountView>,
}

pub(crate) fn try_render_light_page(ctx: LightPageContext<'_>) -> Option<Response> {
    let app_root = resolve_app_root(ctx.workspace_root, ctx.app_id);
    let workspace = load_workspace_config(ctx.workspace_root);
    let upload_rel = upload_rel_from_config(app_root.as_path(), ctx.workspace_root);
    let upload_enabled = upload_rel.is_some();
    let upload_root_label = upload_rel.as_deref().unwrap_or("upload").to_string();
    let upload_files = upload_rel
        .as_deref()
        .map(|rel| list_upload_files(&app_root.join(rel), rel))
        .unwrap_or_default();
    if ctx.route_mode == UiRouteMode::Config {
        let target = ".mei-config.json".to_string();
        let source_path = app_root.join(&target);
        let source = read_source_file(&source_path).unwrap_or_else(|_| String::new());
        let source_meta = source_panel_meta(&source_path, &source);
        let theme_style = page_body_theme_style(&workspace, None, None);
        let mut html = render_config_page(
            ctx.apps,
            ctx.app_title,
            ctx.app_id,
            Some(ctx.topbar_menu),
            Some(source.as_str()),
            Some(&source_meta),
            ctx.lightweight_scene,
            upload_enabled,
            ctx.auth_enabled,
            ctx.account_view,
            theme_style.as_str(),
            &[],
        );
        html = fill_page_shell_placeholders(html, ctx.workspace_root);
        return Some(light_page_response(html));
    }

    if ctx.route_mode == UiRouteMode::Upload {
        let rel = upload_rel?;
        let target = if let Some(file) = ctx
            .request_file
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            format!("{rel}/{file}")
        } else {
            rel.clone()
        };
        let source_path = app_root.join(&target);
        let source = read_source_file(&source_path).unwrap_or_else(|_| String::new());
        let source_meta = source_panel_meta(&source_path, &source);
        let theme_style = page_body_theme_style(&workspace, None, None);
        let mut html = render_upload_page(
            ctx.apps,
            ctx.app_title,
            ctx.app_id,
            Some(ctx.topbar_menu),
            ctx.request_file,
            Some(source.as_str()),
            Some(&source_meta),
            ctx.lightweight_scene,
            upload_enabled,
            Some(upload_root_label.as_str()),
            upload_files.as_slice(),
            ctx.auth_enabled,
            ctx.account_view,
            theme_style.as_str(),
            &[],
        );
        html = fill_page_shell_placeholders(html, ctx.workspace_root);
        return Some(light_page_response(html));
    }

    None
}

fn source_panel_meta(_source_path: &std::path::Path, source: &str) -> SourcePanelMeta {
    let line_count = if source.is_empty() {
        0
    } else {
        source.split('\n').count()
    };
    let char_count = source.chars().count();
    let last_modified_label = None;
    SourcePanelMeta {
        line_count,
        char_count,
        last_modified_label,
    }
}
