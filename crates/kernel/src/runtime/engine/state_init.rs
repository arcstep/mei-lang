use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::runtime::types::{RuntimeState, RuntimeTraceItem};
use crate::SceneContract;

use super::catalog::{base_flags, base_seed, base_statuses, choose_slot};
use super::clock::{initial_clock, timer_seconds};
use super::subject_timers::materialize_subject_timers;

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
        placements: std::collections::BTreeMap::new(),
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

pub(in crate::runtime::engine) fn start_round(contract: &SceneContract, seed: u64) -> RuntimeState {
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
