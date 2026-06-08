//! Agent 请求的业务 scope 规范化：`resource_visibility`、dataset 参数覆盖校验、`read_file` 业务白名单、
//! 以及基于 world inventory 的 direct refs / scene reachable 可达集。

mod dataset_scope;
mod paths;
mod reachability;
mod read_file_scope;
mod resource_scope;
mod visibility;
mod world_inventory_scope;

pub(crate) use dataset_scope::validate_dataset_world_scope_merge;
pub(crate) use reachability::ScopeReachabilitySets;
pub(crate) use read_file_scope::{read_file_allowed_for_agent, resource_world_tools_precheck};
pub(crate) use resource_scope::{
    agent_resource_scope_from_request, agent_resource_scope_from_request_with_snapshot,
};
pub(crate) use visibility::resolve_resource_visibility;
pub(crate) use world_inventory_scope::{
    allowed_world_injection_inventory_ids, resource_inventory_reach_tier,
    world_injection_inventory_item_allowed,
};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use super::visibility::default_resource_visibility;
    use super::*;
    use crate::agent_runtime::bridge::BridgePromptRequest;
    use crate::mei_agent::mode_policy::{AgentMode, AgentModePolicy, RouteMode};
    use crate::mei_agent::resource_tools::{AgentResourceScope, ResourceVisibility};

    fn policy_access_ask() -> AgentModePolicy {
        AgentModePolicy {
            mode: AgentMode::Ask,
            route_mode: RouteMode::Access,
        }
    }

    fn policy_manage_build() -> AgentModePolicy {
        AgentModePolicy {
            mode: AgentMode::Build,
            route_mode: RouteMode::Manage,
        }
    }

    #[test]
    fn default_visibility_follows_route_and_mode() {
        assert_eq!(
            default_resource_visibility(policy_access_ask()),
            ResourceVisibility::AllowSceneReachable
        );
        assert_eq!(
            default_resource_visibility(policy_manage_build()),
            ResourceVisibility::AllowDirectRefs
        );
    }

    #[test]
    fn resolve_visibility_from_request_field() {
        let mut req = BridgePromptRequest {
            text: String::new(),
            app_id: Some("demo".into()),
            scene_id: None,
            target_file: None,
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("access".into()),
            agent: None,
            model: None,
            resource_visibility: Some("local_only".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let vis = resolve_resource_visibility(&req, policy_access_ask());
        assert_eq!(vis, ResourceVisibility::LocalOnly);

        req.resource_visibility = Some("ALLOW_SCENE_REACHABLE".into());
        let vis2 = resolve_resource_visibility(&req, policy_access_ask());
        assert_eq!(vis2, ResourceVisibility::AllowSceneReachable);
    }

    #[test]
    fn read_file_local_only_matches_prefixed_target() {
        let scope = AgentResourceScope {
            scene_id: None,
            target_file: Some("main.mei".into()),
            resource_visibility: ResourceVisibility::LocalOnly,
            browser_query_state: None,
            browser_filter_intents: Vec::new(),
            direct_ref_paths: Arc::new(HashSet::new()),
            scene_reachable_paths: Arc::new(HashSet::new()),
            world_injection_allowed_ids: None,
        };
        assert!(read_file_allowed_for_agent(
            "demo/main.mei",
            Some("demo"),
            &scope
        ));
        assert!(!read_file_allowed_for_agent(
            "demo/other.mei",
            Some("demo"),
            &scope
        ));
    }

    #[test]
    fn read_file_allow_direct_requires_membership() {
        let mut direct = HashSet::new();
        direct.insert("demo/data/x.mei".to_string());
        direct.insert("demo/main.mei".to_string());
        let scope = AgentResourceScope {
            scene_id: Some("s1".into()),
            target_file: Some("demo/main.mei".into()),
            resource_visibility: ResourceVisibility::AllowDirectRefs,
            browser_query_state: None,
            browser_filter_intents: Vec::new(),
            direct_ref_paths: Arc::new(direct),
            scene_reachable_paths: Arc::new(HashSet::new()),
            world_injection_allowed_ids: None,
        };
        assert!(read_file_allowed_for_agent(
            "demo/data/x.mei",
            Some("demo"),
            &scope
        ));
        assert!(!read_file_allowed_for_agent(
            "demo/unlisted.mei",
            Some("demo"),
            &scope
        ));
        assert!(!read_file_allowed_for_agent(
            "otherapp/x.mei",
            Some("demo"),
            &scope
        ));
    }

    #[test]
    fn read_file_scene_reachable_uses_scene_set() {
        let mut scene = HashSet::new();
        scene.insert("demo/panels/a.mei".to_string());
        let scope = AgentResourceScope {
            scene_id: Some("s1".into()),
            target_file: Some("demo/main.mei".into()),
            resource_visibility: ResourceVisibility::AllowSceneReachable,
            browser_query_state: None,
            browser_filter_intents: Vec::new(),
            direct_ref_paths: Arc::new(HashSet::new()),
            scene_reachable_paths: Arc::new(scene),
            world_injection_allowed_ids: None,
        };
        assert!(read_file_allowed_for_agent(
            "demo/panels/a.mei",
            Some("demo"),
            &scope
        ));
        assert!(!read_file_allowed_for_agent(
            "demo/main.mei",
            Some("demo"),
            &scope
        ));
    }

    #[test]
    fn dataset_merge_local_only_must_match() {
        use crate::http::scene_api::WorldScope;
        let base = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("a.mei".into()),
        };
        let ok = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("a.mei".into()),
        };
        assert!(validate_dataset_world_scope_merge(
            &base,
            &ok,
            ResourceVisibility::LocalOnly,
            None,
            Some("demo")
        )
        .is_ok());

        let bad = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("b.mei".into()),
        };
        assert!(validate_dataset_world_scope_merge(
            &base,
            &bad,
            ResourceVisibility::LocalOnly,
            None,
            Some("demo")
        )
        .is_err());
    }

    #[test]
    fn dataset_merge_allow_refs_requires_reachability() {
        use crate::http::scene_api::WorldScope;
        let base = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("demo/a.mei".into()),
        };
        let merged = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("demo/b.mei".into()),
        };
        let mut direct = HashSet::new();
        direct.insert("demo/b.mei".to_string());
        let scope = AgentResourceScope {
            scene_id: base.scene_id.clone(),
            target_file: base.target_file.clone(),
            resource_visibility: ResourceVisibility::AllowDirectRefs,
            browser_query_state: None,
            browser_filter_intents: Vec::new(),
            direct_ref_paths: Arc::new(direct),
            scene_reachable_paths: Arc::new(HashSet::new()),
            world_injection_allowed_ids: None,
        };
        assert!(validate_dataset_world_scope_merge(
            &base,
            &merged,
            ResourceVisibility::AllowDirectRefs,
            Some(&scope),
            Some("demo")
        )
        .is_ok());

        let bad_scope = AgentResourceScope {
            scene_id: base.scene_id.clone(),
            target_file: base.target_file.clone(),
            resource_visibility: ResourceVisibility::AllowDirectRefs,
            browser_query_state: None,
            browser_filter_intents: Vec::new(),
            direct_ref_paths: Arc::new(HashSet::new()),
            scene_reachable_paths: Arc::new(HashSet::new()),
            world_injection_allowed_ids: None,
        };
        assert!(validate_dataset_world_scope_merge(
            &base,
            &merged,
            ResourceVisibility::AllowDirectRefs,
            Some(&bad_scope),
            Some("demo")
        )
        .is_err());

        let bad_scene = WorldScope {
            scene_id: Some("s2".into()),
            target_file: Some("demo/b.mei".into()),
        };
        assert!(validate_dataset_world_scope_merge(
            &base,
            &bad_scene,
            ResourceVisibility::AllowDirectRefs,
            Some(&scope),
            Some("demo")
        )
        .is_err());
    }

    #[test]
    fn resource_inventory_reach_tier_matches_path_sets() {
        use crate::http::scene_api::ResourceInventoryItem;
        let mut direct = HashSet::new();
        direct.insert("demo/main.mei".to_string());
        let mut scene = HashSet::new();
        scene.insert("demo/other.mei".to_string());
        let rs = AgentResourceScope {
            scene_id: None,
            target_file: None,
            resource_visibility: ResourceVisibility::AllowSceneReachable,
            browser_query_state: None,
            browser_filter_intents: Vec::new(),
            direct_ref_paths: Arc::new(direct),
            scene_reachable_paths: Arc::new(scene),
            world_injection_allowed_ids: None,
        };
        let item_direct = ResourceInventoryItem {
            id: "r1".into(),
            resource_type: "resource".into(),
            title: None,
            summary: None,
            source_path: Some("main.mei".into()),
            references: vec![],
            related_to_target: false,
        };
        assert_eq!(
            resource_inventory_reach_tier(&item_direct, &rs, "demo"),
            "direct"
        );
        let item_scene = ResourceInventoryItem {
            id: "r2".into(),
            resource_type: "resource".into(),
            title: None,
            summary: None,
            source_path: Some("other.mei".into()),
            references: vec![],
            related_to_target: false,
        };
        assert_eq!(
            resource_inventory_reach_tier(&item_scene, &rs, "demo"),
            "scene"
        );
        let item_other = ResourceInventoryItem {
            id: "r3".into(),
            resource_type: "resource".into(),
            title: None,
            summary: None,
            source_path: Some("nowhere.mei".into()),
            references: vec![],
            related_to_target: false,
        };
        assert_eq!(
            resource_inventory_reach_tier(&item_other, &rs, "demo"),
            "other"
        );
    }
}
