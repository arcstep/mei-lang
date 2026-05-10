use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::{RuleEffectDecl, SceneContract};

use super::{
    RuntimeCellView, RuntimeEntityView, RuntimeIntent, RuntimeSceneView, RuntimeState,
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
        countdown: timer_seconds(contract),
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
    state.countdown = timer_seconds(contract);
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
    if next.phase != "running" {
        return next;
    }
    if next.countdown > 0 {
        next.countdown -= 1;
    }
    let countdown = next.countdown;
    push_trace(
        &mut next,
        "tick",
        format!("tick -> {}", countdown),
        json!({ "countdown": countdown }),
    );
    if next.countdown <= 0 {
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
    let state = state.unwrap_or_else(|| initial_runtime_state(contract, 1));
    match intent.kind.as_str() {
        "sync" => state,
        "restart" => initial_runtime_state(contract, state.seed.wrapping_add(1)),
        "start" => start_round(contract, state.seed.wrapping_add(1)),
        "click" => click_step(contract, &state, intent.target.as_deref()),
        "tick" => tick_step(contract, &state),
        _ => state,
    }
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
                    let occupants = entities
                        .iter()
                        .filter(|entity| entity.slot.as_deref() == Some(id.as_str()))
                        .cloned()
                        .collect::<Vec<_>>();
                    cells.push(RuntimeCellView {
                        id,
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
        "running" => vec!["tick".to_string()],
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
        inventory: state.inventory.clone(),
        cells: build_cells(contract, &entities),
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
        "<section><h3>{}</h3><p>phase: {}</p><p>countdown: {}</p><p>inventory: {}</p><p>timeline: {}</p></section>",
        view.scene_id,
        view.phase,
        view.countdown,
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
                topology: Some(WorldGridDecl { rows: 2, cols: 2 }),
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
}
