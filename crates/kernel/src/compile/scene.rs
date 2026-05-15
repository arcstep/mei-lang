use std::{collections::BTreeSet, path::Path};

use serde_json::Value;

use crate::model::{AppDecl, CompiledEntryMeta, Diagnostic, EntryDecl, Severity};

use super::decls::SceneFileRefDecl;

pub(super) struct SceneEntryRegistry {
    pub entries: Vec<CompiledEntryMeta>,
    pub default_entry_id: Option<String>,
}

pub(super) fn resolve_scene_entries(
    app_main: &Path,
    app_decl: &AppDecl,
    app_decls: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) -> SceneEntryRegistry {
    let mut entries = Vec::new();
    let mut seen_entry_ids = BTreeSet::new();

    collect_entry_from_app_scene_field(app_decl, &mut entries, &mut seen_entry_ids);
    collect_entries_from_app_decl(app_decl, &mut entries, &mut seen_entry_ids);
    collect_inline_scene_entries(app_decls, &mut entries, &mut seen_entry_ids);
    collect_scene_file_ref_entries(app_decls, &mut entries, &mut seen_entry_ids);

    if entries.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_app_scene".to_string(),
            message:
                "app(...) must bind at least one scene (inline scene, app.scene, entry, or app.add_scene(scene_file_ref(...)))"
                    .to_string(),
            source_path: Some(app_main.to_string_lossy().to_string()),
        });
    }

    let mut default_entry_id = resolve_default_entry_id(app_decl, &entries);
    if default_entry_id.is_none() && app_decl.entries.is_empty() {
        if let Some(default_scene) = app_decl.default_scene.as_deref() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_default_scene".to_string(),
                message: format!(
                    "default_scene `{default_scene}` did not match an inline scene or app.add_scene(scene_file_ref(...))"
                ),
                source_path: Some(app_main.to_string_lossy().to_string()),
            });
        }
    }
    if default_entry_id.is_none() {
        default_entry_id = entries.first().map(|entry| entry.entry_id.clone());
    }
    if let Some(default_id) = default_entry_id.as_deref() {
        for entry in &mut entries {
            entry.is_default = entry.entry_id == default_id;
        }
    }

    SceneEntryRegistry {
        entries,
        default_entry_id,
    }
}

pub(super) fn find_scene_entry<'a>(
    entries: &'a [CompiledEntryMeta],
    selector: &str,
) -> Option<&'a CompiledEntryMeta> {
    entries.iter().find(|entry| {
        entry.entry_id == selector || entry.scene_id == selector || entry.target_file == selector
    })
}

fn collect_entries_from_app_decl(
    app_decl: &AppDecl,
    entries: &mut Vec<CompiledEntryMeta>,
    seen_entry_ids: &mut BTreeSet<String>,
) {
    for entry in &app_decl.entries {
        let Some(target_file) = entry_target_file(entry) else {
            continue;
        };
        let scene_id = entry_scene_id(entry, &target_file);
        let entry_id = entry.id.clone().unwrap_or_else(|| scene_id.clone());
        let frame_id = entry
            .frame
            .as_deref()
            .filter(|value| !is_target_file(value))
            .map(|value| value.to_string());
        let kind = if target_file == "main.mei"
            && entry
                .scene
                .as_deref()
                .is_some_and(|value| !is_target_file(value))
        {
            "declarative".to_string()
        } else {
            infer_entry_kind(&target_file)
        };
        append_entry(
            entries,
            seen_entry_ids,
            CompiledEntryMeta {
                entry_id,
                scene_id,
                frame_id,
                target_file: target_file.clone(),
                kind,
                title: entry.title.clone(),
                is_default: false,
            },
        );
    }
}

fn collect_entry_from_app_scene_field(
    app_decl: &AppDecl,
    entries: &mut Vec<CompiledEntryMeta>,
    seen_entry_ids: &mut BTreeSet<String>,
) {
    let Some(raw_scene) = app_decl.scene.as_ref() else {
        return;
    };
    if let Some(scene_id) = raw_scene
        .as_str()
        .map(str::trim)
        .filter(|scene_id| !scene_id.is_empty())
    {
        append_entry(
            entries,
            seen_entry_ids,
            CompiledEntryMeta {
                entry_id: scene_id.to_string(),
                scene_id: scene_id.to_string(),
                frame_id: None,
                target_file: "main.mei".to_string(),
                kind: "declarative".to_string(),
                title: None,
                is_default: false,
            },
        );
        return;
    }
    let Ok(scene_ref) = serde_json::from_value::<SceneFileRefDecl>(raw_scene.clone()) else {
        return;
    };
    if scene_ref.kind != "scene_file_ref" {
        return;
    }
    let scene_id = scene_ref
        .id
        .clone()
        .unwrap_or_else(|| scene_name_from_path(&scene_ref.path));
    append_entry(
        entries,
        seen_entry_ids,
        CompiledEntryMeta {
            entry_id: scene_id.clone(),
            scene_id,
            frame_id: None,
            target_file: scene_ref.path,
            kind: "file_ref".to_string(),
            title: None,
            is_default: false,
        },
    );
}

fn collect_inline_scene_entries(
    raw: &Value,
    entries: &mut Vec<CompiledEntryMeta>,
    seen_entry_ids: &mut BTreeSet<String>,
) {
    let Some(values) = raw.as_array() else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) != Some("scene") {
            continue;
        }
        let scene_id = value
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| scene_name_from_path("main.mei"));
        append_entry(
            entries,
            seen_entry_ids,
            CompiledEntryMeta {
                entry_id: scene_id.clone(),
                scene_id,
                frame_id: None,
                target_file: "main.mei".to_string(),
                kind: "inline".to_string(),
                title: value
                    .get("summary")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string()),
                is_default: false,
            },
        );
    }
}

fn collect_scene_file_ref_entries(
    raw: &Value,
    entries: &mut Vec<CompiledEntryMeta>,
    seen_entry_ids: &mut BTreeSet<String>,
) {
    let Some(values) = raw.as_array() else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
            continue;
        }
        let Ok(scene_ref) = serde_json::from_value::<SceneFileRefDecl>(
            value.get("scene").cloned().unwrap_or(Value::Null),
        ) else {
            continue;
        };
        if scene_ref.kind != "scene_file_ref" {
            continue;
        }
        let scene_id = scene_ref
            .id
            .clone()
            .unwrap_or_else(|| scene_name_from_path(&scene_ref.path));
        append_entry(
            entries,
            seen_entry_ids,
            CompiledEntryMeta {
                entry_id: scene_id.clone(),
                scene_id,
                frame_id: None,
                target_file: scene_ref.path,
                kind: "file_ref".to_string(),
                title: None,
                is_default: false,
            },
        );
    }
}

fn append_entry(
    entries: &mut Vec<CompiledEntryMeta>,
    seen_entry_ids: &mut BTreeSet<String>,
    entry: CompiledEntryMeta,
) {
    if seen_entry_ids.insert(entry.entry_id.clone()) {
        entries.push(entry);
    }
}

fn resolve_default_entry_id(app_decl: &AppDecl, entries: &[CompiledEntryMeta]) -> Option<String> {
    if let Some(first_entry) = app_decl.entries.first() {
        let explicit_default = first_entry
            .id
            .clone()
            .or_else(|| {
                first_entry
                    .scene
                    .as_deref()
                    .filter(|value| !is_target_file(value))
                    .map(|value| value.to_string())
            })
            .or_else(|| {
                first_entry
                    .frame
                    .as_deref()
                    .filter(|value| !is_target_file(value))
                    .map(|value| value.to_string())
            });
        if let Some(default_id) = explicit_default {
            if find_scene_entry(entries, &default_id).is_some() {
                return Some(default_id);
            }
        }
    }

    if let Some(default_scene) = app_decl.default_scene.as_deref() {
        if let Some(entry) = entries
            .iter()
            .find(|entry| entry.scene_id == default_scene || entry.entry_id == default_scene)
        {
            return Some(entry.entry_id.clone());
        }
    }

    entries
        .iter()
        .find(|entry| entry.kind == "inline")
        .or_else(|| entries.iter().find(|entry| entry.kind == "file_ref"))
        .map(|entry| entry.entry_id.clone())
}

pub(super) fn scene_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

fn entry_target_file(entry: &EntryDecl) -> Option<String> {
    if let Some(scene) = entry.scene.as_deref().filter(|value| is_target_file(value)) {
        return Some(scene.to_string());
    }
    if let Some(frame) = entry.frame.as_deref().filter(|value| is_target_file(value)) {
        return Some(frame.to_string());
    }
    if entry.scene.is_some() || entry.frame.is_some() || entry.id.is_some() {
        return Some("main.mei".to_string());
    }
    None
}

fn entry_scene_id(entry: &EntryDecl, target_file: &str) -> String {
    if let Some(scene_id) = entry
        .scene
        .as_deref()
        .filter(|value| !is_target_file(value))
    {
        return scene_id.to_string();
    }
    entry
        .id
        .clone()
        .or_else(|| {
            entry
                .frame
                .as_deref()
                .filter(|value| !is_target_file(value))
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| normalize_scene_name(target_file))
}

fn infer_entry_kind(target_file: &str) -> String {
    if target_file == "main.mei" {
        "inline".to_string()
    } else if target_file.ends_with(".mei") {
        "file_ref".to_string()
    } else {
        "inline".to_string()
    }
}

fn normalize_scene_name(value: &str) -> String {
    if is_target_file(value) {
        scene_name_from_path(value)
    } else {
        value.to_string()
    }
}

fn is_target_file(value: &str) -> bool {
    value.ends_with(".mei")
}
