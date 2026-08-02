//! Best-effort DF/SQL query audit JSONL (draft plan 2026-08-02).
//!
//! Enabled only when `MEI_DF_AUDIT=1` (or `true`/`yes`). Optional full SQL files when
//! `MEI_DF_AUDIT_SQL=1`. IO failures never fail the query path.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mei_lang_kernel::resolve_app_var_root;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::lower::{audit_sql_plan_shape, SqlPlan, CONTROLLED_SQL_MAX_CHARS, CONTROLLED_SQL_MAX_UNION_ALL};

static AUDIT_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn df_audit_enabled() -> bool {
    env_flag_truthy("MEI_DF_AUDIT")
}

pub fn df_audit_sql_enabled() -> bool {
    env_flag_truthy("MEI_DF_AUDIT_SQL")
}

fn env_flag_truthy(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAuditShape {
    pub sql_chars: usize,
    pub union_all: usize,
    pub width_alias: usize,
    pub row_number: usize,
    pub has_arm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAuditTiming {
    pub lower: u64,
    pub exec: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAuditResult {
    pub rows_out: usize,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub total: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAuditEntry {
    pub schema: u32,
    pub audit_id: String,
    pub ts_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    pub path: String,
    pub controlled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<QueryAuditShape>,
    pub timing_ms: QueryAuditTiming,
    pub result: QueryAuditResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql_file: Option<String>,
    pub rss_delta_kb: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

impl QueryAuditEntry {
    pub fn new_id() -> String {
        let ts = now_ms();
        let seq = AUDIT_SEQ.fetch_add(1, Ordering::Relaxed);
        format!("qa-{ts}-{seq:x}")
    }
}

pub fn shape_from_plan(plan: &SqlPlan) -> QueryAuditShape {
    let audit = audit_sql_plan_shape(plan);
    QueryAuditShape {
        sql_chars: audit.chars,
        union_all: audit.union_all_count,
        width_alias: audit.width_copy_alias_count,
        row_number: audit.row_number_count,
        has_arm: plan.final_sql.contains(" AS _arm "),
    }
}

pub fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 0549 hard budgets used by CLI `gate` (keep in sync with `is_controlled_sql_plan`).
pub fn shape_exceeds_gate(shape: &QueryAuditShape) -> bool {
    shape.sql_chars > CONTROLLED_SQL_MAX_CHARS
        || shape.union_all > CONTROLLED_SQL_MAX_UNION_ALL
        || shape.width_alias > 8
        || shape.row_number > 8
}

/// Human-readable gate failure lines for CLI `query-audit gate` / report.
pub fn query_audit_gate_failures(entries: &[QueryAuditEntry]) -> Vec<String> {
    let mut fails = Vec::new();
    for e in entries {
        if e.controlled == Some(false) {
            fails.push(format!(
                "{} controlled=false error={}",
                e.audit_id,
                e.error.as_deref().unwrap_or("")
            ));
            continue;
        }
        if let Some(shape) = e.shape.as_ref() {
            if shape_exceeds_gate(shape) {
                fails.push(format!(
                    "{} exceeds gate chars={} union_all={} width_alias={} row_number={} (max chars={} union={})",
                    e.audit_id,
                    shape.sql_chars,
                    shape.union_all,
                    shape.width_alias,
                    shape.row_number,
                    CONTROLLED_SQL_MAX_CHARS,
                    CONTROLLED_SQL_MAX_UNION_ALL
                ));
            }
        }
    }
    fails
}

/// Load all entries from `{audit_dir}/{day}.jsonl` (missing file → empty).
pub fn load_query_audit_jsonl(audit_dir: &Path, day: &str) -> std::io::Result<Vec<QueryAuditEntry>> {
    let path = audit_dir.join(format!("{day}.jsonl"));
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)?;
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: QueryAuditEntry = serde_json::from_str(line).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("parse audit jsonl line {}: {e}", idx + 1),
            )
        })?;
        out.push(entry);
    }
    Ok(out)
}

pub fn query_audit_dir(app_root: &Path) -> PathBuf {
    resolve_app_var_root(app_root).join("query-audit")
}

pub fn query_audit_jsonl_path(app_root: &Path, day: &str) -> PathBuf {
    query_audit_dir(app_root).join(format!("{day}.jsonl"))
}

pub fn today_yyyymmdd() -> String {
    use chrono::Local;
    Local::now().format("%Y%m%d").to_string()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Append one audit row. No-op when `MEI_DF_AUDIT` is off. Never returns Err to callers.
pub fn append_query_audit(app_root: &Path, entry: &QueryAuditEntry) {
    if !df_audit_enabled() {
        return;
    }
    if let Err(error) = append_query_audit_inner(app_root, entry) {
        tracing::warn!(
            error = %error,
            audit_id = %entry.audit_id,
            "df query audit append failed (best-effort)"
        );
    }
}

fn append_query_audit_inner(app_root: &Path, entry: &QueryAuditEntry) -> std::io::Result<()> {
    let day = today_yyyymmdd();
    let dir = query_audit_dir(app_root);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{day}.jsonl"));
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    serde_json::to_writer(&mut file, entry).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    })?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Optionally persist full SQL next to JSONL; returns relative-ish path string for the entry.
pub fn maybe_write_sql_file(app_root: &Path, audit_id: &str, sql: &str) -> Option<String> {
    if !df_audit_sql_enabled() {
        return None;
    }
    let dir = query_audit_dir(app_root).join("sql");
    if let Err(error) = fs::create_dir_all(&dir) {
        tracing::warn!(%error, "df query audit sql dir failed");
        return None;
    }
    let path = dir.join(format!("{audit_id}.sql"));
    match fs::write(&path, sql) {
        Ok(()) => Some(path.to_string_lossy().into_owned()),
        Err(error) => {
            tracing::warn!(%error, "df query audit sql write failed");
            None
        }
    }
}

pub fn build_entry_for_plan(
    path: &str,
    metric_id: Option<&str>,
    plan: &SqlPlan,
    lower_ms: u64,
    exec_ms: u64,
    total_ms: u64,
    rows_out: usize,
    page: Option<usize>,
    page_size: Option<usize>,
    total: Option<usize>,
    app_root: &Path,
    error: Option<String>,
) -> QueryAuditEntry {
    build_entry_for_plan_ctx(
        path,
        metric_id,
        None,
        None,
        plan,
        lower_ms,
        exec_ms,
        total_ms,
        rows_out,
        page,
        page_size,
        total,
        app_root,
        error,
    )
}

pub fn build_entry_for_plan_ctx(
    path: &str,
    metric_id: Option<&str>,
    scene_id: Option<&str>,
    dataset_id: Option<&str>,
    plan: &SqlPlan,
    lower_ms: u64,
    exec_ms: u64,
    total_ms: u64,
    rows_out: usize,
    page: Option<usize>,
    page_size: Option<usize>,
    total: Option<usize>,
    app_root: &Path,
    error: Option<String>,
) -> QueryAuditEntry {
    let audit_id = QueryAuditEntry::new_id();
    let shape = shape_from_plan(plan);
    let sql_sha256 = sha256_hex(&plan.final_sql);
    let sql_file = maybe_write_sql_file(app_root, &audit_id, &plan.final_sql);
    let (app_id, generation, instance_id) = runtime_identity();
    QueryAuditEntry {
        schema: 1,
        audit_id,
        ts_ms: now_ms(),
        app_id,
        generation,
        instance_id,
        scene_id: scene_id.map(str::to_string),
        dataset_id: dataset_id.map(str::to_string),
        metric_id: metric_id.map(str::to_string),
        path: path.to_string(),
        controlled: Some(true),
        shape: Some(shape),
        timing_ms: QueryAuditTiming {
            lower: lower_ms,
            exec: exec_ms,
            total: total_ms,
        },
        result: QueryAuditResult {
            rows_out,
            page,
            page_size,
            total,
        },
        sql_sha256: Some(sql_sha256),
        sql_file,
        rss_delta_kb: None,
        error,
        fallback_reason: None,
    }
}

pub fn build_fallback_entry(
    path: &str,
    metric_id: Option<&str>,
    total_ms: u64,
    reason: &str,
    error: Option<String>,
    controlled: Option<bool>,
) -> QueryAuditEntry {
    let (app_id, generation, instance_id) = runtime_identity();
    QueryAuditEntry {
        schema: 1,
        audit_id: QueryAuditEntry::new_id(),
        ts_ms: now_ms(),
        app_id,
        generation,
        instance_id,
        scene_id: None,
        dataset_id: None,
        metric_id: metric_id.map(str::to_string),
        path: path.to_string(),
        controlled,
        shape: None,
        timing_ms: QueryAuditTiming {
            lower: total_ms,
            exec: 0,
            total: total_ms,
        },
        result: QueryAuditResult {
            rows_out: 0,
            page: None,
            page_size: None,
            total: None,
        },
        sql_sha256: None,
        sql_file: None,
        rss_delta_kb: None,
        error,
        fallback_reason: Some(reason.to_string()),
    }
}

fn runtime_identity() -> (Option<String>, Option<String>, Option<String>) {
    (
        std::env::var("MEI_APP_RUNTIME_APP_ID").ok().filter(|s| !s.is_empty()),
        std::env::var("MEI_APP_RUNTIME_GENERATION")
            .ok()
            .filter(|s| !s.is_empty()),
        std::env::var("MEI_APP_RUNTIME_INSTANCE_ID")
            .ok()
            .filter(|s| !s.is_empty()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn append_writes_jsonl_when_enabled() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let dir = tempfile::tempdir().expect("tempdir");
        let app_root = dir.path().join("demo");
        fs::create_dir_all(&app_root).expect("mkdir app");
        let var = dir.path().join("runtime-var");
        fs::create_dir_all(&var).expect("mkdir var");

        std::env::set_var("MEI_DF_AUDIT", "1");
        std::env::remove_var("MEI_DF_AUDIT_SQL");
        std::env::set_var("MEI_APP_RUNTIME_APP_ID", "demo");
        std::env::set_var("MEI_APP_RUNTIME_VAR_ROOT", &var);

        let entry = build_fallback_entry(
            "pipeline_sql_eval",
            Some("m1"),
            12,
            "test",
            None,
            Some(true),
        );
        append_query_audit(&app_root, &entry);

        let day = today_yyyymmdd();
        let path = var.join("query-audit").join(format!("{day}.jsonl"));
        let text = fs::read_to_string(&path).expect("read jsonl");
        assert!(text.contains("m1"));
        assert!(text.contains("pipeline_sql_eval"));

        std::env::remove_var("MEI_DF_AUDIT");
        std::env::remove_var("MEI_APP_RUNTIME_APP_ID");
        std::env::remove_var("MEI_APP_RUNTIME_VAR_ROOT");
    }

    #[test]
    fn shape_gate_matches_0549_budgets() {
        let ok = QueryAuditShape {
            sql_chars: 100,
            union_all: 6,
            width_alias: 0,
            row_number: 2,
            has_arm: true,
        };
        assert!(!shape_exceeds_gate(&ok));
        let bad = QueryAuditShape {
            sql_chars: CONTROLLED_SQL_MAX_CHARS + 1,
            union_all: 0,
            width_alias: 0,
            row_number: 0,
            has_arm: false,
        };
        assert!(shape_exceeds_gate(&bad));
    }

    fn sample_entry(id: &str, shape: QueryAuditShape, controlled: Option<bool>) -> QueryAuditEntry {
        QueryAuditEntry {
            schema: 1,
            audit_id: id.into(),
            ts_ms: 1,
            app_id: Some("zhifa".into()),
            generation: None,
            instance_id: None,
            scene_id: None,
            dataset_id: None,
            metric_id: Some("m".into()),
            path: "pipeline_sql_page".into(),
            controlled,
            shape: Some(shape),
            timing_ms: QueryAuditTiming {
                lower: 1,
                exec: 2,
                total: 3,
            },
            result: QueryAuditResult {
                rows_out: 1,
                page: Some(1),
                page_size: Some(20),
                total: Some(1),
            },
            sql_sha256: None,
            sql_file: None,
            rss_delta_kb: None,
            error: None,
            fallback_reason: None,
        }
    }

    #[test]
    fn gate_green_and_red_fixture_jsonl() {
        let ok = sample_entry(
            "qa-ok",
            QueryAuditShape {
                sql_chars: 15000,
                union_all: 6,
                width_alias: 0,
                row_number: 2,
                has_arm: true,
            },
            Some(true),
        );
        assert!(query_audit_gate_failures(&[ok.clone()]).is_empty());

        let bad = sample_entry(
            "qa-big",
            QueryAuditShape {
                sql_chars: CONTROLLED_SQL_MAX_CHARS + 1,
                union_all: 0,
                width_alias: 0,
                row_number: 0,
                has_arm: false,
            },
            Some(true),
        );
        let uncontrolled = sample_entry(
            "qa-uc",
            QueryAuditShape {
                sql_chars: 10,
                union_all: 0,
                width_alias: 0,
                row_number: 0,
                has_arm: false,
            },
            Some(false),
        );
        assert_eq!(query_audit_gate_failures(&[bad, uncontrolled]).len(), 2);

        let dir = tempfile::tempdir().expect("temp");
        let audit = dir.path().join("query-audit");
        fs::create_dir_all(&audit).unwrap();
        let day = "20260802";
        let line = serde_json::to_string(&ok).unwrap();
        fs::write(audit.join(format!("{day}.jsonl")), format!("{line}\n")).unwrap();
        let loaded = load_query_audit_jsonl(&audit, day).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].audit_id, "qa-ok");
        assert!(query_audit_gate_failures(&loaded).is_empty());
    }
}
