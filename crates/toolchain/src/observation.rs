use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CompileObservation {
    pub app_id: String,
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_revision: Option<String>,
    pub compile_ms: u64,
    pub compile_cache_hit: u64,
    pub compile_cache_lookup_ms: u64,
    pub compile_cache_lock_wait_ms: u64,
}

impl CompileObservation {
    pub fn for_world_bundle(
        app_id: &str,
        scene_id: &str,
        target_file: Option<&str>,
        load_world_bundle_ms: u64,
    ) -> Self {
        Self {
            app_id: app_id.to_string(),
            scene_id: scene_id.to_string(),
            target_file: target_file
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            compile_revision: None,
            compile_ms: load_world_bundle_ms,
            compile_cache_hit: 0,
            compile_cache_lookup_ms: 0,
            compile_cache_lock_wait_ms: 0,
        }
    }

    pub fn write_perf(&self, perf: &mut BTreeMap<String, u64>) {
        perf.insert("compile_ms".to_string(), self.compile_ms);
        perf.insert("compile_cache_hit".to_string(), self.compile_cache_hit);
        perf.insert(
            "compile_cache_lookup_ms".to_string(),
            self.compile_cache_lookup_ms,
        );
        perf.insert(
            "compile_cache_lock_wait_ms".to_string(),
            self.compile_cache_lock_wait_ms,
        );
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct EvalObservation {
    pub response_cache_hit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_cache_key_hash: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub counters: BTreeMap<String, u64>,
}

impl EvalObservation {
    pub fn new(response_cache_hit: bool) -> Self {
        Self {
            response_cache_hit,
            response_cache_key_hash: None,
            counters: BTreeMap::new(),
        }
    }

    pub fn insert_counter(&mut self, key: impl Into<String>, value: u64) {
        self.counters.insert(key.into(), value);
    }

    pub fn write_perf(&self, perf: &mut BTreeMap<String, u64>) {
        perf.insert(
            "response_cache_hit".to_string(),
            u64::from(self.response_cache_hit),
        );
        if let Some(hash) = self.response_cache_key_hash {
            perf.insert("response_cache_key_hash".to_string(), hash);
        }
        for (key, value) in &self.counters {
            perf.insert(key.clone(), *value);
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ExposureManifest {
    pub app_id: String,
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub http_apis: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_schema_version: Option<String>,
}

impl ExposureManifest {
    pub fn for_scene_scope(
        app_id: &str,
        scene_id: &str,
        target_file: Option<&str>,
        query_schema_version: Option<&str>,
    ) -> Self {
        let app = app_id.trim().trim_start_matches('/');
        Self {
            app_id: app.to_string(),
            scene_id: scene_id.to_string(),
            target_file: target_file
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            http_apis: vec![
                format!("/api/datasets/query/{app}"),
                format!("/api/datasets/metrics/{app}"),
                format!("/api/world/context/{app}"),
                format!("/api/world/runtime/{app}"),
            ],
            resource_tools: vec![
                "dataset_query".to_string(),
                "dataset_metric".to_string(),
                "resource_list".to_string(),
                "resource_get".to_string(),
                "resource_runtime_peek".to_string(),
            ],
            runtime_capabilities: vec![
                "rows_query(scene_qualified)".to_string(),
                "metric_query(scene_qualified)".to_string(),
            ],
            query_schema_version: query_schema_version.map(str::to_string),
        }
    }
}
