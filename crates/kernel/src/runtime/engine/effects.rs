use serde_json::{json, Value};

use crate::runtime::types::RuntimeState;
use crate::{RuleEffectDecl, SceneContract};

use super::trace::{push_timeline, push_trace};

pub(in crate::runtime::engine) fn apply_effect(
    contract: &SceneContract,
    state: &mut RuntimeState,
    effect: &RuleEffectDecl,
) {
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
