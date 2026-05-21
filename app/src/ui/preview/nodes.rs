use leptos::prelude::*;
use mei_lang_kernel::{BlockDecl, CompiledApp, SceneContract, UiNodeDecl};
use serde_json::Value;

use super::{
    resolve::{attach_host_meta, resolve_value, RuntimeSceneAnchor},
    PreviewRuntimeContext,
};
use super::style::{
    block_style, panel_body_style, panel_heading_config, panel_show_heading, panel_style,
};
use super::theme::{resolve_panel_props, ThemeResolved};

pub(super) fn panel_view(
    panel: &mei_lang_kernel::PanelDecl,
    frame_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    runtime_ctx: &PreviewRuntimeContext,
    theme: &ThemeResolved,
) -> AnyView {
    let panel_props = resolve_panel_props(theme, &panel.props);
    let blocks = panel
        .blocks
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
            )
        })
        .collect_view();
    let title = panel.title.clone().unwrap_or_else(|| panel.id.clone());
    let show_heading = panel_show_heading(&panel_props);
    let heading = panel_heading_config(&theme.heading, &panel_props);
    let heading_class = format!("panel-heading panel-heading-{}", heading.variant);
    view! {
        <section class="preview-card" style=panel_style(panel.area.as_deref(), frame_layout, &panel_props)>
            {if show_heading {
                view! {
                    <div
                        class=heading_class
                        data-heading-variant=heading.variant.clone()
                    >
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
                        <div class="panel-heading-copy">
                            <h3>{title}</h3>
                            {if let Some(subtitle) = heading.subtitle.clone() {
                                view! { <p>{subtitle}</p> }.into_any()
                            } else {
                                view! { <></> }.into_any()
                            }}
                        </div>
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
                    </div>
                }.into_any()
            } else {
                view! { <></> }.into_any()
            }}
            <div class="grid min-w-0 gap-3" style=panel_body_style(panel.layout.as_ref())>
                {blocks}
            </div>
        </section>
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
        ),
        UiNodeDecl::Block(block) => block_view(
            block,
            parent_layout,
            compiled,
            app_path,
            scene_contract,
            runtime_ctx,
            theme,
        ),
        UiNodeDecl::PanelCapsuleRef(capsule) => {
            let path = capsule.scene_file.clone();
            let label = capsule.title.clone().unwrap_or_else(|| path.clone());
            let area = capsule.area.as_deref();
            view! {
                <section
                    class="preview-card frame-ref-slot"
                    style=block_style(area, parent_layout)
                    data-panel-capsule=path.as_str()
                >
                    <div class="panel-heading">
                        <h3>{label}</h3>
                    </div>
                    <p class="text-sm opacity-70">{"embedded capsule: "}{path.clone()}</p>
                </section>
            }
            .into_any()
        }
        UiNodeDecl::FrameRef(frame_ref) => {
            let path = frame_ref.frame_ref.clone();
            let label = frame_ref
                .title
                .clone()
                .unwrap_or_else(|| path.clone());
            let area = frame_ref.area.as_deref();
            view! {
                <section
                    class="preview-card frame-ref-slot"
                    style=block_style(area, parent_layout)
                    data-panel-capsule=path.as_str()
                >
                    <div class="panel-heading">
                        <h3>{label}</h3>
                    </div>
                    <p class="text-sm opacity-70">{"embedded capsule: "}{path.clone()}</p>
                </section>
            }
            .into_any()
        }
    }
}

fn block_view(
    block: &BlockDecl,
    panel_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    runtime_ctx: &PreviewRuntimeContext,
    theme: &ThemeResolved,
) -> AnyView {
    let scene_anchor = RuntimeSceneAnchor::from_compiled(compiled);
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
    );
    let tag = compiled
        .component_assets
        .iter()
        .find(|asset| asset.key == block.use_key)
        .map(|asset| asset.tag.clone())
        .unwrap_or_else(|| "mei-missing-component".to_string());
    let html = component_html(tag.as_str(), &props);
    view! {
        <section class="component-card" style=block_style(block.area.as_deref(), panel_layout)>
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}
fn component_html(tag: &str, props: &Value) -> String {
    let props_json = props.to_string();
    format!("<{tag} data-props='{props_json}'></{tag}>")
}
