use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value as JsonValue};
use starlark::{
    environment::{GlobalsBuilder, Module},
    eval::Evaluator,
    syntax::{AstModule, Dialect},
};

const FORBIDDEN_TOKENS: &[&str] = &["for", "while", "lambda", "load", "import", "open"];

const MEILANG_PRELUDE: &str = concat!(
    r#"
exports = []

def _declare(value):
    exports.append(value)
    return value

def _clean(values):
    out = {}
    for key, value in values.items():
        if value != None:
            out[key] = value
    return out

def app(id, title = None, default_scene = None, entries = None):
    return _declare(_clean({
        "kind": "app",
        "id": id,
        "title": title,
        "default_scene": default_scene,
        "entries": entries if entries != None else [],
    }))

def entry(scene = None, frame = None, id = None, title = None):
    return _clean({
        "id": id,
        "scene": scene,
        "frame": frame,
        "title": title,
    })

def scene_file_ref(path, id = None):
    return _clean({
        "kind": "scene_file_ref",
        "path": path,
        "id": id,
    })

def app_add_scene(scene = None, id = None, profile = None, summary = None, goal = None, state = None):
    if scene != None:
        return _declare({
            "kind": "app_scene_ref",
            "scene": scene,
        })
    return scene_decl(
        id = id,
        profile = profile,
        summary = summary,
        goal = goal,
        state = state,
    )

def scene_decl(id, profile = None, summary = None, goal = None, state = None):
    return _declare(_clean({
        "kind": "scene",
        "id": id,
        "profile": profile,
        "summary": summary,
        "goal": goal,
        "state": state if state != None else {},
    }))

def scene(id, profile = None, summary = None, goal = None, state = None):
    return scene_decl(
        id = id,
        profile = profile,
        summary = summary,
        goal = goal,
        state = state,
    )

def grid(rows = None, cols = None, columns = None, areas = None, gap = None, padding = None):
    if rows != None and cols != None and columns == None and areas == None:
        return _clean({
            "rows": rows,
            "cols": cols,
        })
    return _clean({
        "type": "grid",
        "rows": rows,
        "cols": cols,
        "columns": columns,
        "areas": areas,
        "gap": gap,
        "padding": padding,
    })

def flex(direction, gap = None, padding = None):
    return _clean({
        "type": "flex",
        "direction": direction,
        "gap": gap,
        "padding": padding,
    })

def frame(title = None, layout = None):
    return _declare(_clean({
        "kind": "frame",
        "title": title,
        "layout": layout,
    }))

def world(topology = None, resources = None, entities = None):
    return _declare(_clean({
        "kind": "world",
        "topology": topology,
        "resources": resources if resources != None else [],
        "entities": entities if entities != None else [],
    }))

def resource(id, kind, title = None, source = None, content = None):
    return _clean({
        "id": id,
        "kind": kind,
        "title": title,
        "source": source,
        "content": content,
    })

def entity(id, kind, label = None, spawns = None, status = None, flags = None):
    return _clean({
        "id": id,
        "kind": kind,
        "label": label,
        "spawns": spawns if spawns != None else [],
        "status": status,
        "flags": flags if flags != None else {},
    })

def start(mode = None, action_label = None):
    return _clean({
        "mode": mode,
        "action_label": action_label,
    })

def has(value):
    return {
        "type": "has",
        "value": value,
    }

def grant(value):
    return {
        "type": "grant",
        "value": value,
    }

def set_status(target, value):
    return {
        "type": "set_status",
        "target": target,
        "value": value,
    }

def set_flag(target, value):
    return {
        "type": "set_flag",
        "target": target,
        "value": value,
    }

def finish(target, value = None):
    return _clean({
        "type": "finish",
        "target": target,
        "value": value,
    })

def effects(items):
    return {
        "type": "effects",
        "effects": items,
    }

def click(target, require = None, effect = None):
    return _clean({
        "target": target,
        "require": require,
        "effect": effect,
    })

def rule_timer(seconds, on_timeout):
    return {
        "seconds": seconds,
        "on_timeout": on_timeout,
    }

def rule_outcome(success = None, fail = None):
    return _clean({
        "success": success,
        "fail": fail,
    })

def flow(start = None, interactions = None, timer = None, outcome = None):
    return _declare(_clean({
        "kind": "flow",
        "start": start,
        "interactions": interactions if interactions != None else [],
        "timer": timer,
        "outcome": outcome,
    }))

def panel(id, title = None, area = None, blocks = None):
    return _declare(_clean({
        "kind": "panel",
        "id": id,
        "title": title,
        "area": area,
        "blocks": blocks if blocks != None else [],
    }))

def component(use, id = None, title = None, area = None, props = None):
    return _clean({
        "kind": "block",
        "use_key": use,
        "id": id,
        "title": title,
        "area": area,
        "props": props if props != None else {},
    })

def markdown(path = None, id = None, title = None, area = None, content = None, resource = None):
    if id == None and title == None and area == None and content == None and resource == None:
        return {
            "kind": "markdown",
            "path": path,
        }
    return component(
        "doc.markdown",
        id = id,
        title = title,
        area = area,
        props = _clean({
            "path": path,
            "content": content,
            "resource": resource,
        }),
    )

def csv(path):
    return {
        "kind": "csv",
        "path": path,
    }

def world_ref(id):
    return {
        "__ref": "world",
        "id": id,
    }

def scene_ref(id):
    return {
        "__ref": "scene",
        "id": id,
    }
"#
);

pub fn describe_dsl() -> JsonValue {
    json!({
        "runtime": "starlark",
        "forbidden_tokens": FORBIDDEN_TOKENS,
        "public_surface": [
            "app",
            "entry",
            "scene_file_ref",
            "scene",
            "world",
            "resource",
            "entity",
            "flow",
            "frame",
            "panel",
            "component",
            "doc.markdown",
            "world_ref",
            "scene_ref",
            "ds.csv",
        ],
    })
}

fn sanitize_for_policy(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    for ch in source.chars() {
        if let Some(quote) = in_string {
            if ch == '\n' {
                out.push('\n');
                escaped = false;
                continue;
            }
            out.push(' ');
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '#' => out.push(' '),
            '"' | '\'' => {
                in_string = Some(ch);
                out.push(' ');
            }
            _ => out.push(ch),
        }
    }
    out
}

fn validate_policy(source: &str) -> Result<()> {
    let sanitized = sanitize_for_policy(source);
    for token in sanitized
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| !token.is_empty())
    {
        if FORBIDDEN_TOKENS.contains(&token) {
            bail!("authoring source contains forbidden token `{token}`");
        }
    }
    Ok(())
}

fn rewrite_namespaces(source: &str) -> String {
    source
        .replace("app.add_scene(", "app_add_scene(")
        .replace("scene.set_world(", "world(")
        .replace("scene.set_flow(", "flow(")
        .replace("scene.set_frame(", "frame(")
        .replace("frame.add_panel(", "panel(")
        .replace("doc.", "")
        .replace("ds.", "")
        .replace("ui.", "")
}

fn normalize_output(raw: &str) -> Result<JsonValue> {
    let value: JsonValue =
        serde_json::from_str(raw).context("Starlark output was not valid JSON")?;
    match value {
        JsonValue::Array(items) => Ok(JsonValue::Array(items)),
        other => Ok(json!([other])),
    }
}

pub fn evaluate_mei_source(filename: &str, source: &str) -> Result<JsonValue> {
    validate_policy(source)?;
    let source = rewrite_namespaces(source);
    let source = format!("{MEILANG_PRELUDE}\n\n{source}");
    let ast = AstModule::parse(filename, source, &Dialect::Standard)
        .map_err(|error| anyhow::anyhow!("failed to parse {filename}: {error}"))?;
    let globals = GlobalsBuilder::standard().build();
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.eval_module(ast, &globals)
        .map_err(|error| anyhow::anyhow!("failed to evaluate {filename}: {error}"))?;
    let exports = module
        .get("exports")
        .context("Starlark file did not produce exports")?;
    let raw_json = exports
        .to_json()
        .context("failed to convert exports to JSON")?;
    normalize_output(&raw_json)
}

pub fn evaluate_mei_file(path: impl AsRef<Path>) -> Result<JsonValue> {
    let path = path.as_ref();
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    evaluate_mei_source(&path.to_string_lossy(), &source)
}
