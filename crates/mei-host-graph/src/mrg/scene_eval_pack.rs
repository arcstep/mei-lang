use std::collections::BTreeMap;
use std::path::Path;

use mei_lang_kernel::LayoutBudgetManifest;
use serde::{Deserialize, Serialize};

use super::client_bootstrap::{
    bootstrap_embed_status, build_client_bootstrap_payload, delivery_class_counts_for_scope,
    empty_client_bootstrap_payload, ClientBootstrapMetric, ClientBootstrapPayload,
    ClientBootstrapScopePayload, NO_CLIENT_BOOTSTRAP_REVISION,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SceneEvalPackStatus {
    PackHit,
    PackMiss,
    RevisionStale,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneEvalPackBuildOptions {
    #[serde(rename = "clientRevision")]
    pub client_revision: Option<String>,
    #[serde(rename = "queryFingerprint")]
    pub fingerprint: Option<String>,
    #[serde(rename = "neighborHops")]
    pub neighbor_hops: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEvalPackEvalLayerRef {
    pub layer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEvalPackResponse {
    pub status: SceneEvalPackStatus,
    #[serde(rename = "clientRevision")]
    pub client_revision: String,
    pub scope: String,
    #[serde(rename = "queryFingerprint", skip_serializing_if = "Option::is_none")]
    pub query_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<ClientBootstrapMetric>,
    #[serde(rename = "bootstrapScopes", default, skip_serializing_if = "Vec::is_empty")]
    pub bootstrap_scopes: Vec<ClientBootstrapScopePayload>,
    #[serde(rename = "layoutBudgetManifest", skip_serializing_if = "Option::is_none")]
    pub layout_budget_manifest: Option<LayoutBudgetManifest>,
    #[serde(rename = "neighborHops", skip_serializing_if = "Option::is_none")]
    pub neighbor_hops: Option<usize>,
    #[serde(rename = "evalLayerRefs", default, skip_serializing_if = "Vec::is_empty")]
    pub eval_layer_refs: Vec<SceneEvalPackEvalLayerRef>,
    #[serde(rename = "deliveryClassCounts", skip_serializing_if = "Option::is_none")]
    pub delivery_class_counts: Option<BTreeMap<String, usize>>,
    #[serde(rename = "bootstrapScope", skip_serializing_if = "Option::is_none")]
    pub bootstrap_scope: Option<String>,
    #[serde(rename = "targetFile", skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    #[serde(rename = "compileEpoch", skip_serializing_if = "Option::is_none")]
    pub compile_epoch: Option<String>,
    #[serde(rename = "dataGeneration", skip_serializing_if = "Option::is_none")]
    pub data_generation: Option<String>,
    #[serde(rename = "appId", skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
}

impl SceneEvalPackResponse {
    pub fn is_pack_hit(&self) -> bool {
        self.status == SceneEvalPackStatus::PackHit
            || self.status == SceneEvalPackStatus::RevisionStale
    }
}

fn eval_layer_refs_for_scope(scope: &str) -> Vec<SceneEvalPackEvalLayerRef> {
    vec![SceneEvalPackEvalLayerRef {
        layer: format!("eval.slot_group.{scope}"),
        digest: None,
    }]
}

fn pack_from_payload(
    payload: ClientBootstrapPayload,
    options: &SceneEvalPackBuildOptions,
    status: SceneEvalPackStatus,
) -> SceneEvalPackResponse {
    let scope = payload.bootstrap_scope().to_string();
    SceneEvalPackResponse {
        status,
        client_revision: payload.client_revision().to_string(),
        scope: scope.clone(),
        query_fingerprint: options.fingerprint.clone(),
        metrics: payload.metrics().to_vec(),
        bootstrap_scopes: payload.bootstrap_scopes().to_vec(),
        layout_budget_manifest: payload.layout_budget_manifest().cloned(),
        neighbor_hops: options.neighbor_hops,
        eval_layer_refs: eval_layer_refs_for_scope(scope.as_str()),
        delivery_class_counts: None,
        bootstrap_scope: Some(payload.bootstrap_scope().to_string()),
        target_file: Some(payload.target_file().to_string()),
        compile_epoch: Some(payload.compile_epoch().to_string()),
        data_generation: Some(payload.data_generation().to_string()),
        app_id: Some(payload.app_id().to_string()),
    }
}

pub fn build_scene_eval_pack(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    options: SceneEvalPackBuildOptions,
) -> SceneEvalPackResponse {
    let bootstrap = bootstrap_embed_status(workspace_root, app_id, scene_id);
    if bootstrap.allowed && bootstrap.reason == "no_client_bootstrap_required" {
        let payload = empty_client_bootstrap_payload(workspace_root, app_id, scene_id);
        return pack_from_payload(payload, &options, SceneEvalPackStatus::PackHit);
    }
    let Some(payload) = build_client_bootstrap_payload(workspace_root, app_id, scene_id) else {
        return SceneEvalPackResponse {
            status: SceneEvalPackStatus::PackMiss,
            client_revision: NO_CLIENT_BOOTSTRAP_REVISION.to_string(),
            scope: scene_id.to_string(),
            query_fingerprint: options.fingerprint.clone(),
            metrics: Vec::new(),
            bootstrap_scopes: Vec::new(),
            layout_budget_manifest: None,
            neighbor_hops: options.neighbor_hops,
            eval_layer_refs: eval_layer_refs_for_scope(scene_id),
            delivery_class_counts: Some(BTreeMap::new()),
            bootstrap_scope: None,
            target_file: None,
            compile_epoch: None,
            data_generation: None,
            app_id: Some(app_id.to_string()),
        };
    };
    let requested_revision = options
        .client_revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let status = if let Some(expected) = requested_revision {
        if expected != payload.client_revision()
            && expected != NO_CLIENT_BOOTSTRAP_REVISION
        {
            SceneEvalPackStatus::RevisionStale
        } else {
            SceneEvalPackStatus::PackHit
        }
    } else {
        SceneEvalPackStatus::PackHit
    };
    let mut pack = pack_from_payload(payload, &options, status);
    pack.delivery_class_counts = Some(delivery_class_counts_for_scope(
        workspace_root,
        app_id,
        scene_id,
    ));
    pack
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scope_returns_pack_hit_with_no_bootstrap_revision() {
        let workspace = std::env::temp_dir().join(format!(
            "mei-scene-eval-pack-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(workspace.as_path());
        let app_root = workspace.join("apps").join("missing-app");
        let env_dir = app_root.join("env/WS-20260101.0");
        let current = app_root.join("env/current");
        std::fs::create_dir_all(env_dir.join("var")).expect("env var");
        std::fs::create_dir_all(current.parent().expect("env parent")).expect("env root");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&env_dir, &current).expect("symlink env/current");
        #[cfg(not(unix))]
        std::fs::create_dir_all(&current).expect("mkdir env/current");
        let pack = build_scene_eval_pack(
            workspace.as_path(),
            "missing-app",
            "home",
            SceneEvalPackBuildOptions::default(),
        );
        assert_eq!(pack.status, SceneEvalPackStatus::PackHit);
        assert_eq!(pack.client_revision, NO_CLIENT_BOOTSTRAP_REVISION);
    }
}
