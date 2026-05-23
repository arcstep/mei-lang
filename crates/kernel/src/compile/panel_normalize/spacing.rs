use serde_json::Value;

use crate::model::PanelDecl;

use super::constants::{
    COCKPIT_CARD_GAP_MAX, COCKPIT_CARD_GAP_MIN, COCKPIT_PANEL_PADDING_MAX, COCKPIT_PANEL_PADDING_MIN,
    LAYOUT_POLICY_METRICS_2_1, LAYOUT_POLICY_METRICS_STRIP, PROP_HAS_HEAD, PROP_LAYOUT_GAP,
    PROP_LAYOUT_PADDING, PROP_LAYOUT_POLICY, PolicySpacing,
};
use super::css_util::{first_css_scalar_px, parse_px, px_track};

pub(super) fn stamp_has_head_prop(panel: &mut PanelDecl, has_head: bool) {
    let map = panel.props.as_object().cloned().unwrap_or_default();
    let mut map = map;
    map.insert(PROP_HAS_HEAD.to_string(), Value::Bool(has_head));
    panel.props = Value::Object(map);
}

pub(super) fn stamp_layout_policy(panel: &mut PanelDecl, policy: &str) {
    let map = panel.props.as_object().cloned().unwrap_or_default();
    let mut map = map;
    map.insert(
        PROP_LAYOUT_POLICY.to_string(),
        Value::String(policy.to_string()),
    );
    panel.props = Value::Object(map);
}

pub(super) fn panel_layout_policy(panel: &PanelDecl) -> Option<String> {
    panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_POLICY))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
pub(super) fn policy_spacing(panel: &PanelDecl, default_gap: &str, default_padding: &str) -> PolicySpacing {
    let raw_gap = panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_GAP))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_gap)
        .to_string();
    let raw_padding = panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_PADDING))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_padding)
        .to_string();
    let policy = panel_layout_policy(panel);
    let gap = normalize_policy_gap(policy.as_deref(), &raw_gap);
    let padding = normalize_policy_padding(policy.as_deref(), &raw_padding);
    PolicySpacing { gap, padding }
}

pub(super) fn normalize_policy_gap(policy: Option<&str>, raw_gap: &str) -> String {
    let Some(px) = first_css_scalar_px(raw_gap) else {
        return raw_gap.to_string();
    };
    let next = if matches!(
        policy,
        Some(LAYOUT_POLICY_METRICS_STRIP | LAYOUT_POLICY_METRICS_2_1)
    ) {
        px.clamp(COCKPIT_CARD_GAP_MIN, COCKPIT_CARD_GAP_MAX)
    } else {
        px
    };
    px_track(next)
}

pub(super) fn normalize_policy_padding(policy: Option<&str>, raw_padding: &str) -> String {
    if !matches!(
        policy,
        Some(LAYOUT_POLICY_METRICS_STRIP | LAYOUT_POLICY_METRICS_2_1)
    ) {
        return raw_padding.to_string();
    }
    let values: Vec<f64> = raw_padding
        .split_whitespace()
        .filter_map(|token| parse_px(token.trim().trim_end_matches(',')))
        .collect();
    if values.is_empty() {
        return raw_padding.to_string();
    }
    let clamped = match values.len() {
        1 => {
            let value = values[0].clamp(COCKPIT_PANEL_PADDING_MIN, COCKPIT_PANEL_PADDING_MAX);
            vec![value]
        }
        2 => vec![
            values[0].clamp(COCKPIT_PANEL_PADDING_MIN, COCKPIT_PANEL_PADDING_MAX),
            values[1].clamp(COCKPIT_PANEL_PADDING_MIN, COCKPIT_PANEL_PADDING_MAX),
        ],
        _ => vec![
            values[0].clamp(COCKPIT_PANEL_PADDING_MIN, COCKPIT_PANEL_PADDING_MAX),
            values[1].clamp(COCKPIT_PANEL_PADDING_MIN, COCKPIT_PANEL_PADDING_MAX),
            values[2].clamp(COCKPIT_PANEL_PADDING_MIN, COCKPIT_PANEL_PADDING_MAX),
            values[3].clamp(COCKPIT_PANEL_PADDING_MIN, COCKPIT_PANEL_PADDING_MAX),
        ],
    };
    clamped
        .into_iter()
        .map(px_track)
        .collect::<Vec<_>>()
        .join(" ")
}
