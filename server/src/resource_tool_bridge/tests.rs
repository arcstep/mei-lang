use super::core::SceneResourceToolExecutor;
mod resource_tool_bridge_tests {
    use super::SceneResourceToolExecutor;
    use crate::agent_runtime::bridge::BridgePromptRequest;
    use crate::http::agent_api::prompt_context::scope_bundle::AgentScopeBundle;
    use crate::mei_agent::mode_policy::AgentModePolicy;
    use crate::mei_agent::resource_tools::ResourceToolExecutor;
    use crate::test_support;

    #[test]
    fn resource_list_smoke_under_workspace_app() {
        let Some(state) = test_support::test_app_state() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_list",
            "{}",
        );
        assert!(
            out.starts_with('{') || out.starts_with("error:"),
            "unexpected output: {}",
            &out[..out.len().min(120)]
        );
    }

    #[test]
    fn resource_get_scope_denied_for_unknown_id() {
        let Some(state) = test_support::test_app_state() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_get",
            r#"{"resource_id":"__definitely_not_in_inventory__"}"#,
        );
        assert!(
            out.contains("scope_denied"),
            "expected scope_denied, got {}",
            &out[..out.len().min(200)]
        );
    }

    #[test]
    fn resource_list_denied_when_world_snapshot_missing() {
        let Some(state) = test_support::test_app_state() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/_invalid/07-app-missing-scene".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/_invalid/07-app-missing-scene"),
            &bundle.resource_scope,
            "resource_list",
            "{}",
        );
        assert!(
            out.contains("missing world snapshot"),
            "{}",
            &out[..out.len().min(200)]
        );
    }

    #[test]
    fn resource_world_tools_rejected_under_local_only() {
        let Some(state) = test_support::test_app_state() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("local_only".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_runtime_peek",
            "{}",
        );
        assert!(out.contains("local_only"), "{}", &out[..out.len().min(200)]);
    }

    #[test]
    fn resource_runtime_peek_ok_with_valid_snapshot_scope() {
        let Some(state) = test_support::test_app_state() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_runtime_peek",
            "{}",
        );
        assert!(
            out.starts_with('{'),
            "expected JSON runtime peek, got {}",
            &out[..out.len().min(160)]
        );
    }

    #[test]
    fn resource_runtime_trace_export_ok_with_valid_snapshot_scope() {
        let Some(state) = test_support::test_app_state() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_runtime_trace_export",
            "{}",
        );
        assert!(
            out.starts_with('{'),
            "expected JSON runtime trace export, got {}",
            &out[..out.len().min(160)]
        );
    }

    #[test]
    fn resource_business_summary_ok_with_bound_scope() {
        let Some(state) = test_support::test_app_state() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
        let mut request = BridgePromptRequest {
            text: String::new(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("local_only".into()),
            browser_context: None,
            host_protocol: None,
            host_contract_schema: None,
        };
        let policy = AgentModePolicy::from_request(&request);
        let _ = policy.validate();
        policy.apply_to_request(&mut request);
        let bundle = AgentScopeBundle::resolve(&state, &request).expect("bundle");
        let exec = SceneResourceToolExecutor::default();
        let out = exec.run_resource_tool(
            state.source_root.as_ref(),
            Some("examples/core/01-single-file-doc"),
            &bundle.resource_scope,
            "resource_business_summary",
            "{}",
        );
        assert!(
            out.starts_with('{'),
            "expected JSON business summary, got {}",
            &out[..out.len().min(160)]
        );
    }
}
