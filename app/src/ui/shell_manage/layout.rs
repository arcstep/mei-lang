use leptos::prelude::*;
use mei_lang_kernel::{
    build_reachability_tree, compile_coordinate_for_node, default_build_node_for_compiled,
    filter_reachability_roots_for_stock_catalog, is_stock_catalog_app, resolve_build_node_context,
    resolve_build_view_query, BuildNodeKind, BuildViewTab, CompiledApp,
    LegacyBuildQuery, WorkspaceAppMeta,
};

use super::super::build_tree::reachability_tree_view;
use super::super::manage_routing::{build_node_href, BuildReviewAxes, WorldSemanticQuery};
use super::super::preview;
use super::super::preview_chrome::asset_preview_body;
use super::super::route::UiRouteMode;
use super::super::prototype_preset::{
    default_build_preset, match_preset, prototype_workspace_primary_tabs,
    prototype_workspace_tool_tabs, PrototypePreset, PROTOTYPE_PRESETS,
};
use super::super::scene_drilldown_context::host_ssr_bootstrap_scripts;
use super::super::statusbar::statusbar_view;
use super::super::topbar::{access_scene_for_topbar, topbar_view};
use super::super::{HostAccountView, SourcePanelMeta, TopbarMenuContext};
use super::build_panels::{
    build_agent_view, build_artifact_panel, build_exec_panel_shell, build_graph_panel,
    build_overview_view, build_provenance_view,
};
use super::world_semantic_inspector::{
    should_show_world_semantic_inspector, world_semantic_inspector_view,
};

pub(crate) fn manage_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    target: Option<&str>,
    source: Option<&str>,
    _source_meta: Option<&SourcePanelMeta>,
    selected_scene: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
    _diag_filter: Option<&str>,
    world_metric: Option<&str>,
    world_dataset: Option<&str>,
    explain: Option<&str>,
    node: Option<&str>,
    scope: Option<&str>,
    focus: Option<&str>,
    catalog: Option<&str>,
    stock_pack: Option<&str>,
    upload_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> AnyView {
    let legacy = LegacyBuildQuery {
        file: target.map(str::to_string),
        scene: selected_scene.map(str::to_string),
        world_metric: world_metric.map(str::to_string),
        world_dataset: world_dataset.map(str::to_string),
        explain: explain.map(str::to_string),
        tab: active_tab.map(str::to_string),
    };
    let resolved =
        resolve_build_view_query(node, scope, active_tab, &legacy).unwrap_or_else(|| {
            let default_node = default_build_node_for_compiled(compiled);
            mei_lang_kernel::ResolvedBuildViewQuery {
                node: default_node.clone(),
                tab: default_node.default_tab(),
                scope: Default::default(),
            }
        });
    let ctx = resolve_build_node_context(compiled, &resolved.node);
    let selected_target = ctx.target_file.clone();
    let semantic = WorldSemanticQuery {
        world_metric: ctx.world_metric.as_deref(),
        world_dataset: ctx.world_dataset.as_deref(),
        explain: ctx.explain.as_deref(),
    };
    let show_inspector =
        should_show_world_semantic_inspector(&ctx.node, selected_target.as_str(), semantic);
    let source_panel = source.unwrap_or("").to_string();
    let build_preview_scope =
        mei_lang_kernel::resolve_build_preview_scope_for_ssr(compiled, &resolved.node);
    let build_preview_component_use_key_owned =
        super::preview_fragment::build_preview_component_use_key(&resolved.node);
    let build_preview_component_use_key =
        build_preview_component_use_key_owned.as_deref();
    let preview = preview::preview_view(
        compiled,
        app_path,
        selected_target.as_str(),
        UiRouteMode::Build,
        semantic,
        build_preview_scope.as_deref(),
        build_preview_component_use_key,
        data_mode,
    );
    let active_scene = ctx.scene_id.as_deref().or(compiled.active_scene.as_deref());
    let scene_for_links = active_scene;
    let reachability_roots = filter_reachability_roots_for_stock_catalog(
        build_reachability_tree(compiled),
        is_stock_catalog_app(app_path),
        catalog,
        stock_pack,
    );
    let active_review_projection = review_projection
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_build_preset().review_projection);
    let review_axes = BuildReviewAxes {
        data_mode,
        review_projection: Some(active_review_projection),
    };
    let active_data_mode = data_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_build_preset().data_mode);
    let active_preset = match_preset(active_data_mode, active_review_projection)
        .copied()
        .unwrap_or_else(|| *default_build_preset());
    let build_tree = reachability_tree_view(
        reachability_roots.as_slice(),
        app_path,
        &resolved.node,
        resolved.tab,
        catalog,
        stock_pack,
        review_axes,
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let active_tab_enum = resolved.tab;
    let topbar = topbar_view(
        apps,
        app_path,
        topbar_menu,
        UiRouteMode::Build,
        access_scene_for_topbar(
            UiRouteMode::Build,
            compiled,
            scene_for_links,
            preview_target,
        ),
        Some(selected_target.as_str()),
        Some(active_tab_enum.slug()),
        catalog,
        stock_pack,
        upload_enabled,
        stage_enabled,
        auth_enabled,
        auth_account,
    );
    let statusbar = statusbar_view(
        app_path,
        UiRouteMode::Build.slug(),
        selected_target.as_str(),
        None,
    );
    let workspace_class = if show_inspector {
        "workspace manage-workspace manage-workspace--with-inspector chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0"
    } else {
        "workspace manage-workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0"
    };
    let inspector = if show_inspector {
        world_semantic_inspector_view(compiled, app_path, selected_target.as_str(), semantic)
    } else {
        view! { <></> }.into_any()
    };
    let shell_class = if stage_enabled {
        "shell shell-surface frame-stage-enabled mei-text-primary"
    } else {
        "shell shell-surface mei-text-primary"
    };
    let preview_scroll_class = if stage_enabled {
        "main-pane-scroll preview-pane-scroll frame-stage-enabled flex-1 min-h-0 overflow-auto p-0"
    } else {
        "main-pane-scroll preview-pane-scroll flex-1 min-h-0 overflow-auto p-0"
    };
    let review_projection_attr = active_review_projection;

    let primary_tabs = prototype_workspace_primary_tabs(resolved.node.kind);
    let tool_tabs = prototype_workspace_tool_tabs(resolved.node.kind);

    let tab_link = |tab: BuildViewTab, class: String| {
        let href = build_node_href(
            app_path,
            &resolved.node,
            tab,
            resolved.scope,
            catalog,
            stock_pack,
            review_axes,
        );
        let is_active = tab == active_tab_enum;
        view! {
            <a
                class=class
                href=href
                role="tab"
                aria-selected=if is_active { "true" } else { "false" }
                data-manage-tab=tab.slug()
            >
                <span class="manage-view-tab-label">{tab.label()}</span>
            </a>
        }
    };

    let primary_tab_links = primary_tabs
        .iter()
        .map(|tab| {
            let class = if *tab == active_tab_enum {
                "manage-view-tab is-active".to_string()
            } else {
                "manage-view-tab".to_string()
            };
            tab_link(*tab, class)
        })
        .collect_view();

    let tool_tab_links = tool_tabs
        .iter()
        .map(|tab| {
            let class = if *tab == active_tab_enum {
                "manage-view-tab manage-view-tab--tool is-active".to_string()
            } else {
                "manage-view-tab manage-view-tab--tool".to_string()
            };
            tab_link(*tab, class)
        })
        .collect_view();

    let node_encoded = resolved.node.encode();
    let focus_encoded = focus
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let tab_slug = active_tab_enum.slug().to_string();
    let ui_scope_provenance_sources = if resolved.node.kind == BuildNodeKind::UiScope {
        compiled
            .ui_layout_index
            .lookup(&resolved.node)
            .map(|entry| entry.source_anchors.as_slice())
    } else {
        None
    };
    let overview_panel = build_overview_view(compiled, &ctx, app_path);
    let provenance_panel = build_provenance_view(&ctx.provenance, ui_scope_provenance_sources);
    let agent_panel = build_agent_view(app_path, node_encoded.as_str(), tab_slug.as_str());
    let exec_panel = build_exec_panel_shell(app_path, node_encoded.as_str());
    let semantic_panel = build_graph_panel("语义图", "semantic", node_encoded.as_str());
    let eval_panel = build_graph_panel("求值图", "eval", node_encoded.as_str());
    let artifact_panel = build_artifact_panel(app_path, node_encoded.as_str());

    let preview_scene_id = scene_for_links.or(compiled.active_scene.as_deref());
    let compile_coord = compile_coordinate_for_node(&resolved.node, compiled);
    let compile_scene = compile_coord
        .as_ref()
        .and_then(|coord| coord.scene_id.clone())
        .unwrap_or_default();
    let compile_target = compile_coord
        .map(|coord| coord.preview_target)
        .unwrap_or_else(|| selected_target.clone());
    let host_ssr_bootstrap = if stage_enabled
        || ctx.projection_id.is_some()
        || matches!(
            resolved.node.kind,
            BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot
        ) {
        Some(host_ssr_bootstrap_scripts(
            compiled,
            app_path,
            preview_scene_id,
            data_mode,
        ))
    } else {
        None
    };

    let projection_attrs = ctx
        .projection_id
        .as_ref()
        .map(|projection| {
            view! {
                <div
                    id="build-projection-preview-host"
                    class="build-projection-banner mb-2 px-3 py-2 mei-font-1"
                    data-scene-id=ctx.scene_id.clone().unwrap_or_default()
                    data-projection-id=projection.clone()
                >
                    "构建视图 · 孤立 overlay 预览 · 非 Access 业务路径"
                </div>
            }
            .into_any()
        })
        .unwrap_or_else(|| view! { <></> }.into_any());

    let preset_link = |preset: &PrototypePreset| {
        let href = build_node_href(
            app_path,
            &resolved.node,
            active_tab_enum,
            resolved.scope,
            catalog,
            stock_pack,
            BuildReviewAxes {
                data_mode: Some(preset.data_mode),
                review_projection: Some(preset.review_projection),
            },
        );
        let class = if preset.slug == active_preset.slug {
            "build-preset-btn is-active"
        } else {
            "build-preset-btn"
        };
        view! {
            <a
                class=class
                href=href
                role="tab"
                aria-selected=if preset.slug == active_preset.slug { "true" } else { "false" }
                data-build-preset=preset.slug
                data-build-data-mode=preset.data_mode
                data-build-review-projection=preset.review_projection
                title=format!("{} · data_mode={} · review_projection={}", preset.label, preset.data_mode, preset.review_projection)
            >
                {preset.label}
            </a>
        }
    };

    let preset_links = PROTOTYPE_PRESETS
        .iter()
        .map(preset_link)
        .collect_view();

    let advanced_data_mode_links = ["eval", "fixture", "static"]
        .iter()
        .map(|mode| {
            let href = build_node_href(
                app_path,
                &resolved.node,
                active_tab_enum,
                resolved.scope,
                catalog,
                stock_pack,
                BuildReviewAxes {
                    data_mode: Some(mode),
                    review_projection: Some(active_review_projection),
                },
            );
            let class = if *mode == active_data_mode {
                "build-advanced-axis-btn is-active"
            } else {
                "build-advanced-axis-btn"
            };
            view! {
                <a class=class href=href data-build-data-mode=mode.to_string()>
                    {*mode}
                </a>
            }
        })
        .collect_view();

    let advanced_projection_links = [
        "plane_region",
        "plane_region_section",
        "static_full",
        "live_full",
    ]
        .iter()
        .map(|projection| {
            let href = build_node_href(
                app_path,
                &resolved.node,
                active_tab_enum,
                resolved.scope,
                catalog,
                stock_pack,
                BuildReviewAxes {
                    data_mode: Some(active_data_mode),
                    review_projection: Some(projection),
                },
            );
            let class = if *projection == active_review_projection {
                "build-advanced-axis-btn is-active"
            } else {
                "build-advanced-axis-btn"
            };
            view! {
                <a class=class href=href data-build-review-projection=projection.to_string()>
                    {*projection}
                </a>
            }
        })
        .collect_view();

    view! {
        <div class=shell_class data-build-node=node_encoded.clone() data-build-focus=focus_encoded data-build-tab=tab_slug.clone() data-app-path=app_path.to_string() data-compile-scene=compile_scene.clone() data-compile-target=compile_target.clone() data-data-mode=active_data_mode data-review-projection=review_projection_attr data-build-preset=active_preset.slug>
            {host_ssr_bootstrap.unwrap_or_else(|| view! { <></> }.into_any())}
            <script
                id="mei-build-reachability-tree"
                type="application/json"
                inner_html=serde_json::to_string(&reachability_roots).unwrap_or_else(|_| "[]".to_string())
            ></script>
            <div
                id="tree-icons-sprite-root"
                class="pointer-events-none absolute left-0 top-0 -z-10 h-0 w-0 overflow-hidden opacity-0"
                aria-hidden="true"
                inner_html=super::super::source_tree::TREE_ICONS_SPRITE_SVG
            ></div>
            {topbar}
            <div class=workspace_class id="workspace-root">
                <aside class="sidebar left workspace-panel workspace-panel-side workspace-panel-nav h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-4 py-2.5">
                    <div class="sidebar-scroll flex-1 min-h-0 overflow-auto">
                        {build_tree}
                    </div>
                </aside>
                <div
                    class="splitter splitter-left"
                    data-workspace-splitter="left"
                    role="separator"
                    aria-orientation="vertical"
                    aria-label="调整左侧资源栏宽度"
                ></div>
                <main class="main h-full min-w-0 min-h-0 overflow-hidden px-0">
                    <section class="main-pane workspace-panel workspace-panel-main min-w-0 min-h-0 flex h-full flex-col overflow-hidden px-2 py-3.5">
                        <div class="manage-workspace-head mb-3 flex min-w-0 flex-wrap items-center justify-between gap-2 pb-2.5">
                            <nav
                                class="manage-view-tabs workspace-tabs-strip flex min-w-0 flex-1 flex-wrap items-center gap-2"
                                role="tablist"
                                aria-label="原型主任务"
                            >
                                <div class="manage-view-tabs-cluster">
                                    <div class="manage-view-tabs-group" role="presentation">
                                        {primary_tab_links}
                                    </div>
                                    {if !tool_tabs.is_empty() {
                                        view! {
                                            <div
                                                class="manage-view-tabs-group manage-view-tabs-group--tools"
                                                role="presentation"
                                                aria-label="更多工具"
                                            >
                                                <span class="manage-view-tabs-tools-label mei-font-1 mei-text-muted">"更多工具"</span>
                                                {tool_tab_links}
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <></> }.into_any()
                                    }}
                                </div>
                            </nav>
                            <div
                                class="build-preset-toggle flex shrink-0 flex-wrap items-center gap-1"
                                role="tablist"
                                aria-label="原型任务预设"
                            >
                                {preset_links}
                            </div>
                            <details class="build-advanced-axes shrink-0">
                                <summary class="build-advanced-axes-summary mei-font-1 mei-text-muted">"高级轴"</summary>
                                <div class="build-advanced-axes-body flex flex-wrap items-center gap-2">
                                    <div class="build-advanced-axis-group" aria-label="data_mode">
                                        {advanced_data_mode_links}
                                    </div>
                                    <div class="build-advanced-axis-group" aria-label="review_projection">
                                        {advanced_projection_links}
                                    </div>
                                </div>
                            </details>
                            <button
                                type="button"
                                id="build-copy-agent-context-top"
                                class="build-toolbar-btn build-toolbar-btn--accent shrink-0"
                                data-app-path=app_path.to_string()
                                data-node=node_encoded.clone()
                                data-tab=tab_slug.clone()
                                data-intent="full"
                                data-data-mode=active_data_mode
                                data-review-projection=active_review_projection
                            >
                                "复制原型调试上下文"
                            </button>
                        </div>
                        <div class="manage-tab-stage min-h-0 min-w-0 flex flex-1 flex-col overflow-hidden">
                        <section
                            class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                            data-manage-tab-panel="overview"
                            hidden=active_tab_enum != BuildViewTab::Overview
                        >
                            {overview_panel}
                        </section>
                        <section
                            class="manage-tab-panel preview-pane min-h-0 min-w-0 flex flex-col overflow-hidden"
                            data-manage-tab-panel="preview"
                            hidden=active_tab_enum != BuildViewTab::Preview
                        >
                            {projection_attrs}
                            <div class=preview_scroll_class data-review-projection=review_projection_attr data-data-mode=active_data_mode>
                                {if selected_target.ends_with(".mei") || selected_target.ends_with(".world.mei") {
                                    preview.into_any()
                                } else {
                                    asset_preview_body(
                                        app_path,
                                        selected_target.as_str(),
                                        source_panel.as_str(),
                                    ).into_any()
                                }}
                            </div>
                            <div
                                id="build-inspect-bar"
                                class="build-inspect-bar shrink-0 border-t mei-border-default px-3 py-2 mei-font-1 mei-text-muted"
                                data-build-inspect-bar="true"
                                hidden=active_tab_enum != BuildViewTab::Preview
                            >
                                <span id="build-inspect-bar-label">"在左侧体验树选择 Panel/Block，或在预览中点击组件以指认上下文。"</span>
                            </div>
                        </section>
                        <section
                            class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                            data-manage-tab-panel="exec"
                            hidden=active_tab_enum != BuildViewTab::Exec
                        >
                            {exec_panel}
                        </section>
                        <section
                            class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                            data-manage-tab-panel="semantic"
                            hidden=active_tab_enum != BuildViewTab::Semantic
                        >
                            {semantic_panel}
                        </section>
                        <section
                            class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                            data-manage-tab-panel="eval"
                            hidden=active_tab_enum != BuildViewTab::Eval
                        >
                            {eval_panel}
                        </section>
                        <section
                            class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                            data-manage-tab-panel="artifact"
                            hidden=active_tab_enum != BuildViewTab::Artifact
                        >
                            {artifact_panel}
                        </section>
                        <section
                            class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                            data-manage-tab-panel="provenance"
                            hidden=active_tab_enum != BuildViewTab::Provenance
                        >
                            {provenance_panel}
                        </section>
                        <section
                            class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                            data-manage-tab-panel="agent"
                            hidden=active_tab_enum != BuildViewTab::Agent
                        >
                            {agent_panel}
                        </section>
                        </div>
                    </section>
                </main>
                {if show_inspector {
                    view! {
                        <>
                            <div
                                class="splitter splitter-right"
                                data-workspace-splitter="right"
                                role="separator"
                                aria-orientation="vertical"
                            ></div>
                            {inspector}
                        </>
                    }
                        .into_any()
                } else {
                    view! { <></> }.into_any()
                }}
            </div>
            {statusbar}
        </div>
    }
    .into_any()
}
