use mei_host_graph::{
    list_scope_routes, mrg_status_json, McgRegistryWriter, MrgRegistry, MrgRegistryWriter,
    ScopeRoute, MaterialState,
};
use mei_lang_kernel::{
    read_links_state, resolve_active_build_identity, ReachabilityTreeNode, ReachabilityTreeRoot,
};
use serde_json::{json, Value};

use crate::build_ops::build_status_aggregate;
use crate::state::ShellState;

pub fn build_runtime_snapshot(shell: &ShellState) -> Value {
    let workspace = shell.ctx.workspace_root.as_path();
    let app_id = shell.ctx.app_id.as_str();
    let ops = build_status_aggregate(shell);
    let identity = resolve_active_build_identity(workspace);
    let mcg = McgRegistryWriter::load(workspace, app_id);
    let mrg = MrgRegistryWriter::load(workspace, app_id);
    let mrg_status = mrg_status_json(workspace, app_id).unwrap_or_else(|_| json!({}));
    let scope_routes = list_scope_routes(workspace, app_id).unwrap_or_default();

    let access_ready = shell.imported;
    let warmup_ready = shell.warmed_up;
    let phase = if !access_ready {
        "starting"
    } else if warmup_ready {
        "ready"
    } else {
        "bound"
    };

    let mut ready_slots = 0usize;
    let mut stale_slots = 0usize;
    let mut failed_slots = 0usize;
    for slot in &mrg.slots {
        match slot.state {
            MaterialState::Ready => ready_slots += 1,
            MaterialState::Stale => stale_slots += 1,
            MaterialState::Failed => failed_slots += 1,
            MaterialState::Missing | MaterialState::Warming => {}
        }
    }
    let stale_ratio = if mrg.slots.is_empty() {
        0.0
    } else {
        stale_slots as f64 / mrg.slots.len() as f64
    };

    let roots = build_management_roots(app_id, &scope_routes, mrg.slots.len());
    let slot_summaries = build_slot_summaries(&mrg);

    json!({
        "appId": app_id,
        "hostShellMgmt": true,
        "roots": roots,
        "scopeRoutes": scope_routes_to_json(&scope_routes),
        "slots": slot_summaries,
        "ops": ops,
        "mrgStatus": mrg_status,
        "host": {
            "phase": phase,
            "appPhase": phase,
            "accessReady": access_ready,
            "scopeGateReady": access_ready,
            "warmupReady": warmup_ready,
        },
        "prebuild": {
            "ok": warmup_ready,
            "inSucceededApps": warmup_ready,
        },
        "diagnostics": {
            "mcg": {
                "nodeCount": mcg.nodes.len(),
                "registryRevision": mcg.registry_revision,
            },
            "mrg": {
                "slotCount": mrg.slots.len(),
                "readySlots": ready_slots,
                "staleSlots": stale_slots,
                "failedSlots": failed_slots,
                "staleRatio": stale_ratio,
                "navigationNodeCount": mrg.edges.len(),
            },
            "build": {
                "toolchainVersion": identity.toolchain_version,
                "workspaceVersion": identity.workspace_version,
                "envActive": read_links_state(workspace).ok().and_then(|l| l.build.active),
            },
            "disk": {},
            "eval": {},
            "cache": {},
        },
    })
}

pub fn management_roots_from_snapshot(snapshot: &Value) -> Vec<ReachabilityTreeRoot> {
    snapshot
        .get("roots")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

fn scope_routes_to_json(routes: &[ScopeRoute]) -> Value {
    Value::Array(
        routes
            .iter()
            .map(|route| {
                json!({
                    "sceneId": route.scene_id,
                    "url": route.url,
                    "assemblyKey": route.assembly_key,
                })
            })
            .collect(),
    )
}

fn build_slot_summaries(registry: &MrgRegistry) -> Value {
    Value::Array(
        registry
            .slots
            .iter()
            .map(|slot| {
                json!({
                    "scopeKey": slot.slot_id.scope_key,
                    "nodeKey": slot.slot_id.node.key,
                    "nodeKind": slot.slot_id.node.kind.slug(),
                    "state": serde_json::to_value(&slot.state).unwrap_or(json!("unknown")),
                    "residentTier": slot.resident_tier,
                    "clientEligible": slot.client_eligible,
                    "ownerResourceId": slot.owner_resource_id,
                    "lastEval": slot.last_eval,
                    "payloadBytes": slot.payload_bytes,
                })
            })
            .collect(),
    )
}

fn build_management_roots(
    _app_id: &str,
    scope_routes: &[ScopeRoute],
    slot_count: usize,
) -> Vec<ReachabilityTreeRoot> {
    let mut mrg_children: Vec<ReachabilityTreeNode> = scope_routes
        .iter()
        .map(|route| ReachabilityTreeNode {
            node_id: format!("mgmt-mrg-scope:{}", route.scene_id),
            id: format!("mgmt-mrg-scope:{}", route.scene_id),
            kind: "mrg_scope".to_string(),
            label: format!("scene · {}", route.scene_id),
            badges: vec!["入口".to_string()],
            compile_scene: route.scene_id.clone(),
            ..Default::default()
        })
        .collect();
    mrg_children.push(ReachabilityTreeNode {
        node_id: "mgmt-mrg-slots".to_string(),
        id: "mgmt-mrg-slots".to_string(),
        kind: "mrg_slots".to_string(),
        label: format!("全部 slots ({slot_count})"),
        badges: Vec::new(),
        ..Default::default()
    });

    vec![ReachabilityTreeRoot {
        group: "host-mgmt".to_string(),
        label: "Host 管理".to_string(),
        default_open: true,
        children: vec![
            ReachabilityTreeNode {
                node_id: "mgmt-version".to_string(),
                id: "mgmt-version".to_string(),
                kind: "host_version".to_string(),
                label: "运行版本".to_string(),
                badges: Vec::new(),
                ..Default::default()
            },
            ReachabilityTreeNode {
                node_id: "mgmt-app-state".to_string(),
                id: "mgmt-app-state".to_string(),
                kind: "host_app_state".to_string(),
                label: "应用状态".to_string(),
                badges: Vec::new(),
                ..Default::default()
            },
            ReachabilityTreeNode {
                node_id: "mgmt-mrg".to_string(),
                id: "mgmt-mrg".to_string(),
                kind: "host_mrg".to_string(),
                label: "MRG".to_string(),
                badges: vec![slot_count.to_string()],
                children: mrg_children,
                ..Default::default()
            },
        ],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_host_core::HostContext;

    #[test]
    fn management_roots_include_version_app_mrg() {
        let routes = vec![ScopeRoute {
            scene_id: "home".to_string(),
            url: format!("/apps/app/demo/scene/home"),
            assembly_key: "home@src/scene/home/assembly.mei".to_string(),
        }];
        let roots = build_management_roots("demo", &routes, 3);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].children.len(), 3);
        assert_eq!(roots[0].children[0].node_id, "mgmt-version");
        assert_eq!(roots[0].children[1].node_id, "mgmt-app-state");
        assert_eq!(roots[0].children[2].node_id, "mgmt-mrg");
    }
}
