use leptos::prelude::*;
use mei_lang_kernel::{
    ui_scope_annotation_for_preview_panel, BlockDecl, BuildNodeId, CompiledApp, PanelDecl,
    SceneContract, UiNodeDecl,
};

use crate::ui::preview::style::container_visual_style;
use crate::ui::preview::style::{
    panel_body_layout_centered,
    panel_card_layout_style, panel_chrome_bare, panel_head_caret_style, panel_head_carets_enabled,
    panel_head_carets_slot_mode, panel_heading_config, panel_heading_style,
    panel_layout_content_on_body_slot, panel_scale_factor, panel_scaled_outer_style,
    panel_show_heading, panel_slot_area_style, panel_slot_typography_style, panel_style,
};
use crate::ui::preview::theme::{
    resolve_panel_body_props, resolve_panel_card_props, resolve_panel_head_props,
    resolve_shared_refs, ThemeResolved,
};
use crate::ui::preview::PreviewRuntimeContext;

use super::component::{block_view_for_decl, panel_ref_embed_removed_view};

const SLOT_HEAD: &str = "head";
const SLOT_BODY: &str = "body";

fn panel_in_build_preview_scope(panel_path: &str, scope: &str) -> bool {
    let scope_tail = scope
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let normalized_scope = if scope_tail.len() >= 2 {
        scope_tail[1..].join("/")
    } else {
        scope.to_string()
    };
    panel_path == normalized_scope
        || normalized_scope.starts_with(&format!("{panel_path}/"))
        || panel_path.starts_with(&format!("{normalized_scope}/"))
}

pub(crate) fn panel_view(
    panel: &mei_lang_kernel::PanelDecl,
    frame_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    runtime_ctx: &PreviewRuntimeContext,
    theme: &ThemeResolved,
    embed_depth: u8,
    preview_scene_path: &str,
    parent_panel_path: Option<&str>,
    parent_panel: Option<&PanelDecl>,
) -> AnyView {
    let card_props = resolve_shared_refs(&resolve_panel_card_props(theme, panel), &theme.shared);
    let head_props = resolve_shared_refs(&resolve_panel_head_props(theme, panel), &theme.shared);
    let body_props = resolve_shared_refs(&resolve_panel_body_props(theme, panel), &theme.shared);
    let chrome_bare = panel_chrome_bare(&card_props);
    let inherited_tier = parent_panel.and_then(|parent| {
        parent
            .props
            .get("__mei_tier")
            .or_else(|| parent.props.get("tier"))
            .and_then(|value| value.as_str())
    });
    let panel_tier = card_props
        .get("__mei_tier")
        .and_then(|v| v.as_str())
        .or(inherited_tier)
        .unwrap_or("t1");
    let panel_viewpoint = card_props
        .get("__mei_viewpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let panel_view_family = card_props
        .get("__mei_view_family")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let panel_stage_kind = card_props
        .get("__mei_stage_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let panel_world_ref = card_props
        .get("__mei_world_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let panel_entity_id = card_props
        .get("entityId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let panel_group_id = card_props
        .get("groupId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let panel_camera_preset = card_props
        .get("cameraPreset")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let has_head = panel_show_heading(&card_props);
    let heading = panel_heading_config(&theme.panel_head, &head_props, &card_props);
    let heading_class = format!("panel-heading panel-heading-{}", heading.variant);
    let head_carets = panel_head_carets_enabled(&head_props);
    let head_carets_slot = head_carets && panel_head_carets_slot_mode(&head_props);
    let mut heading_cell_style = panel_heading_style(&head_props);
    heading_cell_style.push_str(&panel_slot_typography_style(&head_props));
    if head_carets {
        heading_cell_style.push_str(&panel_head_caret_style(&head_props));
    }
    heading_cell_style.push_str(&container_visual_style(&head_props));
    let (head_nodes, body_nodes) = partition_panel_blocks(&panel.blocks, has_head);
    let has_body_slot = !body_nodes.is_empty();
    let content_grid_on_body =
        !has_head && has_body_slot && panel_layout_content_on_body_slot(panel.layout.as_ref());

    let mut body_cell_style = if content_grid_on_body {
        String::new()
    } else {
        panel_slot_area_style(SLOT_BODY)
    };
    body_cell_style.push_str(&panel_slot_typography_style(&body_props));
    body_cell_style.push_str(&container_visual_style(&body_props));
    if content_grid_on_body {
        body_cell_style.push_str(&panel_card_layout_style(panel.layout.as_ref(), &head_props));
    }

    let mut card_style = panel_style(panel.area.as_deref(), frame_layout, &card_props);
    if content_grid_on_body {
        card_style.push_str("display:grid;gap:0;");
    } else {
        card_style.push_str(&panel_card_layout_style(panel.layout.as_ref(), &head_props));
    }

    let card_class = if chrome_bare {
        "preview-card preview-card-bare"
    } else {
        "preview-card"
    };

    let slot_frame_bg_attr = card_props
        .get("__mei_slot_frame_bg")
        .and_then(|v| v.as_bool())
        .filter(|value| *value)
        .map(|_| "true");

    let metric_card_attr = card_props
        .get("__mei_metric_card")
        .and_then(|v| v.as_bool())
        .filter(|value| *value)
        .map(|_| "true");

    let body_layout_centered = panel
        .layout
        .as_ref()
        .is_some_and(panel_body_layout_centered);
    let body_slot_class = if body_layout_centered {
        "panel-body-cell panel-body-slot-center"
    } else {
        "panel-body-cell"
    };

    let label = panel
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panel.id.clone());

    let panel_path = match parent_panel_path {
        Some(parent) => format!("{parent}/{}", panel.id),
        None => panel.id.clone(),
    };

    if let Some(scope) = runtime_ctx.build_preview_scope.as_deref() {
        if !panel_in_build_preview_scope(panel_path.as_str(), scope) {
            return view! { <></> }.into_any();
        }
    }

    let ui_scope_annotation = if runtime_ctx.build_inspect_enabled {
        ui_scope_annotation_for_preview_panel(
            compiled,
            scene_contract.scene.id.as_str(),
            panel_path.as_str(),
            panel.area.as_deref(),
        )
    } else {
        None
    };
    let (ui_scope_attr, ui_role_attr, build_node_id) = match ui_scope_annotation {
        Some(annotation) => (
            Some(annotation.preview_scope),
            Some(annotation.role),
            Some(annotation.node_id),
        ),
        None => (None, None, None),
    };
    let role_for_projection = ui_role_attr.as_deref().unwrap_or("content");
    if !runtime_ctx.ui_role_allowed_for_projection(role_for_projection) {
        return view! { <></> }.into_any();
    }
    let build_node_id = if runtime_ctx.build_inspect_enabled {
        build_node_id.or_else(|| {
            Some(
                BuildNodeId::scene_panel(
                    scene_contract.scene.id.clone(),
                    panel_path.as_str(),
                )
                .encode(),
            )
        })
    } else {
        None
    };

    let render_head_blocks = || {
        head_nodes
            .iter()
            .map(|node| {
                node_view(
                    node,
                    panel.layout.as_ref(),
                    compiled,
                    app_path,
                    scene_contract,
                    runtime_ctx,
                    theme,
                    embed_depth,
                    preview_scene_path,
                    Some(panel_path.as_str()),
                    Some(panel),
                )
            })
            .collect_view()
    };
    let render_body_blocks = || {
        body_nodes
            .iter()
            .map(|node| {
                node_view(
                    node,
                    panel.layout.as_ref(),
                    compiled,
                    app_path,
                    scene_contract,
                    runtime_ctx,
                    theme,
                    embed_depth,
                    preview_scene_path,
                    Some(panel_path.as_str()),
                    Some(panel),
                )
            })
            .collect_view()
    };

    let section = view! {
        <section
            class=card_class
            style=card_style.clone()
            data-mei-panel-id=panel_path.clone()
            data-mei-panel-area=panel.area.clone().unwrap_or_default()
            data-mei-tier=panel_tier
            data-mei-viewpoint=panel_viewpoint
            data-mei-view-family=panel_view_family
            data-mei-stage-kind=panel_stage_kind
            data-mei-world-ref=panel_world_ref
            data-mei-entity-id=panel_entity_id
            data-mei-group-id=panel_group_id
            data-mei-camera-preset=panel_camera_preset
            data-build-node=build_node_id.clone().unwrap_or_default()
            data-preview-scope=panel_path.clone()
            data-mei-ui-scope=ui_scope_attr.clone().unwrap_or_default()
            data-mei-ui-role=ui_role_attr.clone().unwrap_or_default()
            data-mei-slot-frame-bg=slot_frame_bg_attr.unwrap_or_default()
            data-mei-metric-card=metric_card_attr.unwrap_or_default()
        >
            {if has_head {
                let head_carets_attr = head_carets.then_some("true");
                let head_carets_mode_attr = head_carets_slot.then_some("slot");
                view! {
                    <div
                        class=format!("panel-head-cell {heading_class}")
                        style=format!("{}{}", panel_slot_area_style(SLOT_HEAD), heading_cell_style)
                        data-mei-panel-head="true"
                        data-mei-head-carets=head_carets_attr
                        data-mei-head-carets-mode=head_carets_mode_attr
                        data-heading-variant=heading.variant.clone()
                        aria-label=label.clone()
                    >
                        {heading_chrome_decorations(&heading)}
                        <div class="panel-head-slot">
                            {render_head_blocks()}
                        </div>
                    </div>
                }.into_any()
            } else {
                view! { <></> }.into_any()
            }}
            {if has_body_slot {
                view! {
                    <div
                        class=body_slot_class
                        style=body_cell_style.clone()
                        data-mei-panel-body="true"
                    >
                        {render_body_blocks()}
                    </div>
                }.into_any()
            } else {
                view! { <></> }.into_any()
            }}
        </section>
    };
    if let Some(scale) = panel_scale_factor(&card_props) {
        let outer_style =
            panel_scaled_outer_style(panel.area.as_deref(), frame_layout, &card_props, scale);
        let scaled_section_style = format!(
            "{}transform:scale({});transform-origin:top left;",
            card_style, scale
        );
        view! {
            <div class="preview-card-scale-wrap" style=outer_style>
                <section
                    class=card_class
                    style=scaled_section_style
                    data-mei-panel-id=panel_path.clone()
                    data-mei-tier=panel_tier
                    data-mei-viewpoint=panel_viewpoint
                    data-mei-view-family=panel_view_family
                    data-mei-stage-kind=panel_stage_kind
                    data-mei-world-ref=panel_world_ref
                    data-mei-entity-id=panel_entity_id
                    data-mei-group-id=panel_group_id
                    data-mei-camera-preset=panel_camera_preset
                    data-build-node=build_node_id.clone().unwrap_or_default()
                    data-preview-scope=panel_path.clone()
                    data-mei-ui-scope=ui_scope_attr.clone().unwrap_or_default()
                    data-mei-ui-role=ui_role_attr.clone().unwrap_or_default()
                    data-mei-slot-frame-bg=slot_frame_bg_attr.unwrap_or_default()
                    data-mei-metric-card=metric_card_attr.unwrap_or_default()
                >
                    {if has_head {
                        let head_carets_attr = head_carets.then_some("true");
                        let head_carets_mode_attr = head_carets_slot.then_some("slot");
                        view! {
                            <div
                                class=format!("panel-head-cell {heading_class}")
                                style=format!("{}{}", panel_slot_area_style(SLOT_HEAD), heading_cell_style)
                                data-mei-panel-head="true"
                                data-mei-head-carets=head_carets_attr
                                data-mei-head-carets-mode=head_carets_mode_attr
                                data-heading-variant=heading.variant.clone()
                                aria-label=label.clone()
                            >
                                {heading_chrome_decorations(&heading)}
                                <div class="panel-head-slot">
                                    {render_head_blocks()}
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! { <></> }.into_any()
                    }}
                    {if has_body_slot {
                        view! {
                            <div
                                class=body_slot_class
                                style=body_cell_style.clone()
                                data-mei-panel-body="true"
                            >
                                {render_body_blocks()}
                            </div>
                        }.into_any()
                    } else {
                        view! { <></> }.into_any()
                    }}
                </section>
            </div>
        }
        .into_any()
    } else {
        section.into_any()
    }
}

fn partition_panel_blocks(
    blocks: &[UiNodeDecl],
    has_head: bool,
) -> (Vec<&UiNodeDecl>, Vec<&UiNodeDecl>) {
    let mut head = Vec::new();
    let mut body = Vec::new();
    for node in blocks {
        let area = node_area(node).unwrap_or("");
        if has_head && area == SLOT_HEAD {
            head.push(node);
        } else {
            body.push(node);
        }
    }
    (head, body)
}

fn node_area(node: &UiNodeDecl) -> Option<&str> {
    match node {
        UiNodeDecl::Block(block) => block.area.as_deref(),
        UiNodeDecl::Panel(panel) => panel.area.as_deref(),
        UiNodeDecl::PanelRefEmbed(embed) => embed.area.as_deref(),
    }
}

fn heading_chrome_decorations(heading: &crate::ui::preview::style::PanelHeadingConfig) -> AnyView {
    view! {
        {if heading.show_accent {
            view! { <div class="panel-heading-accent" aria-hidden="true"></div> }.into_any()
        } else {
            view! { <></> }.into_any()
        }}
        {if heading.show_flair {
            view! { <div class="panel-heading-flair panel-heading-flair-left" aria-hidden="true"></div> }.into_any()
        } else {
            view! { <></> }.into_any()
        }}
        {if let Some(subtitle) = heading.subtitle.clone() {
            view! {
                <div class="panel-heading-copy panel-heading-subtitle-only">
                    <p>{subtitle}</p>
                </div>
            }.into_any()
        } else {
            view! { <></> }.into_any()
        }}
        {if heading.show_flair {
            view! { <div class="panel-heading-flair panel-heading-flair-right" aria-hidden="true"></div> }.into_any()
        } else {
            view! { <></> }.into_any()
        }}
        {if heading.show_dots {
            view! {
                <div class="panel-heading-dots" aria-hidden="true">
                    <span></span><span></span><span></span>
                </div>
            }.into_any()
        } else {
            view! { <></> }.into_any()
        }}
    }
    .into_any()
}

pub(super) fn block_ordinal_in_panel(panel: &PanelDecl, block: &BlockDecl) -> usize {
    let mut ord = 0usize;
    for node in &panel.blocks {
        if let UiNodeDecl::Block(candidate) = node {
            if std::ptr::eq(candidate, block) {
                return ord;
            }
            ord += 1;
        }
    }
    ord
}

fn node_view(
    node: &UiNodeDecl,
    parent_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    runtime_ctx: &PreviewRuntimeContext,
    theme: &ThemeResolved,
    embed_depth: u8,
    preview_scene_path: &str,
    parent_panel_id: Option<&str>,
    parent_panel: Option<&PanelDecl>,
) -> AnyView {
    match node {
        UiNodeDecl::Panel(panel) => panel_view(
            panel,
            parent_layout,
            compiled,
            app_path,
            scene_contract,
            runtime_ctx,
            theme,
            embed_depth,
            preview_scene_path,
            parent_panel_id,
            parent_panel,
        ),
        UiNodeDecl::Block(block) => {
            if let Some(use_key) = runtime_ctx.build_preview_component_use_key.as_deref() {
                if block.use_key.as_str() != use_key {
                    return ().into_any();
                }
            }
            block_view_for_decl(
                block,
                parent_layout,
                compiled,
                app_path,
                scene_contract,
                runtime_ctx,
                theme,
                preview_scene_path,
                parent_panel_id,
                parent_panel,
            )
        }
        UiNodeDecl::PanelRefEmbed(embed) => panel_ref_embed_removed_view(embed, parent_layout),
    }
}

