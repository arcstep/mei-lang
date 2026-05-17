use serde_json::json;

use crate::runtime::types::{RuntimeState, RuntimeSubjectTimerState};
use crate::{RuleSubjectTimerDecl, SceneContract};

use super::effects::apply_effect;
use super::trace::push_trace;

pub(in crate::runtime::engine) fn subject_timer_runtime_id(
    decl: &RuleSubjectTimerDecl,
    index: usize,
) -> String {
    decl.id
        .clone()
        .unwrap_or_else(|| format!("subject_timer_{index}"))
}

pub(in crate::runtime::engine) fn materialize_subject_timers(
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

pub(in crate::runtime::engine) fn subject_timer_decl_by_id<'a>(
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

pub(in crate::runtime::engine) fn timer_cancelled(
    timer: &RuntimeSubjectTimerState,
    state: &RuntimeState,
) -> bool {
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

pub(in crate::runtime::engine) fn apply_subject_timers(
    contract: &SceneContract,
    state: &mut RuntimeState,
) {
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
