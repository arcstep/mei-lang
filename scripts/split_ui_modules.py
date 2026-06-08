#!/usr/bin/env python3
"""One-shot split of oversized mei-lang-app UI modules."""
import os
import sys
from pathlib import Path

BASE = os.path.join(os.path.dirname(__file__), "..", "app", "src", "ui")
PREVIEW = os.path.join(BASE, "preview")


def lines(path):
    with open(path, encoding="utf-8") as f:
        return f.read().splitlines(keepends=True)


def sl(all_lines, start, end):
    return "".join(all_lines[start - 1 : end])


def write(path, content):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)


def pubsuper(content, fns):
    for fn in fns:
        if f"\npub(super) fn {fn}(" not in content:
            content = content.replace(f"\nfn {fn}(", f"\npub(super) fn {fn}(")
    return content


def split_preview():
    rl = lines(os.path.join(PREVIEW, "resolve.rs"))
    ri = """use std::collections::{BTreeMap, BTreeSet};

use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_dataset_resource_id, resolve_dataset_selector_value,
    host_runtime_capabilities_catalog, host_runtime_contract_descriptor,
    resolve_runtime_metric_def_key, scene_payload_cache_epoch, CompiledApp, LoadedResource,
    RuntimeResourceIndex, SceneContract,
};
use serde_json::{json, Value};

use super::super::theme::resolve_shared_refs;

"""
    context = (
        ri
        + """use super::drilldown::MetricDrilldownMeta;
use super::drilldown::resolve_metric_drilldown_meta;
use super::refs::{resolve_data_ref, resolve_metric_ref, resolve_rows_expr, with_runtime_ref};

"""
        + sl(rl, 13, 48)
        + sl(rl, 131, 471)
    )
    refs = pubsuper(
        ri + sl(rl, 473, 555),
        ["resolve_data_ref", "resolve_metric_ref", "resolve_rows_expr", "with_runtime_ref"],
    )
    drilldown = (
        ri
        + """use super::explain::{
    apply_analyses_value, apply_explain_items, apply_explain_object,
    object_map_from_value, string_array_from_value,
};

"""
        + sl(rl, 49, 129).replace("struct MetricDrilldownMeta", "pub(crate) struct MetricDrilldownMeta")
        + sl(rl, 557, 1197)
        + sl(rl, 1864, 1914)
    )
    drilldown = pubsuper(
        drilldown,
        [
            "resolve_metric_drilldown_meta",
            "first_non_empty_string",
            "apply_ratio_parts",
            "metric_note_text",
            "tabs_from_value",
            "is_empty",
            "has_explain_semantics",
        ],
    )
    drilldown = drilldown.replace("    fn is_empty(", "    pub(crate) fn is_empty(")
    drilldown = drilldown.replace("    fn has_explain_semantics(", "    pub(crate) fn has_explain_semantics(")
    explain = (
        ri
        + "use super::drilldown::{\n"
        + "    apply_ratio_parts, first_non_empty_string, metric_note_text, MetricDrilldownMeta,\n"
        + "};\n\n"
        + sl(rl, 1199, 1862)
    )
    explain = pubsuper(
        explain,
        [
            "string_array_from_value",
            "object_map_from_value",
            "apply_explain_items",
            "apply_explain_object",
            "apply_analyses_value",
        ],
    )
    resolve_dir = os.path.join(PREVIEW, "resolve")
    write(
        os.path.join(resolve_dir, "mod.rs"),
        "mod context;\nmod drilldown;\nmod explain;\nmod refs;\n\n"
        "pub(crate) use context::{attach_host_meta, resolve_value, HostMetaOptions, RuntimeSceneAnchor};\n",
    )
    write(os.path.join(resolve_dir, "context.rs"), context)
    write(os.path.join(resolve_dir, "refs.rs"), refs)
    write(os.path.join(resolve_dir, "drilldown.rs"), drilldown)
    write(os.path.join(resolve_dir, "explain.rs"), explain)
    os.remove(os.path.join(PREVIEW, "resolve.rs"))

    slines = lines(os.path.join(PREVIEW, "style.rs"))
    si = "use serde_json::Value;\n\nuse super::super::theme::deep_merge_value;\n\n"
    layout = si + sl(slines, 14, 90) + sl(slines, 765, 786) + sl(slines, 851, 878) + sl(slines, 898, 979)
    panel = (
        si
        + "use super::layout::{grid_template_areas_style, normalize_css_length, normalize_background_image, surface_layout_style, length_px_from_value};\n\n"
        + sl(slines, 5, 12)
        + sl(slines, 92, 606)
        + sl(slines, 607, 763)
        + sl(slines, 830, 849)
    )
    metric = si + "use super::layout::{length_px_from_props, length_px_from_value};\n\n" + sl(slines, 787, 828) + sl(
        slines, 880, 896
    )
    style_dir = os.path.join(PREVIEW, "style")
    write(
        os.path.join(style_dir, "mod.rs"),
        """mod layout;
mod metric;
mod panel;

pub(crate) use layout::{
    block_style, grid_template_areas_style, length_px_from_props, length_px_from_value,
    normalize_background_image, normalize_css_length, surface_layout_style,
};
pub(crate) use metric::{metric_slot_vertical_host_class, FrameStageContentBounds, frame_stage_content_bounds};
pub(crate) use panel::{
    append_string_style, container_visual_style, container_visual_style_without_background,
    frame_background_color, frame_backdrop_css_vars, frame_viewport_letterbox_style,
    has_frame_backdrop, panel_card_layout_style, panel_chrome_bare, panel_head_caret_style,
    panel_head_carets_enabled, panel_head_carets_slot_mode, panel_heading_config,
    panel_heading_style, panel_layout_content_on_body_slot, panel_body_layout_centered,
    panel_position_style, panel_scale_factor, panel_scaled_outer_style, panel_show_heading,
    panel_slot_area_style, panel_slot_typography_style, panel_style, PanelHeadingConfig,
};
""",
    )
    write(os.path.join(style_dir, "layout.rs"), layout)
    write(os.path.join(style_dir, "panel.rs"), panel)
    write(os.path.join(style_dir, "metric.rs"), metric)
    os.remove(os.path.join(PREVIEW, "style.rs"))

    tl = lines(os.path.join(PREVIEW, "theme.rs"))
    parse = (
        "use mei_lang_kernel::{decode_theme_ref_token, PanelDecl, SceneContract, ThemeDecl};\n"
        "use serde_json::Value;\n\n"
        "use super::{deep_merge_value, resolve_shared_refs};\n\n"
        + sl(tl, 4, 353)
        + sl(tl, 454, 562)
        + """
pub(super) fn theme_css_vars_style(theme: &ThemeResolved) -> String {
    let mut style = String::new();
    style.push_str(&format!("--mei-theme-id:'{}';", theme.id));
    for (key, value) in &theme.css_vars {
        style.push_str(&format!("{key}:{value};"));
    }
    style
}

"""
        + sl(tl, 573, 613)
        + "}\n"
    )
    merge = (
        "use mei_lang_kernel::PanelDecl;\nuse serde_json::Value;\n\nuse super::ThemeResolved;\n\n"
        + sl(tl, 355, 452)
    )
    merge = pubsuper(
        merge,
        [
            "resolve_shared_refs",
            "deep_merge_value",
            "resolve_panel_card_props",
            "resolve_panel_props",
            "resolve_panel_head_props",
            "resolve_panel_body_props",
        ],
    )
    merge += (
        "#[cfg(test)]\nmod tests {\n    use super::*;\n    use serde_json::json;\n\n"
        + sl(tl, 615, 644)
    )
    theme_dir = os.path.join(PREVIEW, "theme")
    write(
        os.path.join(theme_dir, "mod.rs"),
        """mod merge;
mod parse;

pub(crate) use merge::{
    deep_merge_value, resolve_panel_body_props, resolve_panel_card_props, resolve_panel_head_props,
    resolve_panel_props, resolve_shared_refs,
};
pub(crate) use parse::{resolve_theme, theme_css_vars_style, ThemeResolved};
""",
    )
    write(os.path.join(theme_dir, "parse.rs"), parse)
    write(os.path.join(theme_dir, "merge.rs"), merge)
    os.remove(os.path.join(PREVIEW, "theme.rs"))

    vl = lines(os.path.join(PREVIEW, "viewport.rs"))
    vi = """use mei_lang_kernel::LayoutDecl;
use serde_json::{json, Value};

use super::super::style::{
    container_visual_style, container_visual_style_without_background, frame_backdrop_css_vars,
    frame_stage_content_bounds, surface_layout_style, FrameStageContentBounds,
};
use super::super::theme::{theme_css_vars_style, ThemeResolved};
use crate::ui::route::UiRouteMode;

"""
    compute = vi + sl(vl, 11, 181) + sl(vl, 300, 471) + sl(vl, 484, 524)
    compute = pubsuper(compute, ["fluid_relaxed_layout"])
    css = (
        vi
        + """use super::compute::{
    effective_canvas_width, effective_viewport_overflow, effective_viewport_safe_inset,
    fluid_relaxed_layout, frame_stage_content_bounds_for_viewport, FrameViewportConfig,
    viewport_overflow_is_debug,
};

"""
        + sl(vl, 183, 298)
        + sl(vl, 473, 482)
        + sl(vl, 526, 576)
    )
    css = pubsuper(
        css,
        [
            "frame_viewport_style_for_route",
            "frame_viewport_style_page_flow_for_route",
            "frame_viewport_style_fluid_width_for_route",
            "frame_style",
            "frame_stage_style",
        ],
    )
    viewport_dir = os.path.join(PREVIEW, "viewport")
    write(
        os.path.join(viewport_dir, "mod.rs"),
        """mod compute;
mod css;

pub(crate) use compute::{
    default_viewport_for_profile, default_viewport_page_flow, default_viewport_stage_lock,
    effective_canvas_width, effective_viewport_overflow, effective_viewport_safe_inset,
    frame_stage_content_bounds_for_viewport, frame_viewport_config, frame_viewport_is_explicit,
    resolve_frame_viewport, viewport_overflow_is_debug, FrameViewportConfig,
};
pub(crate) use css::{
    frame_stage_style, frame_style, frame_viewport_style_fluid_width_for_route,
    frame_viewport_style_for_route, frame_viewport_style_page_flow_for_route,
};
""",
    )
    write(os.path.join(viewport_dir, "compute.rs"), compute)
    write(os.path.join(viewport_dir, "css.rs"), css)
    os.remove(os.path.join(PREVIEW, "viewport.rs"))


def split_topbar():
    tb = lines(os.path.join(BASE, "topbar.rs"))
    ti = """use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, CompiledSceneRoute, WorkspaceAppMeta};
use std::collections::BTreeMap;

use super::super::manage_routing::{access_scene_query, encode_query_value};
use super::super::route::UiRouteMode;
use super::super::view_routing::{app_scene_href, build_href, config_href, cross_app_href, upload_href};
use super::super::{HostAccountView, HostCapabilities, TopbarMenuConfig, TopbarMenuContext};

"""
    menu_groups = ti.replace("use leptos::prelude::*;\n", "") + sl(tb, 13, 202)
    menu_groups = menu_groups.replace("\nfn build_topbar_menu_groups(", "\npub(crate) fn build_topbar_menu_groups(", 1)
    view = ti + "use super::menu_groups::build_topbar_menu_groups;\n\n" + sl(tb, 204, 684)
    view = pubsuper(view, ["access_scene_for_topbar", "topbar_view"])
    topbar_dir = os.path.join(BASE, "topbar")
    write(
        os.path.join(topbar_dir, "mod.rs"),
        "mod menu_groups;\nmod view;\n\npub(crate) use view::{access_scene_for_topbar, topbar_view};\n",
    )
    write(os.path.join(topbar_dir, "menu_groups.rs"), menu_groups)
    write(os.path.join(topbar_dir, "view.rs"), view)
    os.remove(os.path.join(BASE, "topbar.rs"))


def split_shell_manage():
    sm = lines(os.path.join(BASE, "shell_manage.rs"))
    si = """use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta, WorkspaceNode};

use super::super::compile_status::{
    classify_asset_shell, codemirror_dataset_lang, compiled_has_error_diagnostics,
    is_mei_script_target, visible_diagnostics_count, AssetShellKind, DiagnosticsFilterMode,
};
use super::super::manage_routing::{manage_tab_href, manage_view_tab_from_query, ManageViewTab};
use super::super::preview;
use super::super::preview_chrome::{asset_preview_body, diagnostics_view};
use super::super::route::UiRouteMode;
use super::super::source_tree;
use super::super::statusbar::statusbar_view;
use super::super::topbar::{access_scene_for_topbar, topbar_view};
use super::super::{HostAccountView, SourcePanelMeta, TopbarMenuContext};

"""
    helpers = sl(sm, 17, 48)
    shell_dir = os.path.join(BASE, "shell_manage")
    write(
        os.path.join(shell_dir, "mod.rs"),
        "mod layout;\nmod source_tree;\n\n"
        "pub(crate) use layout::manage_shell;\n"
        "pub(crate) use source_tree::manage_source_shell;\n",
    )
    write(os.path.join(shell_dir, "layout.rs"), pubsuper(si + helpers + sl(sm, 50, 424), ["manage_shell"]))
    write(os.path.join(shell_dir, "source_tree.rs"), pubsuper(si + helpers + sl(sm, 426, 626), ["manage_source_shell"]))
    os.remove(os.path.join(BASE, "shell_manage.rs"))


def pub_crate_struct_fields(content, struct_name):
    import re

    pattern = rf"((?:pub\(crate\) )?struct {struct_name} \{{)([\s\S]*?)(\n\}})"
    match = re.search(pattern, content)
    if not match:
        return content
    head, body, tail = match.group(1), match.group(2), match.group(3)
    if "pub(crate)" not in head and "pub(super)" not in head:
        head = head.replace("struct ", "pub(crate) struct ", 1)
    lines = []
    for line in body.splitlines():
        stripped = line.strip()
        if (
            stripped
            and not stripped.startswith("pub(")
            and ":" in stripped
            and not stripped.startswith("//")
        ):
            indent = line[: len(line) - len(line.lstrip())]
            name = stripped.split(":", 1)[0].strip()
            lines.append(f"{indent}pub(crate) {name}:{stripped.split(':', 1)[1]}")
        else:
            lines.append(line)
    updated = head + "\n".join(lines) + tail
    return content[: match.start()] + updated + content[match.end() :]


def post_process_preview():
    targets = [
        (Path(PREVIEW) / "resolve" / "drilldown.rs", "MetricDrilldownMeta"),
        (Path(PREVIEW) / "theme" / "parse.rs", "ThemeResolved"),
        (Path(PREVIEW) / "style" / "panel.rs", "PanelHeadingConfig"),
        (Path(PREVIEW) / "viewport" / "compute.rs", "FrameViewportConfig"),
        (Path(BASE) / "topbar" / "menu_groups.rs", "TopbarMenuItem"),
        (Path(BASE) / "topbar" / "menu_groups.rs", "TopbarMenuGroup"),
    ]
    for path, struct_name in targets:
        text = path.read_text()
        text = pub_crate_struct_fields(text, struct_name)
        path.write_text(text)


def widen_visibility(base_dir):
    for path in Path(base_dir).rglob("*.rs"):
        text = path.read_text()
        updated = (
            text.replace("pub(super) fn ", "pub(crate) fn ")
            .replace("pub(super) struct ", "pub(crate) struct ")
            .replace("pub(super) ", "pub(crate) ")
        )
        if updated != text:
            path.write_text(updated)


def split_document():
    ui = lines(os.path.join(BASE, "mod.rs"))
    doc = (
        "use leptos::prelude::*;\n\n"
        "use crate::ui::capabilities::HostCapabilities;\n"
        "use crate::ui::preview_chrome::chrome_scripts_view;\n"
        "use crate::ui::route::UiRouteMode;\n"
        "use crate::ui::HostAccountView;\n\n"
        + sl(ui, 107, 210)
    ).replace("\nfn render_document(", "\npub(crate) fn render_document(", 1)
    write(os.path.join(BASE, "document.rs"), doc)
    new_mod = sl(ui, 1, 9) + "mod document;\n" + sl(ui, 10, 21) + "\n" + sl(ui, 23, 31)
    new_mod += "\nuse document::render_document;\n\n"
    new_mod += sl(ui, 33, 106)
    new_mod += "\n" + sl(ui, 212, 502)
    write(os.path.join(BASE, "mod.rs"), new_mod)


def main():
    split_preview()
    split_topbar()
    split_shell_manage()
    split_document()
    widen_visibility(Path(BASE))
    post_process_preview()
    for name in ["resolve", "style", "theme", "viewport"]:
        path = os.path.join(PREVIEW, name, "mod.rs")
        if not os.path.isfile(path):
            print(f"missing {path}", file=sys.stderr)
            sys.exit(1)
    print("split complete")


if __name__ == "__main__":
    main()
