use mei_lang_kernel::{
    build_experience_path, build_overview_backing, experience_layout_hint,
    experience_mount_chain, format_experience_path, ProvenanceAnchor,
};
use mei_lang_toolchain::format_semantic_graph_markdown;

use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::registry::MrgRegistryWriter;

use super::super::graph_markdown;

pub(super) fn append_ux_sections(
    md: &mut String,
    compiled: &mei_lang_kernel::CompiledApp,
    ctx: &mei_lang_kernel::BuildNodeContext,
    node: &mei_lang_kernel::BuildNodeId,
    focus: Option<&str>,
) {
    let path = build_experience_path(compiled, node);
    md.push_str("### 体验路径\n\n");
    md.push_str(&format!("{}\n\n", format_experience_path(&path)));

    md.push_str("### UI 锚点\n\n");
    md.push_str(&format!("- node: `{}`\n", node.encode()));
    if let Some(focus) = focus.map(str::trim).filter(|value| !value.is_empty()) {
        md.push_str(&format!("- focus: `{focus}`\n"));
    }
    md.push_str(&format!("- target_file: `{}`\n", ctx.target_file));
    if let Some(scene) = ctx.scene_id.as_deref() {
        md.push_str(&format!("- scene_id: `{scene}`\n"));
    }
    md.push_str(&format!(
        "- symbol: `{}` ({}) in `{}`\n",
        ctx.provenance.symbol_id, ctx.provenance.symbol_kind, ctx.provenance.file
    ));
    md.push('\n');

    let mount_chain = experience_mount_chain(compiled, node);
    if !mount_chain.is_empty() {
        md.push_str("### 挂载链\n\n");
        for entry in mount_chain {
            md.push_str(&format!(
                "- `{}`#`{}` ({}) \n",
                entry.file, entry.panel_id, entry.role
            ));
        }
        md.push('\n');
    }
    if let Some(hint) = experience_layout_hint(compiled, node) {
        md.push_str("### 布局\n\n");
        md.push_str(&format!("- {hint}\n\n"));
    }

    let backing = build_overview_backing(compiled, node);
    if !backing.is_empty() {
        md.push_str("### Backing\n\n");
        for item in backing {
            md.push_str(&format!("- {item}\n"));
        }
        md.push('\n');
    }
}

pub(super) fn append_board_template_sections(
    md: &mut String,
    compiled: &mei_lang_kernel::CompiledApp,
    node: &mei_lang_kernel::BuildNodeId,
) {
    use mei_lang_kernel::BuildNodeKind;
    if let Some(entry) = compiled.build_board_index.lookup(node) {
        md.push_str("### Board\n\n");
        md.push_str(&format!("- board_file: `{}`\n", entry.board_file));
        md.push_str(&format!("- scene_id: `{}`\n", entry.scene_id));
        if let Some(mode) = entry.layout_mode.as_deref() {
            md.push_str(&format!("- layout_mode: `{mode}`\n"));
        }
        if let Some(params) = entry.params_summary.as_deref() {
            md.push_str(&format!("- params: `{params}`\n"));
        }
        if !entry.slots.is_empty() {
            md.push_str("\n#### Slots\n\n");
            for slot in &entry.slots {
                md.push_str(&format!(
                    "- `{}` component={} backing={:?}\n",
                    slot.slot_id,
                    slot.component.as_deref().unwrap_or("-"),
                    slot.backing_refs
                ));
            }
        }
        md.push('\n');
    }
    if node.kind == BuildNodeKind::Template {
        if let Some(entry) = compiled.build_template_index.lookup(node.key.as_str()) {
            md.push_str("### 模板\n\n");
            md.push_str(&format!("- template_key: `{}`\n", entry.template_key));
            md.push_str(&format!("- template_file: `{}`\n", entry.template_file));
            md.push_str(&format!("- category: `{}`\n", entry.category));
            if !entry.props_schema.is_empty() {
                md.push_str("- props_schema:\n");
                for item in &entry.props_schema {
                    md.push_str(&format!("  - {item}\n"));
                }
            }
            if !entry.consumers.is_empty() {
                md.push_str("- consumers:\n");
                for item in &entry.consumers {
                    md.push_str(&format!("  - {item}\n"));
                }
            }
            if !entry.consumer_anchors.is_empty() {
                md.push_str("- consumer_anchors:\n");
                for anchor in &entry.consumer_anchors {
                    md.push_str(&format!(
                        "  - scene=`{}` panel=`{}` block=`{}` label=`{}`\n",
                        anchor.scene_id, anchor.panel_path, anchor.block_id, anchor.label
                    ));
                }
            }
            if let Some(hint) = entry.agent_hint.as_deref() {
                md.push_str(&format!("\n#### Agent 提示\n\n{hint}\n"));
            }
            md.push('\n');
        }
    }
}

pub(super) fn append_runtime_snapshot(
    md: &mut String,
    compiled: &mei_lang_kernel::CompiledApp,
    ctx: &mei_lang_kernel::BuildNodeContext,
    intent: &str,
) {
    if intent != "debug_data" && intent != "full" {
        return;
    }
    md.push_str("### 运行时快照\n\n");
    md.push_str(&format!(
        "- compile_resources: {}\n",
        compiled.resources.len()
    ));
    if let Some(dataset_id) = ctx.world_dataset.as_deref() {
        if let Some(resource) = compiled.resources.iter().find(|r| r.id == dataset_id) {
            if let Some(dataset) = resource.dataset.as_ref() {
                md.push_str(&format!(
                    "- dataset `{dataset_id}` rows: {}\n",
                    dataset.rows.len()
                ));
            }
        }
    }
    let backing = build_overview_backing(compiled, &ctx.node);
    for item in backing {
        if let Some(stripped) = item.strip_prefix("→ ") {
            let dataset_id = stripped.split("::").next().unwrap_or(stripped).trim();
            if let Some(resource) = compiled.resources.iter().find(|r| r.id == dataset_id) {
                if let Some(dataset) = resource.dataset.as_ref() {
                    md.push_str(&format!(
                        "- backing `{dataset_id}` rows: {}\n",
                        dataset.rows.len()
                    ));
                }
            }
        }
    }
    md.push_str("- query_state: （Build Exec tab / preview runtime 可复现 filter）\n");
    md.push('\n');
}

pub(super) fn append_provenance_section(md: &mut String, anchor: &ProvenanceAnchor) {
    md.push_str("### 溯源\n\n");
    if !anchor.file.is_empty() {
        md.push_str(&format!("- file: `{}`\n", anchor.file));
    }
    md.push_str(&format!("- symbol_id: `{}`\n", anchor.symbol_id));
    md.push_str(&format!("- symbol_kind: `{}`\n", anchor.symbol_kind));
    md.push_str(&format!("- anchor: `{}`\n", anchor.encode()));
    md.push('\n');
}

pub(super) fn append_graph_sections(
    md: &mut String,
    compiled: &mei_lang_kernel::CompiledApp,
    ctx: &mei_lang_kernel::BuildNodeContext,
    include_graph: &str,
) {
    let want_semantic = include_graph.contains("semantic");
    let want_eval = include_graph.contains("eval");
    if !want_semantic && !want_eval {
        return;
    }
    if let Some(metric_id) = ctx.world_metric.as_deref() {
        for resource in &compiled.resources {
            let Some(dataset) = resource.dataset.as_ref() else {
                continue;
            };
            if want_semantic && !dataset.runtime_analysis_graph.nodes.is_empty() {
                md.push_str("### 语义图摘要\n\n");
                md.push_str(&format_semantic_graph_markdown(
                    &dataset.runtime_analysis_graph,
                    Some(metric_id),
                ));
                md.push('\n');
            }
            if want_eval && !dataset.runtime_metric_defs.is_empty() {
                md.push_str("### 求值图摘要\n\n");
                if let Ok(plan_md) =
                    graph_markdown::eval_plan_markdown_for_metric(compiled, dataset, metric_id)
                {
                    md.push_str(&plan_md);
                    md.push('\n');
                }
            }
        }
    }
}

pub(super) fn append_registry_graph_sections(
    md: &mut String,
    source_root: &std::path::Path,
    app_id: &str,
    include_graph: &str,
    node: &mei_lang_kernel::BuildNodeId,
) {
    let want_mcg = include_graph.contains("mcg");
    let want_mrg = include_graph.contains("mrg");
    if !want_mcg && !want_mrg {
        return;
    }
    if want_mcg {
        let registry = McgRegistryWriter::load(source_root, app_id);
        md.push_str("### MCG registry 摘要\n\n");
        md.push_str(&format!("- nodes: `{}`\n", registry.nodes.len()));
        for record in registry.nodes.iter().take(24) {
            md.push_str(&format!(
                "- `{}` state=`{:?}` revision=`{}`\n",
                record.id.stable_key(),
                record.state,
                record.revision
            ));
        }
        md.push('\n');
        if node.kind == mei_lang_kernel::BuildNodeKind::McgNode {
            md.push_str(&format!(
                "- runtime_view: `/apps/runtime/{app_id}`（MRG · Materialization）\n\n"
            ));
        }
    }
    if want_mrg {
        let registry = MrgRegistryWriter::load(source_root, app_id);
        md.push_str("### MRG registry 摘要\n\n");
        md.push_str(&format!("- slots: `{}`\n", registry.slots.len()));
        for slot in registry.slots.iter().take(24) {
            md.push_str(&format!(
                "- `{}@{}` state=`{:?}` revision=`{}`\n",
                slot.slot_id.node.key,
                slot.slot_id.scope_key,
                slot.state,
                slot.slot_revision
            ));
        }
        md.push('\n');
    }
}

pub(super) fn append_readiness_section(md: &mut String, app_id: &str, host_phase: &str) {
    md.push_str("### Readiness\n\n");
    md.push_str(&format!("- app_id: `{app_id}`\n"));
    md.push_str(&format!("- host_phase: `{host_phase}`\n"));
    md.push('\n');
}

pub(super) fn append_suggested_tasks(
    md: &mut String,
    intent: &str,
    anchor: &ProvenanceAnchor,
    gate_error: &Option<String>,
) {
    md.push_str("### 建议 Agent 任务\n\n");
    match intent {
        "debug_render" => {
            md.push_str(&format!(
                "> 检查 `{}` 中符号 `{}` 的 projection / preview 渲染是否与 compile 结构一致。\n",
                anchor.file, anchor.symbol_id
            ));
        }
        "debug_data" => {
            md.push_str(&format!(
                "> 检查 `{}` 中 `{}` 的数据绑定、filter 与 runtime 物化结果；对照 Backing 与运行时快照中的 row_count。\n",
                anchor.file, anchor.symbol_id
            ));
        }
        "debug_eval" => {
            md.push_str(&format!(
                "> 检查 metric `{}` 的 eval plan / hydrate 链；对比 world 定义与 AOT artifact。\n",
                anchor.symbol_id
            ));
        }
        "debug_artifact" => {
            md.push_str("> 检查 prebuild 产物是否齐全且 revision 匹配；必要时运行 `mei-toolchain prebuild --verify`。\n");
            if let Some(err) = gate_error {
                md.push_str(&format!("> Gate 错误: `{err}`\n"));
            }
        }
        "full" => {
            md.push_str("> 掌握当前 node 的编译/runtime 真值；如需改源码，在 IDE 打开溯源 file + symbol_id。\n");
        }
        _ => {
            md.push_str(&format!(
                "> 在 IDE 打开 `{}`，定位符号 `{}`（{}）。\n",
                anchor.file, anchor.symbol_id, anchor.symbol_kind
            ));
        }
    }
}

