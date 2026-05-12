use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::{RuleEffectDecl, SceneContract};

use super::{
    RuntimeCellView, RuntimeClockState, RuntimeEntityView, RuntimeIntent, RuntimeSceneView,
    RuntimeState,
    RuntimeTraceItem,
};

fn base_seed(seed: u64) -> u64 {
    seed.max(1)
}

fn timer_seconds(contract: &SceneContract) -> i64 {
    contract
        .flow
        .as_ref()
        .and_then(|flow| flow.timer.as_ref().map(|timer| timer.seconds as i64))
        .or_else(|| {
            contract
                .scene
                .state
                .get("countdown")
                .and_then(Value::as_i64)
        })
        .unwrap_or_default()
}

fn initial_clock(seconds: i64) -> RuntimeClockState {
    RuntimeClockState {
        countdown_remaining: seconds.max(0) as f64,
        ..RuntimeClockState::default()
    }
}

fn sync_clock_projection(state: &mut RuntimeState) {
    if state.clock.time_unit.trim().is_empty() {
        state.clock.time_unit = "second".to_string();
    }
    if !state.clock.current_time.is_finite() || state.clock.current_time < 0.0 {
        state.clock.current_time = 0.0;
    }
    if !state.clock.rate.is_finite() || state.clock.rate < 0.0 {
        state.clock.rate = 1.0;
    }
    if !state.clock.countdown_remaining.is_finite() {
        state.clock.countdown_remaining = state.countdown.max(0) as f64;
    }
    if state.clock.countdown_remaining <= 0.0 && state.countdown > 0 && state.clock.current_time <= 0.0
    {
        state.clock.countdown_remaining = state.countdown as f64;
    }
    if state.clock.countdown_remaining < 0.0 {
        state.clock.countdown_remaining = 0.0;
    }
    state.countdown = state.clock.countdown_remaining.ceil() as i64;
}

fn next_seed(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed
}

fn choose_slot(seed: &mut u64, candidates: &[String], used: &BTreeSet<String>) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let available = candidates
        .iter()
        .filter(|slot| !used.contains(*slot))
        .cloned()
        .collect::<Vec<_>>();
    let pool = if available.is_empty() {
        candidates.to_vec()
    } else {
        available
    };
    let index = (next_seed(seed) as usize) % pool.len();
    pool.get(index).cloned()
}

fn base_statuses(contract: &SceneContract) -> BTreeMap<String, String> {
    contract
        .world
        .as_ref()
        .map(|world| {
            world
                .entities
                .iter()
                .filter_map(|entity| entity.status.clone().map(|status| (entity.id.clone(), status)))
                .collect()
        })
        .unwrap_or_default()
}

fn base_flags(contract: &SceneContract) -> BTreeMap<String, bool> {
    let mut flags = BTreeMap::new();
    if let Some(world) = &contract.world {
        for entity in &world.entities {
            if let Some(map) = entity.flags.as_object() {
                for (key, value) in map {
                    if let Some(flag) = value.as_bool() {
                        flags.insert(format!("{}.{}", entity.id, key), flag);
                    }
                }
            }
        }
    }
    flags
}

fn push_timeline(state: &mut RuntimeState, message: impl Into<String>) {
    state.timeline.push(message.into());
    if state.timeline.len() > 12 {
        let overflow = state.timeline.len() - 12;
        state.timeline.drain(0..overflow);
    }
}

fn push_trace(
    state: &mut RuntimeState,
    kind: impl Into<String>,
    message: impl Into<String>,
    details: Value,
) {
    state.trace_events.push(RuntimeTraceItem {
        kind: kind.into(),
        message: message.into(),
        details,
    });
    if state.trace_events.len() > 12 {
        let overflow = state.trace_events.len() - 12;
        state.trace_events.drain(0..overflow);
    }
}

pub fn initial_runtime_state(contract: &SceneContract, seed: u64) -> RuntimeState {
    let seconds = timer_seconds(contract);
    RuntimeState {
        seed: base_seed(seed),
        phase: contract
            .scene
            .state
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or("ready")
            .to_string(),
        result: "ready".to_string(),
        reason: None,
        countdown: seconds,
        clock: initial_clock(seconds),
        placements: BTreeMap::new(),
        inventory: Vec::new(),
        statuses: base_statuses(contract),
        flags: base_flags(contract),
        timeline: vec!["等待开始".to_string()],
        trace_events: vec![RuntimeTraceItem {
            kind: "scene_ready".to_string(),
            message: "scene ready".to_string(),
            details: json!({
                "scene_id": contract.scene.id,
            }),
        }],
        subject_timers: Vec::new(),
    }
}

fn start_round(contract: &SceneContract, seed: u64) -> RuntimeState {
    let mut state = initial_runtime_state(contract, seed);
    let mut rng = state.seed;
    let mut used = BTreeSet::new();
    if let Some(world) = &contract.world {
        for entity in &world.entities {
            if let Some(slot) = choose_slot(&mut rng, &entity.spawns, &used) {
                used.insert(slot.clone());
                state.placements.insert(entity.id.clone(), slot);
            }
        }
    }
    state.seed = base_seed(rng);
    state.phase = "running".to_string();
    state.result = "running".to_string();
    state.reason = None;
    let seconds = timer_seconds(contract);
    state.clock = initial_clock(seconds);
    state.countdown = seconds;
    state.timeline = vec!["演练开始".to_string()];
    state.trace_events = vec![RuntimeTraceItem {
        kind: "scene_started".to_string(),
        message: "scene started".to_string(),
        details: json!({
            "scene_id": contract.scene.id,
        }),
    }];
    state
}

fn apply_effect(contract: &SceneContract, state: &mut RuntimeState, effect: &RuleEffectDecl) {
    match effect.effect_type.as_str() {
        "grant" => {
            if let Some(item) = effect.value.as_ref().and_then(Value::as_str) {
                if !state.inventory.iter().any(|owned| owned == item) {
                    state.inventory.push(item.to_string());
                    state.inventory.sort();
                }
                push_timeline(state, format!("获得物品: {item}"));
                push_trace(state, "grant", format!("grant {item}"), json!({ "item": item }));
            }
        }
        "set_status" => {
            if let (Some(target), Some(value)) =
                (effect.target.as_deref(), effect.value.as_ref().and_then(Value::as_str))
            {
                state.statuses.insert(target.to_string(), value.to_string());
                push_timeline(state, format!("{target} -> {value}"));
                push_trace(
                    state,
                    "set_status",
                    format!("{target} -> {value}"),
                    json!({ "target": target, "value": value }),
                );
            }
        }
        "set_flag" => {
            if let (Some(target), Some(value)) =
                (effect.target.as_deref(), effect.value.as_ref().and_then(Value::as_bool))
            {
                state.flags.insert(target.to_string(), value);
                push_timeline(state, format!("{target} = {value}"));
                push_trace(
                    state,
                    "set_flag",
                    format!("{target} = {value}"),
                    json!({ "target": target, "value": value }),
                );
            }
        }
        "finish" => {
            let phase = effect.target.clone().unwrap_or_else(|| "success".to_string());
            let reason = effect
                .value
                .as_ref()
                .and_then(Value::as_str)
                .map(ToString::to_string);
            state.phase = phase.clone();
            state.result = phase.clone();
            state.reason = reason.clone();
            push_timeline(
                state,
                reason.clone().unwrap_or_else(|| format!("scene finished: {phase}")),
            );
            push_trace(
                state,
                "finish",
                format!("scene finished: {phase}"),
                json!({ "phase": phase, "reason": reason }),
            );
        }
        "effects" => {
            for child in &effect.effects {
                apply_effect(contract, state, child);
            }
        }
        _ => {
            let _ = contract;
        }
    }
}

fn click_step(contract: &SceneContract, state: &RuntimeState, target: Option<&str>) -> RuntimeState {
    let mut next = state.clone();
    if next.phase != "running" {
        let phase = next.phase.clone();
        push_trace(
            &mut next,
            "click_ignored",
            "click ignored while scene is not running",
            json!({ "phase": phase }),
        );
        return next;
    }
    let Some(target) = target else {
        push_trace(
            &mut next,
            "click_ignored",
            "click ignored because target is missing",
            json!({}),
        );
        return next;
    };
    let Some(flow) = &contract.flow else {
        return next;
    };
    let Some(rule) = flow.interactions.iter().find(|rule| rule.target == target) else {
        push_trace(
            &mut next,
            "click_ignored",
            format!("click ignored for unknown target {target}"),
            json!({ "target": target }),
        );
        return next;
    };
    if let Some(require) = &rule.require {
        if require.require_type == "has" {
            let item = require.value.as_str();
            if !next.inventory.iter().any(|owned| owned == item) {
                push_timeline(&mut next, format!("缺少物品: {}", require.value));
                push_trace(
                    &mut next,
                    "require_failed",
                    format!("require failed for {target}"),
                    json!({ "target": target, "require": require.value }),
                );
                return next;
            }
        }
    }
    push_trace(
        &mut next,
        "click",
        format!("click -> {target}"),
        json!({ "target": target }),
    );
    apply_effect(contract, &mut next, &rule.effect);
    next
}

fn tick_step(contract: &SceneContract, state: &RuntimeState) -> RuntimeState {
    let mut next = state.clone();
    sync_clock_projection(&mut next);
    if next.phase != "running" {
        return next;
    }
    if next.clock.paused || next.clock.rate <= 0.0 {
        let details = json!({
            "paused": next.clock.paused,
            "rate": next.clock.rate,
            "current_time": next.clock.current_time,
            "countdown": next.countdown,
        });
        push_trace(
            &mut next,
            "tick_paused",
            "tick ignored because scene clock is paused",
            details,
        );
        return next;
    }
    let delta = next.clock.rate;
    next.clock.current_time += delta;
    if next.clock.countdown_remaining > 0.0 {
        next.clock.countdown_remaining -= delta;
        if next.clock.countdown_remaining < 0.0 {
            next.clock.countdown_remaining = 0.0;
        }
    }
    sync_clock_projection(&mut next);
    let countdown = next.countdown;
    let current_time = next.clock.current_time;
    let rate = next.clock.rate;
    let tick_details = json!({
        "countdown": countdown,
        "current_time": current_time,
        "rate": rate,
    });
    push_trace(
        &mut next,
        "tick",
        format!("tick -> {}", countdown),
        tick_details,
    );
    if next.clock.countdown_remaining <= 0.0 {
        if let Some(timer) = contract.flow.as_ref().and_then(|flow| flow.timer.as_ref()) {
            apply_effect(contract, &mut next, &timer.on_timeout);
        }
    }
    next
}

pub fn runtime_step(
    contract: &SceneContract,
    state: Option<RuntimeState>,
    intent: &RuntimeIntent,
) -> RuntimeState {
    let mut state = state.unwrap_or_else(|| initial_runtime_state(contract, 1));
    sync_clock_projection(&mut state);
    let mut next = match intent.kind.as_str() {
        "sync" => state,
        "restart" => initial_runtime_state(contract, state.seed.wrapping_add(1)),
        "start" => start_round(contract, state.seed.wrapping_add(1)),
        "click" => click_step(contract, &state, intent.target.as_deref()),
        "tick" => tick_step(contract, &state),
        "pause" => {
            let mut next = state.clone();
            next.clock.paused = true;
            let details = json!({ "current_time": next.clock.current_time });
            push_trace(
                &mut next,
                "pause",
                "scene clock paused",
                details,
            );
            next
        }
        "resume" => {
            let mut next = state.clone();
            next.clock.paused = false;
            let details = json!({ "current_time": next.clock.current_time });
            push_trace(
                &mut next,
                "resume",
                "scene clock resumed",
                details,
            );
            next
        }
        "rate_half" => {
            let mut next = state.clone();
            next.clock.rate = 0.5;
            let details = json!({ "rate": next.clock.rate });
            push_trace(
                &mut next,
                "rate",
                "scene clock rate -> 0.5",
                details,
            );
            next
        }
        "rate_normal" => {
            let mut next = state.clone();
            next.clock.rate = 1.0;
            let details = json!({ "rate": next.clock.rate });
            push_trace(
                &mut next,
                "rate",
                "scene clock rate -> 1.0",
                details,
            );
            next
        }
        "rate_double" => {
            let mut next = state.clone();
            next.clock.rate = 2.0;
            let details = json!({ "rate": next.clock.rate });
            push_trace(
                &mut next,
                "rate",
                "scene clock rate -> 2.0",
                details,
            );
            next
        }
        _ => state,
    };
    sync_clock_projection(&mut next);
    next
}

fn entity_flags(state: &RuntimeState, entity_id: &str) -> BTreeMap<String, bool> {
    let prefix = format!("{entity_id}.");
    state
        .flags
        .iter()
        .filter_map(|(key, value)| key.strip_prefix(&prefix).map(|short| (short.to_string(), *value)))
        .collect()
}

fn build_cells(contract: &SceneContract, entities: &[RuntimeEntityView]) -> Vec<RuntimeCellView> {
    let mut cells = Vec::new();
    if let Some(world) = &contract.world {
        if let Some(topology) = &world.topology {
            for row in 1..=topology.rows {
                for col in 1..=topology.cols {
                    let id = format!("r{row}c{col}");
                    let declared = topology.cells.iter().find(|cell| cell.id == id);
                    let occupants = entities
                        .iter()
                        .filter(|entity| entity.slot.as_deref() == Some(id.as_str()))
                        .cloned()
                        .collect::<Vec<_>>();
                    cells.push(RuntimeCellView {
                        id,
                        surface_kind: declared.and_then(|cell| cell.surface_kind.clone()),
                        flammable: declared.and_then(|cell| cell.flammable),
                        walkable: declared.and_then(|cell| cell.walkable),
                        occupiable: declared.and_then(|cell| cell.occupiable),
                        hazard_state: declared.and_then(|cell| cell.hazard_state.clone()),
                        tags: declared.map(|cell| cell.tags.clone()).unwrap_or_default(),
                        entities: occupants,
                    });
                }
            }
        }
    }
    cells
}

pub fn project_runtime_view(contract: &SceneContract, state: &RuntimeState) -> RuntimeSceneView {
    let entities = contract
        .world
        .as_ref()
        .map(|world| {
            world
                .entities
                .iter()
                .map(|entity| RuntimeEntityView {
                    id: entity.id.clone(),
                    kind: entity.kind.clone(),
                    label: entity.label.clone(),
                    slot: state.placements.get(&entity.id).cloned(),
                    status: state.statuses.get(&entity.id).cloned(),
                    flags: entity_flags(state, &entity.id),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let available_actions = match state.phase.as_str() {
        "ready" => vec!["start".to_string()],
        "running" => {
            let mut actions = vec![
                "tick".to_string(),
                "rate_half".to_string(),
                "rate_normal".to_string(),
                "rate_double".to_string(),
            ];
            if state.clock.paused {
                actions.push("resume".to_string());
            } else {
                actions.push("pause".to_string());
            }
            actions
        }
        _ => vec!["restart".to_string()],
    };
    RuntimeSceneView {
        scene_id: contract.scene.id.clone(),
        goal: contract.scene.goal.clone(),
        profile: contract.scene.profile.clone(),
        summary: contract.scene.summary.clone(),
        phase: state.phase.clone(),
        result: state.result.clone(),
        reason: state.reason.clone(),
        countdown: state.countdown,
        current_time: state.clock.current_time,
        time_unit: state.clock.time_unit.clone(),
        clock_paused: state.clock.paused,
        time_rate: state.clock.rate,
        inventory: state.inventory.clone(),
        cells: build_cells(contract, &entities),
        subject_timers: state.subject_timers.clone(),
        entities,
        available_actions,
        start_label: contract
            .flow
            .as_ref()
            .and_then(|flow| flow.start.as_ref())
            .and_then(|start| start.action_label.clone()),
    }
}

pub fn render_runtime_html(view: &RuntimeSceneView, state: &RuntimeState) -> String {
    format!(
        "<section><h3>{}</h3><p>phase: {}</p><p>countdown: {}</p><p>current_time: {:.1} {}</p><p>rate: {}</p><p>inventory: {}</p><p>timeline: {}</p></section>",
        view.scene_id,
        view.phase,
        view.countdown,
        view.current_time,
        view.time_unit,
        view.time_rate,
        state.inventory.join(", "),
        state.timeline.join(" | "),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        model::{EntityDecl, WorldDecl, WorldGridDecl},
        FlowDecl, RuleClickDecl, RuleEffectDecl, RuleRequireDecl, RuleStartDecl, RuleTimerDecl,
        SceneContract, SceneDecl,
    };

    use super::{project_runtime_view, runtime_step, RuntimeIntent};

    fn test_contract() -> SceneContract {
        SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "room_fire_click".to_string(),
                profile: Some("simulation".to_string()),
                summary: Some("test".to_string()),
                goal: Some("灭火".to_string()),
                state: json!({"phase": "ready", "countdown": 3}),
            },
            world: Some(WorldDecl {
                kind: "world".to_string(),
                topology: Some(WorldGridDecl {
                    rows: 2,
                    cols: 2,
                    cells: Vec::new(),
                }),
                resources: Vec::new(),
                entities: vec![
                    EntityDecl {
                        id: "room_fire".to_string(),
                        kind: "hazard".to_string(),
                        label: Some("火点".to_string()),
                        spawns: vec!["r1c1".to_string()],
                        status: Some("small".to_string()),
                        flags: json!({}),
                    },
                    EntityDecl {
                        id: "wall_extinguisher".to_string(),
                        kind: "tool".to_string(),
                        label: Some("灭火器".to_string()),
                        spawns: vec!["r2c2".to_string()],
                        status: None,
                        flags: json!({}),
                    },
                ],
            }),
            flow: Some(FlowDecl {
                kind: "flow".to_string(),
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
        assert!(running_view.available_actions.iter().any(|action| action == "pause"));
        assert!(running_view.available_actions.iter().any(|action| action == "rate_half"));
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
        assert!(paused_view.available_actions.iter().any(|action| action == "resume"));
        assert!(!paused_view.available_actions.iter().any(|action| action == "pause"));
    }
}
