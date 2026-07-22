//! Build a redacted, portable `app.toml` for snapshot v2 runtimes.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

const SENSITIVE_KEY_FRAGMENTS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "apikey",
    "api_key",
    "private_key",
    "privatekey",
    "credential",
    "auth_header",
];

/// Keys that typically hold remote endpoints we should not ship as-is.
const ENDPOINT_KEY_FRAGMENTS: &[&str] = &["endpoint", "webhook", "connection_string"];

#[derive(Debug, Clone, Default)]
pub struct PortableSource {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub sheet: Option<String>,
    pub header_row: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct PortableConfigResult {
    /// TOML text for `runtime/app.toml`.
    pub toml: String,
    pub sources: Vec<PortableSource>,
    pub dropped_auth: bool,
    pub dropped_remote_sources: Vec<String>,
    pub dropped_params: Vec<String>,
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SENSITIVE_KEY_FRAGMENTS
        .iter()
        .any(|frag| lower.contains(frag))
        || ENDPOINT_KEY_FRAGMENTS
            .iter()
            .any(|frag| lower.contains(frag))
}

fn looks_absolute_path(value: &str) -> bool {
    let t = value.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with('/') && !t.starts_with("/workspace-") && !t.starts_with("/gis") {
        // Absolute Unix path that is not a known relative-url style.
        if t.starts_with("/Users/") || t.starts_with("/home/") || t.starts_with("/var/") {
            return true;
        }
    }
    // Windows drive
    let bytes = t.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn value_to_toml_inline(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => {
            if looks_absolute_path(s) {
                return None;
            }
            Some(format!("\"{}\"", toml_escape(s)))
        }
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(value_to_toml_inline).collect();
            Some(format!("[{}]", parts.join(", ")))
        }
        Value::Object(_) => {
            // Nested objects for themes are written via toml::Value below.
            None
        }
    }
}

fn json_to_toml_value(value: &Value) -> Option<toml::Value> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(toml::Value::Boolean(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                toml::Value::try_from(f).ok()
            } else {
                None
            }
        }
        Value::String(s) => {
            if looks_absolute_path(s) {
                None
            } else {
                Some(toml::Value::String(s.clone()))
            }
        }
        Value::Array(items) => {
            let arr: Vec<toml::Value> = items.iter().filter_map(json_to_toml_value).collect();
            Some(toml::Value::Array(arr))
        }
        Value::Object(map) => {
            let mut table = toml::map::Map::new();
            for (k, v) in map {
                if is_sensitive_key(k) {
                    continue;
                }
                if let Some(tv) = json_to_toml_value(v) {
                    table.insert(k.clone(), tv);
                }
            }
            Some(toml::Value::Table(table))
        }
    }
}

/// Load workspace app.toml (or JSON pair via kernel is not available here — TOML only)
/// and emit a portable subset.
pub fn build_portable_app_toml(
    app_root: &Path,
    app_id: &str,
) -> anyhow::Result<PortableConfigResult> {
    let toml_path = app_root.join("app.toml");
    let mut result = PortableConfigResult::default();

    let raw = if toml_path.is_file() {
        std::fs::read_to_string(&toml_path)?
    } else {
        // Minimal stub when no app.toml exists.
        result.toml = format!(
            "schema_version = \"mei-app-v1\"\napp_id = \"{}\"\ntitle = \"{}\"\ndefault_stage = \"home\"\n\n[paths]\nupload = \"upload\"\n",
            toml_escape(app_id),
            toml_escape(app_id)
        );
        return Ok(result);
    };

    let parsed: toml::Value = raw
        .parse()
        .map_err(|e| anyhow::anyhow!("parse app.toml: {e}"))?;
    let table = parsed.as_table().cloned().unwrap_or_default();

    let mut out = String::new();
    out.push_str("schema_version = \"mei-app-v1\"\n");

    let title = table
        .get("title")
        .and_then(|v| v.as_str())
        .or_else(|| table.get("label").and_then(|v| v.as_str()))
        .unwrap_or(app_id);
    let default_stage = table
        .get("default_stage")
        .and_then(|v| v.as_str())
        .unwrap_or("home");
    let resolved_app_id = table
        .get("app_id")
        .and_then(|v| v.as_str())
        .unwrap_or(app_id);

    out.push_str(&format!("title = \"{}\"\n", toml_escape(title)));
    out.push_str(&format!(
        "default_stage = \"{}\"\n",
        toml_escape(default_stage)
    ));
    out.push_str(&format!("app_id = \"{}\"\n", toml_escape(resolved_app_id)));
    out.push_str("generation = \"current\"\n");

    // paths — relative only
    out.push_str("\n[paths]\n");
    out.push_str("upload = \"upload\"\n");
    if let Some(paths) = table.get("paths").and_then(|v| v.as_table()) {
        if let Some(proto) = paths.get("prototype").and_then(|v| v.as_str()) {
            if !looks_absolute_path(proto) {
                out.push_str(&format!("prototype = \"{}\"\n", toml_escape(proto)));
            }
        }
    }

    // Drop auth entirely.
    if table.contains_key("auth") {
        result.dropped_auth = true;
    }

    // ops.sources / themes / basemaps / params / fill flags
    let ops = table.get("ops").and_then(|v| v.as_table());
    if let Some(ops) = ops {
        if let Some(strict) = ops
            .get("strictFillDown")
            .or_else(|| ops.get("strict_fill_down"))
        {
            if let Some(b) = strict.as_bool() {
                out.push_str("\n[ops]\n");
                out.push_str(&format!("strictFillDown = {b}\n"));
                if let Some(fd) = ops
                    .get("fillDown")
                    .or_else(|| ops.get("fill_down"))
                    .and_then(|v| v.as_bool())
                {
                    out.push_str(&format!("fillDown = {fd}\n"));
                }
            }
        } else if ops
            .get("fillDown")
            .or_else(|| ops.get("fill_down"))
            .is_some()
        {
            out.push_str("\n[ops]\n");
            if let Some(fd) = ops
                .get("fillDown")
                .or_else(|| ops.get("fill_down"))
                .and_then(|v| v.as_bool())
            {
                out.push_str(&format!("fillDown = {fd}\n"));
            }
        }

        // sources
        if let Some(sources) = ops.get("sources").and_then(|v| v.as_table()) {
            for (sid, entry) in sources {
                let Some(src_table) = entry.as_table() else {
                    continue;
                };
                let kind = src_table
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("xlsx")
                    .to_string();
                if src_table.get("connection").is_some()
                    || kind.eq_ignore_ascii_case("sqlite")
                    || kind.eq_ignore_ascii_case("sql")
                    || kind.eq_ignore_ascii_case("http")
                    || kind.eq_ignore_ascii_case("api")
                {
                    result.dropped_remote_sources.push(sid.clone());
                    continue;
                }
                let path = src_table
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if path.is_empty() || looks_absolute_path(&path) {
                    result.dropped_remote_sources.push(sid.clone());
                    continue;
                }
                out.push_str(&format!("\n[ops.sources.{sid}]\n"));
                out.push_str(&format!("kind = \"{}\"\n", toml_escape(&kind)));
                out.push_str(&format!("path = \"{}\"\n", toml_escape(&path)));
                let sheet = src_table
                    .get("sheet")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                if let Some(ref s) = sheet {
                    out.push_str(&format!("sheet = \"{}\"\n", toml_escape(s)));
                }
                let header_row = src_table
                    .get("header_row")
                    .or_else(|| src_table.get("headerRow"))
                    .and_then(|v| v.as_integer());
                if let Some(h) = header_row {
                    out.push_str(&format!("header_row = {h}\n"));
                }
                result.sources.push(PortableSource {
                    id: sid.clone(),
                    kind,
                    path,
                    sheet,
                    header_row,
                });
            }
        }

        // themes — keep as nested tables via toml serialization
        if let Some(themes) = ops.get("themes") {
            if let Some(tv) = json_to_toml_value(&toml_value_to_json(themes)) {
                if let toml::Value::Table(theme_table) = tv {
                    for (theme_id, theme_val) in theme_table {
                        append_toml_table(&mut out, &format!("ops.themes.{theme_id}"), &theme_val);
                    }
                }
            }
        }

        // basemaps — relative tile URLs + nested style (colors/zoom; required for map chrome)
        if let Some(basemaps) = ops.get("basemaps").and_then(|v| v.as_table()) {
            for (bid, entry) in basemaps {
                let Some(bt) = entry.as_table() else { continue };
                out.push_str(&format!("\n[ops.basemaps.{bid}]\n"));
                for key in ["tilesBaseUrl", "tilejsonPath"] {
                    if let Some(s) = bt.get(key).and_then(|v| v.as_str()) {
                        if looks_absolute_path(s) && !s.starts_with("/gis") {
                            continue;
                        }
                        // Drop localhost absolute URLs with ports
                        if s.contains("127.0.0.1") || s.contains("localhost") {
                            continue;
                        }
                        out.push_str(&format!("{key} = \"{}\"\n", toml_escape(s)));
                    }
                }
                if let Some(style) = bt.get("style") {
                    if let Some(tv) = json_to_toml_value(&toml_value_to_json(style)) {
                        append_toml_table(&mut out, &format!("ops.basemaps.{bid}.style"), &tv);
                    }
                }
            }
        }

        // font_scale — cockpit title/metric type ramp (visual, not secrets)
        if let Some(font_scale) = ops.get("font_scale").or_else(|| ops.get("fontScale")) {
            if let Some(tv) = json_to_toml_value(&toml_value_to_json(font_scale)) {
                append_toml_table(&mut out, "ops.font_scale", &tv);
            }
        }

        // layout — sectionRows / gaps / headerHeight used by runtime plans
        if let Some(layout) = ops.get("layout") {
            if let Some(tv) = json_to_toml_value(&toml_value_to_json(layout)) {
                if let toml::Value::Table(layout_table) = tv {
                    for (layout_id, layout_val) in layout_table {
                        append_toml_table(
                            &mut out,
                            &format!("ops.layout.\"{layout_id}\""),
                            &layout_val,
                        );
                    }
                }
            }
        }

        // theme_selection — active theme id when themes table is present
        if let Some(theme_selection) = ops
            .get("theme_selection")
            .or_else(|| ops.get("themeSelection"))
        {
            if let Some(tv) = json_to_toml_value(&toml_value_to_json(theme_selection)) {
                append_toml_table(&mut out, "ops.theme_selection", &tv);
            }
        }

        // params — filter sensitive / absolute
        if let Some(params) = ops.get("params").and_then(|v| v.as_table()) {
            let mut wrote_header = false;
            for (pk, pv) in params {
                if is_sensitive_key(pk) {
                    result.dropped_params.push(pk.clone());
                    continue;
                }
                let json = toml_value_to_json(pv);
                if let Some(inline) = value_to_toml_inline(&json) {
                    if !wrote_header {
                        out.push_str("\n[ops.params]\n");
                        wrote_header = true;
                    }
                    out.push_str(&format!("{pk} = {inline}\n"));
                } else {
                    result.dropped_params.push(pk.clone());
                }
            }
        }
    }

    // features — keep simple bools only, drop AI external etc. that imply network
    if let Some(features) = table.get("features").and_then(|v| v.as_table()) {
        let mut wrote = false;
        for (fk, fv) in features {
            let lower = fk.to_ascii_lowercase();
            if lower.contains("ai") || lower.contains("external") || is_sensitive_key(fk) {
                continue;
            }
            if let Some(b) = fv.as_bool() {
                if !wrote {
                    out.push_str("\n[features]\n");
                    wrote = true;
                }
                out.push_str(&format!("{fk} = {b}\n"));
            }
        }
    }

    let _seen: BTreeSet<String> = BTreeSet::new();
    let _ = _seen;
    result.toml = out;
    Ok(result)
}

fn toml_value_to_json(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
        toml::Value::Array(items) => Value::Array(items.iter().map(toml_value_to_json).collect()),
        toml::Value::Table(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), toml_value_to_json(v));
            }
            Value::Object(obj)
        }
    }
}

fn append_toml_table(out: &mut String, header: &str, value: &toml::Value) {
    match value {
        toml::Value::Table(map) => {
            let mut scalars = Vec::new();
            let mut nested = Vec::new();
            for (k, v) in map {
                match v {
                    toml::Value::Table(_) => nested.push((k.clone(), v.clone())),
                    _ => scalars.push((k.clone(), v.clone())),
                }
            }
            if !scalars.is_empty() || nested.is_empty() {
                out.push_str(&format!("\n[{header}]\n"));
                for (k, v) in scalars {
                    if let Some(inline) = value_to_toml_inline(&toml_value_to_json(&v)) {
                        out.push_str(&format!("{k} = {inline}\n"));
                    }
                }
            }
            for (k, v) in nested {
                append_toml_table(out, &format!("{header}.{k}"), &v);
            }
        }
        other => {
            if let Some(inline) = value_to_toml_inline(&toml_value_to_json(other)) {
                out.push_str(&format!("\n[{header}]\nvalue = {inline}\n"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn strips_auth_and_connection() {
        let tmp = std::env::temp_dir().join(format!("mei-portable-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(
            tmp.join("app.toml"),
            r#"
title = "Demo"
app_id = "demo"
default_stage = "home"

[auth]
enabled = true
secret = "nope"

[ops.sources.local]
kind = "xlsx"
path = "upload/a.xlsx"

[ops.sources.remote]
kind = "sqlite"
path = "upload/db.sqlite"
connection = "postgres://x"

[ops.params]
home_bg = "url(/workspace-app-assets/demo/assets/bg.png)"
api_token = "secret"
"#,
        )
        .unwrap();
        let result = build_portable_app_toml(&tmp, "demo").unwrap();
        assert!(result.dropped_auth);
        assert!(result.dropped_remote_sources.iter().any(|s| s == "remote"));
        assert!(result.toml.contains("ops.sources.local"));
        assert!(!result.toml.contains("postgres"));
        assert!(!result.toml.contains("api_token"));
        assert!(result.toml.contains("home_bg"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn keeps_basemap_style_font_scale_and_layout() {
        let tmp = std::env::temp_dir().join(format!("mei-portable-style-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(
            tmp.join("app.toml"),
            r##"
title = "Demo"
app_id = "demo"

[ops.basemaps.shapingba]
tilesBaseUrl = "/gis"
tilejsonPath = "/shapingba-z10-16"

[ops.basemaps.shapingba.style]
backgroundColor = "#0c2848"
defaultZoom = 11

[ops.font_scale]
1 = "16px"
3 = "26px"

[ops.layout."home/T1"]
headerHeight = "72px"
"##,
        )
        .unwrap();
        let result = build_portable_app_toml(&tmp, "demo").unwrap();
        assert!(result.toml.contains("tilesBaseUrl"));
        assert!(result.toml.contains("backgroundColor"));
        assert!(result.toml.contains("ops.font_scale"));
        assert!(result.toml.contains("headerHeight"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
