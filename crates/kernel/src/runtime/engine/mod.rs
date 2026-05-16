use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::{RuleEffectDecl, RuleSubjectTimerDecl, SceneContract};

use super::{
    RuntimeClockState, RuntimeIntent, RuntimeSceneView, RuntimeState, RuntimeSubjectTimerState,
    RuntimeTraceItem,
};

mod projection;
pub use projection::project_runtime_view;


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
    if state.clock.countdown_remaining <= 0.0
        && state.countdown > 0
        && state.clock.current_time <= 0.0
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
                .filter_map(|entity| {
                    entity
                        .status
                        .clone()
                        .map(|status| (entity.id.clone(), status))
                })
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
    state.subject_timers = materialize_subject_timers(contract, state.clock.current_time);
    state
}

fn subject_timer_runtime_id(decl: &RuleSubjectTimerDecl, index: usize) -> String {
    decl.id
        .clone()
        .unwrap_or_else(|| format!("subject_timer_{index}"))
}

fn materialize_subject_timers(
    contract: &SceneContract,
    current_time: f64,
) -> Vec<RuntimeSubjectTimerState> {
    let Some(flow) = &contract.flow else {
        return Vec::new();
    };
    flow.subject_timers
        .iter()
        .enumerate()
        .map(|(index, timer)| {
            let delay = timer.delay_seconds.max(0.0);
            let interval = timer
                .interval_seconds
                .and_then(|value| (value > 0.0).then_some(value));
            RuntimeSubjectTimerState {
                id: subject_timer_runtime_id(timer, index),
                subject_ref: timer.subject_ref.clone(),
                timer_kind: timer.timer_kind.clone(),
                started_at: current_time,
                due_at: current_time + delay,
                interval,
                repeat: timer.repeat,
                cancel_when: timer.cancel_when.clone(),
            }
        })
        .collect()
}

fn subject_timer_decl_by_id<'a>(
    contract: &'a SceneContract,
    timer_id: &str,
) -> Option<&'a RuleSubjectTimerDecl> {
    contract.flow.as_ref().and_then(|flow| {
        flow.subject_timers
            .iter()
            .enumerate()
            .find_map(|(index, timer)| {
                (subject_timer_runtime_id(timer, index) == timer_id).then_some(timer)
            })
    })
}

fn timer_cancelled(timer: &RuntimeSubjectTimerState, state: &RuntimeState) -> bool {
    let Some(cancel_when) = timer.cancel_when.as_deref() else {
        return false;
    };
    let expr = cancel_when.trim();
    if let Some(item) = expr.strip_prefix("has:") {
        return state.inventory.iter().any(|owned| owned == item.trim());
    }
    if let Some(spec) = expr.strip_prefix("status:") {
        if let Some((target, value)) = spec.split_once('=') {
            return state
                .statuses
                .get(target.trim())
                .map(|current| current == value.trim())
                .unwrap_or(false);
        }
    }
    if let Some(spec) = expr.strip_prefix("flag:") {
        if let Some((target, value)) = spec.split_once('=') {
            let expect = value.trim().eq_ignore_ascii_case("true");
            return state.flags.get(target.trim()).copied() == Some(expect);
        }
    }
    if let Some(phase) = expr.strip_prefix("phase:") {
        return state.phase == phase.trim();
    }
    false
}

fn apply_subject_timers(contract: &SceneContract, state: &mut RuntimeState) {
    if state.subject_timers.is_empty() {
        return;
    }
    let now = state.clock.current_time;
    let mut fired = Vec::new();
    let mut next_timers = Vec::new();
    for mut timer in state.subject_timers.clone() {
        if timer_cancelled(&timer, state) {
            push_trace(
                state,
                "subject_timer_cancelled",
                format!("subject timer cancelled: {}", timer.id),
                json!({ "timer_id": timer.id }),
            );
            continue;
        }
        if timer.due_at <= now + f64::EPSILON {
            fired.push(timer.id.clone());
            if timer.repeat {
                if let Some(interval) = timer
                    .interval
                    .and_then(|value| (value > 0.0).then_some(value))
                {
                    timer.started_at = now;
                    timer.due_at = now + interval;
                    next_timers.push(timer);
                }
            }
        } else {
            next_timers.push(timer);
        }
    }
    state.subject_timers = next_timers;
    for timer_id in fired {
        if let Some(decl) = subject_timer_decl_by_id(contract, &timer_id) {
            push_trace(
                state,
                "subject_timer_fired",
                format!("subject timer fired: {}", timer_id),
                json!({
                    "timer_id": timer_id,
                    "subject_ref": decl.subject_ref,
                    "timer_type": decl.timer_kind,
                }),
            );
            apply_effect(contract, state, &decl.on_timeout);
        }
    }
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
                push_trace(
                    state,
                    "grant",
                    format!("grant {item}"),
                    json!({ "item": item }),
                );
            }
        }
        "set_status" => {
            if let (Some(target), Some(value)) = (
                effect.target.as_deref(),
                effect.value.as_ref().and_then(Value::as_str),
            ) {
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
            if let (Some(target), Some(value)) = (
                effect.target.as_deref(),
                effect.value.as_ref().and_then(Value::as_bool),
            ) {
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
            let phase = effect
                .target
                .clone()
                .unwrap_or_else(|| "success".to_string());
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
                reason
                    .clone()
                    .unwrap_or_else(|| format!("scene finished: {phase}")),
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

fn click_step(
    contract: &SceneContract,
    state: &RuntimeState,
    target: Option<&str>,
) -> RuntimeState {
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
    apply_subject_timers(contract, &mut next);
    if next.phase == "running" && next.clock.countdown_remaining <= 0.0 {
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
            push_trace(&mut next, "pause", "scene clock paused", details);
            next
        }
        "resume" => {
            let mut next = state.clone();
            next.clock.paused = false;
            let details = json!({ "current_time": next.clock.current_time });
            push_trace(&mut next, "resume", "scene clock resumed", details);
            next
        }
        "rate_half" => {
            let mut next = state.clone();
            next.clock.rate = 0.5;
            let details = json!({ "rate": next.clock.rate });
            push_trace(&mut next, "rate", "scene clock rate -> 0.5", details);
            next
        }
        "rate_normal" => {
            let mut next = state.clone();
            next.clock.rate = 1.0;
            let details = json!({ "rate": next.clock.rate });
            push_trace(&mut next, "rate", "scene clock rate -> 1.0", details);
            next
        }
        "rate_double" => {
            let mut next = state.clone();
            next.clock.rate = 2.0;
            let details = json!({ "rate": next.clock.rate });
            push_trace(&mut next, "rate", "scene clock rate -> 2.0", details);
            next
        }
        _ => state,
    };
    sync_clock_projection(&mut next);
    next
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
        model::{EntityDecl, WorldCellDecl, WorldDecl, WorldGridDecl},
        FlowDecl, RuleClickDecl, RuleEffectDecl, RuleRequireDecl, RuleStartDecl,
        RuleSubjectTimerDecl, RuleTimerDecl, SceneContract, SceneDecl,
    };

    use super::{project_runtime_view, runtime_step, RuntimeIntent};

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
            },
            themes: Vec::new(),
            world: Some(WorldDecl {
                kind: "world".to_string(),
                id: None,
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
            },
            themes: Vec::new(),
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
                entities: vec![EntityDecl {
                    id: "extinguisher_1".to_string(),
                    kind: "tool".to_string(),
                    label: Some("灭火器".to_string()),
                    spawns: vec!["r2c2".to_string()],
                    status: None,
                    flags: json!({}),
                }],
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
