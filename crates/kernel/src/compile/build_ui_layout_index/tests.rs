use crate::{
    build_ui_layout_index, BlockDecl, BuildNodeId, BuildNodeKind, CompiledApp, CompiledSceneRoute,
    LayoutDecl, SceneContract, SceneDecl, UiNodeDecl, UiScopeRole, UiTreeNode,
};
use serde_json::json;

fn sample_left_rail_panel() -> UiNodeDecl {
    UiNodeDecl {
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
        blocks: vec![UiTreeNode::Panel(UiNodeDecl {
            kind: "panel".to_string(),
            id: "enforcement".to_string(),
            title: Some("执法要素".to_string()),
            head: None,
            area: Some("enforcement".to_string()),
            layout: None,
            blocks: vec![UiTreeNode::Panel(compound_micro_panel())],
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

fn compound_micro_panel() -> UiNodeDecl {
    UiNodeDecl {
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
            UiTreeNode::Block(metric_block("first", "执法单位")),
            UiTreeNode::Block(metric_block("second", "执法人员")),
            UiTreeNode::Block(metric_block("third", "执法事项")),
            UiTreeNode::Panel(UiNodeDecl {
                kind: "panel".to_string(),
                id: "enforcement_objects_card".to_string(),
                title: None,
                head: None,
                area: Some("compound".to_string()),
                layout: None,
                blocks: vec![UiTreeNode::Block(metric_block("auto", "执法对象"))],
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
fn ui_layout_index_builds_section_slotted_layout_and_slots() {
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
        build_t2_page_index: Default::default(),
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

    let layout_id = BuildNodeId::ui_scope(
        "home",
        "home/T1/left_rail/enforcement/metric_triptych_compound_body",
    )
    .encode();
    let layout = index
        .lookup_by_encoded(&layout_id)
        .expect("slotted layout node");
    assert_eq!(layout.role, UiScopeRole::Slot);

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
        "section should list slot or content children"
    );
    let slot_children = collect_tree_nodes_with_role(&section_tree.children, "slot");
    assert!(
        !slot_children.is_empty(),
        "section tree should expose slot nodes in the new structure chain"
    );
    assert!(
        !tree_contains_role(&tree.children, "micro_layout"),
        "micro_layout should not appear in structure tree"
    );
    assert!(
        tree_contains_role(&tree.children, "slot"),
        "slot should appear in structure tree"
    );
}

#[test]
fn ui_layout_index_exports_compound_metric_cards_inside_slot_shell() {
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
            panels: vec![UiNodeDecl {
                kind: "panel".to_string(),
                id: "left_rail".to_string(),
                title: None,
                head: None,
                area: Some("body".to_string()),
                layout: Some(LayoutDecl {
                    layout_type: "grid".to_string(),
                    direction: None,
                    columns: None,
                    rows: Some(vec!["1fr".to_string()]),
                    areas: Some(vec![vec!["enforcement".to_string()]]),
                    gap: Some("6px".to_string()),
                    padding: None,
                    align: None,
                    justify: None,
                }),
                blocks: vec![UiTreeNode::Panel(UiNodeDecl {
                    kind: "panel".to_string(),
                    id: "enforcement".to_string(),
                    title: Some("执法要素".to_string()),
                    head: None,
                    area: Some("enforcement".to_string()),
                    layout: None,
                    blocks: vec![UiTreeNode::Panel(UiNodeDecl {
                        kind: "panel".to_string(),
                        id: "enforcement-stats".to_string(),
                        title: None,
                        head: None,
                        area: None,
                        layout: None,
                        blocks: vec![UiTreeNode::Panel(compound_strip_with_wide_metric_shell())],
                        slot: None,
                        props: json!({}),
                        head_props: json!({}),
                        body_props: json!({}),
                        base: None,
                        import_scope: None,
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
            }],
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };

    let result = build_ui_layout_index(&compiled);
    let scopes: Vec<_> = result
        .index
        .nodes
        .values()
        .map(|node| node.preview_scope.as_str())
        .collect();
    assert!(
        scopes
            .iter()
            .any(|scope| scope.contains("/compound/enforcement_objects_card_body")),
        "compound body should exist: {scopes:?}"
    );
    assert!(
        scopes
            .iter()
            .any(|scope| scope.contains("enforcement_objects_top")),
        "compound shell should export top metric card scope, got: {scopes:?}"
    );
    assert!(
        scopes
            .iter()
            .any(|scope| scope.contains("enforcement_objects_b0")),
        "compound shell should export sub metric cards, got: {scopes:?}"
    );
    let compound_content = result
        .index
        .nodes
        .values()
        .find(|node| node.content_kind.as_deref() == Some("compound-metric"));
    assert!(
        compound_content.is_some(),
        "compound body should surface compound-metric content kind"
    );
}

fn metric_card_panel(area: &str, id: &str, label: &str, source_file: &str) -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: id.to_string(),
        title: None,
        head: None,
        area: Some(area.to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Block(BlockDecl {
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

fn compound_micro_panel_with_metric_cards() -> UiNodeDecl {
    UiNodeDecl {
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
            UiTreeNode::Panel(metric_card_panel(
                "first",
                "enforcement_units_card",
                "执法单位",
                "src/scene/home/content/enforcement-units.panel.mei",
            )),
            UiTreeNode::Panel(metric_card_panel(
                "second",
                "enforcement_staff_card",
                "执法人员",
                "src/scene/home/content/enforcement-staff.panel.mei",
            )),
            UiTreeNode::Panel(metric_card_panel(
                "third",
                "enforcement_items_card",
                "执法事项",
                "src/scene/home/content/enforcement-items.panel.mei",
            )),
            UiTreeNode::Panel(metric_card_panel(
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

fn wide_metric_compound_shell_panel() -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: "enforcement_objects_card".to_string(),
        title: None,
        head: None,
        area: Some("compound".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["1fr".to_string()]),
            rows: Some(vec!["1fr".to_string()]),
            areas: Some(vec![vec!["content".to_string()]]),
            gap: Some("0".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![UiTreeNode::Panel(wide_metric_compound_body_panel())],
        slot: None,
        props: json!({"__mei_slot_frame_bg": true}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn wide_metric_compound_body_panel() -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: "enforcement_objects_card_body".to_string(),
        title: None,
        head: None,
        area: Some("content".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec![
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string(),
            ]),
            rows: Some(vec!["44%".to_string(), "1fr".to_string()]),
            areas: Some(vec![
                vec!["top".to_string(), "top".to_string(), "top".to_string()],
                vec!["b0".to_string(), "b1".to_string(), "b2".to_string()],
            ]),
            gap: Some("2px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![
            UiTreeNode::Panel(metric_card_panel(
                "top",
                "enforcement_objects_top",
                "执法对象",
                "src/scene/home/content/enforcement-objects-top.panel.mei",
            )),
            UiTreeNode::Panel(metric_card_panel(
                "b0",
                "enforcement_objects_b0",
                "重点企业",
                "src/scene/home/content/enforcement-objects-b0.panel.mei",
            )),
            UiTreeNode::Panel(metric_card_panel(
                "b1",
                "enforcement_objects_b1",
                "园区",
                "src/scene/home/content/enforcement-objects-b1.panel.mei",
            )),
            UiTreeNode::Panel(metric_card_panel(
                "b2",
                "enforcement_objects_b2",
                "白名单",
                "src/scene/home/content/enforcement-objects-b2.panel.mei",
            )),
        ],
        slot: None,
        props: json!({"__mei_compound_top_band_ratio": "44%"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn compound_strip_with_wide_metric_shell() -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: "enforcement_strip_layout".to_string(),
        title: None,
        head: None,
        area: Some("strip".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec![
                "88px".to_string(),
                "88px".to_string(),
                "88px".to_string(),
                "220px".to_string(),
            ]),
            rows: Some(vec!["1fr".to_string()]),
            areas: Some(vec![vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
                "compound".to_string(),
            ]]),
            gap: Some("2px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![
            UiTreeNode::Panel(metric_card_panel(
                "first",
                "enforcement_units_card",
                "执法单位",
                "src/scene/home/content/enforcement-units.panel.mei",
            )),
            UiTreeNode::Panel(metric_card_panel(
                "second",
                "enforcement_personnel_card",
                "执法人员",
                "src/scene/home/content/enforcement-staff.panel.mei",
            )),
            UiTreeNode::Panel(metric_card_panel(
                "third",
                "enforcement_items_card",
                "执法事项",
                "src/scene/home/content/enforcement-items.panel.mei",
            )),
            UiTreeNode::Panel(wide_metric_compound_shell_panel()),
        ],
        slot: None,
        props: json!({"__mei_macro": "metric_triptych_compound_body"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn sample_section_with_panel_ref_wrapper() -> UiNodeDecl {
    UiNodeDecl {
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
        blocks: vec![UiTreeNode::Panel(UiNodeDecl {
            kind: "panel".to_string(),
            id: "enforcement".to_string(),
            title: Some("执法要素".to_string()),
            head: None,
            area: Some("enforcement".to_string()),
            layout: None,
            blocks: vec![UiTreeNode::Panel(UiNodeDecl {
                kind: "panel".to_string(),
                id: "enforcement-stats".to_string(),
                title: None,
                head: None,
                area: None,
                layout: None,
                blocks: vec![UiTreeNode::Panel(compound_micro_panel_with_metric_cards())],
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };

    let result = build_ui_layout_index(&compiled);
    let section_tree_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/enforcement").encode();
    let section_tree = find_tree_node(&result.tree_root.children, &section_tree_id)
        .expect("section in structure tree");
    let content_children = collect_tree_nodes_with_role(&section_tree.children, "content");
    assert_eq!(
        content_children.len(),
        4,
        "section should expose metric cards as descendant content"
    );
    for child in content_children {
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

fn collect_tree_nodes_with_role<'a>(
    nodes: &'a [crate::compile::reachability_tree::ReachabilityTreeNode],
    role: &str,
) -> Vec<&'a crate::compile::reachability_tree::ReachabilityTreeNode> {
    let mut out = Vec::new();
    collect_tree_nodes_with_role_into(nodes, role, &mut out);
    out
}

fn collect_tree_nodes_with_role_into<'a>(
    nodes: &'a [crate::compile::reachability_tree::ReachabilityTreeNode],
    role: &str,
    out: &mut Vec<&'a crate::compile::reachability_tree::ReachabilityTreeNode>,
) {
    for node in nodes {
        if node.badges.first().map(String::as_str) == Some(role) {
            out.push(node);
        }
        collect_tree_nodes_with_role_into(&node.children, role, out);
    }
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
        build_t2_page_index: Default::default(),
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
        build_t2_page_index: Default::default(),
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
        build_t2_page_index: Default::default(),
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
            scope_path: vec![
                "home".into(),
                "T1".into(),
                "left_rail".into(),
                "stats".into(),
            ],
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: UiLayoutIndex {
            nodes,
            scene_roots: vec![],
        },
    };
    let hit = ui_scope_for_block(&compiled, "home", "left_rail/body", "stats~0");
    assert!(
        hit.is_some(),
        "instance block id should match walker preview_scope stem"
    );
    assert_eq!(hit.unwrap().preview_scope, "left_rail/body/stats");
}

#[test]
fn ui_scope_annotation_for_preview_panel_matches_content_and_section_paths() {
    use crate::compile::build_ui_layout_index::ui_scope_annotation_for_preview_panel;
    use crate::model::{UiLayoutIndex, UiScopeNode, UiScopeRole};

    let mut nodes = std::collections::BTreeMap::new();
    for (scope, role, node_key) in [
        (
            "left_rail/enforcement/enforcement_strip_layout/first/enforcement_units_card",
            UiScopeRole::Content,
            "content",
        ),
        ("left_rail/left_top", UiScopeRole::Section, "section"),
        ("map_stage", UiScopeRole::Region, "region"),
    ] {
        let node_id =
            BuildNodeId::ui_scope("home", &format!("home/T1/left_rail/{node_key}")).encode();
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
        build_t2_page_index: Default::default(),
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
        Some("first"),
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

#[test]
fn ui_scope_annotation_distinguishes_metric_cards_by_panel_area() {
    use crate::compile::build_ui_layout_index::ui_scope_annotation_for_preview_panel;
    use crate::model::{UiLayoutIndex, UiScopeNode, UiScopeRole};

    let mut nodes = std::collections::BTreeMap::new();
    for (scope, slot) in [
        (
            "right_rail/warning/supervision_triptych/first/metric_card",
            "first",
        ),
        (
            "right_rail/warning/supervision_triptych/second/metric_card",
            "second",
        ),
    ] {
        let node_id = BuildNodeId::ui_scope(
            "home",
            &format!("home/T1/right_rail/warning/{slot}/metric_card"),
        )
        .encode();
        nodes.insert(
            node_id.clone(),
            UiScopeNode {
                node_id,
                role: UiScopeRole::Content,
                label: "metric_card".to_string(),
                scope_path: vec!["home".into(), "T1".into(), "right_rail".into()],
                plane: Some("T1".to_string()),
                parent_id: None,
                children: vec![],
                preview_scope: scope.to_string(),
                budget: None,
                source_anchors: vec![],
                content_kind: Some("metric-card".to_string()),
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: UiLayoutIndex {
            nodes,
            scene_roots: vec![],
        },
    };

    let second = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "right_rail/warning/panel/supervision-stats/supervision_triptych/metric_card",
        Some("second"),
    )
    .expect("second metric card");
    assert_eq!(
        second.preview_scope,
        "right_rail/warning/supervision_triptych/second/metric_card"
    );

    let first = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "right_rail/warning/panel/supervision-stats/supervision_triptych/metric_card",
        Some("first"),
    )
    .expect("first metric card");
    assert_eq!(
        first.preview_scope,
        "right_rail/warning/supervision_triptych/first/metric_card"
    );
}

#[test]
fn ui_scope_annotation_section_does_not_tag_deep_content_panels() {
    use crate::compile::build_ui_layout_index::ui_scope_annotation_for_preview_panel;
    use crate::model::{UiLayoutIndex, UiScopeNode, UiScopeRole};

    let node_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/enforcement").encode();
    let mut nodes = std::collections::BTreeMap::new();
    nodes.insert(
        node_id.clone(),
        UiScopeNode {
            node_id,
            role: UiScopeRole::Section,
            label: "enforcement".to_string(),
            scope_path: vec!["home".into(), "T1".into(), "left_rail".into()],
            plane: Some("T1".to_string()),
            parent_id: None,
            children: vec![],
            preview_scope: "left_rail/enforcement".to_string(),
            budget: None,
            source_anchors: vec![],
            content_kind: None,
            scene_id: Some("home".to_string()),
        },
    );
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: UiLayoutIndex {
            nodes,
            scene_roots: vec![],
        },
    };

    let deep = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "left_rail/enforcement/panel/enforcement-stats/enforcement_strip_layout/enforcement_units_card",
        Some("first"),
    );
    assert!(
        deep.is_none() || deep.as_ref().is_some_and(|hit| hit.role != "section"),
        "deep content panel should not inherit section annotation"
    );
}

#[test]
fn ui_scope_annotation_matches_compound_metric_cards() {
    use crate::compile::build_ui_layout_index::ui_scope_annotation_for_preview_panel;
    use crate::model::{UiLayoutIndex, UiScopeNode, UiScopeRole};

    let mut nodes = std::collections::BTreeMap::new();
    for (scope, label) in [
        (
            "left_rail/enforcement/enforcement_strip_layout",
            "enforcement_strip_layout",
        ),
        (
            "left_rail/enforcement/enforcement_strip_layout/compound/enforcement_objects_top",
            "enforcement_objects_top",
        ),
    ] {
        let role = if label == "enforcement_strip_layout" {
            UiScopeRole::Slot
        } else {
            UiScopeRole::Content
        };
        let node_id = BuildNodeId::ui_scope("home", &format!("home/T1/left_rail/{label}")).encode();
        nodes.insert(
            node_id.clone(),
            UiScopeNode {
                node_id,
                role,
                label: label.to_string(),
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: UiLayoutIndex {
            nodes,
            scene_roots: vec![],
        },
    };

    let hit = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "left_rail/enforcement/panel/enforcement-stats/enforcement_strip_layout/enforcement_objects_card/panel/enforcement_objects_top",
        None,
    )
    .expect("compound metric card");
    assert_eq!(hit.role, "content");
    assert_eq!(
        hit.preview_scope,
        "left_rail/enforcement/enforcement_strip_layout/compound/enforcement_objects_top"
    );

    let micro_root = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "left_rail/enforcement/panel/enforcement-stats/enforcement_strip_layout",
        None,
    )
    .expect("micro layout slot panel");
    assert_eq!(micro_root.role, "slot");
    assert_eq!(
        micro_root.preview_scope,
        "left_rail/enforcement/enforcement_strip_layout"
    );
}

#[test]
fn ui_scope_annotation_tags_inspection_micro_layout_slots() {
    use crate::compile::build_ui_layout_index::ui_scope_annotation_for_preview_panel;
    use crate::model::{UiLayoutIndex, UiScopeNode, UiScopeRole};

    let mut nodes = std::collections::BTreeMap::new();
    for (scope, label, role) in [
        ("left_rail/inspection", "行政检查", UiScopeRole::Section),
        (
            "left_rail/inspection/inspection_counts_layout",
            "inspection_counts_layout",
            UiScopeRole::Slot,
        ),
        (
            "left_rail/inspection/ai_compound_card",
            "ai_compound_card",
            UiScopeRole::Slot,
        ),
    ] {
        let node_id = BuildNodeId::ui_scope("home", &format!("home/T1/left_rail/{label}")).encode();
        nodes.insert(
            node_id.clone(),
            UiScopeNode {
                node_id,
                role,
                label: label.to_string(),
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: UiLayoutIndex {
            nodes,
            scene_roots: vec![],
        },
    };

    let counts = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "left_rail/inspection/panel/inspection-stats/inspection_counts_layout",
        Some("block_counts"),
    )
    .expect("inspection counts micro layout");
    assert_eq!(counts.role, "slot");
    assert_eq!(
        counts.preview_scope,
        "left_rail/inspection/inspection_counts_layout"
    );

    let compound = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "left_rail/inspection/panel/inspection-stats/ai_compound_card",
        Some("block_ai"),
    )
    .expect("ai compound micro layout");
    assert_eq!(compound.role, "slot");
    assert_eq!(
        compound.preview_scope,
        "left_rail/inspection/ai_compound_card"
    );

    let section = ui_scope_annotation_for_preview_panel(
        &compiled,
        "home",
        "left_rail/inspection/panel/inspection-stats",
        Some("body"),
    )
    .expect("inspection section panel");
    assert_eq!(section.role, "section");
    assert_eq!(section.preview_scope, "left_rail/inspection");
}

fn metric_card_panel_fixture(id: &str, area: &str, label: &str) -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: id.to_string(),
        title: None,
        head: None,
        area: Some(area.to_string()),
        layout: None,
        blocks: vec![],
        slot: None,
        props: json!({
            "__mei_metric_card": true,
            "source": {"label": label, "value": "1", "unit": "件"},
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn status_flow_panel() -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: "issue_status_flow".to_string(),
        title: None,
        head: None,
        area: Some("status_flow".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["1fr".to_string(); 3]),
            rows: Some(vec!["74px".to_string(), "74px".to_string()]),
            areas: Some(vec![
                vec![
                    "pending".to_string(),
                    "doing".to_string(),
                    "done".to_string(),
                ],
                vec![
                    "summary".to_string(),
                    "summary".to_string(),
                    "summary".to_string(),
                ],
            ]),
            gap: Some("4px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![
            UiTreeNode::Panel(metric_card_panel_fixture("metric_card", "pending", "待办")),
            UiTreeNode::Panel(metric_card_panel_fixture("metric_card", "doing", "在办")),
            UiTreeNode::Panel(metric_card_panel_fixture("metric_card", "done", "已办")),
            UiTreeNode::Panel(metric_card_panel_fixture(
                "metric_card",
                "summary",
                "查实率",
            )),
        ],
        slot: None,
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn summary_stack_panel() -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: "penalty_count_summary".to_string(),
        title: None,
        head: None,
        area: Some("metrics".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["1fr".to_string()]),
            rows: Some(vec![
                "50px".to_string(),
                "32px".to_string(),
                "32px".to_string(),
            ]),
            areas: Some(vec![
                vec!["primary".to_string()],
                vec!["secondary_a".to_string()],
                vec!["secondary_b".to_string()],
            ]),
            gap: Some("4px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![
            UiTreeNode::Panel(metric_card_panel_fixture(
                "penalty_count_summary_primary",
                "primary",
                "总数",
            )),
            UiTreeNode::Panel(metric_card_panel_fixture(
                "penalty_count_summary_secondary_a",
                "secondary_a",
                "近7日",
            )),
            UiTreeNode::Panel(metric_card_panel_fixture(
                "penalty_count_summary_secondary_b",
                "secondary_b",
                "行政复议",
            )),
        ],
        slot: None,
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn section_panel(id: &str, title: &str, body: UiNodeDecl) -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: id.to_string(),
        title: Some(title.to_string()),
        head: None,
        area: Some(id.to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Panel(body)],
        slot: None,
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn sample_rail_with_sections(sections: Vec<UiNodeDecl>) -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: "left_rail".to_string(),
        title: None,
        head: None,
        area: Some("body".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: None,
            rows: Some(vec!["1fr".to_string(); sections.len()]),
            areas: Some(
                sections
                    .iter()
                    .map(|section| vec![section.id.clone()])
                    .collect(),
            ),
            gap: Some("6px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: sections.into_iter().map(UiTreeNode::Panel).collect(),
        slot: None,
        props: json!({"__mei_tier": "t1", "__mei_chrome_role": "rail"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn compiled_with_panels(panels: Vec<UiNodeDecl>) -> CompiledApp {
    CompiledApp {
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
            panels,
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    }
}

#[test]
fn ui_layout_index_status_flow_group_exposes_four_metric_cards() {
    let compiled = compiled_with_panels(vec![sample_rail_with_sections(vec![section_panel(
        "issue",
        "问题办理",
        status_flow_panel(),
    )])]);
    let result = build_ui_layout_index(&compiled);
    assert!(
        result.duplicate_node_ids.is_empty(),
        "status-flow metric cards should have unique node ids: {:?}",
        result.duplicate_node_ids
    );
    let section_tree_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/issue").encode();
    let section_tree = find_tree_node(&result.tree_root.children, &section_tree_id)
        .expect("issue section in tree");
    assert_eq!(
        section_tree.children.len(),
        1,
        "issue section should have one group"
    );
    let group = &section_tree.children[0];
    assert_eq!(group.label, "办理状态");
    assert_eq!(
        group.children.len(),
        4,
        "status-flow group should expose four cards"
    );
    let group_scope = "t1/left_rail/issue/issue_status_flow";
    let group_node = result
        .index
        .nodes
        .values()
        .find(|node| node.preview_scope == group_scope)
        .expect("status-flow content group node");
    let budget = group_node.budget.as_ref().expect("status-flow grid budget");
    assert!(
        budget
            .grid_template_areas
            .as_deref()
            .is_some_and(|areas: &str| areas.contains("pending") && areas.contains("summary")),
        "status-flow budget should keep 2-row areas, got {:?}",
        budget.grid_template_areas
    );
    let manifest = result.index.layout_budget_manifest("test-rev");
    assert!(
        manifest.entries.contains_key(group_scope),
        "layout_budget_manifest should include content-host status-flow grid"
    );
}

#[test]
fn ui_layout_index_metric_summary_group_labels_penalty_stats() {
    let compiled = compiled_with_panels(vec![sample_rail_with_sections(vec![section_panel(
        "penalty",
        "行政处罚",
        summary_stack_panel(),
    )])]);
    let result = build_ui_layout_index(&compiled);
    assert!(result.duplicate_node_ids.is_empty());
    let section_tree_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/penalty").encode();
    let section_tree = find_tree_node(&result.tree_root.children, &section_tree_id)
        .expect("penalty section in tree");
    assert_eq!(section_tree.children.len(), 1);
    assert_eq!(section_tree.children[0].label, "处罚统计");
    assert_eq!(section_tree.children[0].children.len(), 3);
}

#[test]
fn ui_layout_index_contract_level_chart_blocks_surface_in_section() {
    let chart_block = BlockDecl {
        kind: "block".to_string(),
        use_key: "component".to_string(),
        id: Some("party_bars".to_string()),
        title: Some("罚没居前当事人".to_string()),
        area: Some("party_bars".to_string()),
        props: json!({"title": "2025罚没居前当事人（元）"}),
        base: None,
        layout: None,
        blocks: vec![],
        component: Some(json!("chart.column")),
        placement: None,
        interactions: vec![],
        lifecycle: None,
        constraints: None,
        data: None,
    };
    let penalty_stats = UiNodeDecl {
        kind: "panel".to_string(),
        id: "penalty-stats".to_string(),
        title: None,
        head: None,
        area: Some("penalty".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["168px".to_string(), "1fr".to_string()]),
            rows: Some(vec!["144px".to_string()]),
            areas: Some(vec![vec!["metrics".to_string(), "party_bars".to_string()]]),
            gap: Some("4px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![
            UiTreeNode::Panel(summary_stack_panel()),
            UiTreeNode::Block(chart_block),
        ],
        slot: None,
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let compiled = compiled_with_panels(vec![sample_rail_with_sections(vec![UiNodeDecl {
        kind: "panel".to_string(),
        id: "penalty".to_string(),
        title: Some("行政处罚".to_string()),
        head: None,
        area: Some("penalty".to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Panel(penalty_stats)],
        slot: None,
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }])]);
    let result = build_ui_layout_index(&compiled);
    let section_tree_id = BuildNodeId::ui_scope("home", "home/T1/left_rail/penalty").encode();
    let section_tree =
        find_tree_node(&result.tree_root.children, &section_tree_id).expect("penalty section");
    let labels: Vec<_> = section_tree
        .children
        .iter()
        .map(|node| node.label.as_str())
        .collect();
    let party_bars_slot = result
        .index
        .nodes
        .values()
        .find(|node| node.preview_scope.ends_with("/party_bars"));
    assert!(
        party_bars_slot.is_some(),
        "contract-level chart grid area should surface as slot under section body layout: {labels:?}"
    );
    let chart_content = result.index.nodes.values().find(|node| {
        node.role == UiScopeRole::Content
            && (node.label.contains("罚没居前") || node.label.contains("分组柱图"))
    });
    assert!(
        chart_content.is_some(),
        "contract-level chart block should appear under section body layout: {labels:?}"
    );
}

#[test]
fn ui_layout_index_cross_section_duplicate_labels_disambiguate_in_tree() {
    let compiled = compiled_with_panels(vec![sample_rail_with_sections(vec![
        section_panel("inspection", "行政检查", summary_stack_panel()),
        section_panel("penalty", "行政处罚", summary_stack_panel()),
    ])]);
    let result = build_ui_layout_index(&compiled);
    let inspection_section = find_tree_node(
        &result.tree_root.children,
        &BuildNodeId::ui_scope("home", "home/T1/left_rail/inspection").encode(),
    )
    .expect("inspection section");
    let penalty_section = find_tree_node(
        &result.tree_root.children,
        &BuildNodeId::ui_scope("home", "home/T1/left_rail/penalty").encode(),
    )
    .expect("penalty section");
    let inspection_primary = collect_tree_nodes_with_role(&inspection_section.children, "content")
        .into_iter()
        .next()
        .expect("inspection primary content");
    let penalty_primary = collect_tree_nodes_with_role(&penalty_section.children, "content")
        .into_iter()
        .next()
        .expect("penalty primary content");
    assert!(
        inspection_primary.label.contains('·') || penalty_primary.label.contains('·'),
        "duplicate metric labels should be disambiguated with section prefix"
    );
    assert_ne!(inspection_primary.node_id, penalty_primary.node_id);
}

#[test]
fn ui_layout_index_surfaces_map_viewport_operation_chrome() {
    let map_tools_slot = UiNodeDecl {
        kind: "panel".to_string(),
        id: "map-tools-slot".to_string(),
        title: None,
        head: None,
        area: Some("tools".to_string()),
        layout: None,
        blocks: vec![],
        slot: None,
        props: json!({"__mei_chrome_role": "map_tools", "__mei_tier": "t1"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let interaction_surface = UiNodeDecl {
        kind: "panel".to_string(),
        id: "map-interaction-surface".to_string(),
        title: None,
        head: None,
        area: Some("aperture".to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Panel(map_tools_slot)],
        slot: None,
        props: json!({"__mei_chrome_role": "map_interaction_surface", "__mei_tier": "t1"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let map_viewport_content = UiNodeDecl {
        kind: "panel".to_string(),
        id: "map-viewport".to_string(),
        title: None,
        head: None,
        area: None,
        layout: None,
        blocks: vec![UiTreeNode::Panel(interaction_surface)],
        slot: None,
        props: json!({"__mei_chrome_role": "viewport", "__mei_tier": "t1"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let map_viewport_section = UiNodeDecl {
        kind: "panel".to_string(),
        id: "map_viewport".to_string(),
        title: None,
        head: None,
        area: Some("map_viewport".to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Panel(map_viewport_content)],
        slot: None,
        props: json!({"__mei_ui_role": "section", "__mei_tier": "t1"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let center_rail = UiNodeDecl {
        kind: "panel".to_string(),
        id: "center_rail".to_string(),
        title: None,
        head: None,
        area: Some("body".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: None,
            rows: Some(vec![
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string(),
            ]),
            areas: Some(vec![
                vec!["indicator_system".to_string()],
                vec!["map_viewport".to_string()],
                vec!["realtime_table".to_string()],
            ]),
            gap: Some("12px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![UiTreeNode::Panel(map_viewport_section)],
        slot: None,
        props: json!({"__mei_tier": "t1", "__mei_chrome_role": "center_panel"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
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
            panels: vec![center_rail],
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    let ui = build_ui_layout_index(&compiled);
    assert!(
        ui.index.nodes.values().any(|node| {
            node.scope_path
                .iter()
                .any(|segment| segment == "map-tools-slot")
        }),
        "ui index should expose map-tools-slot under map_viewport section"
    );
}

#[test]
fn ui_layout_index_synthesizes_default_section_for_bare_region() {
    let bare_region = UiNodeDecl {
        kind: "panel".to_string(),
        id: "stats_rail".to_string(),
        title: Some("统计栏".to_string()),
        head: None,
        area: Some("body".to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Block(metric_block("summary", "汇总"))],
        slot: None,
        props: json!({"__mei_tier": "t1"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
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
            panels: vec![bare_region],
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    let result = build_ui_layout_index(&compiled);
    let section_id = BuildNodeId::ui_scope("home", "home/T1/stats_rail/_default").encode();
    let section = result
        .index
        .lookup_by_encoded(&section_id)
        .expect("synthetic default section");
    assert_eq!(section.role, UiScopeRole::Section);
    assert_eq!(section.preview_scope, "t1/stats_rail/_default");
    let section_tree =
        find_tree_node(&result.tree_root.children, &section_id).expect("section tree");
    assert!(
        !section_tree.children.is_empty(),
        "default section should expose slot/content children"
    );
}

#[test]
fn ui_layout_index_exposes_fill_section_derived_height() {
    use crate::materialize_fill_section_derived_heights;

    let fill_body = UiNodeDecl {
        kind: "panel".to_string(),
        id: "enforcement-stats".to_string(),
        title: None,
        head: None,
        area: None,
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: None,
            rows: Some(vec!["1fr".to_string()]),
            areas: None,
            gap: None,
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![],
        slot: None,
        props: json!({"__mei_layout_fill": true, "height": "100%"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let section = UiNodeDecl {
        kind: "panel".to_string(),
        id: "enforcement".to_string(),
        title: Some("执法要素".to_string()),
        head: None,
        area: Some("enforcement".to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Panel(fill_body)],
        slot: None,
        props: json!({
            "__mei_ui_role": "section",
            "__mei_tier": "t1",
            "__mei_padding_profile": "dense_strip_100",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let region = UiNodeDecl {
        kind: "panel".to_string(),
        id: "left_rail".to_string(),
        title: None,
        head: None,
        area: Some("body".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: None,
            rows: Some(vec!["1fr".to_string()]),
            areas: Some(vec![vec!["enforcement".to_string()]]),
            gap: Some("12px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![UiTreeNode::Panel(section)],
        slot: None,
        props: json!({
            "__mei_ui_role": "region",
            "__mei_tier": "t1",
            "viewport": {"design_height": 520},
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let mut panels = vec![region];
    let mut diagnostics = Vec::new();
    materialize_fill_section_derived_heights(&mut panels, &mut diagnostics, "test.mei");
    let enforcement = panels[0]
        .blocks
        .iter()
        .filter_map(|n| match n {
            UiTreeNode::Panel(p) if p.id == "enforcement" => Some(p),
            _ => None,
        })
        .next()
        .expect("enforcement section");
    let derived = enforcement
        .props
        .get("__mei_section_derived_height_px")
        .and_then(|v| v.as_f64())
        .expect("fill section derived height px");
    assert!(
        (derived - 520.0).abs() < 2.0,
        "expected ~520px from single 1fr of 520, got {derived}"
    );

    let compiled = CompiledApp {
        app_id: "pretty-panels".to_string(),
        title: "Pretty Panels".to_string(),
        app_root: "/tmp/pretty-panels".to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "src/scene/home/assembly.mei".to_string(),
            kind: "scene".to_string(),
            title: None,
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
            panels,
        }),
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: vec![],
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: vec![],
        diagnostics,
        build_experience_index: Default::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    let ui = build_ui_layout_index(&compiled);
    let section_node = ui
        .index
        .nodes
        .values()
        .find(|n| n.preview_scope.contains("enforcement"));
    assert!(
        section_node
            .and_then(|n| n.budget.as_ref())
            .and_then(|b| b.section_derived_height_px)
            .is_some(),
        "ui index should surface section_derived_height_px for fill section"
    );
}

fn supervision_stats_triptych_panel() -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: "supervision-stats".to_string(),
        title: None,
        head: None,
        area: Some("auto".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec![
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string(),
            ]),
            rows: Some(vec!["1fr".to_string()]),
            areas: Some(vec![vec![
                "items".to_string(),
                "models".to_string(),
                "warnings".to_string(),
            ]]),
            gap: Some("2px".to_string()),
            padding: Some("0".to_string()),
            align: Some("stretch".to_string()),
            justify: Some("stretch".to_string()),
        }),
        blocks: vec![
            UiTreeNode::Panel(triptych_metric_card_panel(
                "supervision_items_card",
                "items",
            )),
            UiTreeNode::Panel(triptych_metric_card_panel(
                "supervision_models_card",
                "models",
            )),
            UiTreeNode::Panel(triptych_metric_card_panel(
                "warnings_count_card",
                "warnings",
            )),
        ],
        slot: None,
        props: json!({
            "__mei_layout_fill": true,
            "variant": "container",
            "chrome": "bare",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn triptych_metric_card_panel(id: &str, area: &str) -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: id.to_string(),
        title: None,
        head: None,
        area: Some(area.to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Block(metric_block("auto", id))],
        slot: None,
        props: json!({"__mei_metric_card": true}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

#[test]
fn content_panel_triptych_projects_grid_manifest_and_slotted_layout_slots() {
    let warning_section = UiNodeDecl {
        kind: "panel".to_string(),
        id: "warning".to_string(),
        title: Some("监督预警".to_string()),
        head: None,
        area: Some("warning".to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Panel(supervision_stats_triptych_panel())],
        slot: None,
        props: json!({
            "__mei_padding_profile": "compact",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let region = UiNodeDecl {
        kind: "panel".to_string(),
        id: "right_rail".to_string(),
        title: None,
        head: None,
        area: Some("right_rail".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: None,
            rows: Some(vec![
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string(),
            ]),
            areas: Some(vec![
                vec!["warning".to_string()],
                vec!["_".to_string()],
                vec!["_".to_string()],
                vec!["_".to_string()],
            ]),
            gap: Some("12px".to_string()),
            padding: None,
            align: None,
            justify: None,
        }),
        blocks: vec![UiTreeNode::Panel(warning_section)],
        slot: None,
        props: json!({"__mei_chrome_role": "rail", "__mei_tier": "t1"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let compiled = CompiledApp {
        app_id: "mini-data".to_string(),
        title: "mini-data".to_string(),
        app_root: "/tmp/mini-data".to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "src/scene/home/assembly.mei".to_string(),
            kind: "scene".to_string(),
            title: None,
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
            panels: vec![region],
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
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };

    let ui = build_ui_layout_index(&compiled);
    let manifest = ui.index.layout_budget_manifest("test-rev");
    let region_entry = manifest
        .entries
        .get("t1/right_rail")
        .expect("right_rail region manifest");
    assert_eq!(
        region_entry.grid_template_rows.as_deref(),
        Some("1fr 1fr 1fr 1fr")
    );
    assert_eq!(
        region_entry.grid_template_areas.as_deref(),
        Some("'warning' '.' '.' '.'")
    );
    assert_eq!(
        region_entry.slot_areas.as_deref(),
        Some(&["warning".to_string()][..])
    );

    let section_entry = manifest
        .entries
        .get("t1/right_rail/warning")
        .expect("warning section manifest");
    assert_eq!(
        section_entry.grid_template_rows.as_deref(),
        Some("auto minmax(0, 1fr)")
    );
    assert_eq!(section_entry.padding_profile.as_deref(), Some("compact"));

    let micro_entry = manifest
        .entries
        .get("t1/right_rail/warning/supervision-stats")
        .expect("supervision-stats slotted layout manifest");
    assert_eq!(
        micro_entry.grid_template_columns.as_deref(),
        Some("1fr 1fr 1fr")
    );
    assert_eq!(micro_entry.gap.as_deref(), Some("2px"));
    assert_eq!(
        micro_entry.slot_areas.as_deref(),
        Some(
            &[
                "items".to_string(),
                "models".to_string(),
                "warnings".to_string()
            ][..]
        )
    );

    let leaked = ui
        .index
        .nodes
        .values()
        .any(|node| node.preview_scope == "t1/right_rail/warning/label");
    assert!(
        !leaked,
        "metric role slots must not leak to section level: {:?}",
        ui.index
            .nodes
            .values()
            .map(|n| n.preview_scope.clone())
            .collect::<Vec<_>>()
    );
}
