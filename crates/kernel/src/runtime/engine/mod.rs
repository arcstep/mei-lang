mod catalog;
mod clock;
mod effects;
mod projection;
mod render_html;
mod state_init;
mod step;
mod subject_timers;
mod trace;

pub use projection::project_runtime_view;
pub use render_html::render_runtime_html;
pub use state_init::initial_runtime_state;
pub use step::runtime_step;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::runtime::types::RuntimeIntent;
    use crate::{
        model::{EntityDecl, WorldCellDecl, WorldDecl, WorldGridDecl},
        FlowDecl, RuleClickDecl, RuleEffectDecl, RuleRequireDecl, RuleStartDecl,
        RuleSubjectTimerDecl, RuleTimerDecl, SceneContract, SceneDecl,
    };

    use super::{project_runtime_view, runtime_step};

    fn test_contract() -> SceneContract {
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

    fn test_cell_timer_contract() -> SceneContract {
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

    #[test]
    fn runtime_flow_reaches_success() {
        let contract = test_contract();
        let running = runtime_step(
            &contract,
            None,
            &RuntimeIntent {
                kind: "start".to_string(),
                target: None,
            },
        );
        assert_eq!(running.phase, "running");
        let with_item = runtime_step(
            &contract,
            Some(running),
            &RuntimeIntent {
                kind: "click".to_string(),
                target: Some("wall_extinguisher".to_string()),
            },
        );
        let done = runtime_step(
            &contract,
            Some(with_item),
            &RuntimeIntent {
                kind: "click".to_string(),
                target: Some("room_fire".to_string()),
            },
        );
        assert_eq!(done.phase, "success");
        assert_eq!(
            done.statuses.get("room_fire").map(String::as_str),
            Some("out")
        );
        let view = project_runtime_view(&contract, &done);
        assert_eq!(view.phase, "success");
    }

    #[test]
    fn runtime_flow_times_out() {
        let contract = test_contract();
        let mut state = runtime_step(
            &contract,
            None,
            &RuntimeIntent {
                kind: "start".to_string(),
                target: None,
            },
        );
        for _ in 0..3 {
            state = runtime_step(
                &contract,
                Some(state),
                &RuntimeIntent {
                    kind: "tick".to_string(),
                    target: None,
                },
            );
        }
        assert_eq!(state.phase, "fail");
        assert_eq!(state.reason.as_deref(), Some("timeout_or_player_dead"));
    }

    #[test]
    fn runtime_subject_timer_drives_cell_hazard_and_click_success() {
        let contract = test_cell_timer_contract();
        let started = runtime_step(
            &contract,
            None,
            &RuntimeIntent {
                kind: "start".to_string(),
                target: None,
            },
        );
        assert_eq!(started.subject_timers.len(), 1);
        assert_eq!(started.subject_timers[0].subject_ref, "cell:r1c1");

        let after_tick = runtime_step(
            &contract,
            Some(started),
            &RuntimeIntent {
                kind: "tick".to_string(),
                target: None,
            },
        );
        assert_eq!(
            after_tick.statuses.get("cell:r1c1").map(String::as_str),
            Some("burning")
        );
        assert!(after_tick.subject_timers.is_empty());

        let with_item = runtime_step(
            &contract,
            Some(after_tick),
            &RuntimeIntent {
                kind: "click".to_string(),
                target: Some("extinguisher_1".to_string()),
            },
        );
        let pickup_view = project_runtime_view(&contract, &with_item);
        let extinguisher = pickup_view
            .entities
            .iter()
            .find(|entity| entity.id == "extinguisher_1")
            .expect("extinguisher should exist");
        assert!(extinguisher.in_inventory);
        assert_eq!(extinguisher.slot, None);
        let tool_cell = pickup_view
            .cells
            .iter()
            .find(|cell| cell.id == "r2c2")
            .expect("r2c2 should exist");
        assert!(tool_cell.entities.is_empty());

        let success = runtime_step(
            &contract,
            Some(with_item),
            &RuntimeIntent {
                kind: "click".to_string(),
                target: Some("cell:r1c1".to_string()),
            },
        );
        assert_eq!(success.phase, "success");
        assert_eq!(success.reason.as_deref(), Some("cell_fire_out"));

        let view = project_runtime_view(&contract, &success);
        let cell = view
            .cells
            .iter()
            .find(|cell| cell.id == "r1c1")
            .expect("r1c1 should exist");
        assert_eq!(cell.hazard_state.as_deref(), Some("out"));
        assert_eq!(cell.interaction_target.as_deref(), Some("cell:r1c1"));
        assert!(cell.clickable);
    }

    #[test]
    fn runtime_clock_supports_pause_resume_and_rate() {
        let contract = test_contract();
        let mut state = runtime_step(
            &contract,
            None,
            &RuntimeIntent {
                kind: "start".to_string(),
                target: None,
            },
        );
        assert_eq!(state.countdown, 3);
        assert_eq!(state.clock.current_time, 0.0);
        assert_eq!(state.clock.rate, 1.0);
        assert!(!state.clock.paused);

        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "rate_half".to_string(),
                target: None,
            },
        );
        assert_eq!(state.clock.rate, 0.5);
        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "tick".to_string(),
                target: None,
            },
        );
        assert_eq!(state.countdown, 3);
        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "tick".to_string(),
                target: None,
            },
        );
        assert_eq!(state.countdown, 2);
        assert_eq!(state.clock.current_time, 1.0);

        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "pause".to_string(),
                target: None,
            },
        );
        let paused_countdown = state.countdown;
        let paused_time = state.clock.current_time;
        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "tick".to_string(),
                target: None,
            },
        );
        assert_eq!(state.countdown, paused_countdown);
        assert_eq!(state.clock.current_time, paused_time);

        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "resume".to_string(),
                target: None,
            },
        );
        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "rate_double".to_string(),
                target: None,
            },
        );
        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "tick".to_string(),
                target: None,
            },
        );
        assert_eq!(state.phase, "fail");
        assert_eq!(state.reason.as_deref(), Some("timeout_or_player_dead"));
    }

    #[test]
    fn runtime_view_exposes_clock_actions() {
        let contract = test_contract();
        let running = runtime_step(
            &contract,
            None,
            &RuntimeIntent {
                kind: "start".to_string(),
                target: None,
            },
        );
        let running_view = project_runtime_view(&contract, &running);
        assert!(running_view
            .available_actions
            .iter()
            .any(|action| action == "pause"));
        assert!(running_view
            .available_actions
            .iter()
            .any(|action| action == "rate_half"));
        assert_eq!(running_view.time_rate, 1.0);
        assert!(!running_view.clock_paused);

        let paused = runtime_step(
            &contract,
            Some(running),
            &RuntimeIntent {
                kind: "pause".to_string(),
                target: None,
            },
        );
        let paused_view = project_runtime_view(&contract, &paused);
        assert!(paused_view
            .available_actions
            .iter()
            .any(|action| action == "resume"));
        assert!(!paused_view
            .available_actions
            .iter()
            .any(|action| action == "pause"));
    }
}
