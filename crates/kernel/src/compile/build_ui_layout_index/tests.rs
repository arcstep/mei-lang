use crate::{
    build_ui_layout_index, BlockDecl, BuildNodeId, BuildNodeKind, CompiledApp, CompiledSceneRoute,
    LayoutDecl, PanelDecl, SceneContract, SceneDecl, UiNodeDecl, UiScopeRole,
};
use serde_json::json;

fn sample_left_rail_panel() -> PanelDecl {
    PanelDecl {
        kind: "panel".to_string(),
        id: "left_rail".to_string(),
        title: None,
        head: None,
        area: Some("body".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: None,
            rows: Some(vec![
                "185px".to_string(),
                "407px".to_string(),
                "377px".to_string(),
            ]),
            areas: Some(vec![
                vec!["enforcement".to_string()],
                vec!["inspection".to_string()],
                vec!["penalty".to_string()],
            ]),
            gap: Some("6px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![
            UiNodeDecl::Panel(PanelDecl {
                kind: "panel".to_string(),
                id: "enforcement".to_string(),
                title: Some("执法要素".to_string()),
                head: None,
                area: Some("enforcement".to_string()),
                layout: None,
                blocks: vec![UiNodeDecl::Panel(compound_micro_panel())],
                slot: None,
                props: json!({}),
                head_props: json!({}),
                body_props: json!({}),
                base: None,
                import_scope: None,
            }),
        ],
        slot: None,
        props: json!({"__mei_tier": "t1", "__mei_chrome_role": "rail"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn compound_micro_panel() -> PanelDecl {
    PanelDecl {
        kind: "panel".to_string(),
        id: "enforcement_strip_layout".to_string(),
        title: None,
        head: None,
        area: Some("auto".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec![
                "88px".to_string(),
                "88px".to_string(),
                "92px".to_string(),
                "220px".to_string(),
            ]),
            rows: Some(vec!["1fr".to_string()]),
            areas: Some(vec![vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
                "compound".to_string(),
            ]]),
            gap: Some("6px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![
            UiNodeDecl::Block(metric_block("first", "执法单位")),
            UiNodeDecl::Block(metric_block("second", "执法人员")),
            UiNodeDecl::Block(metric_block("third", "执法事项")),
            UiNodeDecl::Panel(PanelDecl {
                kind: "panel".to_string(),
                id: "enforcement_objects_card".to_string(),
                title: None,
                head: None,
                area: Some("compound".to_string()),
                layout: None,
                blocks: vec![UiNodeDecl::Block(metric_block("auto", "执法对象"))],
                slot: None,
                props: json!({}),
                head_props: json!({}),
                body_props: json!({}),
                base: None,
                import_scope: None,
            }),
        ],
        slot: None,
        props: json!({
            "__mei_macro": "metric_triptych_compound_body",
            "compound_width": "220px"
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn metric_block(area: &str, label: &str) -> BlockDecl {
    BlockDecl {
        kind: "block".to_string(),
        use_key: "metric-card".to_string(),
        id: Some(format!("{label}_card")),
        title: None,
        area: Some(area.to_string()),
        props: json!({"source": {"label": label, "value": "1", "unit": "个"}}),
        base: None,
        layout: None,
        blocks: vec![],
        component: None,
        placement: None,
        interactions: vec![],
        lifecycle: None,
        constraints: None,
        data: None,
    }
}

#[test]
fn ui_layout_index_builds_section_micro_and_slots() {
    let compiled = CompiledApp {
        app_id: "pretty-panels".to_string(),
        title: "Pretty Panels".to_string(),
        app_root: "/tmp/pretty-panels".to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "src/scene/home/assembly.mei".to_string(),
            kind: "scene".to_string(),
            title: Some("首页".to_string()),
            is_default: true,
            access_export: true,
        }],
        active_scene: Some("home".to_string()),
        active_target_file: "src/scene/home/assembly.mei".to_string(),
        file_tree: vec![],
        scene_contract: Some(SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                world: None,
                flow: None,
                frame: None,
                profile: None,
                theme: None,
                summary: None,
                goal: None,
                state: json!({}),
                shared: json!({}),
                local_nav: json!({}),
                params: json!({}),
                capabilities: json!({}),
                bindings: json!({}),
                examples: json!({}),
                access_export: true,
            },
            themes: vec![],
            shared: json!({}),
            world: None,
            flow: None,
            frame: None,
            panels: vec![sample_left_rail_panel()],
        }),
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: vec![],
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: vec![],
        diagnostics: vec![],
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };

    let result = build_ui_layout_index(&compiled);
    let index = result.index;
    assert!(!index.nodes.is_empty());

    let section_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/enforcement").encode();
    let section = index.lookup_by_encoded(&section_id).expect("section node");
    assert_eq!(section.role, UiScopeRole::Section);
    assert_eq!(section.label, "执法要素");

    let micro_id = BuildNodeId::ui_scope(
        "home",
        "home/T1/left_rail/enforcement/metric_triptych_compound_body",
    )
    .encode();
    let micro = index.lookup_by_encoded(&micro_id).expect("micro node");
    assert_eq!(micro.role, UiScopeRole::MicroLayout);

    let compound_slot_id = BuildNodeId::ui_scope(
        "home",
        "home/T1/left_rail/enforcement/metric_triptych_compound_body/compound",
    )
    .encode();
    let compound = index
        .lookup_by_encoded(&compound_slot_id)
        .expect("compound slot");
    assert_eq!(compound.role, UiScopeRole::Slot);

    let tree = result.tree_root;
    assert_eq!(tree.group, "ui_structure");
    assert_eq!(tree.children.len(), 1);
    let home_tree = &tree.children[0];
    assert_eq!(home_tree.label, "首页");

    let section_tree_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/enforcement").encode();
    let section_tree = find_tree_node(&tree.children, &section_tree_id).expect("section in tree");
    assert!(
        !section_tree.children.is_empty(),
        "section should list content children"
    );
    for child in &section_tree.children {
        assert_eq!(
            child.badges.first().map(String::as_str),
            Some("content"),
            "section tree children should be content nodes"
        );
    }
    assert!(
        !tree_contains_role(&tree.children, "micro_layout"),
        "micro_layout should not appear in structure tree"
    );
    assert!(
        !tree_contains_role(&tree.children, "slot"),
        "slot should not appear in structure tree"
    );
}

fn metric_card_panel(area: &str, id: &str, label: &str, source_file: &str) -> PanelDecl {
    PanelDecl {
        kind: "panel".to_string(),
        id: id.to_string(),
        title: None,
        head: None,
        area: Some(area.to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Block(BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: Some(format!("{id}_label")),
            title: None,
            area: Some("label".to_string()),
            props: json!({"content": {"label": label}}),
            base: None,
            layout: None,
            blocks: vec![],
            component: None,
            placement: None,
            interactions: vec![],
            lifecycle: None,
            constraints: None,
            data: None,
        })],
        slot: None,
        props: json!({
            "__mei_metric_card": true,
            "source": {"label": label, "value": "1", "unit": "个"}
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: Some(source_file.to_string()),
    }
}

fn compound_micro_panel_with_metric_cards() -> PanelDecl {
    PanelDecl {
        kind: "panel".to_string(),
        id: "enforcement_strip_layout".to_string(),
        title: None,
        head: None,
        area: Some("auto".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec![
                "88px".to_string(),
                "88px".to_string(),
                "92px".to_string(),
                "220px".to_string(),
            ]),
            rows: Some(vec!["1fr".to_string()]),
            areas: Some(vec![vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
                "compound".to_string(),
            ]]),
            gap: Some("6px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![
            UiNodeDecl::Panel(metric_card_panel(
                "first",
                "enforcement_units_card",
                "执法单位",
                "src/scene/home/content/enforcement-units.panel.mei",
            )),
            UiNodeDecl::Panel(metric_card_panel(
                "second",
                "enforcement_staff_card",
                "执法人员",
                "src/scene/home/content/enforcement-staff.panel.mei",
            )),
            UiNodeDecl::Panel(metric_card_panel(
                "third",
                "enforcement_items_card",
                "执法事项",
                "src/scene/home/content/enforcement-items.panel.mei",
            )),
            UiNodeDecl::Panel(metric_card_panel(
                "compound",
                "enforcement_objects_card",
                "执法对象",
                "src/scene/home/content/enforcement-objects.panel.mei",
            )),
        ],
        slot: None,
        props: json!({
            "__mei_macro": "metric_triptych_compound_body",
            "compound_width": "220px"
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn sample_section_with_panel_ref_wrapper() -> PanelDecl {
    PanelDecl {
        kind: "panel".to_string(),
        id: "left_rail".to_string(),
        title: None,
        head: None,
        area: Some("body".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: None,
            rows: Some(vec!["185px".to_string()]),
            areas: Some(vec![vec!["enforcement".to_string()]]),
            gap: Some("6px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![UiNodeDecl::Panel(PanelDecl {
            kind: "panel".to_string(),
            id: "enforcement".to_string(),
            title: Some("执法要素".to_string()),
            head: None,
            area: Some("enforcement".to_string()),
            layout: None,
            blocks: vec![UiNodeDecl::Panel(PanelDecl {
                kind: "panel".to_string(),
                id: "enforcement-stats".to_string(),
                title: None,
                head: None,
                area: None,
                layout: None,
                blocks: vec![UiNodeDecl::Panel(compound_micro_panel_with_metric_cards())],
                slot: None,
                props: json!({}),
                head_props: json!({}),
                body_props: json!({}),
                base: None,
                import_scope: Some(
                    "src/scene/home/content/enforcement-stats.panel.mei".to_string(),
                ),
            })],
            slot: None,
            props: json!({}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        })],
        slot: None,
        props: json!({"__mei_tier": "t1", "__mei_chrome_role": "rail"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

#[test]
fn ui_layout_index_penetrates_panel_ref_wrapper_for_metric_card_content() {
    let compiled = CompiledApp {
        app_id: "pretty-panels".to_string(),
        title: "Pretty Panels".to_string(),
        app_root: "/tmp/pretty-panels".to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "src/scene/home/assembly.mei".to_string(),
            kind: "scene".to_string(),
            title: Some("首页".to_string()),
            is_default: true,
            access_export: true,
        }],
        active_scene: Some("home".to_string()),
        active_target_file: "src/scene/home/assembly.mei".to_string(),
        file_tree: vec![],
        scene_contract: Some(SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                world: None,
                flow: None,
                frame: None,
                profile: None,
                theme: None,
                summary: None,
                goal: None,
                state: json!({}),
                shared: json!({}),
                local_nav: json!({}),
                params: json!({}),
                capabilities: json!({}),
                bindings: json!({}),
                examples: json!({}),
                access_export: true,
            },
            themes: vec![],
            shared: json!({}),
            world: None,
            flow: None,
            frame: None,
            panels: vec![sample_section_with_panel_ref_wrapper()],
        }),
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: vec![],
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: vec![],
        diagnostics: vec![],
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };

    let result = build_ui_layout_index(&compiled);
    let section_tree_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/enforcement").encode();
    let section_tree = find_tree_node(&result.tree_root.children, &section_tree_id)
        .expect("section in structure tree");
    assert_eq!(section_tree.children.len(), 4, "section should expose metric cards as content");
    for child in &section_tree.children {
        assert_eq!(
            child.badges.first().map(String::as_str),
            Some("content"),
            "wrapper penetration should surface content nodes"
        );
        assert!(
            !child.source_file.is_empty(),
            "metric card content should carry source file anchor"
        );
    }
}

fn find_tree_node<'a>(
    nodes: &'a [crate::compile::reachability_tree::ReachabilityTreeNode],
    node_id: &str,
) -> Option<&'a crate::compile::reachability_tree::ReachabilityTreeNode> {
    for node in nodes {
        if node.node_id == node_id {
            return Some(node);
        }
        if let Some(found) = find_tree_node(&node.children, node_id) {
            return Some(found);
        }
    }
    None
}

fn tree_contains_role(
    nodes: &[crate::compile::reachability_tree::ReachabilityTreeNode],
    role: &str,
) -> bool {
    nodes.iter().any(|node| {
        node.badges.first().map(String::as_str) == Some(role)
            || tree_contains_role(&node.children, role)
    })
}

#[test]
fn ui_layout_index_dedupes_duplicate_scene_routes() {
    let compiled = CompiledApp {
        app_id: "pretty-panels".to_string(),
        title: "Pretty Panels".to_string(),
        app_root: "/tmp/pretty-panels".to_string(),
        scene_routes: vec![
            CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "src/scene/home/assembly.mei".to_string(),
                kind: "scene".to_string(),
                title: Some("首页 A".to_string()),
                is_default: true,
                access_export: true,
            },
            CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "src/scene/home/assembly.mei".to_string(),
                kind: "scene".to_string(),
                title: Some("首页 B".to_string()),
                is_default: false,
                access_export: true,
            },
            CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "src/scene/home/assembly.mei".to_string(),
                kind: "scene".to_string(),
                title: Some("首页 C".to_string()),
                is_default: false,
                access_export: true,
            },
        ],
        active_scene: Some("home".to_string()),
        active_target_file: "src/scene/home/assembly.mei".to_string(),
        file_tree: vec![],
        scene_contract: Some(SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                world: None,
                flow: None,
                frame: None,
                profile: None,
                theme: None,
                summary: None,
                goal: None,
                state: json!({}),
                shared: json!({}),
                local_nav: json!({}),
                params: json!({}),
                capabilities: json!({}),
                bindings: json!({}),
                examples: json!({}),
                access_export: true,
            },
            themes: vec![],
            shared: json!({}),
            world: None,
            flow: None,
            frame: None,
            panels: vec![sample_left_rail_panel()],
        }),
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: vec![],
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: vec![],
        diagnostics: vec![],
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };

    let result = build_ui_layout_index(&compiled);
    assert_eq!(result.tree_root.children.len(), 1);
}

#[test]
fn resolve_build_preview_scope_for_ssr_skips_ui_scope() {
    use crate::resolve_build_preview_scope_for_ssr;

    let node = BuildNodeId::ui_scope("home", "home/T1/left_rail/enforcement");
    let compiled = CompiledApp {
        app_id: "pretty-panels".to_string(),
        title: "Pretty Panels".to_string(),
        app_root: "/tmp".to_string(),
        scene_routes: vec![],
        active_scene: Some("home".to_string()),
        active_target_file: String::new(),
        file_tree: vec![],
        scene_contract: None,
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: vec![],
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: vec![],
        diagnostics: vec![],
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    assert!(resolve_build_preview_scope_for_ssr(&compiled, &node).is_none());
}

#[test]
fn resolve_build_preview_scope_for_ssr_skips_scene_panel_for_full_scene() {
    use crate::resolve_build_preview_scope_for_ssr;

    let node = BuildNodeId::scene_panel("mini-park", "mini-park/T1/main");
    let compiled = CompiledApp {
        app_id: "pretty-panels".to_string(),
        title: "Pretty Panels".to_string(),
        app_root: "/tmp".to_string(),
        scene_routes: vec![],
        active_scene: Some("mini-park".to_string()),
        active_target_file: "scenes/mini-park.mei".to_string(),
        file_tree: vec![],
        scene_contract: None,
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: vec![],
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: vec![],
        diagnostics: vec![],
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    assert!(resolve_build_preview_scope_for_ssr(&compiled, &node).is_none());
}

#[test]
fn compile_scene_from_ui_scope_node() {
    use crate::compile_scene_from_build_node;

    let node = BuildNodeId::ui_scope("home", "home/T1/left_rail");
    assert_eq!(
        compile_scene_from_build_node(&node).as_deref(),
        Some("home")
    );
}

#[test]
fn ui_scope_node_kind_parses() {
    let node = BuildNodeId::parse("ui-scope:home/home/T1/left_rail/enforcement").expect("parse");
    assert_eq!(node.kind, BuildNodeKind::UiScope);
}

#[test]
fn ui_scope_for_block_matches_instance_id_stem() {
    use crate::compile::build_ui_layout_index::ui_scope_for_block;
    use crate::model::{UiLayoutIndex, UiScopeNode, UiScopeRole};

    let content_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/stats").encode();
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        content_id.clone(),
        UiScopeNode {
            node_id: content_id,
            role: UiScopeRole::Content,
            label: "Stats".to_string(),
            scope_path: vec!["home".into(), "T1".into(), "left_rail".into(), "stats".into()],
            plane: Some("T1".to_string()),
            parent_id: None,
            children: vec![],
            preview_scope: "left_rail/body/stats".to_string(),
            budget: None,
            source_anchors: vec![],
            content_kind: Some("metric-card".to_string()),
            scene_id: Some("home".to_string()),
        },
    );
    let compiled = CompiledApp {
        app_id: "pretty-panels".to_string(),
        title: "Pretty Panels".to_string(),
        app_root: "/tmp/pretty-panels".to_string(),
        scene_routes: vec![],
        active_scene: None,
        active_target_file: String::new(),
        file_tree: vec![],
        scene_contract: None,
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: vec![],
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: vec![],
        diagnostics: vec![],
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: UiLayoutIndex {
            nodes,
            scene_roots: vec![],
        },
    };
    let hit = ui_scope_for_block(&compiled, "home", "left_rail/body", "stats~0");
    assert!(hit.is_some(), "instance block id should match walker preview_scope stem");
    assert_eq!(hit.unwrap().preview_scope, "left_rail/body/stats");
}

#[test]
fn ui_scope_annotation_for_preview_panel_matches_content_and_section_paths() {
    use crate::compile::build_ui_layout_index::ui_scope_annotation_for_preview_panel;
    use crate::model::{UiLayoutIndex, UiScopeNode, UiScopeRole};

    let mut nodes = std::collections::BTreeMap::new();
    for (scope, role, node_key) in [
        (
            "left_rail/enforcement/first/enforcement_units_card",
            UiScopeRole::Content,
            "content",
        ),
        (
            "left_rail/left_top",
            UiScopeRole::Section,
            "section",
        ),
        ("map_stage", UiScopeRole::Region, "region"),
    ] {
        let node_id = BuildNodeId::ui_scope("home", &format!("home/T1/left_rail/{node_key}")).encode();
        nodes.insert(
            node_id.clone(),
            UiScopeNode {
                node_id,
                role,
                label: node_key.to_string(),
                scope_path: vec!["home".into(), "T1".into(), "left_rail".into()],
                plane: Some("T1".to_string()),
                parent_id: None,
                children: vec![],
                preview_scope: scope.to_string(),
                budget: None,
                source_anchors: vec![],
                content_kind: None,
                scene_id: Some("home".to_string()),
            },
        );
    }
    let compiled = CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: "/tmp/demo".to_string(),
        scene_routes: vec![],
        active_scene: None,
        active_target_file: String::new(),
        file_tree: vec![],
        scene_contract: None,
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: vec![],
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: vec![],
        diagnostics: vec![],
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: UiLayoutIndex {
            nodes,
            scene_roots: vec![],
        },
    };

    let content = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "left_rail/enforcement/panel/enforcement-stats/enforcement_strip_layout/enforcement_units_card",
        None,
    )
    .expect("content path");
    assert_eq!(content.role, "content");
    assert!(content.node_id.starts_with("ui-scope:"));

    let section = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "left_rail/lake_pavilion_slot",
        Some("left_top"),
    )
    .expect("section path");
    assert_eq!(section.role, "section");
    assert_eq!(section.preview_scope, "left_rail/left_top");
}
