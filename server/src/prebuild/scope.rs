use super::prelude::*;
use super::*;

#[derive(Debug, Clone)]
pub(crate) struct CompileScope {
    pub(crate) requested_scene_id: Option<String>,
    pub(crate) requested_target_file: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AggregatedWarmupRequest {
    pub(crate) scope: CompileScope,
    pub(crate) dataset_id: String,
    pub(crate) priority: WarmupRequestPriority,
    pub(crate) metric_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PrebuildManifestPlan {
    pub(crate) initial_scope_count: usize,
    pub(crate) hot_scopes: Vec<CompileScope>,
    pub(crate) deferred_scopes: Vec<CompileScope>,
    pub(crate) warmup_requests: Vec<AggregatedWarmupRequest>,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedMetricWorkset {
    pub(crate) logical_node_id: String,
    pub(crate) scope_id: String,
    pub(crate) materialization_key: String,
    pub(crate) dataset_selector: String,
    pub(crate) owner_resource_id: String,
    pub(crate) requested_metric_ids: Vec<String>,
    pub(crate) request_all_metrics: bool,
    pub(crate) scene_id: String,
    pub(crate) scene_path: Option<String>,
    pub(crate) dependency_revision_key: String,
    pub(crate) response_cache_key: String,
    pub(crate) shared_cache_key: String,
    pub(crate) covered_metric_ids: BTreeSet<String>,
    pub(crate) defs_for_hydrate: Arc<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedDataframeArtifact {
    pub(crate) logical_node_id: String,
    pub(crate) scope_id: String,
    pub(crate) materialization_key: String,
    pub(crate) artifact_key: String,
    pub(crate) shared_artifact_key: String,
    pub(crate) owner_resource_id: String,
    pub(crate) resource_selector_id: String,
    pub(crate) dataframe_metric_id: String,
    pub(crate) resolved_metric_id: String,
    pub(crate) page_size: usize,
    pub(crate) scene_id: String,
    pub(crate) scene_path: Option<String>,
    pub(crate) dependency_revision_key: String,
    pub(crate) scope_metric_token: String,
    pub(crate) defs_for_hydrate: Arc<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ScopeArtifactPlan {
    pub(crate) metric_worksets: Vec<PlannedMetricWorkset>,
    pub(crate) dataframe_artifacts: Vec<PlannedDataframeArtifact>,
}

#[derive(Debug, Clone)]
pub(crate) struct WarmupScopeBatch<'a> {
    pub(crate) scope: CompileScope,
    pub(crate) requests: Vec<&'a AggregatedWarmupRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WarmupRequestPriority {
    Critical,
    Deferred,
}

pub(crate) fn warning_quoted_value(error: &str, marker: &str) -> Option<String> {
    let start = error.find(marker)? + marker.len();
    let rest = error.get(start..)?;
    let end = rest.find('`')?;
    let value = rest.get(..end)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub(crate) fn warning_category_from_error(error: &str) -> (&'static str, Option<String>, Option<String>) {
    if error.contains("locate warmup dataset `") {
        return (
            "warmup_dataset_locate_failed",
            warning_quoted_value(error, "locate warmup dataset `"),
            None,
        );
    }
    if error.contains("build metric response artifact for dataset `") {
        return (
            "metric_response_eval_failed",
            warning_quoted_value(error, "build metric response artifact for dataset `"),
            None,
        );
    }
    if error.contains("build metric dataframe artifact for dataset `") {
        return (
            "metric_dataframe_eval_failed",
            warning_quoted_value(error, "build metric dataframe artifact for dataset `"),
            warning_quoted_value(error, "metric `"),
        );
    }
    if error.contains("does not cover all declared metrics") {
        return (
            "artifact_coverage_miss",
            warning_quoted_value(error, "dataset `"),
            None,
        );
    }
    if error.contains("missing metric response artifact")
        || error.contains("missing metric dataframe artifact")
    {
        return ("artifact_index_miss", warning_quoted_value(error, "dataset `"), None);
    }
    if error.contains("metric response index preload failed") {
        return ("metric_response_index_preload_failed", None, None);
    }
    ("prebuild_warning", None, None)
}

pub(crate) fn build_prebuild_warning(
    phase: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    dataset_selector: Option<&str>,
    metric_id: Option<&str>,
    compile_revision: Option<&str>,
    cache_key: Option<&str>,
    error: impl Into<String>,
) -> PrebuildWarningReport {
    let error = error.into();
    let (category, inferred_dataset, inferred_metric) = warning_category_from_error(error.as_str());
    let scene_id = scene_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let target_file = target_file
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let dataset_selector = dataset_selector
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(inferred_dataset);
    let metric_id = metric_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(inferred_metric);
    let compile_revision = compile_revision
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let cache_key = cache_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let message = match (scene_id.as_deref(), target_file.as_deref(), dataset_selector.as_deref()) {
        (Some(scene), Some(target), Some(dataset)) => {
            format!("{phase} scene=`{scene}` target=`{target}` dataset=`{dataset}` failed: {error}")
        }
        (Some(scene), Some(target), None) => {
            format!("{phase} scene=`{scene}` target=`{target}` failed: {error}")
        }
        (Some(scene), None, Some(dataset)) => {
            format!("{phase} scene=`{scene}` dataset=`{dataset}` failed: {error}")
        }
        (None, None, Some(dataset)) => format!("{phase} dataset=`{dataset}` failed: {error}"),
        _ => format!("{phase} failed: {error}"),
    };
    PrebuildWarningReport {
        phase: phase.to_string(),
        category: category.to_string(),
        message,
        scene_id,
        target_file,
        dataset_selector,
        metric_id,
        compile_revision,
        cache_key,
        error,
    }
}

impl CompileScope {
    pub(crate) fn default_scope() -> Self {
        Self {
            requested_scene_id: None,
            requested_target_file: None,
        }
    }

    pub(crate) fn to_options(&self) -> CompileOptions {
        let canonical = self.canonicalized();
        CompileOptions {
            scene: canonical.requested_scene_id,
            preview_target: canonical.requested_target_file,
        }
    }

    pub(crate) fn key(&self) -> String {
        let canonical = self.canonicalized();
        format!(
            "{}|{}",
            canonical.requested_scene_id.as_deref().unwrap_or(""),
            canonical.requested_target_file.as_deref().unwrap_or("")
        )
    }

    pub(crate) fn to_world_scope(&self) -> WorldScope {
        let canonical = self.canonicalized();
        WorldScope {
            scene_id: canonical.requested_scene_id,
            target_file: canonical.requested_target_file,
        }
    }

    pub(crate) fn canonicalized(&self) -> Self {
        let requested_scene_id = self
            .requested_scene_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let requested_target_file = self
            .requested_target_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter(|target| is_script_target(target))
            .map(str::to_string);
        Self {
            requested_scene_id,
            requested_target_file,
        }
    }
}
