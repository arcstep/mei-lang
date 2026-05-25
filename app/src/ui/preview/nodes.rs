use leptos::prelude::*;
use mei_lang_kernel::{BlockDecl, CompiledApp, PanelRefEmbedDecl, SceneContract, UiNodeDecl};
use serde_json::Value;

use super::style::container_visual_style;
use super::style::{
    block_style, metric_slot_vertical_host_class, panel_body_layout_centered,
    panel_card_layout_style, panel_chrome_bare, panel_head_caret_style, panel_head_carets_enabled,
    panel_heading_config, panel_heading_style, panel_layout_content_on_body_slot,
    panel_scale_factor, panel_scaled_outer_style, panel_show_heading, panel_slot_area_style,
    panel_slot_typography_style, panel_style,
};
use super::theme::{
    resolve_panel_body_props, resolve_panel_card_props, resolve_panel_head_props, ThemeResolved,
};
use super::{
    resolve::{attach_host_meta, resolve_value, RuntimeSceneAnchor},
    PreviewRuntimeContext,
};

const SLOT_HEAD: &str = "head";
const SLOT_BODY: &str = "body";

pub(super) fn panel_view(
    panel: &mei_lang_kernel::PanelDecl,
    frame_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    runtime_ctx: &PreviewRuntimeContext,
    theme: &ThemeResolved,
    embed_depth: u8,
    preview_scene_path: &str,
) -> AnyView {
    let card_props = resolve_panel_card_props(theme, panel);
    let head_props = resolve_panel_head_props(theme, panel);
    let body_props = resolve_panel_body_props(theme, panel);
    let chrome_bare = panel_chrome_bare(&card_props);
    let has_head = panel_show_heading(&card_props);
    let heading = panel_heading_config(&theme.panel_head, &head_props, &card_props);
    let heading_class = format!("panel-heading panel-heading-{}", heading.variant);
    let head_carets = panel_head_carets_enabled(&head_props);
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
                )
            })
            .collect_view()
    };

    let section = view! {
        <section
            class=card_class
            style=card_style.clone()
            data-mei-panel-id=panel.id.clone()
        >
            {if has_head {
                let head_carets_attr = head_carets.then_some("true");
                view! {
                    <div
                        class=format!("panel-head-cell {heading_class}")
                        style=format!("{}{}", panel_slot_area_style(SLOT_HEAD), heading_cell_style)
                        data-mei-panel-head="true"
                        data-mei-head-carets=head_carets_attr
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
        let outer_style = panel_scaled_outer_style(
            panel.area.as_deref(),
            frame_layout,
            &card_props,
            scale,
        );
        let scaled_section_style = format!(
            "{}transform:scale({});transform-origin:top left;",
            card_style,
            scale
        );
        view! {
            <div class="preview-card-scale-wrap" style=outer_style>
                <section
                    class=card_class
                    style=scaled_section_style
                    data-mei-panel-id=panel.id.clone()
                >
                    {if has_head {
                        let head_carets_attr = head_carets.then_some("true");
                        view! {
                            <div
                                class=format!("panel-head-cell {heading_class}")
                                style=format!("{}{}", panel_slot_area_style(SLOT_HEAD), heading_cell_style)
                                data-mei-panel-head="true"
                                data-mei-head-carets=head_carets_attr
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

fn heading_chrome_decorations(heading: &super::style::PanelHeadingConfig) -> AnyView {
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
        ),
        UiNodeDecl::Block(block) => block_view(
            block,
            parent_layout,
            compiled,
            app_path,
            scene_contract,
            runtime_ctx,
            theme,
            preview_scene_path,
        ),
        UiNodeDecl::PanelRefEmbed(embed) => panel_ref_embed_removed_view(embed, parent_layout),
    }
}

fn panel_ref_embed_removed_view(
    embed: &PanelRefEmbedDecl,
    parent_layout: Option<&mei_lang_kernel::LayoutDecl>,
) -> AnyView {
    let path = embed.scene_file.trim();
    let label = embed
        .title
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| path.to_string());
    view! {
        <section
            class="preview-card panel-ref-embed-error"
            style=block_style(embed.area.as_deref(), parent_layout)
        >
            <div class="panel-head-cell panel-heading">
                <h3>{label}</h3>
            </div>
            <p class="text-sm text-red-300/90">
                "panel_ref 仅支持在 frame.panels 中按 id 引用外部 panel；block 内 scene 嵌入已移除。"
            </p>
        </section>
    }
    .into_any()
}

fn block_view(
    block: &BlockDecl,
    panel_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    runtime_ctx: &PreviewRuntimeContext,
    theme: &ThemeResolved,
    preview_scene_path: &str,
) -> AnyView {
    let scene_anchor = RuntimeSceneAnchor {
        scene_id: scene_contract.scene.id.clone(),
        scene_path: Some(preview_scene_path.to_string()),
    };
    let props = attach_host_meta(
        resolve_value(
            &block.props,
            scene_contract,
            &runtime_ctx.resources,
            &scene_anchor,
            &runtime_ctx.index,
            compiled,
        ),
        compiled,
        app_path,
        &theme.components,
        Some(preview_scene_path),
    );
    let tag = compiled
        .component_assets
        .iter()
        .find(|asset| asset.key == block.use_key)
        .map(|asset| asset.tag.clone())
        .unwrap_or_else(|| "mei-missing-component".to_string());
    let html = component_html(tag.as_str(), &props);
    let is_header_brand = block.use_key == "cockpit.header-brand";
    let slot_v_class = if is_header_brand {
        String::new()
    } else {
        metric_slot_vertical_host_class(&props).to_string()
    };
    let card_class = if slot_v_class.is_empty() {
        "component-card".to_string()
    } else {
        format!("component-card {slot_v_class}")
    };
    let block_layout = if is_header_brand { None } else { panel_layout };
    view! {
        <section
            class=card_class
            style=block_style(block.area.as_deref(), block_layout)
        >
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}

fn component_html(tag: &str, props: &Value) -> String {
    let props_json = props.to_string();
    format!("<{tag} data-props='{props_json}'></{tag}>")
}
