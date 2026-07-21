use std::sync::Arc;

use crate::http::compile_cache::{RuntimeAccessPolicies, RuntimeArtifactPolicy};
use crate::http::observation::CompileObservation;
use crate::http::pages::metric_api::assembly::MetricQueryGroupRequest;
use crate::http::pages::scene_qualified::SceneQueryCoords;
use mei_lang_kernel::{FilterIntent, QueryState};

pub use axum::http::StatusCode;
pub use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(super) struct MetricQueryExecutionContext<'a> {
    pub(super) app_id: &'a str,
    pub(super) source_root: &'a std::path::Path,
    pub(super) app_root: &'a std::path::Path,
    pub(super) compiled: &'a mei_lang_kernel::CompiledApp,
    pub(super) coords: &'a SceneQueryCoords,
    pub(super) scene_id: &'a str,
    pub(super) scene_path: Option<&'a str>,
    pub(super) compile_observation: CompileObservation,
    pub(super) compile_revision: &'a str,
    pub(super) effective_query_state: &'a QueryState,
    pub(super) filter_intents: &'a [FilterIntent],
    pub(super) access_artifact_only: bool,
    pub(super) runtime_policy: RuntimeArtifactPolicy,
    pub(super) access_policies: RuntimeAccessPolicies,
    pub(super) compile_correctness_fallback: bool,
    pub(super) compile_artifact_backfilled: bool,
}

#[derive(Debug, Clone)]
pub(super) struct MetricQueryExecutionShared {
    pub(super) app_id: String,
    pub(super) source_root: std::path::PathBuf,
    pub(super) app_root: std::path::PathBuf,
    pub(super) compiled: Arc<mei_lang_kernel::CompiledApp>,
    pub(super) coords: SceneQueryCoords,
    pub(super) scene_id: String,
    pub(super) scene_path: Option<String>,
    pub(super) compile_observation: CompileObservation,
    pub(super) compile_revision: String,
    pub(super) effective_query_state: QueryState,
    pub(super) filter_intents: Vec<FilterIntent>,
    pub(super) access_artifact_only: bool,
    pub(super) runtime_policy: RuntimeArtifactPolicy,
    pub(super) access_policies: RuntimeAccessPolicies,
    pub(super) compile_correctness_fallback: bool,
    pub(super) compile_artifact_backfilled: bool,
}

impl MetricQueryExecutionShared {
    pub(super) fn as_borrowed(&self) -> MetricQueryExecutionContext<'_> {
        MetricQueryExecutionContext {
            app_id: &self.app_id,
            source_root: self.source_root.as_path(),
            app_root: self.app_root.as_path(),
            compiled: self.compiled.as_ref(),
            coords: &self.coords,
            scene_id: &self.scene_id,
            scene_path: self.scene_path.as_deref(),
            compile_observation: self.compile_observation.clone(),
            compile_revision: &self.compile_revision,
            effective_query_state: &self.effective_query_state,
            filter_intents: &self.filter_intents,
            access_artifact_only: self.access_artifact_only,
            runtime_policy: self.runtime_policy,
            access_policies: self.access_policies,
            compile_correctness_fallback: self.compile_correctness_fallback,
            compile_artifact_backfilled: self.compile_artifact_backfilled,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct MergedMetricGroupRequest {
    pub(super) request: MetricQueryGroupRequest,
    pub(super) original_indexes: Vec<usize>,
}
