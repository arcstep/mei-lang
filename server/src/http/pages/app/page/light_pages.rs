use std::time::Instant;

use axum::response::{Html, IntoResponse, Response};
use mei_lang_app::{
    render_build_source_page, render_config_page, render_upload_page, shell_body_theme_style,
    UiRouteMode,
};
use mei_lang_kernel::{
    load_workspace_config, read_source_file, resolve_app_entry_main, resolve_app_root, source_tree,
    WorkspaceAppMeta,
};

use crate::AppState;

use crate::http::pages::app::page_render::upload_rel_from_config;
use crate::http::pages::app::query::AppQuery;
use crate::http::pages::app_render::source_panel_meta;
use crate::http::pages::util::{
    elapsed_ms, fill_manage_wall_clock_placeholders, fill_page_shell_placeholders,
    fill_perf_placeholders,
};
use mei_lang_app::HostAccountView;
use mei_lang_app::TopbarMenuContext;

pub(super) struct LightPageContext<'a> {
    pub state: &'a AppState,
    pub route_mode: UiRouteMode,
    pub app_id: &'a str,
    pub query: &'a AppQuery,
    pub apps: &'a [WorkspaceAppMeta],
    pub app_title: &'a str,
    pub topbar_menus: &'a TopbarMenuContext,
    pub lightweight_scene: Option<&'a str>,
    pub upload_enabled: bool,
    pub upload_root_label: &'a str,
    pub upload_files: &'a [mei_lang_app::UploadFileEntry],
    pub auth_enabled: bool,
    pub account_view: Option<&'a HostAccountView>,
    pub request_file: Option<&'a str>,
    pub manage_file: Option<&'a str>,
    pub app_started: Instant,
}

pub(super) fn try_render_light_page(ctx: LightPageContext<'_>) -> Option<Response> {
    let app_root = resolve_app_root(ctx.state.source_root.as_path(), ctx.app_id);
    let workspace = load_workspace_config(ctx.state.source_root.as_path());
    let shell_theme_style = shell_body_theme_style(&workspace);
    if ctx.route_mode == UiRouteMode::Config {
        let target = ".mei-config.json".to_string();
        let source_path = app_root.join(&target);
        let source_started = Instant::now();
        let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
        let source_read_ms = elapsed_ms(source_started);
        let source_meta = source_panel_meta(&source_path, &source);
        let mut html = render_config_page(
            ctx.apps,
            ctx.app_title,
            ctx.app_id,
            Some(ctx.topbar_menus),
            Some(source.as_str()),
            Some(&source_meta),
            ctx.lightweight_scene,
            ctx.upload_enabled,
            ctx.auth_enabled,
            ctx.account_view,
            shell_theme_style.as_str(),
        );
        html = fill_perf_placeholders(html, 0, elapsed_ms(ctx.app_started));
        html = fill_manage_wall_clock_placeholders(html, 0, elapsed_ms(ctx.app_started));
        let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
            &app_root,
            Some(ctx.state.source_root.as_path()),
            None,
        );
        html = fill_page_shell_placeholders(html, &gis, ctx.state.source_root.as_path());
        tracing::info!(
            app_id = %ctx.app_id,
            route_mode = ctx.route_mode.slug(),
            target = %target,
            source_read_ms,
            total_ms = elapsed_ms(ctx.app_started),
            phase = "finish_light_config",
            "app page request finished without compile"
        );
        return Some(Html(html).into_response());
    }
    if ctx.route_mode == UiRouteMode::Upload {
        let rel = upload_rel_from_config(&app_root, &ctx.state.source_root)?;
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
        let source_started = Instant::now();
        let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
        let source_read_ms = elapsed_ms(source_started);
        let source_meta = source_panel_meta(&source_path, &source);
        let mut html = render_upload_page(
            ctx.apps,
            ctx.app_title,
            ctx.app_id,
            Some(ctx.topbar_menus),
            ctx.request_file,
            Some(source.as_str()),
            Some(&source_meta),
            ctx.lightweight_scene,
            ctx.upload_enabled,
            Some(ctx.upload_root_label),
            ctx.upload_files,
            ctx.auth_enabled,
            ctx.account_view,
            shell_theme_style.as_str(),
        );
        html = fill_perf_placeholders(html, 0, elapsed_ms(ctx.app_started));
        html = fill_manage_wall_clock_placeholders(html, 0, elapsed_ms(ctx.app_started));
        let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
            &app_root,
            Some(ctx.state.source_root.as_path()),
            None,
        );
        html = fill_page_shell_placeholders(html, &gis, ctx.state.source_root.as_path());
        tracing::info!(
            app_id = %ctx.app_id,
            route_mode = ctx.route_mode.slug(),
            target = %target,
            source_read_ms,
            total_ms = elapsed_ms(ctx.app_started),
            phase = "finish_light_upload",
            "app page request finished without compile"
        );
        return Some(Html(html).into_response());
    }
    if ctx.route_mode == UiRouteMode::Build
        && ctx
            .query
            .tab
            .as_deref()
            .map(str::trim)
            .is_some_and(|tab| tab.eq_ignore_ascii_case("source"))
    {
        let target = ctx
            .manage_file
            .map(ToString::to_string)
            .unwrap_or_else(|| resolve_app_entry_main(&app_root));
        let source_path = app_root.join(&target);
        let source_started = Instant::now();
        let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
        let source_read_ms = elapsed_ms(source_started);
        let source_meta = source_panel_meta(&source_path, &source);
        let file_tree = source_tree(&app_root).unwrap_or_default();
        let mut html = render_build_source_page(
            ctx.apps,
            ctx.app_title,
            ctx.app_id,
            Some(ctx.topbar_menus),
            &file_tree,
            target.as_str(),
            source.as_str(),
            Some(&source_meta),
            ctx.lightweight_scene,
            ctx.query.tab.as_deref(),
            ctx.upload_enabled,
            ctx.auth_enabled,
            ctx.account_view,
            shell_theme_style.as_str(),
        );
        html = fill_perf_placeholders(html, 0, elapsed_ms(ctx.app_started));
        html = fill_manage_wall_clock_placeholders(html, 0, elapsed_ms(ctx.app_started));
        let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
            &app_root,
            Some(ctx.state.source_root.as_path()),
            None,
        );
        html = fill_page_shell_placeholders(html, &gis, ctx.state.source_root.as_path());
        tracing::info!(
            app_id = %ctx.app_id,
            route_mode = ctx.route_mode.slug(),
            target = %target,
            source_read_ms,
            total_ms = elapsed_ms(ctx.app_started),
            phase = "finish_light_build_source",
            "app page request finished without compile"
        );
        return Some(Html(html).into_response());
    }
    None
}
