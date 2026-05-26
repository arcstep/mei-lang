use std::collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde_json::Value;

use crate::eval::evaluate_mei_file;
use crate::model::CompiledSceneRoute;

use super::catalog::{extract_from_dataset_tokens, extract_metric_ref_tokens};
use super::entry_payload::collect_ref_scene_files_from_value;
use super::scene_payload_cache::file_mtime_ms;

static FILE_CONTENT_HASH_CACHE: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
static FILE_CONTENT_HASH_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static FILE_CONTENT_HASH_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static DEPENDENCY_GRAPH_CACHE: Mutex<BTreeMap<String, DependencyGraph>> =
    Mutex::new(BTreeMap::new());
static DEPENDENCY_GRAPH_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static DEPENDENCY_GRAPH_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
const MAX_FILE_CONTENT_HASH_CACHE_ENTRIES: usize = 512;
const MAX_DEPENDENCY_GRAPH_CACHE_ENTRIES: usize = 64;

#[derive(Debug, Clone, Default)]
pub(crate) struct DependencyGraph {
    route_targets: BTreeSet<String>,
    target_closures: BTreeMap<String, BTreeSet<String>>,
    file_dependents: BTreeMap<String, BTreeSet<String>>,
    edge_count: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DependencyGraphStats {
    pub route_roots: usize,
    pub unique_files: usize,
    pub edges: usize,
    pub max_closure: usize,
}

impl DependencyGraph {
    pub(crate) fn build_cached(
        app_root: &Path,
        app_decls: &Value,
        routes: &[CompiledSceneRoute],
    ) -> DependencyGraph {
        let key = dependency_graph_cache_key(app_root, routes);
        if let Ok(cache) = DEPENDENCY_GRAPH_CACHE.lock() {
            if let Some(graph) = cache.get(&key).cloned() {
                DEPENDENCY_GRAPH_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                return graph;
            }
        }

        DEPENDENCY_GRAPH_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        let graph = Self::build(app_root, app_decls, routes);
        if let Ok(mut cache) = DEPENDENCY_GRAPH_CACHE.lock() {
            if cache.len() >= MAX_DEPENDENCY_GRAPH_CACHE_ENTRIES {
                cache.clear();
            }
            cache.insert(key, graph.clone());
        }
        graph
    }

    pub(crate) fn build(
        app_root: &Path,
        app_decls: &Value,
        routes: &[CompiledSceneRoute],
    ) -> DependencyGraph {
        let mut route_targets = BTreeSet::<String>::new();
        let mut memo = BTreeMap::<String, BTreeSet<String>>::new();
        let mut visiting = BTreeSet::<String>::new();
        let mut edge_count = 0usize;
        let mut target_closures = BTreeMap::<String, BTreeSet<String>>::new();

        for route in routes {
            let target = normalize_rel_path(route.target_file.as_str());
            if target.is_empty() {
                continue;
            }
            route_targets.insert(target.clone());
            let closure = collect_target_closure(
                app_root,
                app_decls,
                target.as_str(),
                &mut memo,
                &mut visiting,
                &mut edge_count,
            );
            target_closures.insert(target, closure);
        }

        let mut file_dependents = BTreeMap::<String, BTreeSet<String>>::new();
        for (target, closure) in &target_closures {
            for file in closure {
                file_dependents
                    .entry(file.clone())
                    .or_default()
                    .insert(target.clone());
            }
        }

        DependencyGraph {
            route_targets,
            target_closures,
            file_dependents,
            edge_count,
        }
    }

    pub(crate) fn closure_for_target(
        &self,
        app_root: &Path,
        app_decls: &Value,
        target_file: &str,
    ) -> BTreeSet<String> {
        let target = normalize_rel_path(target_file);
        if let Some(closure) = self.target_closures.get(&target) {
            return closure.clone();
        }
        let mut memo = BTreeMap::<String, BTreeSet<String>>::new();
        let mut visiting = BTreeSet::<String>::new();
        let mut edge_count = 0usize;
        collect_target_closure(
            app_root,
            app_decls,
            target.as_str(),
            &mut memo,
            &mut visiting,
            &mut edge_count,
        )
    }

    pub(crate) fn dependency_fingerprint_for_target(
        &self,
        app_root: &Path,
        app_decls: &Value,
        target_file: &str,
    ) -> Option<String> {
        let closure = self.closure_for_target(app_root, app_decls, target_file);
        let mut parts = Vec::<String>::new();
        for rel in &closure {
            let signature = file_content_signature(&app_root.join(rel), rel.as_str());
            parts.push(format!("{rel}@{signature}"));
        }
        if parts.is_empty() {
            return None;
        }
        Some(parts.join("|"))
    }

    pub(crate) fn dependent_targets_for_file(&self, rel_file: &str) -> BTreeSet<String> {
        self.file_dependents
            .get(&normalize_rel_path(rel_file))
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn catalog_seed_files(
        &self,
        app_root: &Path,
        app_decls: &Value,
        preview_target: Option<&str>,
    ) -> BTreeSet<String> {
        let mut seeds = BTreeSet::<String>::new();
        match preview_target.map(str::trim).filter(|s| !s.is_empty()) {
            None => {
                for route_target in &self.route_targets {
                    seeds.extend(self.closure_for_target(app_root, app_decls, route_target));
                }
            }
            Some(p) if p.starts_with("data/") || p.contains("/datasets/") => {}
            Some(target) => {
                seeds.extend(self.closure_for_target(app_root, app_decls, target));
            }
        }
        seeds
    }

    pub(crate) fn stats(&self) -> DependencyGraphStats {
        let max_closure = self
            .target_closures
            .values()
            .map(BTreeSet::len)
            .max()
            .unwrap_or(0);
        DependencyGraphStats {
            route_roots: self.route_targets.len(),
            unique_files: self.file_dependents.len(),
            edges: self.edge_count,
            max_closure,
        }
    }
}

pub(crate) fn file_content_hash_cache_metrics_snapshot() -> (u64, u64) {
    (
        FILE_CONTENT_HASH_CACHE_HITS.load(Ordering::Relaxed),
        FILE_CONTENT_HASH_CACHE_MISSES.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
pub(crate) fn clear_file_content_hash_cache_for_tests() {
    if let Ok(mut cache) = FILE_CONTENT_HASH_CACHE.lock() {
        cache.clear();
    }
}

pub(crate) fn dependency_graph_cache_metrics_snapshot() -> (u64, u64) {
    (
        DEPENDENCY_GRAPH_CACHE_HITS.load(Ordering::Relaxed),
        DEPENDENCY_GRAPH_CACHE_MISSES.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
pub(crate) fn clear_dependency_graph_cache_for_tests() {
    if let Ok(mut cache) = DEPENDENCY_GRAPH_CACHE.lock() {
        cache.clear();
    }
}

fn collect_target_closure(
    app_root: &Path,
    app_decls: &Value,
    target_file: &str,
    memo: &mut BTreeMap<String, BTreeSet<String>>,
    visiting: &mut BTreeSet<String>,
    edge_count: &mut usize,
) -> BTreeSet<String> {
    let target = normalize_rel_path(target_file);
    if let Some(cached) = memo.get(&target) {
        return cached.clone();
    }
    if !visiting.insert(target.clone()) {
        let mut cycle = BTreeSet::new();
        cycle.insert(target.clone());
        return cycle;
    }

    let direct = collect_direct_dependencies(app_root, app_decls, target.as_str());
    *edge_count += direct.len();
    let mut closure = BTreeSet::<String>::new();
    closure.insert(target.clone());
    for dep in direct {
        let nested = collect_target_closure(
            app_root,
            app_decls,
            dep.as_str(),
            memo,
            visiting,
            edge_count,
        );
        closure.extend(nested);
    }

    visiting.remove(&target);
    memo.insert(target, closure.clone());
    closure
}

fn collect_direct_dependencies(
    app_root: &Path,
    app_decls: &Value,
    target_file: &str,
) -> BTreeSet<String> {
    let mut deps = BTreeSet::<String>::new();
    let target = normalize_rel_path(target_file);
    let decls = if target == "main.mei" {
        Some(app_decls.clone())
    } else {
        evaluate_mei_file(&app_root.join(&target)).ok()
    };

    if let Some(values) = decls.as_ref().and_then(Value::as_array) {
        for value in values {
            collect_ref_scene_files_from_value(value, &mut deps);
        }
    } else if let Some(value) = decls.as_ref() {
        collect_ref_scene_files_from_value(value, &mut deps);
    }

    if let Ok(content) = std::fs::read_to_string(app_root.join(&target)) {
        for from_dataset in extract_from_dataset_tokens(&content) {
            let dep = normalize_rel_path(&from_dataset);
            if dep.ends_with(".mei") {
                deps.insert(dep);
            }
        }
        for (_, from_dataset) in extract_metric_ref_tokens(&content) {
            if let Some(path) = from_dataset {
                let dep = normalize_rel_path(path.as_str());
                if dep.ends_with(".mei") {
                    deps.insert(dep);
                }
            }
        }
    }

    deps.retain(|dep| !dep.trim().is_empty() && dep.ends_with(".mei"));
    deps
}

fn normalize_rel_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

fn dependency_graph_cache_key(app_root: &Path, routes: &[CompiledSceneRoute]) -> String {
    let mut parts = Vec::<String>::new();
    for route in routes {
        let target = normalize_rel_path(route.target_file.as_str());
        if target.is_empty() {
            continue;
        }
        let mtime = file_mtime_ms(&app_root.join(&target));
        parts.push(format!("{target}@{mtime}"));
    }
    parts.sort();
    let app_mtime = file_mtime_ms(&app_root.join("main.mei"));
    format!(
        "{}|main@{app_mtime}|{}",
        app_root.display(),
        parts.join("|")
    )
}

fn file_content_signature(path: &Path, rel: &str) -> String {
    let mtime = file_mtime_ms(path);
    let key = format!("{}|{mtime}", path.display());
    if let Ok(cache) = FILE_CONTENT_HASH_CACHE.lock() {
        if let Some(signature) = cache.get(&key).cloned() {
            FILE_CONTENT_HASH_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            return signature;
        }
    }

    FILE_CONTENT_HASH_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    let signature = std::fs::read(path)
        .ok()
        .map(|bytes| {
            let mut hasher = DefaultHasher::new();
            rel.hash(&mut hasher);
            bytes.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        })
        .unwrap_or_else(|| format!("mtime:{mtime}"));

    if let Ok(mut cache) = FILE_CONTENT_HASH_CACHE.lock() {
        if cache.len() >= MAX_FILE_CONTENT_HASH_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(key, signature.clone());
    }
    signature
}
