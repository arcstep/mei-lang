//! Typed `provider_binding(...)` catalog and scene-reference lowering.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mei_syntax::v2::{parse_v2_source_file, CallArgs, V2Expr, V2Item};
use walkdir::WalkDir;

use super::admin_registry::{
    AdminApplyPolicy, AdminDangerLevel, ProviderBinding, ProviderPayloadType, ProviderValidator,
};

pub const PROVIDER_BINDING_INVALID: &str = "provider_binding_invalid";
pub const PROVIDER_BINDING_DUPLICATE: &str = "provider_binding_duplicate";
pub const PROVIDER_BINDING_UNKNOWN: &str = "provider_binding_unknown";

pub fn discover_provider_binding_catalog(
    app_root: &Path,
) -> Result<BTreeMap<String, ProviderBinding>, String> {
    let data_root = app_root.join("src/data");
    if !data_root.is_dir() {
        return Ok(BTreeMap::new());
    }
    let mut paths = WalkDir::new(&data_root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension().and_then(|value| value.to_str()) == Some("mei")
        })
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    paths.sort();

    let mut catalog = BTreeMap::new();
    for path in paths {
        let source_anchor = path
            .strip_prefix(app_root)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let source = parse_v2_source_file(path.as_path())
            .map_err(|error| format!("[{PROVIDER_BINDING_INVALID}] {source_anchor}: {error}"))?;
        for item in source.items {
            let V2Item::TopLevel { name, args } = item else {
                continue;
            };
            if name != "provider_binding" {
                continue;
            }
            let binding = lower_provider_binding(&args, source_anchor.as_str())?;
            if catalog
                .insert(binding.binding_id.clone(), binding.clone())
                .is_some()
            {
                return Err(format!(
                    "[{PROVIDER_BINDING_DUPLICATE}] duplicate binding `{}`",
                    binding.binding_id
                ));
            }
        }
    }
    Ok(catalog)
}

pub fn provider_bindings_for_scene(
    scene_path: &Path,
    catalog: &BTreeMap<String, ProviderBinding>,
) -> Result<Vec<ProviderBinding>, String> {
    let source = parse_v2_source_file(scene_path).map_err(|error| {
        format!(
            "[{PROVIDER_BINDING_INVALID}] {}: {error}",
            scene_path.display()
        )
    })?;
    let mut refs = BTreeSet::new();
    for item in &source.items {
        match item {
            V2Item::ModuleConst { value, .. } | V2Item::TemplateDecl { body: value, .. } => {
                collect_provider_refs(value, &mut refs);
            }
            V2Item::TopLevel { args, .. } => collect_refs_from_args(args, &mut refs),
            V2Item::UseTemplate { .. } => {}
        }
    }
    refs.into_iter()
        .map(|binding_id| {
            catalog.get(binding_id.as_str()).cloned().ok_or_else(|| {
                format!(
                    "[{PROVIDER_BINDING_UNKNOWN}] scene references unknown provider binding `{binding_id}`"
                )
            })
        })
        .collect()
}

fn lower_provider_binding(args: &CallArgs, source_anchor: &str) -> Result<ProviderBinding, String> {
    let binding_id = required_string(args, "id")?;
    let provider_id = required_string(args, "provider_id")?;
    let method = required_string(args, "method")?.to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "LIST" | "PUT" | "POST" | "DELETE") {
        return invalid(format!("method `{method}` is not allowed"));
    }
    let target = required_string(args, "target")?;
    if !valid_provider_target(target.as_str()) {
        return invalid(format!(
            "target `{target}` must be under ops.*, env/*/var/admin/*, or var/admin/*"
        ));
    }
    let target_is_admin_area = |area: &str| {
        target.starts_with(&format!("var/admin/{area}/"))
            || target
                .split_once("/var/admin/")
                .is_some_and(|(_, suffix)| suffix.starts_with(&format!("{area}/")))
    };
    let target_matches_provider = match provider_id.as_str() {
        "config-record" => target.starts_with("ops.") || target_is_admin_area("records"),
        "asset-slot" => target_is_admin_area("uploads"),
        "command-job" => target_is_admin_area("jobs"),
        _ => false,
    };
    if !target_matches_provider {
        return invalid(format!(
            "provider `{provider_id}` cannot use target `{target}`"
        ));
    }
    let payload_type = parse_payload_type(required_expr(args, "payload_type")?)?;
    let validator = optional_expr(args, "validator")
        .map(parse_validator)
        .transpose()?;
    let revision = required_string(args, "revision")?;
    if !matches!(revision.as_str(), "required" | "optional" | "none") {
        return invalid(format!("revision `{revision}` is not allowed"));
    }
    let idempotency = required_string(args, "idempotency")?;
    if !matches!(idempotency.as_str(), "required" | "optional" | "none") {
        return invalid(format!("idempotency `{idempotency}` is not allowed"));
    }
    let apply_policy_raw = required_string(args, "apply_policy")?;
    let apply_policy = AdminApplyPolicy::parse(apply_policy_raw.as_str()).ok_or_else(|| {
        invalid_message(format!("apply_policy `{apply_policy_raw}` is not allowed"))
    })?;
    let danger_raw = required_string(args, "danger")?;
    if !matches!(danger_raw.as_str(), "normal" | "elevated" | "critical") {
        return invalid(format!("danger `{danger_raw}` is not allowed"));
    }
    let required_capabilities = string_list(required_expr(args, "required_capabilities")?)?;

    Ok(ProviderBinding {
        binding_id,
        provider_id,
        method,
        target,
        payload_type,
        validator,
        revision,
        idempotency,
        apply_policy,
        danger: AdminDangerLevel::parse(Some(danger_raw.as_str())),
        required_capabilities,
        source_anchor: source_anchor.to_string(),
    })
}

fn valid_provider_target(target: &str) -> bool {
    if target.starts_with("ops.") || target.starts_with("var/admin/") {
        return true;
    }
    let Some(rest) = target.strip_prefix("env/") else {
        return false;
    };
    let Some((generation, suffix)) = rest.split_once('/') else {
        return false;
    };
    !generation.is_empty()
        && generation
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        && suffix.starts_with("var/admin/")
}

fn parse_payload_type(expr: &V2Expr) -> Result<ProviderPayloadType, String> {
    let (name, args) = ref_call(expr, "type_ref")?;
    let schema = keyword_string(args, "schema");
    Ok(ProviderPayloadType { name, schema })
}

fn parse_validator(expr: &V2Expr) -> Result<ProviderValidator, String> {
    let (reference, _) = ref_call(expr, "schema_ref")?;
    Ok(ProviderValidator {
        kind: "schema-ref".to_string(),
        reference,
    })
}

fn ref_call<'a>(expr: &'a V2Expr, expected: &str) -> Result<(String, &'a CallArgs), String> {
    let (name, args) = match expr {
        V2Expr::RefCall { name, args } => (name.as_str(), args),
        V2Expr::Call { path, args } if path.len() == 1 => (path[0].as_str(), args),
        _ => return invalid(format!("expected {expected}(...)")),
    };
    if name != expected {
        return invalid(format!("expected {expected}(...), got {name}(...)"));
    }
    let value = args
        .positional
        .first()
        .and_then(expr_string)
        .or_else(|| keyword_string(args, "id"))
        .ok_or_else(|| invalid_message(format!("{expected} requires a string id")))?;
    Ok((value, args))
}

fn collect_refs_from_args(args: &CallArgs, refs: &mut BTreeSet<String>) {
    for value in &args.positional {
        collect_provider_refs(value, refs);
    }
    for (_, value) in &args.keywords {
        collect_provider_refs(value, refs);
    }
}

fn collect_provider_refs(expr: &V2Expr, refs: &mut BTreeSet<String>) {
    match expr {
        V2Expr::RefCall { name, args } if name == "provider_ref" => {
            if let Some(id) = args.positional.first().and_then(expr_string) {
                refs.insert(id);
            }
            collect_refs_from_args(args, refs);
        }
        V2Expr::Call { path, args } if path.as_slice() == ["provider_ref"] => {
            if let Some(id) = args.positional.first().and_then(expr_string) {
                refs.insert(id);
            }
            collect_refs_from_args(args, refs);
        }
        V2Expr::Call { args, .. } | V2Expr::RefCall { args, .. } => {
            collect_refs_from_args(args, refs);
        }
        V2Expr::List(values) => {
            for value in values {
                collect_provider_refs(value, refs);
            }
        }
        V2Expr::Dict(values) => {
            for (_, value) in values {
                collect_provider_refs(value, refs);
            }
        }
        V2Expr::BinOp { left, right, .. } => {
            collect_provider_refs(left, refs);
            collect_provider_refs(right, refs);
        }
        V2Expr::Member { object, .. } => collect_provider_refs(object, refs),
        V2Expr::ForIn { source, body, .. } => {
            collect_provider_refs(source, refs);
            collect_provider_refs(body, refs);
        }
        V2Expr::EnumMatch {
            subject,
            cases,
            default,
        } => {
            collect_provider_refs(subject, refs);
            for (key, value) in cases {
                collect_provider_refs(key, refs);
                collect_provider_refs(value, refs);
            }
            if let Some(value) = default {
                collect_provider_refs(value, refs);
            }
        }
        V2Expr::String(_)
        | V2Expr::Number(_)
        | V2Expr::Bool(_)
        | V2Expr::None
        | V2Expr::VarRef(_) => {}
    }
}

fn required_expr<'a>(args: &'a CallArgs, key: &str) -> Result<&'a V2Expr, String> {
    optional_expr(args, key)
        .ok_or_else(|| invalid_message(format!("missing required field `{key}`")))
}

fn optional_expr<'a>(args: &'a CallArgs, key: &str) -> Option<&'a V2Expr> {
    args.keywords
        .iter()
        .find_map(|(name, value)| (name == key).then_some(value))
}

fn required_string(args: &CallArgs, key: &str) -> Result<String, String> {
    required_expr(args, key)?
        .as_string()
        .ok_or_else(|| invalid_message(format!("field `{key}` must be a string")))
}

fn keyword_string(args: &CallArgs, key: &str) -> Option<String> {
    optional_expr(args, key).and_then(expr_string)
}

fn expr_string(expr: &V2Expr) -> Option<String> {
    match expr {
        V2Expr::String(value) | V2Expr::VarRef(value) => Some(value.clone()),
        _ => None,
    }
}

trait V2ExprString {
    fn as_string(&self) -> Option<String>;
}

impl V2ExprString for V2Expr {
    fn as_string(&self) -> Option<String> {
        expr_string(self)
    }
}

fn string_list(expr: &V2Expr) -> Result<Vec<String>, String> {
    let V2Expr::List(values) = expr else {
        return invalid("required_capabilities must be a string list");
    };
    values
        .iter()
        .map(|value| {
            expr_string(value)
                .ok_or_else(|| invalid_message("required_capabilities must contain strings"))
        })
        .collect()
}

fn invalid<T>(message: impl Into<String>) -> Result<T, String> {
    Err(invalid_message(message))
}

fn invalid_message(message: impl Into<String>) -> String {
    format!("[{PROVIDER_BINDING_INVALID}] {}", message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_typed_binding_and_lowers_scene_reference() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/data/admin")).unwrap();
        std::fs::create_dir_all(root.path().join("src/scene/admin")).unwrap();
        std::fs::write(
            root.path().join("src/data/admin/providers.mei"),
            r#"
provider_binding(
  id = "organization.load",
  provider_id = "config-record",
  method = "GET",
  target = "ops.params.organization",
  payload_type = type_ref("json-object", schema = "organization"),
  validator = schema_ref("organization"),
  revision = "required",
  idempotency = "none",
  apply_policy = "hot",
  danger = "normal",
  required_capabilities = ["config_upload"],
)
"#,
        )
        .unwrap();
        let scene = root.path().join("src/scene/admin/organization.mei");
        std::fs::write(
            &scene,
            r#"
content_panel(
  id = "form",
  blocks = [provider_ref("organization.load")],
)
"#,
        )
        .unwrap();

        let catalog = discover_provider_binding_catalog(root.path()).unwrap();
        let bindings = provider_bindings_for_scene(&scene, &catalog).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].provider_id, "config-record");
        assert_eq!(bindings[0].target, "ops.params.organization");
    }
}
