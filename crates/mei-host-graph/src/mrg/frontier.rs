use std::collections::BTreeMap;
use std::collections::BTreeSet;

use mei_host_core::HostContext;
use mei_lang_kernel::CompiledApp;
use serde_json::Value;

use crate::assemble::t2_page_scenes_for_section_scope;
use crate::assemble_scope_from_registry;
use crate::load_block_artifact;
use crate::mcg::registry::McgRegistryWriter;
use crate::types::GraphNodeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierMetric {
    pub scope_key: String,
    pub metric_id: String,
    pub owner_resource_id: String,
    pub bundle_key: String,
}

pub fn collect_eval_frontier(
    ctx: &HostContext,
    scope_key: &str,
) -> anyhow::Result<Vec<FrontierMetric>> {
    let outcome =
        assemble_scope_from_registry(ctx.workspace_root.as_path(), ctx.app_id.as_str(), scope_key)?
            .ok_or_else(|| anyhow::anyhow!("scene `{scope_key}` not assembled"))?;
    let metrics = collect_metrics_from_compiled(scope_key, &outcome.compiled);
    if !metrics.is_empty() {
        return augment_scalar_rowset_frontier_metrics(metrics);
    }
    augment_scalar_rowset_frontier_metrics(collect_metrics_from_page_instance(ctx, scope_key)?)
}

pub fn collect_eval_frontier_with_hops(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
) -> anyhow::Result<Vec<FrontierMetric>> {
    let mut metrics = collect_eval_frontier(ctx, scope_key)?;
    if hops == 0 {
        return Ok(metrics);
    }
    let linked = linked_t2_page_scenes_for_scope(ctx, scope_key, hops)?;
    for board_scene in linked {
        let mut board_metrics = collect_eval_frontier(ctx, board_scene.as_str())?;
        metrics.append(&mut board_metrics);
    }
    dedupe_frontier(metrics)
}

fn collect_metrics_from_compiled(scope_key: &str, compiled: &CompiledApp) -> Vec<FrontierMetric> {
    let mut out = Vec::new();
    if let Some(contract) = compiled.scene_contract.as_ref() {
        for panel in &contract.panels {
            if let Ok(value) = serde_json::to_value(&panel.blocks) {
                walk_value_for_metrics(scope_key, &value, Some(compiled), &mut out);
            }
            if let Some(head) = panel.head.as_ref() {
                if let Ok(value) = serde_json::to_value(head.as_ref()) {
                    walk_value_for_metrics(scope_key, &value, Some(compiled), &mut out);
                }
            }
        }
    }
    dedupe_frontier(out).unwrap_or_default()
}

fn collect_metrics_from_page_instance(
    ctx: &HostContext,
    scope_key: &str,
) -> anyhow::Result<Vec<FrontierMetric>> {
    let registry = McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let app_root = ctx.app_root();
    for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
        let is_match = if scope_key == "home" {
            node.id.key.contains("home@")
        } else {
            node.id.key.contains(&format!("#{scope_key}"))
        };
        if !is_match {
            continue;
        }
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = load_block_artifact(app_root.as_path(), pref)? else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let mut out = Vec::new();
        walk_value_for_metrics(scope_key, &payload, None, &mut out);
        let deduped = dedupe_frontier(out)?;
        if !deduped.is_empty() {
            return Ok(deduped);
        }
    }
    Ok(Vec::new())
}

fn walk_value_for_metrics(
    scope_key: &str,
    value: &Value,
    compiled: Option<&CompiledApp>,
    out: &mut Vec<FrontierMetric>,
) {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("metric_ref") {
                if let Some(metric_id) = map
                    .get("__args")
                    .and_then(|args| args.get("arg0"))
                    .and_then(Value::as_str)
                {
                    let bundle_key = map
                        .get("__args")
                        .and_then(|args| args.get("bundle"))
                        .and_then(value_as_metric_bundle)
                        .unwrap_or("");
                    if let Some((owner, bundle)) =
                        resolve_metric_owner(compiled, metric_id, bundle_key)
                    {
                        out.push(FrontierMetric {
                            scope_key: scope_key.to_string(),
                            metric_id: metric_id.to_string(),
                            owner_resource_id: owner,
                            bundle_key: bundle,
                        });
                    }
                }
            }
            for entry in map.values() {
                walk_value_for_metrics(scope_key, entry, compiled, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_value_for_metrics(scope_key, item, compiled, out);
            }
        }
        _ => {}
    }
}

fn resolve_metric_owner(
    compiled: Option<&CompiledApp>,
    metric_id: &str,
    bundle_key: &str,
) -> Option<(String, String)> {
    if !bundle_key.is_empty() {
        return Some((
            format!("__world_metrics__::{bundle_key}"),
            bundle_key.to_string(),
        ));
    }
    let compiled = compiled?;
    for resource in &compiled.resources {
        if let Some(dataset) = resource.dataset.as_ref() {
            if dataset.runtime_metric_defs.contains_key(metric_id) {
                return Some((resource.id.clone(), resource.id.clone()));
            }
        }
    }
    None
}

fn value_as_metric_bundle(value: &Value) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .get("__args")
            .and_then(Value::as_object)
            .and_then(|args| args.get("arg0"))
            .and_then(Value::as_str)
    })
}

fn direct_linked_board_scenes(ctx: &HostContext, scope_key: &str) -> anyhow::Result<Vec<String>> {
    let registry = McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let mut scenes = Vec::new();
    for node in registry.nodes_of_kind(GraphNodeKind::Navigation) {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = load_block_artifact(ctx.app_root().as_path(), pref)? else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let from_scope = payload
            .get("from_scope")
            .or_else(|| payload.get("scope"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let is_home_popup = scope_key == "home"
            && matches!(payload.get("type").and_then(Value::as_str), Some("popup"))
            && matches!(
                payload.get("projection").and_then(Value::as_str),
                Some("overlay") | None
            );
        if !is_home_popup && from_scope != scope_key && !node.id.key.contains(scope_key) {
            continue;
        }
        collect_scene_targets(&payload, &mut scenes);
        if let Some(board) = payload
            .get("board")
            .or_else(|| payload.get("target_scene"))
            .and_then(Value::as_str)
        {
            scenes.push(board.to_string());
        } else if let Some(scene) = payload.get("scene").and_then(Value::as_str) {
            scenes.push(scene.to_string());
        }
    }
    scenes.extend(linked_board_scenes_from_scope_contract(ctx, scope_key)?);
    append_section_children_for_regions(ctx, scope_key, &mut scenes)?;
    scenes.retain(|scene| !scene.starts_with("overlay/"));
    if scenes.iter().any(|scene| scene.contains("/s-")) {
        scenes.retain(|scene| scene.contains("/s-"));
    } else {
        scenes.retain(|scene| !scene.contains("/c-"));
    }
    scenes.sort();
    scenes.dedup();
    Ok(scenes)
}

fn append_section_children_for_regions(
    ctx: &HostContext,
    scope_key: &str,
    scenes: &mut Vec<String>,
) -> anyhow::Result<()> {
    let registry = McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let app_root = ctx.app_root();
    let mut extras = Vec::new();
    let mut region_candidates = scenes.clone();
    if scope_key == "home" {
        region_candidates.push(format!("{}/home/t2/r-drilldown", ctx.app_id));
        region_candidates.push("home/t2/r-drilldown".to_string());
        for node in registry.nodes.iter() {
            let key = node.id.key.as_str();
            let is_t2_region = key.contains("/home/t2/r-")
                && !key.contains("/s-")
                && !key.contains("/c-")
                && !key.ends_with("/r-drilldown");
            if !is_t2_region {
                continue;
            }
            let scope = key
                .split_once('/')
                .map(|(_, tail)| tail.to_string())
                .unwrap_or_else(|| key.to_string());
            region_candidates.push(scope);
        }
    }
    for scene in region_candidates {
        if scene.contains("/s-") {
            continue;
        }
        let region_keys = [format!("{}/{}", ctx.app_id, scene), scene.clone()];
        for node in registry.nodes.iter() {
            if !region_keys.iter().any(|key| node.id.key == *key) {
                continue;
            }
            let Some(pref) = node.payload_ref.as_ref() else {
                continue;
            };
            let Some(artifact) = load_block_artifact(app_root.as_path(), pref)? else {
                continue;
            };
            let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
            let mut section_refs = Vec::new();
            collect_navigation_refs(&payload, &mut section_refs);
            for reference in section_refs {
                if let Some(section) = registry_scope_key_from_reference(reference.as_str()) {
                    extras.push(section);
                }
            }
        }
    }
    scenes.extend(extras);
    Ok(())
}

pub fn linked_t2_page_scenes_for_scope(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
) -> anyhow::Result<Vec<String>> {
    if hops == 0 {
        return Ok(Vec::new());
    }
    let mut seen = std::collections::BTreeSet::from([scope_key.to_string()]);
    let mut frontier = vec![scope_key.to_string()];
    let mut out = Vec::new();
    for _ in 0..hops {
        let mut next = Vec::new();
        for scope in frontier {
            for linked in direct_linked_board_scenes(ctx, scope.as_str())? {
                if seen.insert(linked.clone()) {
                    out.push(linked.clone());
                    next.push(linked);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    if scope_key == "home" && hops > 0 {
        let sections: Vec<String> = out
            .iter()
            .filter(|scene| scene.contains("/s-"))
            .cloned()
            .collect();
        if !sections.is_empty() {
            out = sections;
        } else {
            out = home_neighbor_scope_fallback(scope_key, hops);
        }
    } else if out.is_empty() {
        out = board_neighbor_scope_fallback(ctx, scope_key, hops);
    }
    Ok(out)
}

/// Linked section scopes plus their board page scenes (`page_instance.scene`).
pub fn linked_t2_page_pack_scopes(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
    max_scopes: usize,
) -> anyhow::Result<Vec<String>> {
    if hops == 0 {
        return Ok(vec![scope_key.to_string()]);
    }
    let mut seen = BTreeSet::from([scope_key.to_string()]);
    let mut out = vec![scope_key.to_string()];
    let linked_sections = linked_t2_page_scenes_for_scope(ctx, scope_key, hops)?;
    for section in linked_sections.into_iter().take(max_scopes) {
        if seen.insert(section.clone()) {
            out.push(section.clone());
        }
        for page_scene in t2_page_scenes_for_section_scope(
            ctx.workspace_root.as_path(),
            ctx.app_id.as_str(),
            section.as_str(),
        ) {
            if seen.insert(page_scene.clone()) {
                out.push(page_scene);
            }
        }
    }
    if scope_key == "home" {
        for page_scene in direct_linked_board_scenes(ctx, scope_key)? {
            let is_board_page = !page_scene.contains('/')
                && (page_scene.ends_with("_page") || page_scene.ends_with("_board"));
            if is_board_page && seen.insert(page_scene.clone()) {
                out.push(page_scene);
            }
        }
    }
    Ok(out)
}

pub fn board_neighbor_scope_fallback(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
) -> Vec<String> {
    if hops == 0 {
        return Vec::new();
    }
    if scope_key == "home" {
        return home_neighbor_scope_fallback(scope_key, hops);
    }
    if scope_key.contains("/t2/r-drilldown/s-")
        || (scope_key.contains("/t2/r-") && !scope_key.contains("/s-") && !scope_key.contains("/c-"))
    {
        let mut siblings = Vec::new();
        for section in home_neighbor_scope_fallback("home", hops) {
            if section != scope_key {
                siblings.push(section.clone());
            }
            for page in t2_page_scenes_for_section_scope(
                ctx.workspace_root.as_path(),
                ctx.app_id.as_str(),
                section.as_str(),
            ) {
                siblings.push(page);
            }
        }
        siblings.sort();
        siblings.dedup();
        return siblings;
    }
    Vec::new()
}

pub fn home_neighbor_scope_fallback(scope_key: &str, hops: usize) -> Vec<String> {
    if hops == 0 || scope_key != "home" {
        return Vec::new();
    }
    vec![
        "home/t2/r-drilldown/s-inspection-dashboard".to_string(),
        "home/t2/r-drilldown/s-supervision-warning".to_string(),
        "home/t2/r-inspection-total".to_string(),
        "home/t2/r-warnings".to_string(),
    ]
}

pub fn record_navigation_edges_for_scope(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
) -> anyhow::Result<usize> {
    if hops == 0 {
        return Ok(0);
    }
    let linked = direct_linked_board_scenes(ctx, scope_key)?;
    let mut registry = crate::mrg::registry::MrgRegistryWriter::load(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
    );
    let mut added = 0usize;
    for scene in linked {
        added +=
            crate::mrg::warmup::record_navigation_edge(&mut registry, scope_key, scene.as_str());
    }
    if added > 0 {
        registry.finalize();
        crate::mrg::registry::MrgRegistryWriter::save(ctx.workspace_root.as_path(), &registry)?;
    }
    Ok(added)
}

fn augment_scalar_rowset_frontier_metrics(
    metrics: Vec<FrontierMetric>,
) -> anyhow::Result<Vec<FrontierMetric>> {
    let mut out = metrics;
    for metric in out.clone() {
        if metric.metric_id.contains("::__scalar_rowset__") {
            continue;
        }
        let rowset_id = format!("{}::__scalar_rowset__", metric.metric_id);
        if out.iter().any(|entry| entry.metric_id == rowset_id) {
            continue;
        }
        out.push(FrontierMetric {
            scope_key: metric.scope_key.clone(),
            metric_id: rowset_id,
            owner_resource_id: metric.owner_resource_id.clone(),
            bundle_key: metric.bundle_key.clone(),
        });
    }
    dedupe_frontier(out)
}

fn dedupe_frontier(metrics: Vec<FrontierMetric>) -> anyhow::Result<Vec<FrontierMetric>> {
    let mut map = BTreeMap::new();
    for metric in metrics {
        map.insert(
            (
                metric.scope_key.clone(),
                metric.metric_id.clone(),
                metric.owner_resource_id.clone(),
            ),
            metric,
        );
    }
    Ok(map.into_values().collect())
}

fn linked_board_scenes_from_scope_contract(
    ctx: &HostContext,
    scope_key: &str,
) -> anyhow::Result<Vec<String>> {
    let outcome =
        assemble_scope_from_registry(ctx.workspace_root.as_path(), ctx.app_id.as_str(), scope_key)?
            .ok_or_else(|| anyhow::anyhow!("scene `{scope_key}` not assembled"))?;
    let registry = McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let app_root = ctx.app_root();
    let contract = outcome.compiled.scene_contract.as_ref();
    let mut refs = Vec::new();
    if let Some(contract) = contract {
        if let Ok(value) = serde_json::to_value(contract) {
            collect_navigation_refs(&value, &mut refs);
        }
    }
    let mut scenes = Vec::new();
    for ref_key in refs {
        if let Some(scene) = registry_scope_key_from_reference(ref_key.as_str())
            .or_else(|| scene_id_from_reference(ref_key.as_str()))
        {
            scenes.push(scene);
            continue;
        }
        for node in registry.nodes_of_kind(GraphNodeKind::Navigation) {
            if node.id.key != ref_key {
                continue;
            }
            let Some(pref) = node.payload_ref.as_ref() else {
                continue;
            };
            let Some(artifact) = load_block_artifact(app_root.as_path(), pref)? else {
                continue;
            };
            let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
            collect_scene_targets(&payload, &mut scenes);
        }
    }
    scenes.sort();
    scenes.dedup();
    Ok(scenes)
}

fn collect_navigation_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("__ref").and_then(Value::as_str) {
                if matches!(
                    reference,
                    "link_ref"
                        | "scene_ref"
                        | "board_ref"
                        | "assembly_ref"
                        | "section_ref"
                        | "region_ref"
                ) {
                    if let Some(arg0) = map
                        .get("__args")
                        .and_then(Value::as_object)
                        .and_then(|args| args.get("arg0"))
                        .and_then(Value::as_str)
                    {
                        out.push(arg0.to_string());
                    }
                }
            }
            for entry in map.values() {
                collect_navigation_refs(entry, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_navigation_refs(item, out);
            }
        }
        _ => {}
    }
}

fn collect_scene_targets(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(raw) => {
            if let Some(scene) = scene_id_from_reference(raw.as_str()) {
                out.push(scene);
            }
        }
        Value::Object(map) => {
            if let Some(scene) = map.get("scene").and_then(Value::as_str) {
                out.push(scene.to_string());
            }
            for entry in map.values() {
                collect_scene_targets(entry, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_scene_targets(item, out);
            }
        }
        _ => {}
    }
}

fn scene_id_from_reference(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((_, scene_id)) = trimmed.split_once('#') {
        return Some(scene_id.to_string());
    }
    let path = trimmed
        .split_once('@')
        .map(|(_, tail)| tail)
        .unwrap_or(trimmed);
    if let Some((_, tail)) = path.split_once("/scene/") {
        let scene = tail
            .split('/')
            .next()
            .unwrap_or(tail)
            .trim_end_matches(".mei");
        if !scene.is_empty() {
            return Some(scene.to_string());
        }
    }
    None
}

fn registry_scope_key_from_reference(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.contains('/') {
        return None;
    }
    let path = trimmed
        .split_once('@')
        .map(|(_, tail)| tail)
        .unwrap_or(trimmed);
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() < 2 || segments[1] != "home" {
        return None;
    }
    let scope_segments = &segments[1..];
    let mut normalized = Vec::new();
    for segment in scope_segments {
        normalized.push(*segment);
        if segment.starts_with("s-") {
            break;
        }
    }
    Some(normalized.join("/"))
}

#[cfg(test)]
mod tests {
    use super::registry_scope_key_from_reference;

    #[test]
    fn registry_scope_key_from_golden_case_region_ref() {
        assert_eq!(
            registry_scope_key_from_reference("pretty-panels/home/t2/r-warnings"),
            Some("home/t2/r-warnings".to_string())
        );
    }

    #[test]
    fn registry_scope_key_from_section_ref() {
        assert_eq!(
            registry_scope_key_from_reference(
                "data-demo/home/t2/r-drilldown/s-inspection-dashboard"
            ),
            Some("home/t2/r-drilldown/s-inspection-dashboard".to_string())
        );
    }

    #[test]
    fn registry_scope_key_from_content_ref_truncates_at_section() {
        assert_eq!(
            registry_scope_key_from_reference(
                "data-demo/home/t2/r-drilldown/s-supervision-warning/c-warnings-analytics"
            ),
            Some("home/t2/r-drilldown/s-supervision-warning".to_string())
        );
    }
}
