//! Enrich DefaultObjectAssembly.object_field_links with mapping sidecars and
//! detail page open stubs so browsers can render field-level object links.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mei_lang_kernel::{
    ObjectCatalog, ObjectFieldLinkKeyMode, ObjectFieldLinkResolve, ObjectFieldLinkTarget,
};
use serde_json::{json, Value};

use crate::import::load_block_artifact;
use crate::mcg::registry::McgRegistry;
use crate::types::GraphNodeKind;

/// Rewrite placeholder mapping columns and fill targetsByValue / detail open stubs.
pub fn enrich_object_catalogs_field_links(
    app_root: &Path,
    registry: &McgRegistry,
    catalogs: &mut [ObjectCatalog],
) {
    let detail_by_type = collect_detail_pages(catalogs);
    let mut mapping_cache: BTreeMap<String, Value> = BTreeMap::new();

    for catalog in catalogs.iter_mut() {
        for assembly in &mut catalog.default_assemblies {
            let intent = catalog
                .intents
                .iter()
                .find(|intent| intent.intent_id == assembly.intent_id);
            let identity_fields = intent
                .map(|intent| intent.identity.fields.clone())
                .unwrap_or_default();

            if assembly.object_field_links.is_empty() {
                if let Some(intent) = intent {
                    assembly.object_field_links = mei_lang_kernel::derive_object_field_links(
                        intent.object_type_id.as_str(),
                        &intent.identity.fields,
                        &intent.slots,
                        &intent.relations,
                    );
                }
            }

            let mut rewritten: BTreeMap<String, Vec<ObjectFieldLinkTarget>> = BTreeMap::new();
            let pending = std::mem::take(&mut assembly.object_field_links);
            for (column, targets) in pending {
                for mut target in targets {
                    // Drop stale CAS relation targets that reused the object's own identity
                    // column (e.g. Warning.预警ID → IssueResult chooser).
                    if target.role == "relation"
                        && target.resolve == ObjectFieldLinkResolve::RowValue
                        && identity_fields.iter().any(|field| {
                            field.trim()
                                == target
                                    .source_field
                                    .as_deref()
                                    .unwrap_or(column.as_str())
                                    .trim()
                        })
                    {
                        continue;
                    }
                    if target.resolve == ObjectFieldLinkResolve::Mapping {
                        if let Some(mapping_ref) = target.mapping_ref.clone() {
                            let mapping =
                                mapping_cache.entry(mapping_ref.clone()).or_insert_with(|| {
                                    load_mapping_document(app_root, mapping_ref.as_str())
                                        .unwrap_or(Value::Null)
                                });
                            apply_mapping_to_target(&mut target, mapping);
                        }
                    }

                    if target.has_detail.is_none() {
                        if let Some(detail) = detail_by_type.get(target.object_type.as_str()) {
                            target.has_detail = Some(true);
                            target.detail_page = Some(detail.clone());
                        } else {
                            target.has_detail = Some(false);
                        }
                    }
                    if target.detail_page.is_none() {
                        if let Some(detail) = detail_by_type.get(target.object_type.as_str()) {
                            target.detail_page = Some(detail.clone());
                            target.has_detail = Some(true);
                        }
                    }
                    if target.open_popup.is_none() {
                        if let Some(page_key) = target.detail_page.clone() {
                            target.open_popup =
                                resolve_page_open_popup(app_root, registry, page_key.as_str());
                        }
                    }
                    if target.filter_key.is_none() {
                        let field = target
                            .key_field
                            .as_deref()
                            .filter(|value| !value.trim().is_empty())
                            .or_else(|| target.source_field.as_deref());
                        if let Some(field) = field {
                            target.filter_key = heuristic_filter_key(field);
                        }
                    }
                    if target.key_mode == ObjectFieldLinkKeyMode::ForeignKey
                        && target.filter_key.is_none()
                    {
                        if let Some(field) = identity_fields.first() {
                            target.filter_key = heuristic_filter_key(field);
                        }
                    }

                    let column_key = target
                        .source_field
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| column.clone());
                    if column_key.starts_with("__mapping__:") {
                        continue;
                    }
                    rewritten.entry(column_key).or_default().push(target);
                }
            }
            assembly.object_field_links = rewritten;
        }
    }
}

/// Flatten assemblies into objectType → field links for presentation_map.
pub fn collect_object_field_links_by_type(
    catalogs: &[ObjectCatalog],
) -> BTreeMap<String, BTreeMap<String, Vec<ObjectFieldLinkTarget>>> {
    let mut out: BTreeMap<String, BTreeMap<String, Vec<ObjectFieldLinkTarget>>> = BTreeMap::new();
    for catalog in catalogs {
        for assembly in &catalog.default_assemblies {
            let Some(intent) = catalog
                .intents
                .iter()
                .find(|intent| intent.intent_id == assembly.intent_id)
            else {
                continue;
            };
            if assembly.object_field_links.is_empty() {
                continue;
            }
            out.insert(
                intent.object_type_id.clone(),
                assembly.object_field_links.clone(),
            );
        }
    }
    out
}

fn collect_detail_pages(catalogs: &[ObjectCatalog]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for catalog in catalogs {
        for intent in &catalog.intents {
            if let Some(detail) = intent.slots.get("detail") {
                if detail.kind == "page_ref" && !detail.id.trim().is_empty() {
                    out.insert(intent.object_type_id.clone(), detail.id.clone());
                }
            }
        }
        for object_type in &catalog.types {
            if out.contains_key(&object_type.id) {
                continue;
            }
            if let Some(detail) = object_type.projections.iter().find(|p| p.role == "detail") {
                if detail.kind == "page_ref" && !detail.id.trim().is_empty() {
                    out.insert(object_type.id.clone(), detail.id.clone());
                }
            }
        }
    }
    out
}

fn apply_mapping_to_target(target: &mut ObjectFieldLinkTarget, mapping_doc: &Value) {
    let relation = target.relation.as_deref().unwrap_or("");
    let relations = mapping_doc
        .get("relations")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let entry = relations.get(relation).cloned().or_else(|| {
        relations
            .values()
            .find(|value| {
                value
                    .get("relation")
                    .and_then(Value::as_str)
                    .map(|text| text.contains(target.object_type.as_str()))
                    .unwrap_or(false)
            })
            .cloned()
    });
    let Some(entry) = entry else {
        return;
    };
    if let Some(source_field) = entry
        .get("sourceField")
        .or_else(|| entry.get("source_field"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        target.source_field = Some(source_field.to_string());
    }
    // filterKey 应对准目标对象身份/过滤字段，不能落到 sourceField（如预警模型→category）。
    if target.filter_key.is_none() {
        if let Some(identity_field) = entry
            .get("targetIdentityField")
            .or_else(|| entry.get("target_identity_field"))
            .or_else(|| entry.get("targetField"))
            .or_else(|| entry.get("target_field"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            target.filter_key = heuristic_filter_key(identity_field);
        }
    }
    let mut targets_by_value = BTreeMap::new();
    if let Some(explicit) = entry
        .get("explicitMappings")
        .or_else(|| entry.get("explicit_mappings"))
        .and_then(Value::as_object)
    {
        for (key, value) in explicit {
            targets_by_value.insert(key.clone(), value.clone());
        }
    }
    target.targets_by_value = targets_by_value;
    if let Some(fields) = entry
        .get("qualifierFields")
        .or_else(|| entry.get("qualifier_fields"))
        .and_then(Value::as_array)
    {
        target.qualifier_fields = fields
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
    }
}

fn load_mapping_document(app_root: &Path, mapping_ref: &str) -> Option<Value> {
    let relative = mapping_ref.trim().trim_start_matches('/');
    if relative.is_empty() {
        return None;
    }
    let candidates = [
        app_root.join("src").join(relative),
        app_root.join(relative),
        PathBuf::from(relative),
    ];
    for path in candidates {
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                return Some(value);
            }
        }
    }
    None
}

fn resolve_page_open_popup(
    app_root: &Path,
    registry: &McgRegistry,
    page_key: &str,
) -> Option<Value> {
    let node = registry
        .nodes
        .iter()
        .find(|node| node.id.kind == GraphNodeKind::PageInstance && node.id.key == page_key)?;
    let pref = node.payload_ref.as_ref()?;
    let artifact = load_block_artifact(app_root, pref).ok()??;
    let payload = artifact.get("payload")?;
    let scene_id = payload
        .get("scene")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let scene_file = page_source_file_from_payload(payload)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{page_key}.mei"));
    let examples = payload.get("examples").and_then(Value::as_array);
    let params = examples
        .and_then(|items| items.first())
        .and_then(|item| item.get("params"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    Some(json!({
        "kind": "scene_open",
        "mode": "popup",
        "type": "popup",
        "projection": "overlay",
        "overlay_size": "large",
        "scene_id": scene_id,
        "scene_file": scene_file,
        "page_scene_id": scene_id,
        "page_scene_file": scene_file,
        "params": params,
        "context": { "params": params },
    }))
}

fn page_source_file_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("source_file")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|source_file| {
            if source_file.starts_with("src/") {
                source_file.to_string()
            } else {
                format!("src/{source_file}")
            }
        })
}

fn heuristic_filter_key(field: &str) -> Option<String> {
    match field.trim() {
        "预警ID" | "关联预警ID" | "warning_id" | "warningId" => Some("warningId".to_string()),
        "处理结果ID" | "result_id" | "resultId" => Some("resultId".to_string()),
        "模型ID" | "model_id" | "modelId" => Some("modelId".to_string()),
        "序号" | "matterId" | "matter_id" => Some("matterId".to_string()),
        "监督事项" | "风险事项" | "matter" => Some("matter".to_string()),
        "问题分类名称" | "预警模型" | "category" => Some("category".to_string()),
        "机制名称" | "健全机制" | "mechanismName" | "mechanism_name" => {
            Some("mechanismName".to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{derive_object_field_links, ObjectProjectionRef};

    #[test]
    fn derive_keeps_multi_value_mapping_placeholder_until_enrichment() {
        let mut slots = BTreeMap::new();
        slots.insert(
            "detail".to_string(),
            ObjectProjectionRef {
                role: "slot:detail".to_string(),
                kind: "page_ref".to_string(),
                id: "zhifa/home/plane-warning-detail".to_string(),
                source_anchor: "warning.objects.mei".to_string(),
            },
        );
        let mut relations = BTreeMap::new();
        relations.insert(
            "alertModel.reference.byCategoryName".to_string(),
            vec![
                ObjectProjectionRef {
                    role: "relation:alertModel.reference.byCategoryName".to_string(),
                    kind: "object_ref".to_string(),
                    id: "zhifa.AlertModel".to_string(),
                    source_anchor: "warning.objects.mei".to_string(),
                },
                ObjectProjectionRef {
                    role: "relation:alertModel.reference.byCategoryName".to_string(),
                    kind: "mapping_ref".to_string(),
                    id: "relations/category-object-relations.mapping.json".to_string(),
                    source_anchor: "warning.objects.mei".to_string(),
                },
            ],
        );
        let links =
            derive_object_field_links("zhifa.Warning", &["预警ID".to_string()], &slots, &relations);
        assert!(links.contains_key("预警ID"));
        assert!(links.keys().any(|key| key.starts_with("__mapping__:")));
        let self_link = &links["预警ID"][0];
        assert_eq!(self_link.role, "self");
        assert_eq!(self_link.has_detail, Some(true));
    }

    #[test]
    fn apply_mapping_inlines_all_alert_model_ids() {
        let mut target = ObjectFieldLinkTarget {
            role: "relation".to_string(),
            object_type: "zhifa.AlertModel".to_string(),
            resolve: ObjectFieldLinkResolve::Mapping,
            relation: Some("alertModel.reference.byCategoryName".to_string()),
            source_field: None,
            key_field: None,
            mapping_ref: Some("relations/x.json".to_string()),
            targets_by_value: BTreeMap::new(),
            key_mode: ObjectFieldLinkKeyMode::Identity,
            filter_key: None,
            has_detail: None,
            detail_page: None,
            open_popup: None,
            qualifier_fields: Vec::new(),
        };
        let mapping = json!({
            "relations": {
                "alertModel.reference.byCategoryName": {
                    "sourceField": "问题分类名称",
                    "targetIdentityField": "模型ID",
                    "qualifierFields": ["预警等级", "规则类型"],
                    "explicitMappings": {
                        "行政检查频次过高|蓝|频次类监督": "2025006",
                        "行政检查频次过高|蓝|基准值比较": "2025007"
                    }
                }
            }
        });
        apply_mapping_to_target(&mut target, &mapping);
        assert_eq!(target.source_field.as_deref(), Some("问题分类名称"));
        assert_eq!(
            target.qualifier_fields,
            vec!["预警等级".to_string(), "规则类型".to_string()]
        );
        assert_eq!(
            target.targets_by_value.get("行政检查频次过高|蓝|频次类监督"),
            Some(&json!("2025006"))
        );
    }
}
