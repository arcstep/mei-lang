#[cfg(test)]
use serde_json::json;

#[cfg(test)]
use crate::{
    model::{EntityDecl, WorldCellDecl, WorldDecl, WorldGridDecl},
    FlowDecl, RuleClickDecl, RuleEffectDecl, RuleRequireDecl, RuleStartDecl, RuleSubjectTimerDecl,
    RuleTimerDecl, SceneContract, SceneDecl,
};

#[cfg(test)]
pub(super) fn test_contract() -> SceneContract {
    SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: "room_fire_click".to_string(),
            world: None,
            flow: None,
            frame: None,
            profile: Some("simulation".to_string()),
            theme: None,
            summary: Some("test".to_string()),
            goal: Some("灭火".to_string()),
            state: json!({"phase": "ready", "countdown": 3}),
            shared: json!({}),
            local_nav: serde_json::json!({}),
            params: serde_json::json!({}),
            capabilities: serde_json::Value::Null,
            bindings: serde_json::json!({}),
            examples: serde_json::json!([]),
            access_export: true,
        },
        themes: Vec::new(),
        shared: json!({}),
        world: Some(WorldDecl {
            kind: "world".to_string(),
            id: None,
            topology: Some(WorldGridDecl {
                rows: 2,
                cols: 2,
                cells: Vec::new(),
            }),
            resources: Vec::new(),
            datasets: Vec::new(),
            metrics: Vec::new(),
            metric_packs: Vec::new(),
            entities: vec![
                EntityDecl {
                    id: "room_fire".to_string(),
                    kind: "hazard".to_string(),
                    label: Some("火点".to_string()),
                    spawns: vec!["r1c1".to_string()],
                    status: Some("small".to_string()),
                    flags: json!({}),
                    base: None,
                },
                EntityDecl {
                    id: "wall_extinguisher".to_string(),
                    kind: "tool".to_string(),
                    label: Some("灭火器".to_string()),
                    spawns: vec!["r2c2".to_string()],
                    status: None,
                    flags: json!({}),
                    base: None,
                },
            ],
            base: None,
        }),
        flow: Some(FlowDecl {
            kind: "flow".to_string(),
            id: None,
            start: Some(RuleStartDecl {
                mode: Some("manual".to_string()),
                action_label: Some("开始演练".to_string()),
            }),
            interactions: vec![
                RuleClickDecl {
                    target: "wall_extinguisher".to_string(),
                    require: None,
                    effect: RuleEffectDecl {
                        effect_type: "grant".to_string(),
                        target: None,
                        value: Some(json!("wall_extinguisher")),
                        effects: Vec::new(),
                    },
                },
                RuleClickDecl {
                    target: "room_fire".to_string(),
                    require: Some(RuleRequireDecl {
                        require_type: "has".to_string(),
                        value: "wall_extinguisher".to_string(),
                    }),
                    effect: RuleEffectDecl {
                        effect_type: "effects".to_string(),
                        target: None,
                        value: None,
                        effects: vec![
                            RuleEffectDecl {
                                effect_type: "set_status".to_string(),
                                target: Some("room_fire".to_string()),
                                value: Some(json!("out")),
                                effects: Vec::new(),
                            },
                            RuleEffectDecl {
                                effect_type: "finish".to_string(),
                                target: Some("success".to_string()),
                                value: Some(json!("fire_out_before_timeout")),
                                effects: Vec::new(),
                            },
                        ],
                    },
                },
            ],
            timer: Some(RuleTimerDecl {
                seconds: 3,
                on_timeout: RuleEffectDecl {
                    effect_type: "finish".to_string(),
                    target: Some("fail".to_string()),
                    value: Some(json!("timeout_or_player_dead")),
                    effects: Vec::new(),
                },
            }),
            subject_timers: Vec::new(),
            outcome: None,
            base: None,
        }),
        frame: None,
        panels: Vec::new(),
    }
}

#[cfg(test)]
pub(super) fn test_cell_timer_contract() -> SceneContract {
    SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: "minimal_fire_cells".to_string(),
            world: None,
            flow: None,
            frame: None,
            profile: Some("simulation".to_string()),
            theme: None,
            summary: Some("cell timer test".to_string()),
            goal: Some("扑灭火格".to_string()),
            state: json!({"phase": "ready", "countdown": 5}),
            shared: json!({}),
            local_nav: serde_json::json!({}),
            params: serde_json::json!({}),
            capabilities: serde_json::Value::Null,
            bindings: serde_json::json!({}),
            examples: serde_json::json!([]),
            access_export: true,
        },
        themes: Vec::new(),
        shared: json!({}),
        world: Some(WorldDecl {
            kind: "world".to_string(),
            id: None,
            topology: Some(WorldGridDecl {
                rows: 2,
                cols: 2,
                cells: vec![WorldCellDecl {
                    id: "r1c1".to_string(),
                    row: Some(1),
                    col: Some(1),
                    surface_kind: Some("floor".to_string()),
                    flammable: Some(true),
                    walkable: Some(true),
                    occupiable: Some(true),
                    capacity: None,
                    hazard_state: Some("smoke".to_string()),
                    tags: vec!["ignition_candidate".to_string()],
                }],
            }),
            resources: Vec::new(),
            datasets: Vec::new(),
            metrics: Vec::new(),
            metric_packs: Vec::new(),
            entities: vec![EntityDecl {
                id: "extinguisher_1".to_string(),
                kind: "tool".to_string(),
                label: Some("灭火器".to_string()),
                spawns: vec!["r2c2".to_string()],
                status: None,
                flags: json!({}),
                base: None,
            }],
            base: None,
        }),
        flow: Some(FlowDecl {
            kind: "flow".to_string(),
            id: None,
            start: Some(RuleStartDecl {
                mode: Some("manual".to_string()),
                action_label: Some("开始".to_string()),
            }),
            interactions: vec![
                RuleClickDecl {
                    target: "extinguisher_1".to_string(),
                    require: None,
                    effect: RuleEffectDecl {
                        effect_type: "grant".to_string(),
                        target: None,
                        value: Some(json!("extinguisher_1")),
                        effects: Vec::new(),
                    },
                },
                RuleClickDecl {
                    target: "cell:r1c1".to_string(),
                    require: Some(RuleRequireDecl {
                        require_type: "has".to_string(),
                        value: "extinguisher_1".to_string(),
                    }),
                    effect: RuleEffectDecl {
                        effect_type: "effects".to_string(),
                        target: None,
                        value: None,
                        effects: vec![
                            RuleEffectDecl {
                                effect_type: "set_status".to_string(),
                                target: Some("cell:r1c1".to_string()),
                                value: Some(json!("out")),
                                effects: Vec::new(),
                            },
                            RuleEffectDecl {
                                effect_type: "finish".to_string(),
                                target: Some("success".to_string()),
                                value: Some(json!("cell_fire_out")),
                                effects: Vec::new(),
                            },
                        ],
                    },
                },
            ],
            timer: Some(RuleTimerDecl {
                seconds: 5,
                on_timeout: RuleEffectDecl {
                    effect_type: "finish".to_string(),
                    target: Some("fail".to_string()),
                    value: Some(json!("timeout")),
                    effects: Vec::new(),
                },
            }),
            subject_timers: vec![RuleSubjectTimerDecl {
                id: Some("cell-smoke-to-burning".to_string()),
                subject_ref: "cell:r1c1".to_string(),
                timer_kind: "state_transition".to_string(),
                delay_seconds: 1.0,
                interval_seconds: None,
                repeat: false,
                on_timeout: RuleEffectDecl {
                    effect_type: "set_status".to_string(),
                    target: Some("cell:r1c1".to_string()),
                    value: Some(json!("burning")),
                    effects: Vec::new(),
                },
                cancel_when: None,
            }],
            outcome: None,
            base: None,
        }),
        frame: None,
        panels: Vec::new(),
    }
}
