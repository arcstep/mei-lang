//! `mei-toolchain query-audit` — offline DF/SQL audit JSONL tools.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use mei_lang_datasets::{load_query_audit_jsonl, today_yyyymmdd, QueryAuditEntry};
use mei_lang_kernel::resolve_app_root;
use serde_json::json;

use crate::cli::args::{
    QueryAuditArgs, QueryAuditCommand, QueryAuditCommonArgs, QueryAuditExplainArgs,
    QueryAuditGateArgs, QueryAuditReplayArgs, QueryAuditReportArgs, QueryAuditTailArgs,
};
use crate::cli::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};

pub fn query_audit_command(args: QueryAuditArgs) -> Result<()> {
    match args.command {
        QueryAuditCommand::Tail(a) => tail_command(a),
        QueryAuditCommand::Explain(a) => explain_command(a),
        QueryAuditCommand::Gate(a) => gate_command(a),
        QueryAuditCommand::Replay(a) => replay_command(a),
        QueryAuditCommand::Report(a) => report_command(a),
    }
}

fn resolve_app_and_audit_dir(common: &QueryAuditCommonArgs) -> Result<(PathBuf, PathBuf, String)> {
    let package_root = resolve_package_root()?;
    let raw = resolve_source_root_arg(
        &package_root,
        common.workspace.as_deref(),
        &common.source_root,
    )?;
    let source_root = resolve_cli_source_root(&package_root, &raw)?;
    let app_id = common.app.trim();
    if app_id.is_empty() {
        bail!("--app is required");
    }
    let app_root = resolve_app_root(source_root.as_path(), app_id);
    let day = common
        .day
        .clone()
        .unwrap_or_else(today_yyyymmdd);
    let audit_dir = if let Some(var_root) = common.var_root.as_ref() {
        var_root.join("query-audit")
    } else {
        mei_lang_kernel::resolve_app_var_root(app_root.as_path()).join("query-audit")
    };
    Ok((app_root, audit_dir, day))
}

fn load_entries(audit_dir: &Path, day: &str) -> Result<Vec<QueryAuditEntry>> {
    load_query_audit_jsonl(audit_dir, day)
        .with_context(|| format!("read audit jsonl in {}", audit_dir.display()))
}

fn filter_metric(entries: Vec<QueryAuditEntry>, metric: Option<&str>) -> Vec<QueryAuditEntry> {
    let Some(metric) = metric.map(str::trim).filter(|s| !s.is_empty()) else {
        return entries;
    };
    entries
        .into_iter()
        .filter(|e| e.metric_id.as_deref() == Some(metric))
        .collect()
}

fn find_by_id(entries: &[QueryAuditEntry], id: &str) -> Result<QueryAuditEntry> {
    entries
        .iter()
        .find(|e| e.audit_id == id)
        .cloned()
        .with_context(|| format!("audit_id not found: {id}"))
}

fn gate_failures(entries: &[QueryAuditEntry]) -> Vec<String> {
    mei_lang_datasets::query_audit_gate_failures(entries)
}

fn tail_command(args: QueryAuditTailArgs) -> Result<()> {
    let (_app_root, audit_dir, day) = resolve_app_and_audit_dir(&args.common)?;
    let entries = filter_metric(load_entries(&audit_dir, &day)?, args.metric.as_deref());
    let limit = args.limit.max(1);
    let slice: Vec<_> = entries.into_iter().rev().take(limit).collect();
    if args.json {
        print_json_output(
            &json!({
                "schema_version": "mei-cli-v1",
                "command": "query-audit.tail",
                "day": day,
                "audit_dir": audit_dir,
                "entries": slice,
            }),
            true,
        )?;
        return Ok(());
    }
    println!(
        "day={day} dir={} count={}",
        audit_dir.display(),
        slice.len()
    );
    println!(
        "{:<28} {:<12} {:>8} {:>5} {:>5} {:>8} {}",
        "audit_id", "path", "chars", "ua", "arm", "exec_ms", "metric"
    );
    for e in slice.iter().rev() {
        let shape = e.shape.as_ref();
        println!(
            "{:<28} {:<12} {:>8} {:>5} {:>5} {:>8} {}",
            e.audit_id,
            e.path,
            shape.map(|s| s.sql_chars).unwrap_or(0),
            shape.map(|s| s.union_all).unwrap_or(0),
            shape
                .map(|s| if s.has_arm { "Y" } else { "N" })
                .unwrap_or("-"),
            e.timing_ms.exec,
            e.metric_id.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}

fn explain_command(args: QueryAuditExplainArgs) -> Result<()> {
    let (_app_root, audit_dir, day) = resolve_app_and_audit_dir(&args.common)?;
    let entries = load_entries(&audit_dir, &day)?;
    let entry = find_by_id(&entries, args.id.trim())?;
    if args.json {
        print_json_output(
            &json!({
                "schema_version": "mei-cli-v1",
                "command": "query-audit.explain",
                "entry": entry,
            }),
            true,
        )?;
        return Ok(());
    }
    println!("audit_id={}", entry.audit_id);
    println!("path={} metric={}", entry.path, entry.metric_id.as_deref().unwrap_or("-"));
    println!(
        "controlled={:?} fallback={:?}",
        entry.controlled, entry.fallback_reason
    );
    if let Some(shape) = &entry.shape {
        println!(
            "shape chars={} union_all={} width_alias={} row_number={} has_arm={}",
            shape.sql_chars, shape.union_all, shape.width_alias, shape.row_number, shape.has_arm
        );
    }
    println!(
        "timing lower={} exec={} total={}",
        entry.timing_ms.lower, entry.timing_ms.exec, entry.timing_ms.total
    );
    println!(
        "result rows_out={} page={:?} page_size={:?} total={:?}",
        entry.result.rows_out, entry.result.page, entry.result.page_size, entry.result.total
    );
    if let Some(sha) = &entry.sql_sha256 {
        println!("sql_sha256={sha}");
    }
    match &entry.sql_file {
        Some(path) => {
            println!("sql_file={path}");
            if Path::new(path).is_file() {
                let sql = fs::read_to_string(path)?;
                let preview: String = sql.chars().take(500).collect();
                println!("sql_preview:\n{preview}");
                if sql.chars().count() > 500 {
                    println!("… (truncated)");
                }
            }
        }
        None => {
            println!("sql_file=(none — enable MEI_DF_AUDIT_SQL=1 at capture time)");
        }
    }
    if let Some(err) = &entry.error {
        println!("error={err}");
    }
    Ok(())
}

fn gate_command(args: QueryAuditGateArgs) -> Result<()> {
    let (_app_root, audit_dir, day) = resolve_app_and_audit_dir(&args.common)?;
    let entries = filter_metric(load_entries(&audit_dir, &day)?, args.metric.as_deref());
    let fails = gate_failures(&entries);
    let ok = fails.is_empty();
    if args.json {
        print_json_output(
            &json!({
                "schema_version": "mei-cli-v1",
                "command": "query-audit.gate",
                "day": day,
                "checked": entries.len(),
                "ok": ok,
                "failures": fails,
            }),
            true,
        )?;
    } else {
        println!(
            "gate day={day} checked={} ok={ok}",
            entries.len()
        );
        for f in &fails {
            println!("  FAIL {f}");
        }
        if ok {
            println!("all rows within 0549 budgets and controlled!=false");
        }
    }
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}

fn replay_command(args: QueryAuditReplayArgs) -> Result<()> {
    let (app_root, audit_dir, day) = resolve_app_and_audit_dir(&args.common)?;
    let entries = load_entries(&audit_dir, &day)?;
    let entry = find_by_id(&entries, args.id.trim())?;
    let sql_path = entry
        .sql_file
        .as_ref()
        .context("sql_file missing; capture with MEI_DF_AUDIT_SQL=1")?;
    let sql = fs::read_to_string(sql_path)
        .with_context(|| format!("read sql_file {sql_path}"))?;
    let n = args.bench.max(1);
    mei_lang_datasets::ensure_query_engine_session(&app_root)?;
    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let ms = mei_lang_datasets::bench_sql_text(&app_root, &sql)
            .with_context(|| format!("replay iteration {i} failed"))?;
        samples.push(ms);
    }
    samples.sort_unstable();
    let p50 = percentile_ms(&samples, 50);
    let p95 = percentile_ms(&samples, 95);
    if args.json {
        print_json_output(
            &json!({
                "schema_version": "mei-cli-v1",
                "command": "query-audit.replay",
                "audit_id": entry.audit_id,
                "bench": n,
                "samples_ms": samples,
                "p50_ms": p50,
                "p95_ms": p95,
            }),
            true,
        )?;
    } else {
        println!(
            "replay id={} n={n} p50_ms={p50} p95_ms={p95} samples_ms={samples:?}",
            entry.audit_id
        );
    }
    Ok(())
}

fn percentile_ms(sorted: &[u64], pct: u8) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((pct as usize) * (sorted.len() - 1)) / 100;
    sorted[idx]
}

fn report_command(args: QueryAuditReportArgs) -> Result<()> {
    let (_app_root, audit_dir, day) = resolve_app_and_audit_dir(&args.common)?;
    let entries = filter_metric(load_entries(&audit_dir, &day)?, args.metric.as_deref());
    let fails = gate_failures(&entries);
    let package_root = resolve_package_root()?;
    let out = if args.out.is_absolute() {
        args.out.clone()
    } else {
        package_root.join(&args.out)
    };
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut md = String::new();
    md.push_str("# zhifa DF/SQL 审计报告\n\n");
    md.push_str(&format!("> **桶**：`draft:mei-lang`  \n"));
    md.push_str(&format!("> **日期**：{day}  \n"));
    md.push_str(&format!(
        "> **审计目录**：`{}`  \n\n",
        audit_dir.display()
    ));
    md.push_str("## 1. 环境\n\n");
    md.push_str(&format!("- app: `{}`\n", args.common.app));
    md.push_str(&format!("- day: `{day}`\n"));
    md.push_str(&format!("- entries: {}\n", entries.len()));
    md.push_str(&format!(
        "- gate: {}\n\n",
        if fails.is_empty() { "PASS" } else { "FAIL" }
    ));
    md.push_str("## 2. 总览表\n\n");
    md.push_str(
        "| audit_id | path | metric | sql_chars | union_all | has_arm | exec_ms | total_ms | controlled |\n",
    );
    md.push_str("|---|---|---:|---:|---:|---:|---:|---:|---|\n");
    for e in &entries {
        let shape = e.shape.as_ref();
        md.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {:?} |\n",
            e.audit_id,
            e.path,
            e.metric_id.as_deref().unwrap_or("-"),
            shape.map(|s| s.sql_chars).unwrap_or(0),
            shape.map(|s| s.union_all).unwrap_or(0),
            shape.map(|s| s.has_arm).unwrap_or(false),
            e.timing_ms.exec,
            e.timing_ms.total,
            e.controlled,
        ));
    }
    md.push_str("\n## 3. Gate\n\n");
    if fails.is_empty() {
        md.push_str("全部行通过 0549 形状门禁（`controlled!=false` 且未超阈）。\n");
    } else {
        md.push_str("失败项：\n\n");
        for f in &fails {
            md.push_str(&format!("- {f}\n"));
        }
    }
    md.push_str("\n## 4. 说明\n\n");
    md.push_str("本文件由 `mei-toolchain query-audit report` 生成；采集细节与结论由人工在 M4 补全。\n");
    fs::write(&out, md).with_context(|| format!("write report {}", out.display()))?;
    println!("wrote report {}", out.display());
    Ok(())
}
