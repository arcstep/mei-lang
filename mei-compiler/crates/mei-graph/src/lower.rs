use std::collections::{BTreeMap, BTreeSet};

use mei_syntax::v2::{CallArgs, V2Expr, V2Item, V2SourceFile};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{artifact_expand, object_recipes};

#[derive(Debug, Error)]
pub enum LowerGraphError {
    #[error("{0}")]
    Lower(String),
    #[error("[{code}] {message} @ {source_anchor}")]
    Diagnostic {
        code: &'static str,
        message: String,
        source_anchor: String,
    },
}

impl LowerGraphError {
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Lower(_) => None,
            Self::Diagnostic { code, .. } => Some(code),
        }
    }

    pub fn source_anchor(&self) -> Option<&str> {
        match self {
            Self::Lower(_) => None,
            Self::Diagnostic { source_anchor, .. } => Some(source_anchor),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphBlock {
    pub kind: String,
    pub block_id: String,
    pub schema: String,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphOutcome {
    pub graph_schema_version: String,
    pub source_file: String,
    pub blocks: Vec<GraphBlock>,
}

pub fn lower_v2_file(
    source_file: &str,
    file: &V2SourceFile,
) -> Result<GraphOutcome, LowerGraphError> {
    let mut blocks = Vec::new();
    for item in &file.items {
        if let V2Item::TopLevel { name, args } = item {
            blocks.push(lower_top_level(source_file, name, args)?);
        }
    }
    Ok(GraphOutcome {
        graph_schema_version: "mei-compiler-graph-v2".to_string(),
        source_file: source_file.to_string(),
        blocks,
    })
}

fn lower_top_level(
    source_file: &str,
    name: &str,
    args: &CallArgs,
) -> Result<GraphBlock, LowerGraphError> {
    let (kind, mut payload) = match name {
        "object" => ("object_catalog", lower_object_intent(source_file, args)?),
        "object_catalog" => ("object_catalog", lower_object_catalog(source_file, args)?),
        _ => (name, call_args_to_json(args)?),
    };
    if matches!(
        name,
        "scene"
            | "presentation"
            | "plane_layout"
            | "region_layout"
            | "section_layout"
            | "slide_layout"
            | "map_spec"
            | "view_spec"
            | "page_instance"
            | "link_decl"
            | "navigation"
    ) {
        if let Some(obj) = payload.as_object_mut() {
            obj.entry("source_file".to_string())
                .or_insert(JsonValue::String(source_file.to_string()));
            if obj.get("key").is_none() {
                let block_id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        LowerGraphError::Lower(format!(
                            "{name} top-level must declare non-empty `id`"
                        ))
                    })?;
                obj.insert(
                    "key".to_string(),
                    JsonValue::String(format!("{block_id}@{source_file}")),
                );
            }
        }
    }
    if name == "slide_layout" {
        validate_slide_layout_payload(&payload)?;
    }
    let block_id = derive_block_id(kind, source_file, &payload)?;
    let schema = schema_for_constructor(kind);
    Ok(GraphBlock {
        kind: kind.to_string(),
        block_id,
        schema: schema.to_string(),
        payload,
    })
}

fn validate_slide_layout_payload(payload: &JsonValue) -> Result<(), LowerGraphError> {
    let obj = payload
        .as_object()
        .ok_or_else(|| LowerGraphError::Lower("slide_layout payload must be object".into()))?;
    if let Some(pattern) = obj.get("pattern").and_then(|v| v.as_str()) {
        if mei_syntax::v2::slide_pattern_areas(pattern).is_none() {
            return Err(LowerGraphError::Lower(format!(
                "slide_layout unknown pattern `{pattern}`; expected one of: {}",
                mei_syntax::v2::SLIDE_PATTERNS.join(", ")
            )));
        }
    }
    Ok(())
}

fn schema_for_constructor(name: &str) -> &'static str {
    match name {
        "app_skeleton" => "mei-app-skeleton-artifact-v1",
        "scene" => "mei-scene-semantic-v1",
        "presentation" => "mei-presentation-semantic-v1",
        "plane_layout" | "region_layout" | "section_layout" => "mei-scene-layout-fragment-v1",
        "slide_layout" => "mei-presentation-slide-fragment-v1",
        "map_spec" => "mei-map-spec-v1",
        "view_spec" => "mei-view-spec-v1",
        "page_instance" => "mei-projection-assembly-v1",
        "content_panel" => "mei-panel-contract-artifact-v1",
        "metric_def_bundle" => "mei-metric-def-bundle-artifact-v1",
        "navigation" | "link_decl" => "mei-navigation-artifact-v1",
        "warmup_policy" => "mei-warmup-policy-artifact-v1",
        "object_catalog" => "mei-object-catalog-v1",
        _ => "mei-graph-block-v2",
    }
}

fn derive_block_id(
    name: &str,
    source_file: &str,
    payload: &JsonValue,
) -> Result<String, LowerGraphError> {
    let obj = payload
        .as_object()
        .ok_or_else(|| LowerGraphError::Lower("payload must be object".into()))?;
    match name {
        "app_skeleton" => kw_string(obj, "id").map(|id| format!("app_skeleton:{id}")),
        "scene" | "presentation" => {
            let stage_id = kw_string(obj, "id")?;
            let key = obj
                .get("key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{stage_id}@{source_file}"));
            Ok(format!("{name}:{key}"))
        }
        "plane_layout" | "region_layout" | "section_layout" | "slide_layout" | "map_spec"
        | "view_spec" => {
            let id = kw_string(obj, "id")?;
            let key = obj
                .get("key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{id}@{source_file}"));
            Ok(format!("{name}:{key}"))
        }
        "navigation" | "link_decl" => kw_string(obj, "key").map(|key| format!("{name}:{key}")),
        "page_instance" => kw_string(obj, "key").map(|key| format!("page_instance:{key}")),
        "content_panel" => {
            if let Some(key) = obj.get("key").and_then(|value| value.as_str()) {
                return Ok(format!("content_panel:{key}"));
            }
            let id = kw_string(obj, "id")?;
            if let Some(scope) = obj.get("scope").and_then(|v| v.as_str()) {
                Ok(format!("content_panel:{scope}:{id}"))
            } else {
                Ok(format!("content_panel:{id}"))
            }
        }
        "metric_def_bundle" => kw_string(obj, "key").map(|key| format!("metric_def_bundle:{key}")),
        "object_catalog" => kw_string(obj, "id").map(|id| format!("object_catalog:{id}")),
        "world" => kw_string(obj, "id").map(|id| format!("world_model:{id}")),
        "warmup_policy" => {
            let scope = obj.get("scope").cloned().unwrap_or(JsonValue::Null);
            Ok(format!("warmup_policy:{scope}"))
        }
        other => Ok(format!("{other}:anonymous")),
    }
}

fn kw_string(obj: &Map<String, JsonValue>, key: &str) -> Result<String, LowerGraphError> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| LowerGraphError::Lower(format!("missing string field `{key}`")))
}

fn call_args_to_json(args: &CallArgs) -> Result<JsonValue, LowerGraphError> {
    let mut map = Map::new();
    for (idx, expr) in args.positional.iter().enumerate() {
        map.insert(
            format!("arg{idx}"),
            artifact_expand::expr_to_json(expr)
                .map_err(|e| LowerGraphError::Lower(e.to_string()))?,
        );
    }
    for (name, expr) in &args.keywords {
        map.insert(
            name.clone(),
            artifact_expand::expr_to_json(expr)
                .map_err(|e| LowerGraphError::Lower(e.to_string()))?,
        );
    }
    Ok(JsonValue::Object(map))
}

fn lower_object_intent(source_anchor: &str, args: &CallArgs) -> Result<JsonValue, LowerGraphError> {
    if !args.positional.is_empty() {
        return Err(diagnostic(
            "object_intent_shape_invalid",
            "object(...) accepts keyword arguments only",
            source_anchor,
        ));
    }
    reject_duplicate_object_keywords(args, source_anchor)?;
    if keyword(args, "objectId")
        .or_else(|| keyword(args, "object_id"))
        .is_some()
    {
        return Err(diagnostic(
            "object_intent_object_id_forbidden",
            "objectId is compiler-owned and cannot be authored",
            source_anchor,
        ));
    }
    for (name, _) in &args.keywords {
        if !matches!(
            name.as_str(),
            "type"
                | "source"
                | "identity"
                | "recipe"
                | "slots"
                | "relations"
                | "override"
                | "objectId"
                | "object_id"
        ) {
            return Err(diagnostic(
                "object_intent_unknown_field",
                format!("object(...) has unknown field `{name}`"),
                source_anchor,
            ));
        }
    }

    let object_type_id = required_keyword_string(
        args,
        "type",
        "object_intent_missing_type",
        "object(...) must declare a non-empty string `type`",
        source_anchor,
    )?;
    let source = lower_intent_ref(
        keyword(args, "source"),
        "source",
        &[
            "dataset_ref",
            "dataframe_ref",
            "world_ref",
            "world",
            "entity_ref",
        ],
        "object_intent_missing_source",
        "object source must be dataset_ref(...), world_ref(...), or entity_ref(...)",
        source_anchor,
    )?;
    let identity = lower_intent_ref(
        keyword(args, "identity"),
        "identity",
        &[
            "field_ref",
            "object_key",
            "objectKey",
            "entity_id",
            "entityId",
        ],
        "object_intent_missing_identity",
        "object identity must be field_ref(...), objectKey(...), or entityId(...)",
        source_anchor,
    )?;
    let recipe = lower_intent_ref(
        keyword(args, "recipe"),
        "recipe",
        &["stock_ref"],
        "object_intent_invalid_recipe",
        "object recipe must be stock_ref(\"alert\"|\"case\"|\"place\"|\"event\")",
        source_anchor,
    )?;
    let recipe_id = recipe["id"].as_str().unwrap_or_default();
    if !matches!(recipe_id, "alert" | "case" | "place" | "event") {
        return Err(diagnostic(
            "object_intent_invalid_recipe",
            format!("unknown object recipe `{recipe_id}`; expected alert, case, place, or event"),
            source_anchor,
        ));
    }

    let mut diagnostics = Vec::new();
    let slots = lower_intent_slots(
        keyword(args, "slots"),
        recipe_id,
        source_anchor,
        &mut diagnostics,
    )?;
    let relations = lower_intent_relations(keyword(args, "relations"), source_anchor)?;
    let override_props = match keyword(args, "override") {
        None => JsonValue::Null,
        Some(value @ V2Expr::Dict(_)) => artifact_expand::expr_to_json(value)
            .map_err(|error| LowerGraphError::Lower(error.to_string()))?,
        Some(_) => {
            return Err(diagnostic(
                "object_intent_override_invalid",
                "object override must be a map",
                source_anchor,
            ))
        }
    };

    let identity_kind = identity["kind"].as_str().unwrap_or_default();
    let identity_id = identity["id"].as_str().unwrap_or_default();
    let source_kind = source["kind"].as_str().unwrap_or_default();
    let source_id = source["id"].as_str().unwrap_or_default();
    let mut effective_slots = slots.clone();
    if recipe_id == "place" && identity_kind == "entity_id" && !slots.contains_key("entityId") {
        effective_slots.insert(
            "entityId".to_string(),
            projection_ref_json("slot:entityId", identity_kind, identity_id, source_anchor),
        );
    }
    let slots_fingerprint = serde_json::to_string(&effective_slots).unwrap_or_default();
    let relations_fingerprint = serde_json::to_string(&relations).unwrap_or_default();
    let override_fingerprint = serde_json::to_string(&override_props).unwrap_or_default();
    let digest = stable_object_intent_digest(&[
        source_anchor,
        object_type_id.as_str(),
        source_kind,
        source_id,
        identity_kind,
        identity_id,
        recipe_id,
        slots_fingerprint.as_str(),
        relations_fingerprint.as_str(),
        override_fingerprint.as_str(),
    ]);
    let intent_id = format!("intent_{digest}");
    let catalog_id = format!("objects_{digest}");
    let assembly_id = format!("assembly_{digest}");

    let mut owner_hints = vec![source.clone(), identity.clone(), recipe.clone()];
    owner_hints.extend(effective_slots.values().cloned());
    for refs in relations.values() {
        owner_hints.extend(refs.iter().cloned());
    }
    deduplicate_projection_refs(&mut owner_hints);

    let materialization = match source_kind {
        "dataset_ref" | "dataframe_ref" if identity_kind == "field_ref" => "dataset_row",
        _ => "declared",
    };
    let identity_contract = json!({
        "materialization": materialization,
        "fields": [identity_id],
        "locator": identity,
        "aliases": [],
        "normalization": null,
    });
    let projections = owner_hints
        .iter()
        .filter(|projection| projection["role"] != "source" && projection["role"] != "identity")
        .cloned()
        .collect::<Vec<_>>();
    let object_type = json!({
        "id": object_type_id,
        "intent_id": intent_id,
        "label": null,
        "identity": identity_contract,
        "source": source,
        "capabilities": [],
        "projections": projections,
        "source_anchor": source_anchor,
    });
    let intent = json!({
        "intent_id": intent_id,
        "object_type_id": object_type_id,
        "source": source,
        "identity": identity_contract,
        "recipe": recipe,
        "slots": slots,
        "relations": relations,
        "override": override_props,
        "owner_hints": owner_hints,
        "source_anchor": source_anchor,
    });
    let index_entry = json!({
        "kind": "internal_object_index",
        "key": format!("{object_type_id}::{source_kind}:{source_id}::{identity_kind}:{identity_id}"),
        "intent_id": intent_id,
        "object_type_id": object_type_id,
        "source": source,
        "identity": identity,
        "recipe": recipe,
        "owner_hints": owner_hints,
        "source_anchor": source_anchor,
    });
    let recipe_assembly = object_recipes::assemble(
        recipe_id,
        object_type_id.as_str(),
        intent_id.as_str(),
        source_anchor,
        &effective_slots,
        &override_props,
    )
    .map_err(|message| {
        diagnostic(
            "object_intent_identity_override_forbidden",
            message,
            source_anchor,
        )
    })?;
    diagnostics.extend(recipe_assembly.diagnostics);
    let object_field_links = derive_object_field_links_json(
        object_type_id.as_str(),
        identity_id,
        &effective_slots,
        &relations,
    );
    let default_assembly = json!({
        "kind": "default_object_assembly",
        "id": assembly_id,
        "intent_id": intent_id,
        "recipe": recipe,
        "recipe_contract": recipe_assembly.contract,
        "slots": effective_slots,
        "relations": relations,
        "object_field_links": object_field_links,
        "override": override_props,
        "effective_override": recipe_assembly.effective_override,
        "override_sources": recipe_assembly.override_sources,
        "projections": recipe_assembly.projections,
        "source_anchor": source_anchor,
    });

    Ok(json!({
        "schema_version": "mei-object-catalog-v1",
        "id": catalog_id,
        "authoring_mode": "author_intent",
        "types": [object_type],
        "refs": [],
        "intents": [intent],
        "index": [index_entry],
        "default_assemblies": [default_assembly],
        "interaction_bindings": recipe_assembly.interaction_bindings,
        "responders": recipe_assembly.responders,
        "diagnostics": diagnostics,
        "source_anchor": source_anchor,
    }))
}

fn reject_duplicate_object_keywords(
    args: &CallArgs,
    source_anchor: &str,
) -> Result<(), LowerGraphError> {
    let mut seen = BTreeSet::new();
    for (name, _) in &args.keywords {
        if !seen.insert(name.as_str()) {
            let code = if matches!(name.as_str(), "source" | "identity" | "recipe") {
                "object_intent_ambiguous_owner"
            } else {
                "object_intent_duplicate_field"
            };
            return Err(diagnostic(
                code,
                format!("object(...) field `{name}` is declared more than once"),
                source_anchor,
            ));
        }
    }
    Ok(())
}

fn lower_intent_ref(
    expr: Option<&V2Expr>,
    role: &str,
    allowed_kinds: &[&str],
    code: &'static str,
    message: &str,
    source_anchor: &str,
) -> Result<JsonValue, LowerGraphError> {
    let Some(expr) = expr else {
        return Err(diagnostic(code, message, source_anchor));
    };
    let Some((kind, args)) = thin_ref_parts(expr) else {
        return Err(diagnostic(code, message, source_anchor));
    };
    if !allowed_kinds.contains(&kind) {
        return Err(diagnostic(code, message, source_anchor));
    }
    let id = thin_ref_id(args).ok_or_else(|| diagnostic(code, message, source_anchor))?;
    let canonical_kind = match kind {
        "world" => "world_ref",
        "objectKey" => "object_key",
        "entityId" => "entity_id",
        other => other,
    };
    Ok(projection_ref_json(role, canonical_kind, id, source_anchor))
}

fn lower_intent_slots(
    expr: Option<&V2Expr>,
    recipe: &str,
    source_anchor: &str,
    diagnostics: &mut Vec<JsonValue>,
) -> Result<BTreeMap<String, JsonValue>, LowerGraphError> {
    let Some(expr) = expr else {
        return Ok(BTreeMap::new());
    };
    let V2Expr::Dict(entries) = expr else {
        return Err(diagnostic(
            "object_intent_slots_invalid",
            "object slots must be a map of slot names to thin references",
            source_anchor,
        ));
    };
    let known = object_recipes::known_slots(recipe);
    let mut slots = BTreeMap::new();
    for (name, value) in entries {
        if slots.contains_key(name) {
            return Err(diagnostic(
                "object_intent_ambiguous_owner",
                format!("object slot `{name}` has more than one owner"),
                source_anchor,
            ));
        }
        let reference = lower_intent_any_ref(
            value,
            format!("slot:{name}").as_str(),
            "object_intent_slot_ref_invalid",
            "object slot values must be thin *_ref(...) references",
            source_anchor,
        )?;
        if !known.contains(&name.as_str()) {
            diagnostics.push(json!({
                "code": "object_intent_extension_slot",
                "severity": "warning",
                "message": format!("slot `{name}` is not declared by recipe `{recipe}` and is preserved as an extension"),
                "source_anchor": source_anchor,
            }));
        }
        slots.insert(name.clone(), reference);
    }
    Ok(slots)
}

fn lower_intent_relations(
    expr: Option<&V2Expr>,
    source_anchor: &str,
) -> Result<BTreeMap<String, Vec<JsonValue>>, LowerGraphError> {
    let Some(expr) = expr else {
        return Ok(BTreeMap::new());
    };
    let V2Expr::Dict(entries) = expr else {
        return Err(diagnostic(
            "object_intent_relations_invalid",
            "object relations must be a map of relation names to thin references",
            source_anchor,
        ));
    };
    let mut relations = BTreeMap::new();
    for (name, value) in entries {
        if relations.contains_key(name) {
            return Err(diagnostic(
                "object_intent_ambiguous_owner",
                format!("object relation `{name}` is declared more than once"),
                source_anchor,
            ));
        }
        let values = match value {
            V2Expr::List(values) => values,
            value => std::slice::from_ref(value),
        };
        let refs = values
            .iter()
            .map(|value| {
                lower_intent_any_ref(
                    value,
                    format!("relation:{name}").as_str(),
                    "object_intent_relation_ref_invalid",
                    "object relation values must be thin *_ref(...) references",
                    source_anchor,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if refs.is_empty() {
            return Err(diagnostic(
                "object_intent_relation_ref_invalid",
                format!("object relation `{name}` must contain at least one thin reference"),
                source_anchor,
            ));
        }
        relations.insert(name.clone(), refs);
    }
    Ok(relations)
}

fn derive_object_field_links_json(
    object_type_id: &str,
    identity_field: &str,
    slots: &BTreeMap<String, JsonValue>,
    relations: &BTreeMap<String, Vec<JsonValue>>,
) -> BTreeMap<String, Vec<JsonValue>> {
    let mut links: BTreeMap<String, Vec<JsonValue>> = BTreeMap::new();
    let self_detail = slots
        .get("detail")
        .and_then(|slot| {
            if slot.get("kind").and_then(JsonValue::as_str) == Some("page_ref") {
                slot.get("id")
                    .and_then(JsonValue::as_str)
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_string)
            } else {
                None
            }
        });
    let self_has_detail = self_detail.is_some();
    let identity_field = identity_field.trim();
    if !identity_field.is_empty() {
        links
            .entry(identity_field.to_string())
            .or_default()
            .push(json!({
                "role": "self",
                "objectType": object_type_id,
                "resolve": "row_value",
                "sourceField": identity_field,
                "keyMode": "identity",
                "filterKey": heuristic_filter_key_json(identity_field),
                "hasDetail": self_has_detail,
                "detailPage": self_detail,
            }));
    }

    for (relation_name, refs) in relations {
        if relation_name.starts_with("objectSet.") {
            continue;
        }
        let object_ref = refs
            .iter()
            .find(|r| r.get("kind").and_then(JsonValue::as_str) == Some("object_ref"));
        let field_ref = refs
            .iter()
            .find(|r| r.get("kind").and_then(JsonValue::as_str) == Some("field_ref"));
        let mapping_ref = refs
            .iter()
            .find(|r| r.get("kind").and_then(JsonValue::as_str) == Some("mapping_ref"));
        let Some(object_ref) = object_ref else {
            continue;
        };
        let target_type = object_ref
            .get("id")
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .unwrap_or("");
        if target_type.is_empty() {
            continue;
        }

        if let Some(field_ref) = field_ref {
            let field = field_ref
                .get("id")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .unwrap_or("");
            if field.is_empty() {
                continue;
            }
            // Identity column opens self only; related objects use their own identity cells.
            if field == identity_field {
                continue;
            }
            links.entry(field.to_string()).or_default().push(json!({
                "role": "relation",
                "objectType": target_type,
                "resolve": "row_value",
                "relation": relation_name,
                "sourceField": field,
                "keyMode": "identity",
                "filterKey": heuristic_filter_key_json(field),
            }));
            continue;
        }

        if let Some(mapping_ref) = mapping_ref {
            let mapping_id = mapping_ref
                .get("id")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .unwrap_or("");
            if mapping_id.is_empty() {
                continue;
            }
            let placeholder = format!("__mapping__:{relation_name}");
            links.entry(placeholder).or_default().push(json!({
                "role": "relation",
                "objectType": target_type,
                "resolve": "mapping",
                "relation": relation_name,
                "mappingRef": mapping_id,
                "keyMode": "identity",
            }));
        }
    }
    links
}

fn heuristic_filter_key_json(field: &str) -> JsonValue {
    match field.trim() {
        "预警ID" | "warning_id" | "warningId" => json!("warningId"),
        "处理结果ID" | "result_id" | "resultId" => json!("resultId"),
        "模型ID" | "model_id" | "modelId" => json!("modelId"),
        "监督事项" | "matter" => json!("matter"),
        "问题分类名称" | "category" => json!("category"),
        _ => JsonValue::Null,
    }
}

fn lower_intent_any_ref(
    expr: &V2Expr,
    role: &str,
    code: &'static str,
    message: &str,
    source_anchor: &str,
) -> Result<JsonValue, LowerGraphError> {
    let Some((kind, args)) = thin_ref_parts(expr) else {
        return Err(diagnostic(code, message, source_anchor));
    };
    if !kind.ends_with("_ref") {
        return Err(diagnostic(code, message, source_anchor));
    }
    let id = thin_ref_id(args).ok_or_else(|| diagnostic(code, message, source_anchor))?;
    Ok(projection_ref_json(role, kind, id, source_anchor))
}

fn thin_ref_parts(expr: &V2Expr) -> Option<(&str, &CallArgs)> {
    match expr {
        V2Expr::RefCall { name, args } => Some((name.as_str(), args)),
        V2Expr::Call { path, args } if path.len() == 1 => Some((path[0].as_str(), args)),
        _ => None,
    }
}

fn deduplicate_projection_refs(refs: &mut Vec<JsonValue>) {
    let mut seen = BTreeSet::new();
    refs.retain(|reference| {
        seen.insert((
            reference["role"].as_str().unwrap_or_default().to_string(),
            reference["kind"].as_str().unwrap_or_default().to_string(),
            reference["id"].as_str().unwrap_or_default().to_string(),
        ))
    });
}

fn stable_object_intent_digest(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn lower_object_catalog(
    source_anchor: &str,
    args: &CallArgs,
) -> Result<JsonValue, LowerGraphError> {
    let id = required_keyword_string(
        args,
        "id",
        "object_catalog_missing_id",
        "object_catalog must declare a non-empty string `id`",
        source_anchor,
    )?;
    let mut seen_type_ids = BTreeSet::new();
    let mut types = Vec::new();
    if let Some(types_expr) = keyword(args, "types") {
        let V2Expr::List(type_exprs) = types_expr else {
            return Err(diagnostic(
                "object_type_shape_invalid",
                "object_catalog `types` must be a list of object_type(...) calls",
                source_anchor,
            ));
        };
        for type_expr in type_exprs {
            let object_type = lower_object_type(type_expr, source_anchor)?;
            let type_id = object_type["id"].as_str().unwrap_or_default();
            if !seen_type_ids.insert(type_id.to_string()) {
                return Err(diagnostic(
                    "object_type_duplicate_id",
                    format!("duplicate object type id `{type_id}`"),
                    source_anchor,
                ));
            }
            types.push(object_type);
        }
    }

    let mut refs = Vec::new();
    if let Some(refs_expr) = keyword(args, "refs") {
        let V2Expr::List(ref_exprs) = refs_expr else {
            return Err(diagnostic(
                "object_ref_shape_invalid",
                "object_catalog `refs` must be a list of object_ref(...) calls",
                source_anchor,
            ));
        };
        for expr in ref_exprs {
            refs.push(lower_explicit_object_ref(expr, source_anchor)?);
        }
    }

    Ok(json!({
        "schema_version": "mei-object-catalog-v1",
        "id": id,
        "authoring_mode": "legacy",
        "types": types,
        "refs": refs,
        "intents": [],
        "index": [],
        "default_assemblies": [],
        "interaction_bindings": [],
        "responders": [],
        "diagnostics": [{
            "code": "object_catalog_legacy_authoring",
            "severity": "warning",
            "message": "object_catalog(...) is legacy authoring; prefer high-level object(...) intent",
            "source_anchor": source_anchor,
        }],
        "source_anchor": source_anchor,
    }))
}

fn lower_object_type(expr: &V2Expr, source_anchor: &str) -> Result<JsonValue, LowerGraphError> {
    let V2Expr::Call { path, args } = expr else {
        return Err(diagnostic(
            "object_type_shape_invalid",
            "catalog types must contain object_type(...) calls",
            source_anchor,
        ));
    };
    if path.as_slice() != ["object_type"] {
        return Err(diagnostic(
            "object_type_shape_invalid",
            "catalog types must contain object_type(...) calls",
            source_anchor,
        ));
    }

    let id = required_keyword_string(
        args,
        "id",
        "object_type_missing_id",
        "object_type must declare a non-empty string `id`",
        source_anchor,
    )?;
    let source = lower_object_source(keyword(args, "source"), source_anchor)?;
    let materialization = match source["kind"].as_str() {
        Some("dataset_ref" | "dataframe_ref") => "dataset_row",
        _ => "declared",
    };
    let identity =
        lower_object_identity(keyword(args, "identity"), materialization, source_anchor)?;
    let label = keyword(args, "label")
        .and_then(expr_string)
        .map(str::to_string);
    let capabilities = optional_non_empty_string_list(
        keyword(args, "capabilities"),
        "object_capabilities_invalid",
        "object_type capabilities must be a list of non-empty strings",
        source_anchor,
    )?;

    let mut projections = Vec::new();
    for (role, value) in &args.keywords {
        if matches!(
            role.as_str(),
            "id" | "label" | "identity" | "source" | "capabilities"
        ) {
            continue;
        }
        collect_projection_refs(value, role, source_anchor, &mut projections)?;
    }
    if let Some(label_expr) = keyword(args, "label") {
        if !matches!(label_expr, V2Expr::String(_)) {
            collect_projection_refs(label_expr, "label", source_anchor, &mut projections)?;
        }
    }

    Ok(json!({
        "id": id,
        "label": label,
        "identity": identity,
        "source": source,
        "capabilities": capabilities,
        "projections": projections,
        "source_anchor": source_anchor,
    }))
}

fn lower_object_identity(
    identity: Option<&V2Expr>,
    materialization: &str,
    source_anchor: &str,
) -> Result<JsonValue, LowerGraphError> {
    let Some(V2Expr::Call { path, args }) = identity else {
        return Err(diagnostic(
            "object_identity_missing_fields",
            "object_type identity must be object_identity(fields = [non-empty strings])",
            source_anchor,
        ));
    };
    if path.as_slice() != ["object_identity"] {
        return Err(diagnostic(
            "object_identity_missing_fields",
            "object_type identity must be object_identity(fields = [non-empty strings])",
            source_anchor,
        ));
    }
    let fields = keyword(args, "fields")
        .and_then(|value| match value {
            V2Expr::List(items) => Some(
                items
                    .iter()
                    .filter_map(expr_string)
                    .map(str::trim)
                    .filter(|field| !field.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .filter(|fields| {
            keyword(args, "fields").and_then(|value| match value {
                V2Expr::List(items) => Some(items.len()),
                _ => None,
            }) == Some(fields.len())
                && !fields.is_empty()
        })
        .ok_or_else(|| {
            diagnostic(
                "object_identity_missing_fields",
                "object_identity requires at least one non-empty string field",
                source_anchor,
            )
        })?;
    let normalization = keyword(args, "normalization")
        .or_else(|| keyword(args, "normalize"))
        .and_then(expr_string);
    let aliases = optional_non_empty_string_list(
        keyword(args, "aliases"),
        "object_identity_aliases_invalid",
        "object_identity aliases must be a list of non-empty strings",
        source_anchor,
    )?;
    Ok(json!({
        "materialization": materialization,
        "fields": fields,
        "aliases": aliases,
        "normalization": normalization,
    }))
}

fn lower_object_source(
    source: Option<&V2Expr>,
    source_anchor: &str,
) -> Result<JsonValue, LowerGraphError> {
    let Some(source) = source else {
        return Err(diagnostic(
            "object_type_missing_source",
            "object_type must declare a thin `source = *_ref(...)`",
            source_anchor,
        ));
    };
    lower_projection_ref(
        source,
        "source",
        source_anchor,
        "object_type_source_invalid",
    )?
    .ok_or_else(|| {
        diagnostic(
            "object_type_source_invalid",
            "object_type source must be a thin `*_ref(...)` with one string id",
            source_anchor,
        )
    })
}

fn lower_explicit_object_ref(
    expr: &V2Expr,
    source_anchor: &str,
) -> Result<JsonValue, LowerGraphError> {
    let V2Expr::RefCall { name, args } = expr else {
        return Err(diagnostic(
            "object_ref_shape_invalid",
            "catalog refs must contain object_ref(\"Type.Id\") or object_ref(id = \"Type.Id\")",
            source_anchor,
        ));
    };
    if name != "object_ref" {
        return Err(diagnostic(
            "object_ref_shape_invalid",
            "catalog refs must contain object_ref(\"Type.Id\") or object_ref(id = \"Type.Id\")",
            source_anchor,
        ));
    }
    let id = thin_ref_id(args).ok_or_else(|| {
        diagnostic(
            "object_ref_shape_invalid",
            "object_ref requires exactly one non-empty string id",
            source_anchor,
        )
    })?;
    Ok(projection_ref_json("catalog", name, id, source_anchor))
}

fn collect_projection_refs(
    expr: &V2Expr,
    role: &str,
    source_anchor: &str,
    output: &mut Vec<JsonValue>,
) -> Result<(), LowerGraphError> {
    if let Some(reference) = lower_projection_ref(
        expr,
        role,
        source_anchor,
        "object_projection_ref_shape_invalid",
    )? {
        output.push(reference);
        return Ok(());
    }
    match expr {
        V2Expr::List(items) => {
            for item in items {
                collect_projection_refs(item, role, source_anchor, output)?;
            }
        }
        V2Expr::Dict(entries) => {
            for (_, value) in entries {
                collect_projection_refs(value, role, source_anchor, output)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn lower_projection_ref(
    expr: &V2Expr,
    role: &str,
    source_anchor: &str,
    invalid_code: &'static str,
) -> Result<Option<JsonValue>, LowerGraphError> {
    let (kind, args) = match expr {
        V2Expr::RefCall { name, args } => (name.as_str(), args),
        V2Expr::Call { path, args } if path.len() == 1 && path[0].ends_with("_ref") => {
            (path[0].as_str(), args)
        }
        _ => return Ok(None),
    };
    let id = thin_ref_id(args).ok_or_else(|| {
        diagnostic(
            invalid_code,
            format!("{kind} requires exactly one non-empty string id"),
            source_anchor,
        )
    })?;
    Ok(Some(projection_ref_json(role, kind, id, source_anchor)))
}

fn projection_ref_json(role: &str, kind: &str, id: &str, source_anchor: &str) -> JsonValue {
    json!({
        "role": role,
        "kind": kind,
        "id": id,
        "source_anchor": source_anchor,
    })
}

fn thin_ref_id(args: &CallArgs) -> Option<&str> {
    match (args.positional.as_slice(), args.keywords.as_slice()) {
        ([value], []) => expr_string(value)
            .map(str::trim)
            .filter(|id| !id.is_empty()),
        ([], [(key, value)]) if key == "id" => expr_string(value)
            .map(str::trim)
            .filter(|id| !id.is_empty()),
        _ => None,
    }
}

fn required_keyword_string(
    args: &CallArgs,
    key: &str,
    code: &'static str,
    message: &str,
    source_anchor: &str,
) -> Result<String, LowerGraphError> {
    keyword(args, key)
        .and_then(expr_string)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| diagnostic(code, message, source_anchor))
}

fn optional_non_empty_string_list(
    value: Option<&V2Expr>,
    code: &'static str,
    message: &str,
    source_anchor: &str,
) -> Result<Vec<String>, LowerGraphError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let V2Expr::List(items) = value else {
        return Err(diagnostic(code, message, source_anchor));
    };
    items
        .iter()
        .map(|item| {
            expr_string(item)
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
                .ok_or_else(|| diagnostic(code, message, source_anchor))
        })
        .collect()
}

fn keyword<'a>(args: &'a CallArgs, key: &str) -> Option<&'a V2Expr> {
    args.keywords
        .iter()
        .find_map(|(name, value)| (name == key).then_some(value))
}

fn expr_string(expr: &V2Expr) -> Option<&str> {
    match expr {
        V2Expr::String(value) => Some(value),
        _ => None,
    }
}

fn diagnostic(
    code: &'static str,
    message: impl Into<String>,
    source_anchor: &str,
) -> LowerGraphError {
    LowerGraphError::Diagnostic {
        code,
        message: message.into(),
        source_anchor: source_anchor.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_syntax::v2::parse_v2_source;

    #[test]
    fn lowers_presentation_and_slide_schemas() {
        let source = r#"
presentation(
    id = "intro",
    planes = [plane_ref(id = "p")],
)

slide_layout(
    id = "slide-01-cover",
    title = "Cover",
    pattern = "full_bleed",
    regions = [region_ref(id = "r-main")],
)
"#;
        let file = parse_v2_source(source).expect("parse");
        let outcome = lower_v2_file("presentation/intro/presentation.mei", &file).expect("lower");
        let presentation = outcome
            .blocks
            .iter()
            .find(|b| b.kind == "presentation")
            .expect("presentation block");
        assert_eq!(presentation.schema, "mei-presentation-semantic-v1");
        assert!(presentation.block_id.starts_with("presentation:"));
        let slide = outcome
            .blocks
            .iter()
            .find(|b| b.kind == "slide_layout")
            .expect("slide_layout block");
        assert_eq!(slide.schema, "mei-presentation-slide-fragment-v1");
        assert!(slide.block_id.starts_with("slide_layout:"));
    }

    #[test]
    fn rejects_unknown_slide_pattern() {
        let source = r#"
slide_layout(
    id = "slide-bad",
    pattern = "two_columns",
    regions = [region_ref(id = "r-main")],
)
"#;
        let file = parse_v2_source(source).expect("parse");
        let err = lower_v2_file("p/slide-bad.mei", &file).expect_err("unknown pattern");
        let msg = err.to_string();
        assert!(
            msg.contains("unknown pattern") && msg.contains("two_columns"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn lowers_object_catalog_to_structured_row_free_payload() {
        let source = r#"
object_catalog(
    id = "warning_objects",
    types = [
        object_type(
            id = "zhifa.Warning",
            label = "Warning",
            identity = object_identity(
                fields = ["warning_id"],
                aliases = ["warningId", "legacy_warning_id"],
                normalization = "trim",
            ),
            source = dataset_ref("warning_rows"),
            capabilities = ["select", "explain"],
            label_field = field_ref("title"),
            metrics = [metric_ref("warning_metrics::detail")],
            relations = [object_ref("zhifa.Entity")],
            mirrors = [scene_ref("home::warning_detail")],
        ),
    ],
    refs = [object_ref("thunder.StormEvent")],
)
"#;
        let file = parse_v2_source(source).expect("parse");
        let outcome =
            lower_v2_file("domain/warnings.objects.mei", &file).expect("lower object catalog");
        let block = &outcome.blocks[0];
        assert_eq!(block.kind, "object_catalog");
        assert_eq!(block.block_id, "object_catalog:warning_objects");
        assert_eq!(block.schema, "mei-object-catalog-v1");
        assert_eq!(
            block.payload["source_anchor"],
            "domain/warnings.objects.mei"
        );
        assert_eq!(block.payload["types"][0]["id"], "zhifa.Warning");
        assert_eq!(
            block.payload["types"][0]["identity"]["fields"][0],
            "warning_id"
        );
        assert_eq!(
            block.payload["types"][0]["identity"]["materialization"],
            "dataset_row"
        );
        assert_eq!(
            block.payload["types"][0]["identity"]["aliases"][1],
            "legacy_warning_id"
        );
        assert_eq!(
            block.payload["types"][0]["capabilities"],
            json!(["select", "explain"])
        );
        assert_eq!(block.payload["types"][0]["source"]["kind"], "dataset_ref");
        assert_eq!(
            block.payload["types"][0]["projections"][0]["kind"],
            "field_ref"
        );
        assert_eq!(block.payload["refs"][0]["kind"], "object_ref");
        assert_eq!(
            block.payload["types"][0]["projections"][1]["id"],
            "warning_metrics::detail"
        );
        assert_eq!(
            block.payload["types"][0]["projections"][1]["role"],
            "metrics"
        );
        assert_eq!(
            block.payload["types"][0]["projections"][3]["role"],
            "mirrors"
        );
        assert!(block.payload["types"][0].get("rows").is_none());
        assert!(block.payload["types"][0].get("schema").is_none());
    }

    #[test]
    fn diagnoses_missing_catalog_and_type_contract_fields() {
        let missing_catalog =
            parse_v2_source(r#"object_catalog(types = [])"#).expect("parse missing catalog id");
        let error = lower_v2_file("missing.objects.mei", &missing_catalog)
            .expect_err("catalog id must be required");
        assert_eq!(error.code(), Some("object_catalog_missing_id"));
        assert_eq!(error.source_anchor(), Some("missing.objects.mei"));

        let missing_type_id = parse_v2_source(
            r#"
object_catalog(
    id = "objects",
    types = [
        object_type(
            identity = object_identity(fields = ["id"]),
            source = dataset_ref("rows"),
        ),
    ],
)
"#,
        )
        .expect("parse missing type id");
        let error = lower_v2_file("missing.objects.mei", &missing_type_id)
            .expect_err("type id must be required");
        assert_eq!(error.code(), Some("object_type_missing_id"));

        let missing_identity = parse_v2_source(
            r#"
object_catalog(
    id = "objects",
    types = [
        object_type(id = "demo.Type", source = dataset_ref("rows")),
    ],
)
"#,
        )
        .expect("parse missing identity");
        let error = lower_v2_file("missing.objects.mei", &missing_identity)
            .expect_err("identity fields must be required");
        assert_eq!(error.code(), Some("object_identity_missing_fields"));

        let missing_source = parse_v2_source(
            r#"
object_catalog(
    id = "objects",
    types = [
        object_type(
            id = "demo.Type",
            identity = object_identity(fields = ["id"]),
        ),
    ],
)
"#,
        )
        .expect("parse missing source");
        let error = lower_v2_file("missing.objects.mei", &missing_source)
            .expect_err("source must be required");
        assert_eq!(error.code(), Some("object_type_missing_source"));
    }

    #[test]
    fn diagnoses_duplicate_type_and_malformed_object_ref() {
        let duplicate = parse_v2_source(
            r#"
object_catalog(
    id = "objects",
    types = [
        object_type(
            id = "demo.Type",
            identity = object_identity(fields = ["id"]),
            source = dataset_ref("rows"),
        ),
        object_type(
            id = "demo.Type",
            identity = object_identity(fields = ["other_id"]),
            source = dataset_ref("other_rows"),
        ),
    ],
)
"#,
        )
        .expect("parse duplicate");
        let error = lower_v2_file("duplicate.objects.mei", &duplicate)
            .expect_err("duplicate type must fail");
        assert_eq!(error.code(), Some("object_type_duplicate_id"));

        let malformed_ref = parse_v2_source(
            r#"
object_catalog(
    id = "objects",
    refs = [object_ref(id = "demo.Type", alias = "bad")],
)
"#,
        )
        .expect("parse malformed ref");
        let error = lower_v2_file("bad-ref.objects.mei", &malformed_ref)
            .expect_err("malformed object_ref must fail");
        assert_eq!(error.code(), Some("object_ref_shape_invalid"));
    }

    #[test]
    fn diagnoses_invalid_aliases_capabilities_and_ref_bundle_kwargs() {
        let invalid_aliases = parse_v2_source(
            r#"
object_catalog(
    id = "objects",
    types = [
        object_type(
            id = "demo.Type",
            identity = object_identity(fields = ["id"], aliases = [""]),
            source = dataset_ref("rows"),
        ),
    ],
)
"#,
        )
        .expect("parse invalid aliases");
        let error = lower_v2_file("bad-alias.objects.mei", &invalid_aliases)
            .expect_err("empty alias must fail");
        assert_eq!(error.code(), Some("object_identity_aliases_invalid"));

        let invalid_capabilities = parse_v2_source(
            r#"
object_catalog(
    id = "objects",
    types = [
        object_type(
            id = "demo.Type",
            identity = object_identity(fields = ["id"]),
            source = dataset_ref("rows"),
            capabilities = "select",
        ),
    ],
)
"#,
        )
        .expect("parse invalid capabilities");
        let error = lower_v2_file("bad-capability.objects.mei", &invalid_capabilities)
            .expect_err("capabilities scalar must fail");
        assert_eq!(error.code(), Some("object_capabilities_invalid"));

        let bundled_ref = parse_v2_source(
            r#"
object_catalog(
    id = "objects",
    types = [
        object_type(
            id = "demo.Type",
            identity = object_identity(fields = ["id"]),
            source = dataset_ref("rows"),
            metrics = [metric_ref(id = "detail", bundle = "metrics")],
        ),
    ],
)
"#,
        )
        .expect("parse bundled ref");
        let error = lower_v2_file("bundled-ref.objects.mei", &bundled_ref)
            .expect_err("thin ref must reject bundle kwargs");
        assert_eq!(error.code(), Some("object_projection_ref_shape_invalid"));
    }

    #[test]
    fn diagnoses_missing_object_intent_fields_invalid_recipe_and_authored_object_id() {
        for (source, code) in [
            (
                r#"object(source = dataset_ref("rows"), identity = field_ref("id"), recipe = stock_ref("alert"))"#,
                "object_intent_missing_type",
            ),
            (
                r#"object(type = "demo.Type", identity = field_ref("id"), recipe = stock_ref("alert"))"#,
                "object_intent_missing_source",
            ),
            (
                r#"object(type = "demo.Type", source = dataset_ref("rows"), recipe = stock_ref("alert"))"#,
                "object_intent_missing_identity",
            ),
            (
                r#"object(type = "demo.Type", source = dataset_ref("rows"), identity = field_ref("id"), recipe = stock_ref("dashboard"))"#,
                "object_intent_invalid_recipe",
            ),
            (
                r#"object(type = "demo.Type", source = dataset_ref("rows"), identity = field_ref("id"), recipe = stock_ref("alert"), objectId = "manual")"#,
                "object_intent_object_id_forbidden",
            ),
        ] {
            let file = parse_v2_source(source).expect("parse object intent diagnostic fixture");
            let error = lower_v2_file("domain/object.objects.mei", &file)
                .expect_err("invalid object intent must fail");
            assert_eq!(error.code(), Some(code), "unexpected error: {error}");
        }
    }

    #[test]
    fn diagnoses_ambiguous_slot_owner_and_preserves_known_extension_slot() {
        let ambiguous = parse_v2_source(
            r#"
object(
    type = "demo.Alert",
    source = dataset_ref("rows"),
    identity = field_ref("id"),
    recipe = stock_ref("alert"),
    slots = {"summary": field_ref("title"), "summary": field_ref("other_title")},
)
"#,
        )
        .expect("parse ambiguous slot");
        let error = lower_v2_file("domain/alerts.objects.mei", &ambiguous)
            .expect_err("ambiguous slot owner must fail");
        assert_eq!(error.code(), Some("object_intent_ambiguous_owner"));

        let extension = parse_v2_source(
            r#"
object(
    type = "demo.Alert",
    source = dataset_ref("rows"),
    identity = field_ref("id"),
    recipe = stock_ref("alert"),
    slots = {"custom": field_ref("custom")},
)
"#,
        )
        .expect("parse extension slot");
        let outcome =
            lower_v2_file("domain/alerts.objects.mei", &extension).expect("lower extension slot");
        assert_eq!(
            outcome.blocks[0].payload["diagnostics"][0]["code"],
            "object_intent_extension_slot"
        );
        assert_eq!(
            outcome.blocks[0].payload["intents"][0]["slots"]["custom"]["id"],
            "custom"
        );
        assert_eq!(
            outcome.blocks[0].payload["intents"][0]["recipe"]["id"],
            "alert"
        );
    }

    #[test]
    fn lowers_supported_source_identity_and_recipe_variants() {
        for (source, identity, recipe, source_kind, identity_kind) in [
            (
                r#"dataset_ref("rows")"#,
                r#"field_ref("row_id")"#,
                "alert",
                "dataset_ref",
                "field_ref",
            ),
            (
                r#"world_ref("city")"#,
                r#"objectKey("place-key")"#,
                "place",
                "world_ref",
                "object_key",
            ),
            (
                r#"entity_ref("events")"#,
                r#"entityId("event-id")"#,
                "event",
                "entity_ref",
                "entity_id",
            ),
            (
                r#"entity_ref("cases")"#,
                r#"entityId("case-id")"#,
                "case",
                "entity_ref",
                "entity_id",
            ),
        ] {
            let source = format!(
                r#"
object(
    type = "demo.Type",
    source = {source},
    identity = {identity},
    recipe = stock_ref("{recipe}"),
)
"#
            );
            let file = parse_v2_source(&source).expect("parse supported object intent");
            let outcome =
                lower_v2_file("domain/objects.mei", &file).expect("lower supported object intent");
            let payload = &outcome.blocks[0].payload;
            assert_eq!(payload["intents"][0]["source"]["kind"], source_kind);
            assert_eq!(
                payload["intents"][0]["identity"]["locator"]["kind"],
                identity_kind
            );
            assert_eq!(payload["intents"][0]["recipe"]["id"], recipe);
        }
    }

    #[test]
    fn keeps_legacy_top_level_lowering_compatible() {
        let file = parse_v2_source(
            r#"
content_panel(id = "legacy", chrome = "bare", blocks = [])
"#,
        )
        .expect("parse old input");
        let outcome = lower_v2_file("scene/legacy.mei", &file).expect("lower old input");
        assert_eq!(outcome.blocks[0].kind, "content_panel");
        assert_eq!(outcome.blocks[0].schema, "mei-panel-contract-artifact-v1");
    }
}
