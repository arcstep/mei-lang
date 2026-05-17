use serde_json::json;

use crate::runtime::types::{RuntimeIntent, RuntimeState};
use crate::SceneContract;

use super::clock::sync_clock_projection;
use super::effects::apply_effect;
use super::state_init::{initial_runtime_state, start_round};
use super::subject_timers::apply_subject_timers;
use super::trace::{push_timeline, push_trace};

pub(in crate::runtime::engine) fn click_step(
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

pub(in crate::runtime::engine) fn tick_step(
    contract: &SceneContract,
    state: &RuntimeState,
) -> RuntimeState {
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
