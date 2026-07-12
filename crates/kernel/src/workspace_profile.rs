use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{discover_apps_with_config, RuntimeMode, RuntimePlanApp, WorkspaceConfig};

const DEFAULT_PROFILE_ID: &str = "default";

#[derive(Debug, Error)]
pub enum WorkspaceProfileError {
    #[error("invalid workspace profile id")]
    InvalidId,
    #[error("workspace profile not found")]
    NotFound,
    #[error("workspace profile path is not a regular JSON file")]
    InvalidPath,
    #[error("workspace profile JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("workspace profile schema is invalid")]
    InvalidSchema(Vec<WorkspaceProfileValidationIssue>),
    #[error("workspace profile revision conflict")]
    RevisionConflict {
        expected: Option<String>,
        current: Option<String>,
    },
    #[error("workspace profile I/O failed: {0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProfileValidationIssue {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProfileValidation {
    pub valid: bool,
    #[serde(default)]
    pub issues: Vec<WorkspaceProfileValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProfileSummary {
    pub id: String,
    pub file: String,
    pub revision: String,
    pub valid: bool,
    #[serde(default)]
    pub issues: Vec<WorkspaceProfileValidationIssue>,
    pub label: Option<String>,
    pub default_app: Option<String>,
    pub default_mode: RuntimeMode,
    pub configured_app_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProfileDocument {
    pub id: String,
    pub file: String,
    pub revision: String,
    pub config: Value,
    pub validation: WorkspaceProfileValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProfileDryRun {
    pub profile: WorkspaceProfileSummary,
    pub default_app: Option<String>,
    pub discovered_apps: Vec<String>,
    pub apps: Vec<RuntimePlanAppDryRun>,
    #[serde(default)]
    pub unresolved_scopes: Vec<RuntimePlanReferenceCheck>,
    #[serde(default)]
    pub unresolved_metrics: Vec<RuntimePlanReferenceCheck>,
    #[serde(default)]
    pub deferred: Vec<RuntimePlanReferenceCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePlanAppDryRun {
    pub app_id: String,
    pub discovered: bool,
    pub default_mode: RuntimeMode,
    pub target_rule_count: usize,
    pub metric_override_count: usize,
    pub target_modes: BTreeMap<String, usize>,
    pub metric_modes: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePlanReferenceCheck {
    pub app_id: String,
    pub kind: String,
    pub value: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct WorkspaceProfileService {
    workspace_root: PathBuf,
}

impl WorkspaceProfileService {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    pub fn list(&self) -> Result<Vec<WorkspaceProfileSummary>, WorkspaceProfileError> {
        let mut ids = Vec::new();
        let default_path = self.workspace_root.join("workspace.json");
        if regular_json_file(&default_path) {
            ids.push(DEFAULT_PROFILE_ID.to_string());
        }
        let configs_dir = self.workspace_root.join("configs");
        if configs_dir.exists() {
            self.ensure_configs_dir(false)?;
            let entries = fs::read_dir(&configs_dir)
                .map_err(|error| WorkspaceProfileError::Io(error.to_string()))?;
            for entry in entries {
                let entry = entry.map_err(|error| WorkspaceProfileError::Io(error.to_string()))?;
                let path = entry.path();
                if !regular_json_file(&path)
                    || path.extension().and_then(|value| value.to_str()) != Some("json")
                {
                    continue;
                }
                let Some(id) = path.file_stem().and_then(|value| value.to_str()) else {
                    continue;
                };
                if valid_config_id(id) {
                    ids.push(id.to_string());
                }
            }
        }
        ids.sort_by(|left, right| {
            match (
                left.as_str() == DEFAULT_PROFILE_ID,
                right.as_str() == DEFAULT_PROFILE_ID,
            ) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => left.cmp(right),
            }
        });
        ids.dedup();
        ids.into_iter().map(|id| self.summary(&id)).collect()
    }

    pub fn read(&self, id: &str) -> Result<WorkspaceProfileDocument, WorkspaceProfileError> {
        let path = self.profile_path(id, false)?;
        let bytes = read_regular_file(&path)?;
        let revision = revision_hash(&bytes);
        let config: Value = serde_json::from_slice(&bytes)
            .map_err(|error| WorkspaceProfileError::InvalidJson(error.to_string()))?;
        let validation = validate_workspace_value(&config);
        Ok(WorkspaceProfileDocument {
            id: normalize_id(id)?.to_string(),
            file: self.relative_file(id)?,
            revision,
            config,
            validation,
        })
    }

    pub fn validate(&self, id: &str) -> Result<WorkspaceProfileValidation, WorkspaceProfileError> {
        Ok(self.read(id)?.validation)
    }

    pub fn validate_config(&self, config: &Value) -> WorkspaceProfileValidation {
        validate_workspace_value(config)
    }

    pub fn save(
        &self,
        id: &str,
        expected_revision: Option<&str>,
        config: Value,
    ) -> Result<WorkspaceProfileDocument, WorkspaceProfileError> {
        let validation = validate_workspace_value(&config);
        if !validation.valid {
            return Err(WorkspaceProfileError::InvalidSchema(validation.issues));
        }
        let path = self.profile_path(id, true)?;
        let current_revision = if path.exists() {
            Some(revision_hash(&read_regular_file(&path)?))
        } else {
            None
        };
        let expected = expected_revision.map(str::to_string);
        if expected != current_revision {
            return Err(WorkspaceProfileError::RevisionConflict {
                expected,
                current: current_revision,
            });
        }
        let mut raw = serde_json::to_string_pretty(&config)
            .map_err(|error| WorkspaceProfileError::InvalidJson(error.to_string()))?;
        raw.push('\n');
        crate::mei_config::write_string_atomically(&path, &raw)
            .map_err(|error| WorkspaceProfileError::Io(error.to_string()))?;
        self.read(id)
    }

    pub fn dry_run(&self, id: &str) -> Result<WorkspaceProfileDryRun, WorkspaceProfileError> {
        let document = self.read(id)?;
        self.dry_run_document(document)
    }

    pub fn dry_run_config(
        &self,
        id: &str,
        config: Value,
    ) -> Result<WorkspaceProfileDryRun, WorkspaceProfileError> {
        let mut document = self.read(id)?;
        document.validation = validate_workspace_value(&config);
        document.config = config;
        self.dry_run_document(document)
    }

    fn dry_run_document(
        &self,
        document: WorkspaceProfileDocument,
    ) -> Result<WorkspaceProfileDryRun, WorkspaceProfileError> {
        if !document.validation.valid {
            return Err(WorkspaceProfileError::InvalidSchema(
                document.validation.issues,
            ));
        }
        let config: WorkspaceConfig = serde_json::from_value(document.config.clone())
            .map_err(|error| WorkspaceProfileError::InvalidJson(error.to_string()))?;
        let runtime_plan = config.deploy.effective_runtime_plan();
        let discovered = discover_apps_with_config(&self.workspace_root, &config)
            .map_err(|error| WorkspaceProfileError::Io(error.to_string()))?;
        let discovered_ids = discovered
            .iter()
            .map(|app| app.id.clone())
            .collect::<BTreeSet<_>>();
        let mut app_ids = discovered_ids.clone();
        app_ids.extend(
            runtime_plan
                .apps
                .keys()
                .filter(|id| id.as_str() != "*")
                .cloned(),
        );

        let mut unresolved_scopes = Vec::new();
        let mut unresolved_metrics = Vec::new();
        let mut deferred = Vec::new();
        let mut apps = Vec::new();
        for app_id in app_ids {
            let plan = runtime_plan
                .apps
                .get(&app_id)
                .or_else(|| runtime_plan.apps.get("*"));
            let empty = RuntimePlanApp::default();
            let plan = plan.unwrap_or(&empty);
            apps.push(app_dry_run(
                &app_id,
                discovered_ids.contains(&app_id),
                runtime_plan.default_mode,
                plan,
            ));
            if !discovered_ids.contains(&app_id) {
                for target in &plan.targets {
                    unresolved_scopes.push(reference_check(
                        &app_id,
                        "scope",
                        &target.scope,
                        "app_not_discovered",
                    ));
                }
                for metric_id in plan.metric_overrides.keys() {
                    unresolved_metrics.push(reference_check(
                        &app_id,
                        "metric",
                        metric_id,
                        "app_not_discovered",
                    ));
                }
                continue;
            }
            let Some(meta) = discovered.iter().find(|app| app.id == app_id) else {
                continue;
            };
            let evidence = collect_app_evidence(Path::new(&meta.root));
            for target in &plan.targets {
                let scope = normalize_scope(&target.scope);
                if evidence.scope_match(&scope) {
                    continue;
                }
                if evidence.compiled {
                    unresolved_scopes.push(reference_check(
                        &app_id,
                        "scope",
                        &target.scope,
                        "not_found_in_current_compiled_structure",
                    ));
                } else {
                    deferred.push(reference_check(
                        &app_id,
                        "scope",
                        &target.scope,
                        "compiled_structure_unavailable",
                    ));
                }
            }
            for metric_id in plan.metric_overrides.keys() {
                if evidence.metrics.contains(metric_id) {
                    continue;
                }
                if evidence.compiled {
                    unresolved_metrics.push(reference_check(
                        &app_id,
                        "metric",
                        metric_id,
                        "not_found_in_current_compiled_or_source_structure",
                    ));
                } else {
                    deferred.push(reference_check(
                        &app_id,
                        "metric",
                        metric_id,
                        "compiled_metric_index_unavailable",
                    ));
                }
            }
        }

        let profile = summary_from_document(&document)?;
        Ok(WorkspaceProfileDryRun {
            profile,
            default_app: config.workspace.default_app,
            discovered_apps: discovered.into_iter().map(|app| app.id).collect(),
            apps,
            unresolved_scopes,
            unresolved_metrics,
            deferred,
        })
    }

    fn summary(&self, id: &str) -> Result<WorkspaceProfileSummary, WorkspaceProfileError> {
        let path = self.profile_path(id, false)?;
        let bytes = read_regular_file(&path)?;
        let revision = revision_hash(&bytes);
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(config) => {
                let validation = validate_workspace_value(&config);
                summary_from_document(&WorkspaceProfileDocument {
                    id: normalize_id(id)?.to_string(),
                    file: self.relative_file(id)?,
                    revision,
                    config,
                    validation,
                })
            }
            Err(error) => Ok(WorkspaceProfileSummary {
                id: normalize_id(id)?.to_string(),
                file: self.relative_file(id)?,
                revision,
                valid: false,
                issues: vec![WorkspaceProfileValidationIssue {
                    code: "invalid_json".to_string(),
                    path: "$".to_string(),
                    message: error.to_string(),
                }],
                label: None,
                default_app: None,
                default_mode: RuntimeMode::Hot,
                configured_app_count: 0,
            }),
        }
    }

    fn relative_file(&self, id: &str) -> Result<String, WorkspaceProfileError> {
        let id = normalize_id(id)?;
        Ok(if id == DEFAULT_PROFILE_ID {
            "workspace.json".to_string()
        } else {
            format!("configs/{id}.json")
        })
    }

    fn profile_path(
        &self,
        id: &str,
        create_parent: bool,
    ) -> Result<PathBuf, WorkspaceProfileError> {
        let id = normalize_id(id)?;
        let root = fs::canonicalize(&self.workspace_root)
            .map_err(|error| WorkspaceProfileError::Io(error.to_string()))?;
        if id == DEFAULT_PROFILE_ID {
            return Ok(root.join("workspace.json"));
        }
        let configs = self.ensure_configs_dir(create_parent)?;
        if !configs.starts_with(&root) {
            return Err(WorkspaceProfileError::InvalidPath);
        }
        Ok(configs.join(format!("{id}.json")))
    }

    fn ensure_configs_dir(&self, create: bool) -> Result<PathBuf, WorkspaceProfileError> {
        let path = self.workspace_root.join("configs");
        if !path.exists() && create {
            fs::create_dir(&path).map_err(|error| WorkspaceProfileError::Io(error.to_string()))?;
        }
        if !path.exists() {
            let root = fs::canonicalize(&self.workspace_root)
                .map_err(|error| WorkspaceProfileError::Io(error.to_string()))?;
            return Ok(root.join("configs"));
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| WorkspaceProfileError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(WorkspaceProfileError::InvalidPath);
        }
        fs::canonicalize(path).map_err(|error| WorkspaceProfileError::Io(error.to_string()))
    }
}

fn normalize_id(id: &str) -> Result<&str, WorkspaceProfileError> {
    let id = id.trim();
    if id == DEFAULT_PROFILE_ID || valid_config_id(id) {
        Ok(id)
    } else {
        Err(WorkspaceProfileError::InvalidId)
    }
}

fn valid_config_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && !id.starts_with('.')
        && id != DEFAULT_PROFILE_ID
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn regular_json_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, WorkspaceProfileError> {
    if !path.exists() {
        return Err(WorkspaceProfileError::NotFound);
    }
    if !regular_json_file(path) {
        return Err(WorkspaceProfileError::InvalidPath);
    }
    fs::read(path).map_err(|error| WorkspaceProfileError::Io(error.to_string()))
}

fn revision_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_workspace_value(value: &Value) -> WorkspaceProfileValidation {
    let mut issues = Vec::new();
    match serde_json::from_value::<WorkspaceConfig>(value.clone()) {
        Ok(config) => {
            if let Some(runtime_plan) = &config.deploy.runtime_plan {
                if let Err(error) = runtime_plan.validate() {
                    issues.push(WorkspaceProfileValidationIssue {
                        code: "runtime_plan_invalid".to_string(),
                        path: "deploy.runtimePlan".to_string(),
                        message: error.to_string(),
                    });
                }
            }
        }
        Err(error) => issues.push(WorkspaceProfileValidationIssue {
            code: "schema_invalid".to_string(),
            path: schema_error_path(&error.to_string()),
            message: error.to_string(),
        }),
    }
    WorkspaceProfileValidation {
        valid: issues.is_empty(),
        issues,
    }
}

fn schema_error_path(message: &str) -> String {
    if message.contains("defaultMode") {
        "deploy.runtimePlan.defaultMode".to_string()
    } else if message.contains("metricOverrides") {
        "deploy.runtimePlan.apps.*.metricOverrides".to_string()
    } else if message.contains("targets") || message.contains("mode") {
        "deploy.runtimePlan.apps.*.targets".to_string()
    } else {
        "$".to_string()
    }
}

fn summary_from_document(
    document: &WorkspaceProfileDocument,
) -> Result<WorkspaceProfileSummary, WorkspaceProfileError> {
    let config = serde_json::from_value::<WorkspaceConfig>(document.config.clone()).ok();
    let runtime_plan = config
        .as_ref()
        .map(|config| config.deploy.effective_runtime_plan())
        .unwrap_or_default();
    Ok(WorkspaceProfileSummary {
        id: document.id.clone(),
        file: document.file.clone(),
        revision: document.revision.clone(),
        valid: document.validation.valid,
        issues: document.validation.issues.clone(),
        label: config
            .as_ref()
            .and_then(|config| config.workspace.label.clone()),
        default_app: config
            .as_ref()
            .and_then(|config| config.workspace.default_app.clone()),
        default_mode: runtime_plan.default_mode,
        configured_app_count: runtime_plan
            .apps
            .keys()
            .filter(|id| id.as_str() != "*")
            .count(),
    })
}

fn app_dry_run(
    app_id: &str,
    discovered: bool,
    default_mode: RuntimeMode,
    plan: &RuntimePlanApp,
) -> RuntimePlanAppDryRun {
    RuntimePlanAppDryRun {
        app_id: app_id.to_string(),
        discovered,
        default_mode,
        target_rule_count: plan.targets.len(),
        metric_override_count: plan.metric_overrides.len(),
        target_modes: mode_counts(plan.targets.iter().map(|target| target.mode)),
        metric_modes: mode_counts(plan.metric_overrides.values().copied()),
    }
}

fn mode_counts(modes: impl Iterator<Item = RuntimeMode>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::from([
        ("frozen".to_string(), 0),
        ("hot".to_string(), 0),
        ("lazy".to_string(), 0),
    ]);
    for mode in modes {
        *counts.entry(mode.slug().to_string()).or_default() += 1;
    }
    counts
}

fn reference_check(
    app_id: &str,
    kind: &str,
    value: &str,
    reason: &str,
) -> RuntimePlanReferenceCheck {
    RuntimePlanReferenceCheck {
        app_id: app_id.to_string(),
        kind: kind.to_string(),
        value: value.to_string(),
        reason: reason.to_string(),
    }
}

fn normalize_scope(value: &str) -> String {
    value
        .trim()
        .trim_matches('/')
        .strip_prefix("content:")
        .or_else(|| value.trim().trim_matches('/').strip_prefix("scope:"))
        .unwrap_or_else(|| value.trim().trim_matches('/'))
        .trim_matches('/')
        .to_string()
}

#[derive(Default)]
struct AppEvidence {
    compiled: bool,
    scopes: BTreeSet<String>,
    metrics: BTreeSet<String>,
}

impl AppEvidence {
    fn scope_match(&self, scope: &str) -> bool {
        scope == "*"
            || self.scopes.iter().any(|candidate| {
                candidate == scope
                    || candidate.starts_with(&format!("{scope}/"))
                    || scope.starts_with(&format!("{candidate}/"))
            })
    }
}

fn collect_app_evidence(app_root: &Path) -> AppEvidence {
    let mut evidence = AppEvidence::default();
    let source_root = app_root.join("src");
    let metric_re =
        Regex::new(r#"(?s)(?:metric|metric_ref)\s*\([^)]*?\bid\s*=\s*["']([^"']+)["']"#)
            .expect("metric regex");
    if source_root.is_dir() {
        for entry in WalkDir::new(&source_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("mei")
            })
        {
            if let Ok(raw) = fs::read_to_string(entry.path()) {
                for captures in metric_re.captures_iter(&raw) {
                    if let Some(metric_id) = captures.get(1) {
                        evidence.metrics.insert(metric_id.as_str().to_string());
                    }
                }
            }
        }
    }

    let artifact_root = app_root
        .join("env/current/build")
        .join("artifacts/compiled_app");
    if artifact_root.is_dir() {
        for entry in WalkDir::new(artifact_root)
            .max_depth(2)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            })
        {
            let Ok(raw) = fs::read(entry.path()) else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<Value>(&raw) else {
                continue;
            };
            evidence.compiled = true;
            collect_compiled_evidence(&value, None, &mut evidence);
        }
    }
    evidence
}

fn collect_compiled_evidence(value: &Value, parent_key: Option<&str>, evidence: &mut AppEvidence) {
    match value {
        Value::Object(map) => {
            if matches!(
                parent_key,
                Some("world_metrics" | "runtime_metric_defs" | "metrics")
            ) {
                evidence.metrics.extend(map.keys().cloned());
            }
            for (key, child) in map {
                if matches!(
                    key.as_str(),
                    "preview_scope" | "previewScope" | "scene_id" | "sceneId"
                ) {
                    if let Some(scope) = child.as_str() {
                        evidence.scopes.insert(normalize_scope(scope));
                    }
                }
                if matches!(key.as_str(), "metric_id" | "metricId") {
                    if let Some(metric_id) = child.as_str() {
                        evidence.metrics.insert(metric_id.to_string());
                    }
                }
                collect_compiled_evidence(child, Some(key), evidence);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_compiled_evidence(child, parent_key, evidence);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_workspace(root: &Path, name: &str, raw: &str) {
        let path = if name == DEFAULT_PROFILE_ID {
            root.join("workspace.json")
        } else {
            fs::create_dir_all(root.join("configs")).expect("configs");
            root.join("configs").join(format!("{name}.json"))
        };
        fs::write(path, raw).expect("write profile");
    }

    #[test]
    fn enumerates_default_and_named_profiles_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_workspace(dir.path(), DEFAULT_PROFILE_ID, "{}");
        write_workspace(dir.path(), "local", "{}");
        fs::write(dir.path().join("configs/ignored.txt"), "{}").expect("ignored");
        let service = WorkspaceProfileService::new(dir.path());
        let ids = service
            .list()
            .expect("list")
            .into_iter()
            .map(|profile| profile.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["default", "local"]);
    }

    #[test]
    fn rejects_profile_path_traversal() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_workspace(dir.path(), DEFAULT_PROFILE_ID, "{}");
        let service = WorkspaceProfileService::new(dir.path());
        assert!(matches!(
            service.read("../workspace"),
            Err(WorkspaceProfileError::InvalidId)
        ));
        assert!(matches!(
            service.read("nested/name"),
            Err(WorkspaceProfileError::InvalidId)
        ));
    }

    #[test]
    fn save_detects_revision_conflict_and_preserves_unknown_fields() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_workspace(
            dir.path(),
            DEFAULT_PROFILE_ID,
            r#"{"future":{"kept":true}}"#,
        );
        let service = WorkspaceProfileService::new(dir.path());
        let current = service.read(DEFAULT_PROFILE_ID).expect("read");
        let conflict = service.save(DEFAULT_PROFILE_ID, Some("stale"), current.config.clone());
        assert!(matches!(
            conflict,
            Err(WorkspaceProfileError::RevisionConflict { .. })
        ));
        let saved = service
            .save(DEFAULT_PROFILE_ID, Some(&current.revision), current.config)
            .expect("save");
        assert_eq!(saved.config["future"]["kept"], true);
    }

    #[test]
    fn invalid_runtime_mode_is_rejected() {
        let value = serde_json::json!({
            "deploy": {
                "runtimePlan": {
                    "defaultMode": "eager",
                    "apps": {}
                }
            }
        });
        let validation = validate_workspace_value(&value);
        assert!(!validation.valid);
        assert_eq!(validation.issues[0].code, "schema_invalid");
    }

    #[test]
    fn legacy_dev_eval_maps_warmup_to_hot_and_eval_to_lazy() {
        let config: WorkspaceConfig = serde_json::from_value(serde_json::json!({
            "deploy": {
                "devEval": {
                    "profile": "scoped",
                    "evalScopes": ["home/t1"],
                    "warmupScopes": ["home/t1/hot"]
                }
            }
        }))
        .expect("config");
        let plan = config.deploy.effective_runtime_plan();
        assert_eq!(plan.default_mode, RuntimeMode::Frozen);
        let targets = &plan.apps["*"].targets;
        assert_eq!(
            targets
                .iter()
                .find(|target| target.scope == "home/t1")
                .map(|target| target.mode),
            Some(RuntimeMode::Lazy)
        );
        assert_eq!(
            targets
                .iter()
                .find(|target| target.scope == "home/t1/hot")
                .map(|target| target.mode),
            Some(RuntimeMode::Hot)
        );
        let serialized = serde_json::to_value(&config).expect("serialize legacy config");
        assert!(serialized["deploy"].get("devEval").is_some());
        assert!(serialized["deploy"].get("runtimePlan").is_none());
    }

    #[test]
    fn explicit_runtime_plan_overrides_legacy_even_when_hot_by_default() {
        let config: WorkspaceConfig = serde_json::from_value(serde_json::json!({
            "deploy": {
                "devEval": {"profile": "static"},
                "runtimePlan": {"defaultMode": "hot"}
            }
        }))
        .expect("config");
        let plan = config.deploy.effective_runtime_plan();
        assert_eq!(plan.default_mode, RuntimeMode::Hot);
        assert!(plan.apps.is_empty());
    }

    #[test]
    fn dry_run_reports_unresolved_and_deferred_references() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("apps/demo/src")).expect("app source");
        fs::write(
            dir.path().join("apps/demo/app.config.json"),
            r#"{"schemaVersion":1}"#,
        )
        .expect("app config");
        fs::write(
            dir.path().join("apps/demo/src/main.mei"),
            r#"mei.metric(id = "known_metric")"#,
        )
        .expect("source");
        write_workspace(
            dir.path(),
            DEFAULT_PROFILE_ID,
            r#"{
                "workspace": {"defaultApp": "demo"},
                "deploy": {"runtimePlan": {
                    "defaultMode": "frozen",
                    "apps": {"demo": {
                        "targets": [{"scope": "home/t1", "mode": "hot"}],
                        "metricOverrides": {
                            "known_metric": "hot",
                            "unknown_metric": "lazy"
                        }
                    }}
                }}
            }"#,
        );
        let service = WorkspaceProfileService::new(dir.path());
        let dry_run = service.dry_run(DEFAULT_PROFILE_ID).expect("dry run");
        assert_eq!(dry_run.default_app.as_deref(), Some("demo"));
        assert_eq!(dry_run.apps[0].target_rule_count, 1);
        assert_eq!(dry_run.apps[0].metric_override_count, 2);
        assert!(dry_run.unresolved_scopes.is_empty());
        assert!(dry_run.unresolved_metrics.is_empty());
        assert_eq!(dry_run.deferred.len(), 2);
        assert!(dry_run.deferred.iter().any(|item| item.value == "home/t1"));
        assert!(dry_run
            .deferred
            .iter()
            .any(|item| item.value == "unknown_metric"));
    }

    #[test]
    fn dry_run_uses_current_compiled_structure_for_unresolved_references() {
        let dir = tempfile::tempdir().expect("tempdir");
        let app_root = dir.path().join("apps/demo");
        fs::create_dir_all(app_root.join("src")).expect("app source");
        fs::write(app_root.join("app.config.json"), "{}").expect("app config");
        fs::write(app_root.join("src/main.mei"), "").expect("source");
        let artifacts = app_root.join("env/current/build/artifacts/compiled_app");
        fs::create_dir_all(&artifacts).expect("compiled artifacts");
        fs::write(
            artifacts.join("current.json"),
            r#"{
                "ui_layout_index": {
                    "nodes": {"scope": {"preview_scope": "home/t1/known"}}
                },
                "world_metrics": {"known_metric": {}}
            }"#,
        )
        .expect("compiled structure");
        write_workspace(
            dir.path(),
            DEFAULT_PROFILE_ID,
            r#"{
                "deploy": {"runtimePlan": {
                    "defaultMode": "frozen",
                    "apps": {"demo": {
                        "targets": [{"scope": "home/t1/missing", "mode": "hot"}],
                        "metricOverrides": {"missing_metric": "lazy"}
                    }}
                }}
            }"#,
        );

        let dry_run = WorkspaceProfileService::new(dir.path())
            .dry_run(DEFAULT_PROFILE_ID)
            .expect("dry run");
        assert_eq!(dry_run.unresolved_scopes.len(), 1);
        assert_eq!(dry_run.unresolved_metrics.len(), 1);
        assert!(dry_run.deferred.is_empty());
    }

    #[test]
    fn draft_dry_run_does_not_write_profile() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_workspace(
            dir.path(),
            DEFAULT_PROFILE_ID,
            r#"{"deploy":{"runtimePlan":{"defaultMode":"hot"}}}"#,
        );
        let service = WorkspaceProfileService::new(dir.path());
        let before = service.read(DEFAULT_PROFILE_ID).expect("read before");
        let draft = serde_json::json!({
            "deploy": {"runtimePlan": {"defaultMode": "lazy"}}
        });

        let preview = service
            .dry_run_config(DEFAULT_PROFILE_ID, draft)
            .expect("draft dry run");
        let after = service.read(DEFAULT_PROFILE_ID).expect("read after");

        assert_eq!(preview.profile.default_mode, RuntimeMode::Lazy);
        assert_eq!(before.revision, after.revision);
        assert_eq!(after.config["deploy"]["runtimePlan"]["defaultMode"], "hot");
    }
}
