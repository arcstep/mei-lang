use leptos::prelude::*;
use mei_lang_kernel::{
    block_instance_id, ui_scope_for_block, BlockDecl, BuildNodeId, CompiledApp, PanelDecl,
    PanelRefEmbedDecl, SceneContract,
};
use serde_json::Value;

use crate::ui::preview::style::{
    block_style, metric_slot_vertical_host_class,
};
use crate::ui::preview::theme::ThemeResolved;
use crate::ui::preview::{
    resolve::{attach_host_meta, resolve_value, HostMetaOptions, RuntimeSceneAnchor},
    PreviewRuntimeContext,
};

use super::panel::block_ordinal_in_panel;

pub(super) fn panel_ref_embed_removed_view(
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

pub(crate) fn block_view_for_decl(
    block: &BlockDecl,
    panel_layout: Option<&mei_lang_kernel::LayoutDecl>,
    compiled: &CompiledApp,
    app_path: &str,
    scene_contract: &SceneContract,
    runtime_ctx: &PreviewRuntimeContext,
    theme: &ThemeResolved,
    preview_scene_path: &str,
    parent_panel_id: Option<&str>,
    parent_panel: Option<&PanelDecl>,
) -> AnyView {
    let scene_anchor = RuntimeSceneAnchor::for_preview(
        compiled,
        Some(preview_scene_path),
        Some(scene_contract.scene.id.as_str()),
    );
    let resolved = resolve_value(
        &block.props,
        &theme.shared,
        scene_contract,
        &runtime_ctx.resources,
        &scene_anchor,
        &runtime_ctx.index,
        compiled,
        runtime_ctx.host_ssr_slim_payload,
    );
    let host_meta_options = HostMetaOptions {
        include_scene_drilldown_context: should_include_scene_drilldown_context(
            block.use_key.as_str(),
            &resolved,
            runtime_ctx.host_ssr_slim_payload,
        ),
        host_ssr_slim_payload: runtime_ctx.host_ssr_slim_payload,
    };
    let props = attach_host_meta(
        resolved,
        compiled,
        app_path,
        &theme.components,
        Some(preview_scene_path),
        host_meta_options,
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
    let block_id = parent_panel
        .map(|panel| block_instance_id(block, block_ordinal_in_panel(panel, block)))
        .unwrap_or_else(|| {
            block
                .id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(block.use_key.as_str())
                .to_string()
        });
    let block_viewpoint = block
        .props
        .get("__mei_viewpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let block_view_family = block
        .props
        .get("__mei_view_family")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let block_stage_kind = block
        .props
        .get("__mei_stage_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let block_world_ref = block
        .props
        .get("__mei_world_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let block_entity_id = block
        .props
        .get("entityId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let block_group_id = block
        .props
        .get("groupId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let block_camera_preset = block
        .props
        .get("cameraPreset")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let ui_scope_annotation = if runtime_ctx.build_inspect_enabled {
        parent_panel_id.and_then(|panel_path| {
            ui_scope_for_block(
                compiled,
                scene_contract.scene.id.as_str(),
                panel_path,
                block_id.as_str(),
            )
        })
    } else {
        None
    };
    let build_node_id = if runtime_ctx.build_inspect_enabled {
        ui_scope_annotation.as_ref().map(|annotation| annotation.node_id.clone()).or_else(|| {
            parent_panel_id.map(|panel_id| {
                BuildNodeId::scene_block(
                    scene_contract.scene.id.clone(),
                    panel_id,
                    block_id.as_str(),
                )
                .encode()
            })
        })
    } else {
        None
    };
    let ui_scope_attr = ui_scope_annotation
        .as_ref()
        .map(|annotation| annotation.preview_scope.clone());
    let ui_role_attr = ui_scope_annotation
        .as_ref()
        .map(|annotation| annotation.role.clone());
    view! {
        <section
            class=card_class
            style=block_style(block.area.as_deref(), block_layout)
            data-mei-block-id=block_id.to_string()
            data-mei-use-key=block.use_key.clone()
            data-mei-viewpoint=block_viewpoint
            data-mei-view-family=block_view_family
            data-mei-stage-kind=block_stage_kind
            data-mei-world-ref=block_world_ref
            data-mei-entity-id=block_entity_id
            data-mei-group-id=block_group_id
            data-mei-camera-preset=block_camera_preset
            data-build-node=build_node_id.clone().unwrap_or_default()
            data-build-focus=build_node_id.clone().unwrap_or_default()
            data-mei-ui-scope=ui_scope_attr.clone().unwrap_or_default()
            data-mei-ui-role=ui_role_attr.clone().unwrap_or_default()
        >
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}

fn should_include_scene_drilldown_context(
    use_key: &str,
    props: &Value,
    host_ssr_slim_payload: bool,
) -> bool {
    // Host 视图通过 `#mei-scene-drilldown-context` 全局注入，避免每个组件 data-props 重复膨胀。
    if host_ssr_slim_payload {
        return false;
    }
    if use_key == "mei.text" {
        return true;
    }
    if use_key == "cockpit.data-table" {
        if let Some(map) = props.as_object() {
            return map.contains_key("popup") || map.contains_key("analysis");
        }
    }
    false
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn component_html(tag: &str, props: &Value) -> String {
    let props_json = props.to_string();
    let escaped = escape_html_attr(&props_json);
    format!("<{tag} data-props=\"{escaped}\"></{tag}>")
}
