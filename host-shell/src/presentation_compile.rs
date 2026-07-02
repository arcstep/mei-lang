use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use mei_lang_kernel::{
    catalog_scene_routes_from_app_root, compile_app_from_root, resolve_app_root, PanelDecl,
    UiNodeDecl,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub struct PresentationCompileRequest {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub source: String,
    #[serde(rename = "sceneId")]
    pub scene_id: Option<String>,
    #[serde(rename = "presentationId")]
    pub presentation_id: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PresentationCompileDiagnostic {
    pub level: String,
    pub code: String,
    pub message: String,
    #[serde(rename = "stepId", skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(rename = "refKind", skip_serializing_if = "Option::is_none")]
    pub ref_kind: Option<String>,
    #[serde(rename = "refId", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
}

#[derive(Default)]
struct PresentationSurfaceIndex {
    viewpoints: BTreeMap<String, PresentationViewpointEntry>,
    pages: BTreeSet<String>,
    metrics: BTreeSet<String>,
    world_stages: Vec<WorldStageContract>,
    diagnostics: Vec<PresentationCompileDiagnostic>,
    warnings: Vec<PresentationCompileDiagnostic>,
}

#[derive(Debug, Clone, Default)]
struct PresentationViewpointEntry {
    panel_id: Option<String>,
    view_family: Option<String>,
    stage_kind: Option<String>,
    world_ref: Option<String>,
    entity_id: Option<String>,
    group_id: Option<String>,
    camera_preset: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct WorldStageContract {
    panel_id: String,
    block_id: Option<String>,
    view_family: Option<String>,
    stage_kind: Option<String>,
    world_ref: Option<String>,
    entity_id: Option<String>,
    group_id: Option<String>,
    camera_preset: Option<String>,
    targets: WorldTargetsIndex,
}

#[derive(Debug, Clone, Default)]
struct WorldTargetsIndex {
    anchors: BTreeSet<String>,
    camera_presets: BTreeSet<String>,
    entities: BTreeSet<String>,
    groups: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct ResolvedWorldActionTarget {
    viewpoint_id: Option<String>,
    panel_id: Option<String>,
    view_family: Option<String>,
    stage_kind: Option<String>,
    world_ref: Option<String>,
    entity_id: Option<String>,
    group_id: Option<String>,
    camera_preset: Option<String>,
}

impl ResolvedWorldActionTarget {
    fn has_host_hints(&self) -> bool {
        self.panel_id.is_some()
            || self.view_family.is_some()
            || self.stage_kind.is_some()
            || self.world_ref.is_some()
    }

    fn has_contract_refs(&self) -> bool {
        self.entity_id.is_some() || self.group_id.is_some() || self.camera_preset.is_some()
    }
}

fn read_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn read_string_from_map(obj: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(read_string))
}

fn read_string_from_value(value: &Value, keys: &[&str]) -> Option<String> {
    value
        .as_object()
        .and_then(|obj| read_string_from_map(obj, keys))
}

fn read_object_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Map<String, Value>> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_object))
}

fn describe_stage_contract(stage: &WorldStageContract) -> String {
    match stage.block_id.as_deref() {
        Some(block_id) if !block_id.is_empty() => {
            format!("panel `{}` / block `{block_id}`", stage.panel_id)
        }
        _ => format!("panel `{}`", stage.panel_id),
    }
}

fn world_contract_diagnostic(
    code: &str,
    stage: &WorldStageContract,
    message: impl Into<String>,
    ref_kind: Option<&str>,
    ref_id: Option<&str>,
) -> PresentationCompileDiagnostic {
    diagnostic(
        code,
        format!("{}（{}）", message.into(), describe_stage_contract(stage)),
        None,
        ref_kind,
        ref_id,
    )
}

fn world_contract_warning(
    code: &str,
    stage: &WorldStageContract,
    message: impl Into<String>,
) -> PresentationCompileDiagnostic {
    PresentationCompileDiagnostic {
        level: "warn".to_string(),
        code: code.to_string(),
        message: format!("{}（{}）", message.into(), describe_stage_contract(stage)),
        step_id: None,
        ref_kind: None,
        ref_id: None,
    }
}

fn validate_string_array_field(
    stage: &WorldStageContract,
    owner_kind: &str,
    owner_id: &str,
    field_name: &str,
    value: Option<&Value>,
    diagnostics: &mut Vec<PresentationCompileDiagnostic>,
) -> bool {
    let Some(value) = value else {
        return false;
    };
    let Some(items) = value.as_array() else {
        diagnostics.push(world_contract_diagnostic(
            "world_targets_invalid_string_array",
            stage,
            format!("`worldTargets.{owner_kind}.{owner_id}.{field_name}` 必须是字符串数组"),
            Some(field_name),
            Some(owner_id),
        ));
        return false;
    };
    let all_strings = items
        .iter()
        .all(|item| item.as_str().map(str::trim).is_some_and(|value| !value.is_empty()));
    if !all_strings {
        diagnostics.push(world_contract_diagnostic(
            "world_targets_invalid_string_array",
            stage,
            format!("`worldTargets.{owner_kind}.{owner_id}.{field_name}` 必须是非空字符串数组"),
            Some(field_name),
            Some(owner_id),
        ));
    }
    all_strings
}

fn validate_number_field(
    stage: &WorldStageContract,
    owner_kind: &str,
    owner_id: &str,
    field_name: &str,
    value: Option<&Value>,
    diagnostics: &mut Vec<PresentationCompileDiagnostic>,
) -> bool {
    let Some(value) = value else {
        return false;
    };
    if value.as_f64().is_some() {
        return true;
    }
    diagnostics.push(world_contract_diagnostic(
        "world_targets_invalid_number",
        stage,
        format!("`worldTargets.{owner_kind}.{owner_id}.{field_name}` 必须是数字"),
        Some(field_name),
        Some(owner_id),
    ));
    false
}

fn is_coordinate_pair(value: &Value) -> bool {
    value.as_array().is_some_and(|items| {
        items.len() == 2 && items.iter().all(|item| item.as_f64().is_some())
    })
}

fn validate_center_field(
    stage: &WorldStageContract,
    owner_kind: &str,
    owner_id: &str,
    value: Option<&Value>,
    diagnostics: &mut Vec<PresentationCompileDiagnostic>,
) -> bool {
    let Some(value) = value else {
        return false;
    };
    if is_coordinate_pair(value) {
        return true;
    }
    diagnostics.push(world_contract_diagnostic(
        "world_targets_invalid_center",
        stage,
        format!("`worldTargets.{owner_kind}.{owner_id}.center` 必须是 `[lng, lat]`"),
        Some("center"),
        Some(owner_id),
    ));
    false
}

fn validate_bounds_field(
    stage: &WorldStageContract,
    owner_kind: &str,
    owner_id: &str,
    value: Option<&Value>,
    diagnostics: &mut Vec<PresentationCompileDiagnostic>,
) -> bool {
    let Some(value) = value else {
        return false;
    };
    let valid = value.as_array().is_some_and(|items| {
        items.len() == 2 && items.iter().all(is_coordinate_pair)
    });
    if valid {
        return true;
    }
    diagnostics.push(world_contract_diagnostic(
        "world_targets_invalid_bounds",
        stage,
        format!("`worldTargets.{owner_kind}.{owner_id}.bounds` 必须是 `[[lng, lat], [lng, lat]]`"),
        Some("bounds"),
        Some(owner_id),
    ));
    false
}

fn validate_world_targets_config(
    stage: &WorldStageContract,
    value: &Value,
    diagnostics: &mut Vec<PresentationCompileDiagnostic>,
) -> WorldTargetsIndex {
    let Some(_config) = value.as_object() else {
        diagnostics.push(world_contract_diagnostic(
            "world_targets_invalid",
            stage,
            "`worldTargets` 必须是对象",
            Some("worldTargets"),
            stage.block_id.as_deref().or(Some(stage.panel_id.as_str())),
        ));
        return WorldTargetsIndex::default();
    };

    let mut index = WorldTargetsIndex::default();

    if let Some(anchors) = read_object_field(value, &["anchors"]) {
        for (anchor_id, anchor_value) in anchors {
            let Some(anchor_obj) = anchor_value.as_object() else {
                diagnostics.push(world_contract_diagnostic(
                    "world_targets_invalid_anchor",
                    stage,
                    format!("`worldTargets.anchors.{anchor_id}` 必须是对象"),
                    Some("anchor"),
                    Some(anchor_id),
                ));
                continue;
            };
            let x = anchor_obj.get("x").and_then(Value::as_f64);
            let y = anchor_obj.get("y").and_then(Value::as_f64);
            if x.is_none() || y.is_none() {
                diagnostics.push(world_contract_diagnostic(
                    "world_targets_invalid_anchor",
                    stage,
                    format!("`worldTargets.anchors.{anchor_id}` 必须包含数字 `x` 与 `y`"),
                    Some("anchor"),
                    Some(anchor_id),
                ));
                continue;
            }
            index.anchors.insert(anchor_id.clone());
        }
    }

    if let Some(groups) = read_object_field(value, &["groups"]) {
        index.groups.extend(groups.keys().cloned());
    }
    if let Some(camera_presets) = read_object_field(value, &["cameraPresets", "camera_presets"]) {
        index.camera_presets.extend(camera_presets.keys().cloned());
    }
    if let Some(entities) = read_object_field(value, &["entities"]) {
        index.entities.extend(entities.keys().cloned());
    }

    if let Some(groups) = read_object_field(value, &["groups"]) {
        for (group_id, group_value) in groups {
            let Some(group_obj) = group_value.as_object() else {
                diagnostics.push(world_contract_diagnostic(
                    "world_targets_invalid_group",
                    stage,
                    format!("`worldTargets.groups.{group_id}` 必须是对象"),
                    Some("group"),
                    Some(group_id),
                ));
                continue;
            };
            let mut has_known_fields = false;
            for field_name in ["shapeIds", "hotspotIds", "layerIds", "layers"] {
                if group_obj.contains_key(field_name) {
                    has_known_fields = true;
                }
                validate_string_array_field(
                    stage,
                    "groups",
                    group_id,
                    field_name,
                    group_obj.get(field_name),
                    diagnostics,
                );
            }
            if !has_known_fields {
                diagnostics.push(world_contract_diagnostic(
                    "world_targets_group_empty",
                    stage,
                    format!(
                        "`worldTargets.groups.{group_id}` 至少应声明 `shapeIds` / `hotspotIds` / `layerIds` / `layers` 之一"
                    ),
                    Some("group"),
                    Some(group_id),
                ));
            }
        }
    }

    if let Some(camera_presets) = read_object_field(value, &["cameraPresets", "camera_presets"]) {
        for (preset_id, preset_value) in camera_presets {
            let Some(preset_obj) = preset_value.as_object() else {
                diagnostics.push(world_contract_diagnostic(
                    "world_targets_invalid_camera_preset",
                    stage,
                    format!("`worldTargets.cameraPresets.{preset_id}` 必须是对象"),
                    Some("cameraPreset"),
                    Some(preset_id),
                ));
                continue;
            };
            let mut has_known_fields = false;
            if let Some(anchor_id) = read_string_from_map(preset_obj, &["anchorId", "anchor_id"]) {
                has_known_fields = true;
                if !index.anchors.contains(anchor_id.as_str()) {
                    diagnostics.push(world_contract_diagnostic(
                        "world_targets_unknown_anchor",
                        stage,
                        format!(
                            "`worldTargets.cameraPresets.{preset_id}.anchorId` 引用了不存在的 anchor `{anchor_id}`"
                        ),
                        Some("anchor"),
                        Some(anchor_id.as_str()),
                    ));
                }
            }
            if let Some(group_id) = read_string_from_map(preset_obj, &["groupId", "group_id"]) {
                has_known_fields = true;
                if !index.groups.contains(group_id.as_str()) {
                    diagnostics.push(world_contract_diagnostic(
                        "world_targets_unknown_group",
                        stage,
                        format!(
                            "`worldTargets.cameraPresets.{preset_id}.groupId` 引用了不存在的 group `{group_id}`"
                        ),
                        Some("group"),
                        Some(group_id.as_str()),
                    ));
                }
            }
            for field_name in ["zoom", "bearing", "pitch"] {
                if preset_obj.contains_key(field_name) {
                    has_known_fields = true;
                }
                validate_number_field(
                    stage,
                    "cameraPresets",
                    preset_id,
                    field_name,
                    preset_obj.get(field_name),
                    diagnostics,
                );
            }
            if preset_obj.contains_key("center") {
                has_known_fields = true;
            }
            validate_center_field(
                stage,
                "cameraPresets",
                preset_id,
                preset_obj.get("center"),
                diagnostics,
            );
            if preset_obj.contains_key("bounds") {
                has_known_fields = true;
            }
            validate_bounds_field(
                stage,
                "cameraPresets",
                preset_id,
                preset_obj.get("bounds"),
                diagnostics,
            );
            for field_name in ["shapeIds", "hotspotIds", "layerIds", "layers"] {
                if preset_obj.contains_key(field_name) {
                    has_known_fields = true;
                }
                validate_string_array_field(
                    stage,
                    "cameraPresets",
                    preset_id,
                    field_name,
                    preset_obj.get(field_name),
                    diagnostics,
                );
            }
            if !has_known_fields {
                diagnostics.push(world_contract_diagnostic(
                    "world_targets_camera_preset_empty",
                    stage,
                    format!(
                        "`worldTargets.cameraPresets.{preset_id}` 至少应声明 anchor / camera / group / layer 相关字段之一"
                    ),
                    Some("cameraPreset"),
                    Some(preset_id),
                ));
            }
        }
    }

    if let Some(entities) = read_object_field(value, &["entities"]) {
        for (entity_id, entity_value) in entities {
            let Some(entity_obj) = entity_value.as_object() else {
                diagnostics.push(world_contract_diagnostic(
                    "world_targets_invalid_entity",
                    stage,
                    format!("`worldTargets.entities.{entity_id}` 必须是对象"),
                    Some("entity"),
                    Some(entity_id),
                ));
                continue;
            };
            let mut has_known_fields = false;
            if let Some(anchor_id) = read_string_from_map(entity_obj, &["anchorId", "anchor_id"]) {
                has_known_fields = true;
                if !index.anchors.contains(anchor_id.as_str()) {
                    diagnostics.push(world_contract_diagnostic(
                        "world_targets_unknown_anchor",
                        stage,
                        format!(
                            "`worldTargets.entities.{entity_id}.anchorId` 引用了不存在的 anchor `{anchor_id}`"
                        ),
                        Some("anchor"),
                        Some(anchor_id.as_str()),
                    ));
                }
            }
            if let Some(camera_preset) =
                read_string_from_map(entity_obj, &["cameraPreset", "camera_preset"])
            {
                has_known_fields = true;
                if !index.camera_presets.contains(camera_preset.as_str()) {
                    diagnostics.push(world_contract_diagnostic(
                        "world_targets_unknown_camera_preset",
                        stage,
                        format!(
                            "`worldTargets.entities.{entity_id}.cameraPreset` 引用了不存在的 preset `{camera_preset}`"
                        ),
                        Some("cameraPreset"),
                        Some(camera_preset.as_str()),
                    ));
                }
            }
            if let Some(group_id) = read_string_from_map(entity_obj, &["groupId", "group_id"]) {
                has_known_fields = true;
                if !index.groups.contains(group_id.as_str()) {
                    diagnostics.push(world_contract_diagnostic(
                        "world_targets_unknown_group",
                        stage,
                        format!(
                            "`worldTargets.entities.{entity_id}.groupId` 引用了不存在的 group `{group_id}`"
                        ),
                        Some("group"),
                        Some(group_id.as_str()),
                    ));
                }
            }
            for field_name in ["shapeIds", "hotspotIds", "layerIds", "layers"] {
                if entity_obj.contains_key(field_name) {
                    has_known_fields = true;
                }
                validate_string_array_field(
                    stage,
                    "entities",
                    entity_id,
                    field_name,
                    entity_obj.get(field_name),
                    diagnostics,
                );
            }
            if !has_known_fields {
                diagnostics.push(world_contract_diagnostic(
                    "world_targets_entity_empty",
                    stage,
                    format!(
                        "`worldTargets.entities.{entity_id}` 至少应声明 anchor / cameraPreset / group / layer 相关字段之一"
                    ),
                    Some("entity"),
                    Some(entity_id),
                ));
            }
        }
    }

    index
}

fn collect_world_stage_contracts_from_nodes(
    panel: &PanelDecl,
    nodes: &[UiNodeDecl],
    out: &mut Vec<WorldStageContract>,
    diagnostics: &mut Vec<PresentationCompileDiagnostic>,
    warnings: &mut Vec<PresentationCompileDiagnostic>,
) {
    for node in nodes {
        match node {
            UiNodeDecl::Block(block) => {
                let target_config = block
                    .props
                    .get("worldTargets")
                    .or_else(|| block.props.get("world_targets"));
                if let Some(target_config) = target_config {
                    let mut stage = WorldStageContract {
                        panel_id: panel.id.clone(),
                        block_id: block.id.clone(),
                        view_family: read_string_from_value(
                            &block.props,
                            &["__mei_view_family", "viewFamily", "view_family"],
                        )
                        .or_else(|| {
                            read_string_from_value(
                                &panel.props,
                                &["__mei_view_family", "viewFamily", "view_family"],
                            )
                        }),
                        stage_kind: read_string_from_value(
                            &block.props,
                            &["__mei_stage_kind", "stageKind", "stage_kind"],
                        )
                        .or_else(|| {
                            read_string_from_value(
                                &panel.props,
                                &["__mei_stage_kind", "stageKind", "stage_kind"],
                            )
                        }),
                        world_ref: read_string_from_value(
                            &block.props,
                            &["__mei_world_ref", "worldRef", "world_ref"],
                        )
                        .or_else(|| {
                            read_string_from_value(
                                &panel.props,
                                &["__mei_world_ref", "worldRef", "world_ref"],
                            )
                        }),
                        entity_id: read_string_from_value(&block.props, &["entityId", "entity_id"])
                            .or_else(|| {
                                read_string_from_value(&panel.props, &["entityId", "entity_id"])
                            }),
                        group_id: read_string_from_value(&block.props, &["groupId", "group_id"])
                            .or_else(|| {
                                read_string_from_value(&panel.props, &["groupId", "group_id"])
                            }),
                        camera_preset: read_string_from_value(
                            &block.props,
                            &["cameraPreset", "camera_preset"],
                        )
                        .or_else(|| {
                            read_string_from_value(
                                &panel.props,
                                &["cameraPreset", "camera_preset"],
                            )
                        }),
                        targets: WorldTargetsIndex::default(),
                    };
                    if stage.world_ref.is_none() || stage.stage_kind.is_none() {
                        warnings.push(world_contract_warning(
                            "world_stage_route_hints_missing",
                            &stage,
                            "该 world stage 缺少 `worldRef` 或 `stageKind`，多舞台并存时 runtime 路由会退回启发式匹配",
                        ));
                    }
                    stage.targets =
                        validate_world_targets_config(&stage, target_config, diagnostics);
                    if let Some(entity_id) = stage.entity_id.as_deref() {
                        if !stage.targets.entities.contains(entity_id) {
                            diagnostics.push(world_contract_diagnostic(
                                "world_targets_unknown_entity",
                                &stage,
                                format!("stage 默认 `entityId` `{entity_id}` 未在 `worldTargets.entities` 中声明"),
                                Some("entity"),
                                Some(entity_id),
                            ));
                        }
                    }
                    if let Some(group_id) = stage.group_id.as_deref() {
                        if !stage.targets.groups.contains(group_id) {
                            diagnostics.push(world_contract_diagnostic(
                                "world_targets_unknown_group",
                                &stage,
                                format!("stage 默认 `groupId` `{group_id}` 未在 `worldTargets.groups` 中声明"),
                                Some("group"),
                                Some(group_id),
                            ));
                        }
                    }
                    if let Some(camera_preset) = stage.camera_preset.as_deref() {
                        if !stage.targets.camera_presets.contains(camera_preset) {
                            diagnostics.push(world_contract_diagnostic(
                                "world_targets_unknown_camera_preset",
                                &stage,
                                format!(
                                    "stage 默认 `cameraPreset` `{camera_preset}` 未在 `worldTargets.cameraPresets` 中声明"
                                ),
                                Some("cameraPreset"),
                                Some(camera_preset),
                            ));
                        }
                    }
                    out.push(stage);
                }
            }
            UiNodeDecl::Panel(nested) => {
                collect_world_stage_contracts_from_nodes(
                    nested,
                    &nested.blocks,
                    out,
                    diagnostics,
                    warnings,
                );
            }
            _ => {}
        }
    }
}

fn collect_world_stage_contracts(
    panels: &[PanelDecl],
) -> (
    Vec<WorldStageContract>,
    Vec<PresentationCompileDiagnostic>,
    Vec<PresentationCompileDiagnostic>,
) {
    let mut world_stages = Vec::new();
    let mut diagnostics = Vec::new();
    let mut warnings = Vec::new();
    for panel in panels {
        collect_world_stage_contracts_from_nodes(
            panel,
            &panel.blocks,
            &mut world_stages,
            &mut diagnostics,
            &mut warnings,
        );
    }
    (world_stages, diagnostics, warnings)
}

fn viewpoint_entry_from_value(value: &Value) -> PresentationViewpointEntry {
    PresentationViewpointEntry {
        panel_id: read_string_from_value(value, &["panelId", "panel_id"]),
        view_family: read_string_from_value(value, &["viewFamily", "view_family"]),
        stage_kind: read_string_from_value(value, &["stageKind", "stage_kind"]),
        world_ref: read_string_from_value(value, &["worldRef", "world_ref"]),
        entity_id: read_string_from_value(value, &["entityId", "entity_id"]),
        group_id: read_string_from_value(value, &["groupId", "group_id"]),
        camera_preset: read_string_from_value(value, &["cameraPreset", "camera_preset"]),
    }
}

fn resolve_world_action_target(
    action_map: &Map<String, Value>,
    viewpoint_entry: Option<&PresentationViewpointEntry>,
) -> ResolvedWorldActionTarget {
    ResolvedWorldActionTarget {
        viewpoint_id: read_string_from_map(action_map, &["viewpoint", "viewpointId"]),
        panel_id: viewpoint_entry.and_then(|entry| entry.panel_id.clone()),
        view_family: read_string_from_map(action_map, &["viewFamily", "view_family"])
            .or_else(|| viewpoint_entry.and_then(|entry| entry.view_family.clone())),
        stage_kind: read_string_from_map(action_map, &["stageKind", "stage_kind"])
            .or_else(|| viewpoint_entry.and_then(|entry| entry.stage_kind.clone())),
        world_ref: read_string_from_map(action_map, &["worldRef", "world_ref"])
            .or_else(|| viewpoint_entry.and_then(|entry| entry.world_ref.clone())),
        entity_id: read_string_from_map(action_map, &["entityId", "entity_id"])
            .or_else(|| viewpoint_entry.and_then(|entry| entry.entity_id.clone())),
        group_id: read_string_from_map(action_map, &["groupId", "group_id"])
            .or_else(|| viewpoint_entry.and_then(|entry| entry.group_id.clone())),
        camera_preset: read_string_from_map(action_map, &["cameraPreset", "camera_preset"])
            .or_else(|| viewpoint_entry.and_then(|entry| entry.camera_preset.clone())),
    }
}

fn world_stage_matches_target(stage: &WorldStageContract, target: &ResolvedWorldActionTarget) -> bool {
    if let Some(panel_id) = target.panel_id.as_deref() {
        if panel_id != stage.panel_id {
            return false;
        }
    }
    if let Some(view_family) = target.view_family.as_deref() {
        if stage.view_family.as_deref() != Some(view_family) {
            return false;
        }
    }
    if let Some(stage_kind) = target.stage_kind.as_deref() {
        if stage.stage_kind.as_deref() != Some(stage_kind) {
            return false;
        }
    }
    if let Some(world_ref) = target.world_ref.as_deref() {
        if stage.world_ref.as_deref() != Some(world_ref) {
            return false;
        }
    }
    true
}

fn matching_world_stage_contracts<'a>(
    surfaces: &'a PresentationSurfaceIndex,
    target: &ResolvedWorldActionTarget,
) -> Vec<&'a WorldStageContract> {
    if let Some(panel_id) = target.panel_id.as_deref() {
        let panel_matches = surfaces
            .world_stages
            .iter()
            .filter(|stage| stage.panel_id == panel_id)
            .collect::<Vec<_>>();
        let exact_panel_matches = panel_matches
            .iter()
            .copied()
            .filter(|stage| world_stage_matches_target(stage, target))
            .collect::<Vec<_>>();
        if !exact_panel_matches.is_empty() {
            return exact_panel_matches;
        }
        if !panel_matches.is_empty() {
            return panel_matches;
        }
    }
    surfaces
        .world_stages
        .iter()
        .filter(|stage| world_stage_matches_target(stage, target))
        .collect()
}

fn validate_world_action_contract(
    surfaces: &PresentationSurfaceIndex,
    action_type: &str,
    step_id: Option<&str>,
    target: &ResolvedWorldActionTarget,
) -> Vec<PresentationCompileDiagnostic> {
    let mut diagnostics = Vec::new();
    if !target.has_host_hints() && !target.has_contract_refs() {
        return diagnostics;
    }
    let matches = matching_world_stage_contracts(surfaces, target);
    if matches.is_empty() {
        let host_label = target
            .viewpoint_id
            .as_deref()
            .map(|viewpoint_id| format!("viewpoint `{viewpoint_id}`"))
            .or_else(|| {
                target
                    .world_ref
                    .as_deref()
                    .map(|world_ref| format!("worldRef `{world_ref}`"))
            })
            .unwrap_or_else(|| "当前 world action".to_string());
        diagnostics.push(diagnostic(
            "world_target_host_missing",
            format!("`{action_type}` 找不到可匹配的 world stage（{host_label}）"),
            step_id,
            Some("world_stage"),
            target.viewpoint_id.as_deref(),
        ));
        return diagnostics;
    }

    if let Some(entity_id) = target.entity_id.as_deref() {
        if !matches
            .iter()
            .any(|stage| stage.targets.entities.contains(entity_id))
        {
            diagnostics.push(diagnostic(
                "world_targets_unknown_entity",
                format!(
                    "`{action_type}` 引用了不存在的 entity `{entity_id}`"
                ),
                step_id,
                Some("entity"),
                Some(entity_id),
            ));
        }
    }
    if let Some(group_id) = target.group_id.as_deref() {
        if !matches.iter().any(|stage| stage.targets.groups.contains(group_id)) {
            diagnostics.push(diagnostic(
                "world_targets_unknown_group",
                format!("`{action_type}` 引用了不存在的 group `{group_id}`"),
                step_id,
                Some("group"),
                Some(group_id),
            ));
        }
    }
    if let Some(camera_preset) = target.camera_preset.as_deref() {
        if !matches
            .iter()
            .any(|stage| stage.targets.camera_presets.contains(camera_preset))
        {
            diagnostics.push(diagnostic(
                "world_targets_unknown_camera_preset",
                format!(
                    "`{action_type}` 引用了不存在的 cameraPreset `{camera_preset}`"
                ),
                step_id,
                Some("cameraPreset"),
                Some(camera_preset),
            ));
        }
    }
    diagnostics
}

fn compile_script_path(package_root: &Path) -> std::path::PathBuf {
    package_root.join("scripts").join("compile-presentation.mjs")
}

fn compile_manifest_via_node(
    package_root: &Path,
    source: &str,
    options: &Value,
) -> Result<Value> {
    let script = compile_script_path(package_root);
    if !script.is_file() {
        anyhow::bail!("presentation compile script not found: {}", script.display());
    }
    let payload = json!({
        "source": source,
        "options": options,
    });
    let mut command = Command::new("node");
    command.arg(&script);
    command.arg("--stdin-json");
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn node for {}", script.display()))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(payload.to_string().as_bytes())
            .context("failed to write compile payload to node stdin")?;
    }
    let output = child
        .wait_with_output()
        .context("failed to wait for presentation compile node process")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "presentation compile script failed (status={}): {}",
            output.status,
            stderr.trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let manifest = serde_json::from_str::<Value>(stdout.trim())
        .context("failed to parse manifest JSON from node stdout")?;
    Ok(manifest)
}

fn build_surface_index(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Result<PresentationSurfaceIndex> {
    let app_root = resolve_app_root(workspace_root, app_id);
    let compiled = compile_app_from_root(workspace_root, app_root.as_path())
        .with_context(|| format!("failed to compile app `{app_id}` for presentation validation"))?;
    let mut surfaces = PresentationSurfaceIndex::default();
    for route in catalog_scene_routes_from_app_root(app_root.as_path()) {
        let scene_id = route.scene_id.trim();
        if !scene_id.is_empty() {
            surfaces.pages.insert(scene_id.to_string());
        }
    }
    for resource in &compiled.resources {
        if let Some(dataset) = resource.dataset.as_ref() {
            for metric_id in dataset.metrics.keys() {
                let metric_id = metric_id.trim();
                if !metric_id.is_empty() {
                    surfaces.metrics.insert(metric_id.to_string());
                }
            }
            for metric_id in dataset.runtime_metric_defs.keys() {
                let metric_id = metric_id.trim();
                if !metric_id.is_empty() {
                    surfaces.metrics.insert(metric_id.to_string());
                }
            }
        }
    }
    if let Ok(Some(outcome)) =
        mei_host_graph::assemble_scope_from_registry(workspace_root, app_id, scene_id)
    {
        if let Some(viewpoints) = outcome
            .presentation_map
            .get("viewpoints")
            .and_then(Value::as_object)
        {
            for viewpoint_id in viewpoints.keys() {
                let viewpoint_id = viewpoint_id.trim();
                if !viewpoint_id.is_empty() {
                    if let Some(viewpoint_value) = viewpoints.get(viewpoint_id) {
                        surfaces
                            .viewpoints
                            .insert(viewpoint_id.to_string(), viewpoint_entry_from_value(viewpoint_value));
                    }
                }
            }
        }
        if let Some(scene_contract) = outcome.compiled.scene_contract.as_ref() {
            let (world_stages, diagnostics, warnings) =
                collect_world_stage_contracts(&scene_contract.panels);
            surfaces.world_stages = world_stages;
            surfaces.diagnostics.extend(diagnostics);
            surfaces.warnings.extend(warnings);
        }
    }
    Ok(surfaces)
}

fn diagnostic(
    code: &str,
    message: impl Into<String>,
    step_id: Option<&str>,
    ref_kind: Option<&str>,
    ref_id: Option<&str>,
) -> PresentationCompileDiagnostic {
    PresentationCompileDiagnostic {
        level: "error".to_string(),
        code: code.to_string(),
        message: message.into(),
        step_id: step_id.map(str::to_string),
        ref_kind: ref_kind.map(str::to_string),
        ref_id: ref_id.map(str::to_string),
    }
}

fn warn(code: &str, message: impl Into<String>) -> PresentationCompileDiagnostic {
    PresentationCompileDiagnostic {
        level: "warn".to_string(),
        code: code.to_string(),
        message: message.into(),
        step_id: None,
        ref_kind: None,
        ref_id: None,
    }
}

fn step_actions(step: &Map<String, Value>) -> Vec<Value> {
    if let Some(actions) = step.get("actions").and_then(Value::as_array) {
        return actions.to_vec();
    }
    step.get("cockpit")
        .and_then(Value::as_object)
        .and_then(|cockpit| cockpit.get("actions"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn validate_manifest_refs(
    manifest: &Value,
    surfaces: &PresentationSurfaceIndex,
) -> (Vec<PresentationCompileDiagnostic>, Vec<PresentationCompileDiagnostic>) {
    let mut diagnostics = surfaces.diagnostics.clone();
    let mut warnings = surfaces.warnings.clone();
    let mut warned_unvalidated_chart = false;
    let mut warned_unvalidated_image = false;
    let Some(steps) = manifest.get("steps").and_then(Value::as_array) else {
        diagnostics.push(diagnostic(
            "manifest_steps_missing",
            "presentation manifest 缺少 steps 数组",
            None,
            None,
            None,
        ));
        return (diagnostics, warnings);
    };
    for step in steps {
        let Some(step_map) = step.as_object() else {
            continue;
        };
        let step_id = step_map.get("id").and_then(Value::as_str);
        for action in step_actions(step_map) {
            let Some(action_map) = action.as_object() else {
                continue;
            };
            let action_type = action_map
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            match action_type {
                "highlight"
                | "focus"
                | "camera_move"
                | "focus_entity"
                | "show_group"
                | "hide_group" => {
                    if let Some(viewpoint_id) =
                        action_map.get("viewpoint").and_then(Value::as_str).map(str::trim)
                    {
                        if !viewpoint_id.is_empty() && !surfaces.viewpoints.contains_key(viewpoint_id)
                        {
                            diagnostics.push(diagnostic(
                                "unknown_viewpoint",
                                format!("未知 viewpoint `{viewpoint_id}`"),
                                step_id,
                                Some("viewpoint"),
                                Some(viewpoint_id),
                            ));
                        }
                    }
                    let viewpoint_entry = read_string_from_map(action_map, &["viewpoint", "viewpointId"])
                        .as_deref()
                        .and_then(|viewpoint_id| surfaces.viewpoints.get(viewpoint_id));
                    diagnostics.extend(validate_world_action_contract(
                        surfaces,
                        action_type,
                        step_id,
                        &resolve_world_action_target(action_map, viewpoint_entry),
                    ));
                }
                "open_t2_page" => {
                    if let Some(page_scene_id) =
                        action_map.get("pageSceneId").and_then(Value::as_str).map(str::trim)
                    {
                        if !page_scene_id.is_empty() && !surfaces.pages.contains(page_scene_id) {
                            diagnostics.push(diagnostic(
                                "unknown_page_scene",
                                format!("未知 page_scene_id `{page_scene_id}`"),
                                step_id,
                                Some("page_scene_id"),
                                Some(page_scene_id),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        let slot_arrays = step_map
            .get("slide")
            .and_then(Value::as_object)
            .and_then(|slide| slide.get("slots"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for slot in slot_arrays {
            let Some(slot_map) = slot.as_object() else {
                continue;
            };
            let embeds = slot_map
                .get("embeds")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for embed in embeds {
                let Some(embed_map) = embed.as_object() else {
                    continue;
                };
                let kind = embed_map.get("kind").and_then(Value::as_str).unwrap_or("").trim();
                let ref_id = embed_map.get("ref").and_then(Value::as_str).unwrap_or("").trim();
                if ref_id.is_empty() {
                    continue;
                }
                match kind {
                    "embed" => {
                        if !surfaces.viewpoints.contains_key(ref_id) {
                            diagnostics.push(diagnostic(
                                "unknown_embed_viewpoint",
                                format!("未知 embed viewpoint `{ref_id}`"),
                                step_id,
                                Some("viewpoint"),
                                Some(ref_id),
                            ));
                        }
                    }
                    "metric" => {
                        if !surfaces.metrics.contains(ref_id) {
                            diagnostics.push(diagnostic(
                                "unknown_metric",
                                format!("未知 metric `{ref_id}`"),
                                step_id,
                                Some("metric"),
                                Some(ref_id),
                            ));
                        }
                    }
                    "chart" if !warned_unvalidated_chart => {
                        warned_unvalidated_chart = true;
                        warnings.push(warn(
                            "chart_validation_not_enabled",
                            "当前临时 compile API 尚未对 chart 引用做严格存在性校验",
                        ));
                    }
                    "image" if !warned_unvalidated_image => {
                        warned_unvalidated_image = true;
                        warnings.push(warn(
                            "image_validation_not_enabled",
                            "当前临时 compile API 尚未对 image 引用做严格存在性校验",
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    (diagnostics, warnings)
}

pub async fn api_presentation_compile(
    State(state): State<SharedState>,
    Json(request): Json<PresentationCompileRequest>,
) -> Response {
    let app_id = request.app_id.trim();
    let source = request.source.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "manifest": Value::Null,
                "diagnostics": [{
                    "level": "error",
                    "code": "app_id_required",
                    "message": "appId 不能为空"
                }],
                "warnings": [],
            })),
        )
            .into_response();
    }
    if source.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "manifest": Value::Null,
                "diagnostics": [{
                    "level": "error",
                    "code": "source_required",
                    "message": "source 不能为空"
                }],
                "warnings": [],
            })),
        )
            .into_response();
    }
    if let Some(mode) = request.mode.as_deref() {
        let mode = mode.trim();
        if !mode.is_empty() && mode != "ephemeral" {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "manifest": Value::Null,
                    "diagnostics": [{
                        "level": "error",
                        "code": "unsupported_mode",
                        "message": format!("仅支持 mode=ephemeral，收到 `{mode}`")
                    }],
                    "warnings": [],
                })),
            )
                .into_response();
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    let package_root = guard.package_root.clone();
    drop(guard);
    let scene_id = request
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");
    let options = json!({
        "id": request.presentation_id.as_deref().map(str::trim).filter(|value| !value.is_empty()).unwrap_or("ephemeral"),
        "defaultScene": scene_id,
    });
    let manifest = match compile_manifest_via_node(package_root.as_path(), source, &options) {
        Ok(manifest) => manifest,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "manifest": Value::Null,
                    "diagnostics": [{
                        "level": "error",
                        "code": "compile_failed",
                        "message": error.to_string(),
                    }],
                    "warnings": [],
                })),
            )
                .into_response();
        }
    };
    let surfaces = match build_surface_index(workspace_root.as_path(), app_id, scene_id) {
        Ok(index) => index,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "manifest": Value::Null,
                    "diagnostics": [{
                        "level": "error",
                        "code": "surface_index_failed",
                        "message": error.to_string(),
                    }],
                    "warnings": [],
                })),
            )
                .into_response();
        }
    };
    let (diagnostics, warnings) = validate_manifest_refs(&manifest, &surfaces);
    if !diagnostics.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "manifest": Value::Null,
                "diagnostics": diagnostics,
                "warnings": warnings,
            })),
        )
            .into_response();
    }
    Json(json!({
        "manifest": manifest,
        "diagnostics": diagnostics,
        "warnings": warnings,
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_manifest_refs_reports_unknown_viewpoint_page_and_metric() {
        let manifest = json!({
            "id": "ephemeral",
            "steps": [{
                "id": "step_1",
                "actions": [
                    { "type": "highlight", "viewpoint": "missing_viewpoint" },
                    { "type": "camera_move", "viewpoint": "missing_viewpoint" },
                    { "type": "focus_entity", "viewpoint": "missing_viewpoint" },
                    { "type": "show_group", "viewpoint": "missing_viewpoint" },
                    { "type": "hide_group", "viewpoint": "missing_viewpoint" },
                    { "type": "open_t2_page", "pageSceneId": "missing_page" }
                ],
                "slide": {
                    "slots": [{
                        "name": "evidence",
                        "embeds": [
                            { "kind": "embed", "ref": "missing_viewpoint" },
                            { "kind": "metric", "ref": "missing_metric" }
                        ]
                    }]
                }
            }]
        });
        let surfaces = PresentationSurfaceIndex::default();
        let (diagnostics, warnings) = validate_manifest_refs(&manifest, &surfaces);
        assert!(warnings.is_empty());
        let codes = diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"unknown_viewpoint"));
        assert!(codes.contains(&"unknown_page_scene"));
        assert!(codes.contains(&"unknown_embed_viewpoint"));
        assert!(codes.contains(&"unknown_metric"));
    }

    #[test]
    fn validate_manifest_refs_accepts_known_refs() {
        let manifest = json!({
            "id": "ephemeral",
            "steps": [{
                "id": "step_1",
                "actions": [
                    { "type": "highlight", "viewpoint": "known_viewpoint" },
                    { "type": "camera_move", "viewpoint": "known_viewpoint" },
                    { "type": "focus_entity", "viewpoint": "known_viewpoint" },
                    { "type": "show_group", "viewpoint": "known_viewpoint" },
                    { "type": "hide_group", "viewpoint": "known_viewpoint" },
                    { "type": "open_t2_page", "pageSceneId": "known_page" }
                ],
                "slide": {
                    "slots": [{
                        "name": "evidence",
                        "embeds": [
                            { "kind": "embed", "ref": "known_viewpoint" },
                            { "kind": "metric", "ref": "known_metric" }
                        ]
                    }]
                }
            }]
        });
        let surfaces = PresentationSurfaceIndex {
            viewpoints: BTreeMap::from([(
                "known_viewpoint".to_string(),
                PresentationViewpointEntry::default(),
            )]),
            pages: BTreeSet::from(["known_page".to_string()]),
            metrics: BTreeSet::from(["known_metric".to_string()]),
            ..PresentationSurfaceIndex::default()
        };
        let (diagnostics, warnings) = validate_manifest_refs(&manifest, &surfaces);
        assert!(diagnostics.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn validate_manifest_refs_reports_unknown_world_contract_refs() {
        let manifest = json!({
            "id": "ephemeral",
            "steps": [{
                "id": "step_1",
                "actions": [
                    { "type": "focus_entity", "viewpoint": "park_point_1_entry" },
                    { "type": "show_group", "viewpoint": "park_point_1_entry" },
                    { "type": "camera_move", "viewpoint": "park_point_1_entry" }
                ]
            }]
        });
        let surfaces = PresentationSurfaceIndex {
            viewpoints: BTreeMap::from([(
                "park_point_1_entry".to_string(),
                PresentationViewpointEntry {
                    panel_id: Some("basemap".to_string()),
                    view_family: Some("map".to_string()),
                    stage_kind: Some("map-stage".to_string()),
                    world_ref: Some("park_world".to_string()),
                    entity_id: Some("lake_pavilion_typo".to_string()),
                    group_id: Some("lake_group_typo".to_string()),
                    camera_preset: Some("camera_typo".to_string()),
                },
            )]),
            world_stages: vec![WorldStageContract {
                panel_id: "basemap".to_string(),
                block_id: Some("basemap_stage".to_string()),
                view_family: Some("map".to_string()),
                stage_kind: Some("map-stage".to_string()),
                world_ref: Some("park_world".to_string()),
                targets: WorldTargetsIndex {
                    entities: BTreeSet::from(["lake_pavilion".to_string()]),
                    groups: BTreeSet::from(["lake_pavilion_story".to_string()]),
                    camera_presets: BTreeSet::from(["lake_pavilion_focus".to_string()]),
                    ..WorldTargetsIndex::default()
                },
                ..WorldStageContract::default()
            }],
            ..PresentationSurfaceIndex::default()
        };
        let (diagnostics, warnings) = validate_manifest_refs(&manifest, &surfaces);
        assert!(warnings.is_empty());
        let codes = diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"world_targets_unknown_entity"));
        assert!(codes.contains(&"world_targets_unknown_group"));
        assert!(codes.contains(&"world_targets_unknown_camera_preset"));
    }

    #[test]
    fn validate_world_targets_config_reports_invalid_cross_refs() {
        let stage = WorldStageContract {
            panel_id: "basemap".to_string(),
            block_id: Some("basemap_stage".to_string()),
            ..WorldStageContract::default()
        };
        let mut diagnostics = Vec::new();
        let targets = validate_world_targets_config(
            &stage,
            &json!({
                "anchors": {
                    "lake_center": { "x": 10, "y": 20 }
                },
                "cameraPresets": {
                    "focus": {
                        "anchorId": "missing_anchor",
                        "groupId": "missing_group",
                        "zoom": 1.4
                    }
                },
                "entities": {
                    "lake_pavilion": {
                        "cameraPreset": "missing_preset",
                        "groupId": "missing_group"
                    }
                },
                "groups": {
                    "story": {
                        "shapeIds": ["park_outline"]
                    }
                }
            }),
            &mut diagnostics,
        );
        assert!(targets.anchors.contains("lake_center"));
        let codes = diagnostics
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"world_targets_unknown_anchor"));
        assert!(codes.contains(&"world_targets_unknown_group"));
        assert!(codes.contains(&"world_targets_unknown_camera_preset"));
    }
}
