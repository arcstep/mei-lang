use serde_json::Value;

use crate::model::LayoutDecl;

pub(super) fn layout_gap_y_px(layout: &LayoutDecl) -> f64 {
    layout
        .gap
        .as_deref()
        .and_then(first_css_scalar_px)
        .unwrap_or(0.0)
}

pub(super) fn layout_padding_vertical_px(layout: &LayoutDecl) -> f64 {
    layout
        .padding
        .as_deref()
        .map(padding_vertical_px)
        .unwrap_or(0.0)
}

pub(super) fn layout_padding_horizontal_px(layout: &LayoutDecl) -> f64 {
    layout
        .padding
        .as_deref()
        .map(padding_horizontal_px)
        .unwrap_or(0.0)
}

pub(super) fn px_track(value: f64) -> String {
    format!("{}px", value.round())
}

pub(super) fn sum_fixed_px_tracks(tracks: &[String]) -> Option<f64> {
    let mut sum = 0.0;
    for track in tracks {
        let value = track.trim();
        if let Some(px) = parse_px(value) {
            sum += px.max(0.0);
            continue;
        }
        return None;
    }
    Some(sum)
}

pub(super) fn is_degenerate_track(token: &str) -> bool {
    let token = token.trim().to_ascii_lowercase();
    token == "0" || token == "0px" || token.starts_with("minmax(0")
}

pub(super) fn css_scalar_numbers(value: &str) -> Vec<f64> {
    value
        .split_whitespace()
        .filter_map(|token| {
            let token = token.trim().trim_end_matches(',');
            parse_px(token).or_else(|| token.parse::<f64>().ok())
        })
        .collect()
}

pub(super) fn first_css_scalar_px(value: &str) -> Option<f64> {
    value
        .split_whitespace()
        .find_map(|token| parse_px(token.trim().trim_end_matches(',')))
}

pub(super) fn padding_vertical_px(value: &str) -> f64 {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.is_empty() {
        return 0.0;
    }
    let top = parse_px(tokens[0]).unwrap_or(0.0);
    let bottom = if tokens.len() >= 3 {
        parse_px(tokens[2]).unwrap_or(top)
    } else {
        top
    };
    top + bottom
}

pub(super) fn padding_horizontal_px(value: &str) -> f64 {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.is_empty() {
        return 0.0;
    }
    let right = if tokens.len() >= 2 {
        parse_px(tokens[1]).unwrap_or(0.0)
    } else {
        parse_px(tokens[0]).unwrap_or(0.0)
    };
    let left = if tokens.len() >= 4 {
        parse_px(tokens[3]).unwrap_or(right)
    } else {
        right
    };
    right + left
}

pub(super) fn parse_px(value: &str) -> Option<f64> {
    let raw = value.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(px) = raw.strip_suffix("px") {
        return px.trim().parse::<f64>().ok();
    }
    if raw == "0" {
        return Some(0.0);
    }
    None
}

pub(super) fn value_as_px(value: &Value) -> Option<f64> {
    if let Some(raw) = value.as_str() {
        return parse_px(raw);
    }
    value.as_f64()
}
