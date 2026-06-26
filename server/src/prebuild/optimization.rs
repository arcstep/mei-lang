use super::prelude::*;
use super::*;

pub(crate) fn emit_prebuild_optimization_report(
    app_id: &str,
    app_root: &Path,
    reports: &[PrebuildScopeReport],
    coverage: &PrebuildCoverageReport,
    diagnostics: &PrebuildDiagnostics,
    plan_nodes: &PrebuildPlanNodeStatsReport,
    compile_phase_ms: u64,
    artifacts_phase_ms: u64,
    max_parallelism: usize,
    warning_count: usize,
    canonical_identity_count: usize,
    session_entries_before_clear: (usize, usize, usize),
    session_entries_after_clear: (usize, usize, usize),
    warmup_reuse_hits: usize,
) {
    diagnostics.sample_memory_peak();
    prebuild_emit_progress(format!("[{app_id}] ══ 优化诊断（重复 vs 耗时）══"));

    let total_checks = reports.len();
    let real_compiles = reports.iter().filter(|report| !report.cache_hit).count();
    let cache_hits = reports.iter().filter(|report| report.cache_hit).count();
    let cache_probe_ms: u64 = reports
        .iter()
        .filter(|report| report.cache_hit)
        .map(|report| {
            report
                .cache_lookup_ms
                .saturating_add(report.artifact_load_ms)
        })
        .sum();
    let compile_miss_ms: u64 = reports
        .iter()
        .filter(|report| !report.cache_hit)
        .map(|report| report.compile_ms)
        .sum();

    prebuild_emit_progress(format!(
        "■ 汇总 | scope 检查 {total_checks} | 真实编译 {real_compiles} | 缓存命中 {cache_hits} | 编译阶段 {compile_phase_s:.1}s | 产物阶段 {artifacts_phase_s:.1}s",
        compile_phase_s = compile_phase_ms as f64 / 1000.0,
        artifacts_phase_s = artifacts_phase_ms as f64 / 1000.0,
    ));
    prebuild_emit_progress(format!(
        "  时间构成 | 真实编译 {compile_miss_s:.1}s | 缓存探测约 {cache_probe_s:.1}s",
        compile_miss_s = compile_miss_ms as f64 / 1000.0,
        cache_probe_s = cache_probe_ms as f64 / 1000.0,
    ));
    prebuild_emit_progress(format!(
        "  产物 | response 就绪 {} (新建 {}) | dataframe 就绪 {} (本次计算 {})",
        coverage.metric_response_artifacts_ready,
        coverage.metric_response_artifacts_built,
        coverage.metric_dataframe_artifacts_ready,
        coverage.metric_dataframe_artifacts_built,
    ));

    let mut by_active: BTreeMap<String, (usize, usize, u64)> = BTreeMap::new();
    for report in reports {
        let entry = by_active
            .entry(compile_active_identity(report))
            .or_insert((0, 0, 0));
        entry.0 += 1;
        if report.cache_hit {
            entry.2 += report
                .cache_lookup_ms
                .saturating_add(report.artifact_load_ms);
        } else {
            entry.1 += 1;
            entry.2 += report.compile_ms;
        }
    }
    let unique_active = by_active.len();
    let expansion_ratio = if unique_active > 0 {
        total_checks as f64 / unique_active as f64
    } else {
        1.0
    };
    let redundant_checks = total_checks.saturating_sub(unique_active);
    prebuild_emit_progress(format!(
        "■ 数量统计 | 编译检查 {total_checks} | 唯一编译结果 {unique_active} | 展开倍率 {expansion_ratio:.1}x | 冗余检查约 {redundant_checks}"
    ));
    prebuild_emit_progress(format!(
        "  RSS 相关 | canonical outcomes {} | session(before) scope/cache/identity = {}/{}/{} | session(after) = {}/{}/{} | warmup 直接复用 {}",
        canonical_identity_count,
        session_entries_before_clear.0,
        session_entries_before_clear.1,
        session_entries_before_clear.2,
        session_entries_after_clear.0,
        session_entries_after_clear.1,
        session_entries_after_clear.2,
        warmup_reuse_hits
    ));
    prebuild_emit_progress(format!(
        "  DAG 计划 | manifest scope {} (hot {} / deferred {}) | warmup req {} / scope {} | workset {} | response {} | dataframe {} | canonical nodes {} / budget {}",
        plan_nodes.manifest_compile_scope_nodes,
        plan_nodes.hot_compile_scope_nodes,
        plan_nodes.deferred_compile_scope_nodes,
        plan_nodes.planned_warmup_request_nodes,
        plan_nodes.planned_warmup_scope_nodes,
        plan_nodes.planned_metric_workset_nodes,
        plan_nodes.planned_response_artifact_nodes,
        plan_nodes.planned_dataframe_artifact_nodes,
        plan_nodes.canonical_prebuild_nodes,
        plan_nodes.budget.canonical_node_limit
    ));
    if plan_nodes.budget.over_canonical_node_limit {
        prebuild_emit_progress(format!(
            "  预算告警 | canonical prebuild nodes {} 超过预算 {}，请继续收缩 manifest/fanout/workset",
            plan_nodes.canonical_prebuild_nodes,
            plan_nodes.budget.canonical_node_limit
        ));
    }
    let preload_reuse_hits = diagnostics
        .compile_preload_reuse_hits
        .load(Ordering::Relaxed);
    let postload_identity_collapses = diagnostics
        .compile_postload_identity_collapses
        .load(Ordering::Relaxed);
    let compile_index_hits = diagnostics.compile_index_hits.load(Ordering::Relaxed);
    let compile_index_misses = diagnostics.compile_index_misses.load(Ordering::Relaxed);
    let compile_index_stale_entries = diagnostics
        .compile_index_stale_entries
        .load(Ordering::Relaxed);
    let compile_fallback_loads = diagnostics.compile_fallback_loads.load(Ordering::Relaxed);
    if preload_reuse_hits > 0 {
        prebuild_emit_progress(format!(
            "  预加载复用 {preload_reuse_hits} 次（命中已知 scope/cache key，跳过探测/加载）"
        ));
    }
    if compile_index_hits > 0 || compile_index_misses > 0 || compile_index_stale_entries > 0 {
        prebuild_emit_progress(format!(
            "  compile 索引 | hit {} | miss {} | stale {} | fallback_loads {}",
            compile_index_hits,
            compile_index_misses,
            compile_index_stale_entries,
            compile_fallback_loads
        ));
    }
    let manifest_probes = diagnostics.compile_manifest_probes.load(Ordering::Relaxed);
    let manifest_stale_skips = diagnostics
        .compile_manifest_stale_skips
        .load(Ordering::Relaxed);
    let artifact_loads_avoided = diagnostics
        .compile_artifact_loads_avoided
        .load(Ordering::Relaxed);
    let mrg_eval_skips = diagnostics.mrg_eval_skips.load(Ordering::Relaxed);
    let dataframe_eval_skips = diagnostics.dataframe_eval_skips.load(Ordering::Relaxed);
    if manifest_probes > 0 || artifact_loads_avoided > 0 {
        prebuild_emit_progress(format!(
            "  manifest 探测 {manifest_probes} | stale 跳过 {manifest_stale_skips} | 避免全量 load {artifact_loads_avoided}"
        ));
    }
    if mrg_eval_skips > 0 || dataframe_eval_skips > 0 {
        prebuild_emit_progress(format!(
            "  MRG eval 跳过 response {mrg_eval_skips} | dataframe {dataframe_eval_skips}"
        ));
    }
    if postload_identity_collapses > 0 {
        prebuild_emit_progress(format!(
            "  load 后 identity 折叠 {postload_identity_collapses} 次（不同请求 scope 收敛到同一编译结果）"
        ));
    }
    prebuild_emit_progress(format!(
        "  逻辑产物 | compile {}/{} | 数据集导入 {}/{} | metric response {}/{} | metric dataframe {}/{} | missing {}",
        coverage.compile_artifacts_ready,
        coverage.compile_artifacts_planned,
        coverage.dataset_import_artifacts_ready,
        coverage.dataset_import_artifacts_planned,
        coverage.metric_response_artifacts_ready,
        coverage.metric_response_artifacts_planned,
        coverage.metric_dataframe_artifacts_ready,
        coverage.metric_dataframe_artifacts_planned,
        coverage.total_missing_artifacts,
    ));

    if prebuild_disk_diagnostics_enabled() {
        let eval_root = mei_lang_kernel::resolve_app_var_root(app_root).join("eval-results");
        let response_dir = eval_root.join("results").join("metric-response");
        let dataframe_dir = eval_root.join("results").join("metric-dataframe");
        let response_disk = dir_size_summary(response_dir.as_path());
        let dataframe_disk = dir_size_summary(dataframe_dir.as_path());
        let eval_disk = dir_size_summary(eval_root.as_path());
        prebuild_emit_progress(format!(
            "■ 磁盘占用 | eval-results 合计 {} ({} 文件)",
            format_bytes(eval_disk.bytes),
            eval_disk.files,
        ));
        prebuild_emit_progress(format!(
            "  metric-response {} ({} 文件) | metric-dataframe {} ({} 文件)",
            format_bytes(response_disk.bytes),
            response_disk.files,
            format_bytes(dataframe_disk.bytes),
            dataframe_disk.files,
        ));
    } else {
        prebuild_emit_progress(
            "■ 磁盘占用 | 已跳过目录扫描（设置 MEI_PREBUILD_DISK_DIAGNOSTICS=1 可启用）",
        );
    }

    let current_rss = current_process_rss_bytes();
    let peak_rss = diagnostics.peak_rss_bytes.load(Ordering::Relaxed);
    match (current_rss, peak_rss) {
        (Some(current), peak) if peak > 0 => {
            prebuild_emit_progress(format!(
                "■ 内存 | 进程 RSS 当前 {} | 峰值 {}",
                format_bytes(current),
                format_bytes(peak as u64),
            ));
        }
        (Some(current), _) => {
            prebuild_emit_progress(format!("■ 内存 | 进程 RSS 当前 {}", format_bytes(current),));
        }
        (None, peak) if peak > 0 => {
            prebuild_emit_progress(format!(
                "■ 内存 | 进程 RSS 峰值 {}",
                format_bytes(peak as u64),
            ));
        }
        _ => {}
    }

    let mut duplicates: Vec<_> = by_active
        .into_iter()
        .filter(|(_, (count, _, _))| *count > 1)
        .collect();
    duplicates.sort_by_key(|(_, (count, _, _))| std::cmp::Reverse(*count));
    if duplicates.is_empty() {
        prebuild_emit_progress("■ 重复检查 | 无（每个编译结果仅检查 1 次）".to_string());
    } else {
        prebuild_emit_progress(format!(
            "■ 重复检查 Top {}（同 scene+file 被多次处理；优化方向：减少 discover 展开）",
            duplicates.len().min(10)
        ));
        for (identity, (count, miss_count, cost_ms)) in duplicates.into_iter().take(10) {
            let (scene, file) = identity
                .split_once('|')
                .map(|(scene, file)| (scene, file))
                .unwrap_or((identity.as_str(), ""));
            prebuild_emit_progress(format!(
                "  {count}x | scene={scene} | file={file} | 真实编译 {miss_count} | 累计 {:.1}s",
                cost_ms as f64 / 1000.0
            ));
        }
    }

    let mut miss_by_file: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for report in reports.iter().filter(|report| !report.cache_hit) {
        let file = report.active_target_file.as_str();
        let entry = miss_by_file.entry(file.to_string()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += report.compile_ms;
    }
    let mut repeat_miss: Vec<_> = miss_by_file
        .into_iter()
        .filter(|(_, (count, _))| *count > 1)
        .collect();
    repeat_miss.sort_by_key(|(_, (count, _))| std::cmp::Reverse(*count));
    if repeat_miss.is_empty() {
        prebuild_emit_progress("■ 重复真实编译 | 无（同一文件未重复编译）".to_string());
    } else {
        prebuild_emit_progress("■ 重复真实编译（应优先消除）".to_string());
        for (file, (count, ms)) in repeat_miss.into_iter().take(8) {
            prebuild_emit_progress(format!(
                "  {count}x | file={file} | 合计 {:.1}s",
                ms as f64 / 1000.0
            ));
        }
    }

    let mut slow_compiles: Vec<&PrebuildScopeReport> = reports
        .iter()
        .filter(|report| !report.cache_hit && report.compile_ms > 0)
        .collect();
    slow_compiles.sort_by_key(|report| std::cmp::Reverse(report.compile_ms));
    if slow_compiles.is_empty() {
        prebuild_emit_progress("■ 编译最慢 | 无真实编译（全部缓存命中）".to_string());
    } else {
        prebuild_emit_progress(format!(
            "■ 编译最慢 Top {}（优化 .mei / 减少 scope）",
            slow_compiles.len().min(8)
        ));
        emit_slow_compile_report(app_id, reports);
    }

    let metric_builds = diagnostics
        .metric_builds
        .lock()
        .expect("lock prebuild diagnostics")
        .clone();
    if metric_builds.is_empty() {
        prebuild_emit_progress("■ 指标求值最慢 | 无（本次未重新计算指标）".to_string());
    } else {
        let mut slow_metrics = metric_builds;
        slow_metrics.sort_by_key(|entry| std::cmp::Reverse(entry.ms));
        prebuild_emit_progress(format!(
            "■ 指标求值最慢 Top {}（优化 metric 口径 / 数据加载）",
            slow_metrics.len().min(8)
        ));
        for entry in slow_metrics.into_iter().take(8) {
            prebuild_emit_progress(format!(
                "  {:.1}s | {} | {} | metric={} | scene={}",
                entry.ms as f64 / 1000.0,
                entry.kind,
                entry.dataset,
                entry.metric,
                entry.scene
            ));
        }
    }

    let cpu_count = thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1);
    let parallelism_cap = prebuild_max_parallelism_cap();
    let home_compile_ms = reports
        .iter()
        .filter(|report| {
            !report.cache_hit
                && report
                    .active_target_file
                    .as_str()
                    .ends_with("scenes/home.mei")
        })
        .map(|report| report.compile_ms)
        .sum::<u64>();
    let home_compile_share = if compile_miss_ms > 0 {
        home_compile_ms as f64 * 100.0 / compile_miss_ms as f64
    } else {
        0.0
    };

    prebuild_emit_progress("■ 提速建议（按收益排序）".to_string());
    if expansion_ratio >= 2.0
        && redundant_checks > 0
        && compile_index_hits == 0
        && preload_reuse_hits == 0
    {
        prebuild_emit_progress(format!(
            "  1. [高] discover 展开 {expansion_ratio:.1}x：{total_checks} 次检查仅 {unique_active} 种结果，合并同源 scope 约可省 {:.0}s 缓存探测",
            cache_probe_ms as f64 / 1000.0 * redundant_checks as f64 / total_checks as f64
        ));
    } else if preload_reuse_hits > 0 || postload_identity_collapses > 0 || compile_index_hits > 0 {
        prebuild_emit_progress(format!(
            "  1. [已启用] 结果复用已消化重复检查（预加载复用 {preload_reuse_hits} / compile索引命中 {compile_index_hits} / load后折叠 {postload_identity_collapses}）；增量场景用 prebuild --verify 可进一步压到秒级"
        ));
    }
    if home_compile_ms > 0 {
        prebuild_emit_progress(format!(
            "  2. [高] scenes/home.mei 真实编译 {:.1}s（占真实编译 {home_compile_share:.0}%）→ 精简首页或拆分重模块",
            home_compile_ms as f64 / 1000.0
        ));
    }
    if max_parallelism < cpu_count && max_parallelism < parallelism_cap {
        prebuild_emit_progress(format!(
            "  3. [中] 当前 {max_parallelism} 路并行（本机 {cpu_count} 核）→ 可设 MEI_PREBUILD_MAX_PARALLELISM={} 再跑",
            cpu_count.min(16)
        ));
    } else if parallelism_cap == PREBUILD_MAX_PARALLELISM && cpu_count > PREBUILD_MAX_PARALLELISM {
        prebuild_emit_progress(format!(
            "  3. [中] 本机 {cpu_count} 核，可试 MEI_PREBUILD_MAX_PARALLELISM=16（当前上限 {PREBUILD_MAX_PARALLELISM}）"
        ));
    }
    prebuild_emit_progress(
        "  4. [中] 使用 release 构建：cargo build --release -p mei-lang-server（debug 编译通常慢 2-3x）"
            .to_string(),
    );
    prebuild_emit_progress(
        "  5. [中] 未改 .mei 时用 prebuild --verify（秒级校验，跳过全量重算）".to_string(),
    );
    if warning_count > 0 {
        prebuild_emit_progress(format!(
            "  6. [低] 修复 {warning_count} 条 warning（失败 scope 会拖慢产物阶段并可能重复重试）"
        ));
    }
}

