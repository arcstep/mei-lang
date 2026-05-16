use serde_json::Value;

use crate::SceneContract;
use crate::runtime::types::{RuntimeClockState, RuntimeState};

pub(in crate::runtime::engine) fn timer_seconds(contract: &SceneContract) -> i64 {
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

pub(in crate::runtime::engine) fn initial_clock(seconds: i64) -> RuntimeClockState {
    RuntimeClockState {
        countdown_remaining: seconds.max(0) as f64,
        ..RuntimeClockState::default()
    }
}

pub(in crate::runtime::engine) fn sync_clock_projection(state: &mut RuntimeState) {
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
