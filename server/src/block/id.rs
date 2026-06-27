use anyhow::{anyhow, Result};

use crate::graph::types::GraphNodeKind;

use super::types::BlockId;

/// Parse `kind:key` or `kind:key@scopeKey` (scopeKey may contain `@` in path form).
pub fn parse_block_id(raw: &str) -> Result<BlockId> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(anyhow!("block node id must not be empty"));
    }
    let Some((kind_slug, rest)) = raw.split_once(':') else {
        return Err(anyhow!(
            "invalid block id `{raw}`; expected `kind:key` or `kind:key@scopeKey`"
        ));
    };
    let kind = parse_kind_slug(kind_slug)?;
    let (key, scope_key) = if let Some(at) = rest.rfind('@') {
        let (key, scope) = rest.split_at(at);
        let scope = scope.strip_prefix('@').unwrap_or(scope);
        if key.is_empty() {
            return Err(anyhow!("block key must not be empty in `{raw}`"));
        }
        (
            key.to_string(),
            Some(scope.to_string()).filter(|s| !s.is_empty()),
        )
    } else {
        (rest.to_string(), None)
    };
    if key.is_empty() {
        return Err(anyhow!("block key must not be empty in `{raw}`"));
    }
    Ok(BlockId {
        kind,
        key,
        scope_key,
    })
}

fn parse_kind_slug(slug: &str) -> Result<GraphNodeKind> {
    match slug.trim() {
        "app_skeleton" => Ok(GraphNodeKind::AppSkeleton),
        "scene_payload" => Ok(GraphNodeKind::ScenePayload),
        "panel_contract" => Ok(GraphNodeKind::PanelContract),
        "catalog_resource" => Ok(GraphNodeKind::CatalogResource),
        "metric_def_bundle" => Ok(GraphNodeKind::MetricDefBundle),
        "semantic_graph" => Ok(GraphNodeKind::SemanticGraph),
        "assembly_view" => Ok(GraphNodeKind::AssemblyView),
        "data_source" => Ok(GraphNodeKind::DataSource),
        "eval_plan" => Ok(GraphNodeKind::EvalPlan),
        "workset" => Ok(GraphNodeKind::Workset),
        "material_slot" => Ok(GraphNodeKind::MaterialSlot),
        "navigation" => Ok(GraphNodeKind::Navigation),
        other => Err(anyhow!("unknown block kind slug `{other}`")),
    }
}

pub fn parse_material_states(raw: &str) -> Vec<&'static str> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .flat_map(|s| match s {
            "stale" => Some("stale"),
            "missing" => Some("missing"),
            "failed" => Some("failed"),
            "ready" => Some("ready"),
            "warming" => Some("warming"),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scene_payload_block_id() {
        let id = parse_block_id("scene_payload:src/scenes/home.mei").expect("parse");
        assert_eq!(id.kind, GraphNodeKind::ScenePayload);
        assert_eq!(id.key, "src/scenes/home.mei");
        assert!(id.scope_key.is_none());
    }

    #[test]
    fn parse_material_slot_with_scope() {
        let id = parse_block_id(
            "material_slot:workset|app=zhifa|owner=warning_list|metrics=[*]@home@src/scenes/home.mei",
        )
        .expect("parse");
        assert_eq!(id.kind, GraphNodeKind::MaterialSlot);
        assert_eq!(
            id.scope_key.as_deref(),
            Some("home@src/scenes/home.mei")
        );
    }
}
