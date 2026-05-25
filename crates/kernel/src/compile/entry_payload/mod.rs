use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::eval::evaluate_mei_file;
use crate::model::{CompiledSceneRoute, ComponentAsset, Diagnostic, Severity};
use crate::typed_refs::SceneRegistry;

pub(super) mod clone_merge;
mod compile;
pub(super) mod helpers;
pub(super) mod import_scope;

use compile::compile_scene_payload;

#[derive(Debug, Clone, Default)]
pub(super) struct CompiledScenePayload {
    pub(super) scene_contract: Option<crate::model::SceneContract>,
    pub(super) resources: Vec<crate::model::LoadedResource>,
    pub(super) component_assets: Vec<ComponentAsset>,
    pub(super) diagnostics: Vec<Diagnostic>,
}

pub(super) fn compile_scene_payload_for_target_uncached(
    app_root: &Path,
    app_decls: &Value,
    asset_map: &std::collections::BTreeMap<String, ComponentAsset>,
    target_file: &str,
    route_meta: Option<&CompiledSceneRoute>,
    scene_registry: &SceneRegistry,
) -> CompiledScenePayload {
    match load_entry_decls(app_root, app_decls, target_file) {
        Ok(entry_decls) => {
            match compile_scene_payload(
                app_root,
                asset_map,
                target_file,
                &entry_decls,
                route_meta,
                scene_registry,
            ) {
                Ok(payload) => payload,
                Err(error) => CompiledScenePayload {
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Error,
                        code: "compile_scene_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(target_file.to_string()),
                    }],
                    ..CompiledScenePayload::default()
                },
            }
        }
        Err(error) => CompiledScenePayload {
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                code: "load_scene_file_failed".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            }],
            ..CompiledScenePayload::default()
        },
    }
}

fn load_entry_decls(app_root: &Path, app_decls: &Value, target_file: &str) -> Result<Value> {
    if target_file == "main.mei" {
        Ok(app_decls.clone())
    } else {
        let entry_path = app_root.join(target_file);
        evaluate_mei_file(&entry_path)
    }
}
