#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";

const RED_THRESHOLDS = {
  html_ready_ratio: 0.15,
  stable_render_ratio: 0.15,
  interactive_ratio: 0.15,
  critical_ready_ratio: 0.2,
  local_feedback_ratio: 0.2,
  compile_ratio: 0.2,
  metric_total_ratio: 0.2,
  request_count_ratio: 0.3,
};

const YELLOW_THRESHOLDS = {
  hydrate_ratio: 0.15,
  eval_ratio: 0.15,
  metric_request_start_ratio: 0.25,
  metric_request_total_ratio: 0.25,
  compile_cache_lookup_ratio: 0.25,
  compile_cache_lock_wait_ratio: 0.25,
};

const args = parseArgs(process.argv.slice(2));
if (args.help) {
  printHelp();
  process.exit(0);
}

const samplePath = path.resolve(
  args.sample || process.env.MEI_SAMPLE_PATH || process.env.MEI_OUTPUT_JSONL || ""
);
if (!samplePath || samplePath === path.resolve("")) {
  throw new Error("--sample <jsonl> is required");
}

const scenarioFile = args.scenarioFile
  ? path.resolve(args.scenarioFile)
  : process.env.MEI_SCENARIO_FILE
    ? path.resolve(process.env.MEI_SCENARIO_FILE)
    : "";
const scenarioConfig = scenarioFile ? await readJson(scenarioFile) : {};
const scenarioDir = scenarioFile ? path.dirname(scenarioFile) : process.cwd();
const compareMode = String(
  args.mode || process.env.MEI_COMPARE_MODE || "auto"
).trim().toLowerCase();

const ledgerPath = resolveOptionalPath(
  args.ledger || process.env.MEI_LEDGER_PATH || scenarioConfig.ledger_path || "",
  scenarioDir
);
const pinnedBaselinePath = resolveOptionalPath(
  args.baselineFile ||
    process.env.MEI_BASELINE_FILE ||
    scenarioConfig.pinned_baseline_path ||
    "",
  scenarioDir
);
const reportFormat = String(args.format || process.env.MEI_REPORT_FORMAT || "text")
  .trim()
  .toLowerCase();
const reportOutput = resolveOptionalPath(
  args.reportOutput || process.env.MEI_REPORT_OUTPUT || "",
  process.cwd()
);

const currentRecords = await readJsonlOrEmpty(samplePath);
if (currentRecords.length === 0) {
  throw new Error("sample output is empty, cannot generate report");
}

const currentEntries = aggregateRecords(currentRecords);
const pinnedBaselineEntries = pinnedBaselinePath
  ? aggregateRecords(await readJsonlOrEmpty(pinnedBaselinePath))
  : [];
const ledgerHistory = ledgerPath ? await readJsonlOrEmpty(ledgerPath) : [];
const pinnedBaselineMap = new Map(
  pinnedBaselineEntries.map((entry) => [baselineKey(entry), entry])
);

let redCount = 0;
let yellowCount = 0;
let noBaselineCount = 0;
const reportRows = [];

for (const entry of currentEntries) {
  const baseline = resolveBaseline({
    compareMode,
    entry,
    pinnedBaselineMap,
    pinnedBaselineEntries,
    ledgerHistory,
  });
  if (!baseline) {
    noBaselineCount += 1;
    reportRows.push({
      scenario_id: entry.scenario_id,
      status: "NO_BASELINE",
      baseline_source: "-",
      details: ["无可用基线"],
    });
    continue;
  }
  const { red, yellow } = compareEntry(baseline.entry, entry);
  const contextDetails = buildContextDetails(baseline.entry, entry);
  if (red.length > 0) {
    redCount += 1;
    reportRows.push({
      scenario_id: entry.scenario_id,
      status: "RED",
      baseline_source: baseline.source,
      details: [...red, ...contextDetails],
    });
    continue;
  }
  if (yellow.length > 0) {
    yellowCount += 1;
    reportRows.push({
      scenario_id: entry.scenario_id,
      status: "YELLOW",
      baseline_source: baseline.source,
      details: [...yellow, ...contextDetails],
    });
    continue;
  }
  reportRows.push({
    scenario_id: entry.scenario_id,
    status: "OK",
    baseline_source: baseline.source,
    details: [`sample_count=${entry.sample_count}`, ...contextDetails],
  });
}

const summary = {
  checked: currentEntries.length,
  red: redCount,
  yellow: yellowCount,
  no_baseline: noBaselineCount,
  compare_mode: compareMode,
};

const rendered =
  reportFormat === "markdown"
    ? renderMarkdownReport(reportRows, summary)
    : renderTextReport(reportRows, summary);

if (reportOutput) {
  await fs.writeFile(reportOutput, rendered, "utf8");
}

process.stdout.write(rendered);
if (!rendered.endsWith("\n")) {
  process.stdout.write("\n");
}

if (redCount > 0) {
  process.exit(1);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--help" || token === "-h") {
      parsed.help = true;
      continue;
    }
    if (token.startsWith("--") && index + 1 < argv.length) {
      const key = token
        .slice(2)
        .replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
      parsed[key] = argv[index + 1];
      index += 1;
    }
  }
  return parsed;
}

function printHelp() {
  console.log(`host-perf-report.mjs

Usage:
  node ./scripts/perf/host-perf-report.mjs --sample <jsonl> [options]

Options:
  --sample <path>         Current sample JSONL
  --scenario-file <path>  Optional scenario config JSON
  --ledger <path>         Optional ledger JSONL
  --baseline-file <path>  Optional pinned baseline JSONL
  --mode <auto|latest|pinned>
  --format <text|markdown>
  --report-output <path>  Optional report file output
  --help                  Show help
`);
}

async function readJson(filePath) {
  return JSON.parse(await fs.readFile(filePath, "utf8"));
}

async function readJsonlOrEmpty(filePath) {
  if (!filePath) {
    return [];
  }
  try {
    const raw = await fs.readFile(filePath, "utf8");
    return raw
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  } catch (error) {
    if (error?.code === "ENOENT") {
      return [];
    }
    throw error;
  }
}

function resolveOptionalPath(rawPath, baseDir) {
  const value = String(rawPath || "").trim();
  if (!value) {
    return "";
  }
  return path.isAbsolute(value) ? value : path.resolve(baseDir, value);
}

function baselineKey(entry) {
  return [
    entry.record_kind || "scenario_sample",
    entry.app_id || "",
    entry.scenario_id,
    entry.run_kind || "",
    String(entry.environment || ""),
  ].join("|");
}

function aggregateRecords(records) {
  const groups = new Map();
  for (const record of records) {
    const key = baselineKey(record);
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key).push(record);
  }
  return [...groups.values()].map(aggregateRecordGroup);
}

function aggregateRecordGroup(group) {
  const [first] = group;
  const perf = {};
  const perfKeys = new Set();
  for (const record of group) {
    Object.keys(record.perf || {}).forEach((key) => perfKeys.add(key));
  }
  for (const key of perfKeys) {
    const values = group
      .map((record) => toFinite(record.perf?.[key]))
      .filter((value) => Number.isFinite(value))
      .sort((left, right) => left - right);
    if (values.length > 0) {
      perf[key] = median(values);
    }
  }
  return {
    ...first,
    record_kind: first.record_kind || "scenario_sample",
    revision: first.revision,
    measured_at: first.measured_at,
    perf,
    sample_count: group.length,
  };
}

function resolveBaseline({ compareMode, entry, pinnedBaselineMap, pinnedBaselineEntries, ledgerHistory }) {
  const pinned = pinnedBaselineMap.get(baselineKey(entry));
  if (compareMode === "pinned") {
    if (pinned) {
      return { source: "pinned", entry: pinned };
    }
    const pinnedAnyEnv = findPinnedAnyEnv(pinnedBaselineEntries, entry);
    return pinnedAnyEnv ? { source: "pinned:any-env", entry: pinnedAnyEnv } : null;
  }
  if (compareMode === "latest") {
    const latest = findLastHistory(ledgerHistory, entry);
    return latest ? { source: latest.source, entry: latest.entry } : null;
  }
  if (pinned) {
    return { source: "pinned", entry: pinned };
  }
  const pinnedAnyEnv = findPinnedAnyEnv(pinnedBaselineEntries, entry);
  if (pinnedAnyEnv) {
    return { source: "pinned:any-env", entry: pinnedAnyEnv };
  }
  const latest = findLastHistory(ledgerHistory, entry);
  return latest ? { source: latest.source, entry: latest.entry } : null;
}

function findPinnedAnyEnv(entries, currentEntry) {
  for (const entry of entries) {
    if (
      (entry.record_kind || "scenario_sample") === (currentEntry.record_kind || "scenario_sample") &&
      String(entry.app_id || "") === String(currentEntry.app_id || "") &&
      entry.scenario_id === currentEntry.scenario_id &&
      entry.run_kind === currentEntry.run_kind
    ) {
      return entry;
    }
  }
  return null;
}

function findLastHistory(list, currentEntry) {
  for (let index = list.length - 1; index >= 0; index -= 1) {
    const row = list[index];
    if (
      (row.record_kind || "scenario_sample") === (currentEntry.record_kind || "scenario_sample") &&
      String(row.app_id || "") === String(currentEntry.app_id || "") &&
      row.scenario_id === currentEntry.scenario_id &&
      row.run_kind === currentEntry.run_kind &&
      String(row.environment || "") === String(currentEntry.environment || "")
    ) {
      return { source: "latest", entry: row };
    }
  }
  for (let index = list.length - 1; index >= 0; index -= 1) {
    const row = list[index];
    if (
      (row.record_kind || "scenario_sample") === (currentEntry.record_kind || "scenario_sample") &&
      String(row.app_id || "") === String(currentEntry.app_id || "") &&
      row.scenario_id === currentEntry.scenario_id &&
      row.run_kind === currentEntry.run_kind
    ) {
      return { source: "latest:any-env", entry: row };
    }
  }
  return null;
}

function compareEntry(prev, curr) {
  if ((curr.record_kind || "scenario_sample") === "startup_run") {
    return compareStartupEntry(prev, curr);
  }
  return compareScenarioEntry(prev, curr);
}

function compareScenarioEntry(prev, curr) {
  const red = [];
  const yellow = [];
  const prevPerf = prev.perf || {};
  const currPerf = curr.perf || {};

  pushRatioRegression(
    red,
    "handler_html_ready_ms",
    prevPerf.handler_html_ready_ms,
    currPerf.handler_html_ready_ms,
    RED_THRESHOLDS.html_ready_ratio
  );
  pushRatioRegression(
    red,
    "first_stable_render_ms",
    prevPerf.first_stable_render_ms,
    currPerf.first_stable_render_ms,
    RED_THRESHOLDS.stable_render_ratio
  );
  pushRatioRegression(
    red,
    "first_interactive_ms",
    prevPerf.first_interactive_ms,
    currPerf.first_interactive_ms,
    RED_THRESHOLDS.interactive_ratio
  );
  pushRatioRegression(
    red,
    "access_critical_metrics_ready_ms",
    prevPerf.access_critical_metrics_ready_ms,
    currPerf.access_critical_metrics_ready_ms,
    RED_THRESHOLDS.critical_ready_ratio
  );
  pushRatioRegression(
    red,
    "local_edit_feedback_ms",
    prevPerf.local_edit_feedback_ms,
    currPerf.local_edit_feedback_ms,
    RED_THRESHOLDS.local_feedback_ratio
  );
  pushRatioRegression(red, "compile_ms", prevPerf.compile_ms, currPerf.compile_ms, RED_THRESHOLDS.compile_ratio);
  pushRatioRegression(
    red,
    "metric_total_ms",
    prevPerf.metric_total_ms,
    currPerf.metric_total_ms,
    RED_THRESHOLDS.metric_total_ratio
  );
  pushRatioRegression(
    red,
    "metrics_request_count",
    prevPerf.metrics_request_count,
    currPerf.metrics_request_count,
    RED_THRESHOLDS.request_count_ratio
  );
  pushRatioRegression(
    red,
    "server_request_count",
    prevPerf.server_request_count,
    currPerf.server_request_count,
    RED_THRESHOLDS.request_count_ratio
  );
  pushRatioRegression(
    red,
    "server_response_bytes_total",
    prevPerf.server_response_bytes_total,
    currPerf.server_response_bytes_total,
    RED_THRESHOLDS.request_count_ratio
  );
  pushRatioRegression(
    red,
    "browser_api_bytes_total",
    prevPerf.browser_api_bytes_total,
    currPerf.browser_api_bytes_total,
    RED_THRESHOLDS.request_count_ratio
  );

  if (
    toFinite(prevPerf.manage_dataset_resources) <= 9 &&
    toFinite(currPerf.manage_dataset_resources) >= 15
  ) {
    red.push(
      `manage_dataset_resources ${toFinite(prevPerf.manage_dataset_resources)} -> ${toFinite(currPerf.manage_dataset_resources)}`
    );
  }

  if (
    curr.run_kind === "warm" &&
    toFinite(prevPerf.compile_cache_hit) === 0 &&
    toFinite(currPerf.compile_cache_hit) === 0
  ) {
    red.push("warm compile_cache_hit 连续为 0");
  }

  if (toFinite(prevPerf.metric_response_cache_hit) === 1 && toFinite(currPerf.metric_response_cache_hit) === 0) {
    yellow.push("metric_response_cache_hit 1 -> 0");
  }
  if (
    toFinite(prevPerf.stable_render_within_window) === 1 &&
    toFinite(currPerf.stable_render_within_window) === 0
  ) {
    yellow.push("stable_render_within_window 1 -> 0");
  }
  if (
    toFinite(prevPerf.interactive_within_window) === 1 &&
    toFinite(currPerf.interactive_within_window) === 0
  ) {
    yellow.push("interactive_within_window 1 -> 0");
  }

  pushRatioRegression(
    yellow,
    "metric_hydrate_datasets_ms",
    prevPerf.metric_hydrate_datasets_ms,
    currPerf.metric_hydrate_datasets_ms,
    YELLOW_THRESHOLDS.hydrate_ratio
  );
  pushRatioRegression(
    yellow,
    "metric_eval_ms",
    prevPerf.metric_eval_ms,
    currPerf.metric_eval_ms,
    YELLOW_THRESHOLDS.eval_ratio
  );
  pushRatioRegression(
    yellow,
    "first_metric_request_start_ms",
    prevPerf.first_metric_request_start_ms,
    currPerf.first_metric_request_start_ms,
    YELLOW_THRESHOLDS.metric_request_start_ratio
  );
  pushRatioRegression(
    yellow,
    "metric_request_total_ms",
    prevPerf.metric_request_total_ms,
    currPerf.metric_request_total_ms,
    YELLOW_THRESHOLDS.metric_request_total_ratio
  );
  pushRatioRegression(
    yellow,
    "compile_cache_lookup_ms",
    prevPerf.compile_cache_lookup_ms,
    currPerf.compile_cache_lookup_ms,
    YELLOW_THRESHOLDS.compile_cache_lookup_ratio
  );

  const prevLockWait = bestFinite(
    prevPerf.metric_compile_cache_lock_wait_ms,
    prevPerf.dataset_compile_cache_lock_wait_ms
  );
  const currLockWait = bestFinite(
    currPerf.metric_compile_cache_lock_wait_ms,
    currPerf.dataset_compile_cache_lock_wait_ms
  );
  pushRatioRegression(
    yellow,
    "compile_cache_lock_wait_ms",
    prevLockWait,
    currLockWait,
    YELLOW_THRESHOLDS.compile_cache_lock_wait_ratio
  );

  return { red, yellow };
}

function compareStartupEntry(prev, curr) {
  const red = [];
  const yellow = [];
  const prevPerf = prev.perf || {};
  const currPerf = curr.perf || {};
  const correctnessRed = compareStartupCorrectness(prev, curr);
  if (correctnessRed.length > 0) {
    return { red: correctnessRed, yellow };
  }
  pushRatioRegression(
    red,
    "startup_run_wall_ms",
    prevPerf.startup_run_wall_ms,
    currPerf.startup_run_wall_ms,
    RED_THRESHOLDS.local_feedback_ratio
  );
  pushRatioRegression(
    red,
    "startup_hot_total_ms",
    prevPerf.startup_hot_total_ms,
    currPerf.startup_hot_total_ms,
    RED_THRESHOLDS.compile_ratio
  );
  pushRatioRegression(
    red,
    "startup_full_total_ms",
    prevPerf.startup_full_total_ms,
    currPerf.startup_full_total_ms,
    RED_THRESHOLDS.compile_ratio
  );
  pushRatioRegression(
    red,
    "startup_peak_rss_bytes",
    prevPerf.startup_peak_rss_bytes,
    currPerf.startup_peak_rss_bytes,
    RED_THRESHOLDS.compile_ratio
  );
  pushRatioRegression(
    red,
    "startup_eval_artifact_bytes",
    prevPerf.startup_eval_artifact_bytes,
    currPerf.startup_eval_artifact_bytes,
    RED_THRESHOLDS.request_count_ratio
  );
  pushRatioRegression(
    red,
    "startup_real_compile_count",
    prevPerf.startup_real_compile_count,
    currPerf.startup_real_compile_count,
    RED_THRESHOLDS.request_count_ratio
  );
  pushRatioRegression(
    yellow,
    "startup_canonical_prebuild_nodes",
    prevPerf.startup_canonical_prebuild_nodes,
    currPerf.startup_canonical_prebuild_nodes,
    YELLOW_THRESHOLDS.metric_request_total_ratio
  );
  pushRatioRegression(
    yellow,
    "startup_expansion_ratio",
    prevPerf.startup_expansion_ratio,
    currPerf.startup_expansion_ratio,
    YELLOW_THRESHOLDS.metric_request_total_ratio
  );
  pushRatioRegression(
    yellow,
    "startup_compile_index_stale_entries",
    prevPerf.startup_compile_index_stale_entries,
    currPerf.startup_compile_index_stale_entries,
    YELLOW_THRESHOLDS.metric_request_total_ratio
  );
  pushRatioRegression(
    yellow,
    "startup_deferred_warmup_total_ms",
    prevPerf.startup_deferred_warmup_total_ms,
    currPerf.startup_deferred_warmup_total_ms,
    YELLOW_THRESHOLDS.metric_request_total_ratio
  );
  return { red, yellow };
}

function compareStartupCorrectness(prev, curr) {
  const red = [];
  const prevPerf = prev?.perf || {};
  const currPerf = curr?.perf || {};
  const prevCategories = startupWarningCategories(prev);
  const currCategories = startupWarningCategories(curr);
  const currFailingDatasets = startupFailingDatasets(curr);
  const startupNodeBudgetOverflow = toFinite(currPerf.startup_node_budget_overflow);
  const startupNodeBudgetLimit = toFinite(currPerf.startup_node_budget_limit);
  const startupCanonicalNodes = toFinite(currPerf.startup_canonical_prebuild_nodes);
  const startupFullTotalMs = toFinite(currPerf.startup_full_total_ms);
  if (toFinite(currPerf.startup_outcome_ready) === 0) {
    red.push("correctness startup_outcome_ready=0");
  }
  if (toFinite(currPerf.startup_access_artifacts_ready) === 0) {
    red.push("correctness access_artifacts_ready=0");
  }
  if (toFinite(currPerf.startup_correctness_failed) === 1) {
    red.push("correctness startup_correctness_failed=1");
  }
  if (toFinite(currPerf.startup_last_failed_app_count) > 0) {
    red.push(`correctness startup_last_failed_app_count=${toFinite(currPerf.startup_last_failed_app_count)}`);
  }
  for (const [field, label] of [
    ["startup_warmup_dataset_locate_failed_count", "warmup_dataset_locate_failed"],
    ["startup_metric_response_eval_failed_count", "metric_response_eval_failed"],
    ["startup_metric_dataframe_eval_failed_count", "metric_dataframe_eval_failed"],
    ["startup_artifact_coverage_miss_count", "artifact_coverage_miss"],
    ["startup_artifact_index_miss_count", "artifact_index_miss"],
  ]) {
    const value = toFinite(currPerf[field]);
    if (Number.isFinite(value) && value > 0) {
      red.push(`correctness ${label}=${value}`);
    }
  }
  if (currCategories.length > 0) {
    red.push(`correctness warning_categories=${currCategories.join(",")}`);
  }
  if (prevCategories.join(",") !== currCategories.join(",")) {
    red.push(
      `correctness warning_categories_changed ${prevCategories.join(",") || "-"} -> ${currCategories.join(",") || "-"}`
    );
  }
  if (currFailingDatasets.length > 0) {
    red.push(`correctness failing_datasets=${currFailingDatasets.join(",")}`);
  }
  if (startupNodeBudgetOverflow === 1) {
    red.push(
      `budget canonical_prebuild_nodes=${startupCanonicalNodes} exceeds limit=${startupNodeBudgetLimit}`
    );
  }
  if (Number.isFinite(startupFullTotalMs) && startupFullTotalMs > 60000) {
    red.push(`budget startup_full_total_ms=${startupFullTotalMs} exceeds limit=60000`);
  }
  return red;
}

function pushRatioRegression(bucket, field, prev, curr, threshold) {
  const base = toFinite(prev);
  const now = toFinite(curr);
  if (!Number.isFinite(base) || !Number.isFinite(now) || base <= 0) {
    return;
  }
  const ratio = (now - base) / base;
  if (ratio >= threshold) {
    bucket.push(`${field} ${base} -> ${now} (+${Math.round(ratio * 100)}%)`);
  }
}

function buildContextDetails(prev, curr) {
  if ((curr.record_kind || "scenario_sample") === "startup_run") {
    return buildStartupContextDetails(prev, curr);
  }
  const compileStage = formatStageComparison(prev?.perf || {}, curr?.perf || {}, [
    "dependency_graph_build_ms",
    "active_payload_pick_or_compile_ms",
    "catalog_compile_ms",
    "world_finalize_ms",
  ]);
  const warmupPhase = formatWarmupComparison(prev?.perf || {}, curr?.perf || {});
  const requestPhase = formatRequestComparison(prev?.perf || {}, curr?.perf || {});
  return [compileStage, warmupPhase, requestPhase].filter(Boolean);
}

function buildStartupContextDetails(prev, curr) {
  const prevPerf = prev?.perf || {};
  const currPerf = curr?.perf || {};
  const parts = [];
  const correctness = [];
  const currCategories = startupWarningCategories(curr);
  const currFailingDatasets = startupFailingDatasets(curr);
  if (Number.isFinite(toFinite(currPerf.startup_correctness_failed))) {
    correctness.push(`correctness_failed=${toFinite(currPerf.startup_correctness_failed)}`);
  }
  if (Number.isFinite(toFinite(currPerf.startup_warning_category_count))) {
    correctness.push(`warning_category_count=${toFinite(currPerf.startup_warning_category_count)}`);
  }
  if (currCategories.length > 0) {
    correctness.push(`warning_categories=${currCategories.join(",")}`);
  }
  if (currFailingDatasets.length > 0) {
    correctness.push(`failing_datasets=${currFailingDatasets.join(",")}`);
  }
  if (correctness.length > 0) {
    parts.push(`correctness ${correctness.join(" ")}`);
  }
  for (const field of [
    "startup_hot_total_ms",
    "startup_full_total_ms",
    "startup_peak_rss_bytes",
    "startup_eval_artifact_bytes",
    "startup_real_compile_count",
    "startup_canonical_prebuild_nodes",
  ]) {
    const before = toFinite(prevPerf[field]);
    const after = toFinite(currPerf[field]);
    if (!Number.isFinite(before) && !Number.isFinite(after)) continue;
    parts.push(Number.isFinite(before) ? `${field}=${before}->${after}` : `${field}=${after}`);
  }
  if (curr.host_build_version || prev.host_build_version) {
    parts.push(`build=${prev.host_build_version || "-"}->${curr.host_build_version || "-"}`);
  }
  if (curr.host_run_id) {
    parts.push(`run=${curr.host_run_id}`);
  }
  return parts.length > 0 ? parts : [];
}

function startupWarningCategories(row) {
  return ensureStringList(row?.startup_run_summary?.run?.warningCategories);
}

function startupFailingDatasets(row) {
  return ensureStringList(row?.startup_run_summary?.run?.failingDatasets);
}

function ensureStringList(value) {
  return Array.isArray(value) ? value.map((item) => String(item || "").trim()).filter(Boolean) : [];
}

function formatStageComparison(prevPerf, currPerf, fields) {
  const parts = [];
  for (const field of fields) {
    const prev = toFinite(prevPerf?.[field]);
    const curr = toFinite(currPerf?.[field]);
    if (!Number.isFinite(prev) && !Number.isFinite(curr)) {
      continue;
    }
    parts.push(
      Number.isFinite(prev)
        ? `${shortStageField(field)}=${prev}->${curr}`
        : `${shortStageField(field)}=${curr}`
    );
  }
  if (parts.length === 0) {
    return "";
  }
  return `compile_stage ${parts.join(" ")}`;
}

function formatWarmupComparison(prevPerf, currPerf) {
  const parts = [];
  for (const field of [
    "host_full_warmup_ready",
    "host_deferred_warmup_pending",
    "host_last_build_total_ms",
    "host_last_build_compile_ms",
    "host_last_build_warmup_ms",
  ]) {
    const prev = toFinite(prevPerf?.[field]);
    const curr = toFinite(currPerf?.[field]);
    if (!Number.isFinite(prev) && !Number.isFinite(curr)) {
      continue;
    }
    parts.push(
      Number.isFinite(prev) ? `${field}=${prev}->${curr}` : `${field}=${curr}`
    );
  }
  if (parts.length === 0) {
    return "";
  }
  return `warmup_state ${parts.join(" ")}`;
}

function formatRequestComparison(prevPerf, currPerf) {
  const parts = [];
  for (const field of [
    "server_request_count",
    "server_response_bytes_total",
    "browser_api_total",
    "browser_api_bytes_total",
  ]) {
    const prev = toFinite(prevPerf?.[field]);
    const curr = toFinite(currPerf?.[field]);
    if (!Number.isFinite(prev) && !Number.isFinite(curr)) {
      continue;
    }
    parts.push(Number.isFinite(prev) ? `${field}=${prev}->${curr}` : `${field}=${curr}`);
  }
  if (parts.length === 0) {
    return "";
  }
  return `request_profile ${parts.join(" ")}`;
}

function shortStageField(field) {
  switch (field) {
    case "dependency_graph_build_ms":
      return "graph";
    case "active_payload_pick_or_compile_ms":
      return "active";
    case "catalog_compile_ms":
      return "catalog";
    case "world_finalize_ms":
      return "finalize";
    default:
      return field;
  }
}

function rowLabel(row) {
  if ((row.record_kind || "scenario_sample") === "startup_run") {
    return `startup:${row.app_id || "-"}`;
  }
  return row.scenario_id;
}

function groupRowsByRecordKind(rows) {
  return {
    startup: rows.filter((row) => (row.record_kind || "scenario_sample") === "startup_run"),
    scenarios: rows.filter((row) => (row.record_kind || "scenario_sample") !== "startup_run"),
  };
}

function renderTextReport(rows, summary) {
  const lines = ["==> host perf report", ""];
  const groups = groupRowsByRecordKind(rows);
  if (groups.startup.length > 0) {
    lines.push("startup:");
    for (const row of groups.startup) {
      lines.push(`- ${rowLabel(row)}: ${row.status} [${row.baseline_source}] ${row.details.join("; ")}`);
    }
    lines.push("");
  }
  if (groups.scenarios.length > 0) {
    lines.push("scenarios:");
    for (const row of groups.scenarios) {
      lines.push(`- ${rowLabel(row)}: ${row.status} [${row.baseline_source}] ${row.details.join("; ")}`);
    }
  }
  lines.push("");
  lines.push(
    `summary: red=${summary.red} yellow=${summary.yellow} no_baseline=${summary.no_baseline} checked=${summary.checked} mode=${summary.compare_mode}`
  );
  return lines.join("\n");
}

function renderMarkdownReport(rows, summary) {
  const lines = [
    "## Host Perf Report",
    "",
    "### Startup Runs",
    "",
    "| Startup | Status | Baseline | Details |",
    "|---|---|---|---|",
  ];
  const groups = groupRowsByRecordKind(rows);
  for (const row of groups.startup) {
    lines.push(
      `| ${rowLabel(row)} | ${row.status} | ${row.baseline_source} | ${row.details.join("<br>")} |`
    );
  }
  lines.push("");
  lines.push("### Scenarios");
  lines.push("");
  lines.push("| Scenario | Status | Baseline | Details |");
  lines.push("|---|---|---|---|");
  for (const row of groups.scenarios) {
    lines.push(
      `| ${rowLabel(row)} | ${row.status} | ${row.baseline_source} | ${row.details.join("<br>")} |`
    );
  }
  lines.push("");
  lines.push(
    `summary: red=${summary.red} yellow=${summary.yellow} no_baseline=${summary.no_baseline} checked=${summary.checked} mode=${summary.compare_mode}`
  );
  return lines.join("\n");
}

function bestFinite(...values) {
  for (const value of values) {
    const n = toFinite(value);
    if (Number.isFinite(n)) {
      return n;
    }
  }
  return NaN;
}

function median(values) {
  const mid = Math.floor(values.length / 2);
  if (values.length % 2 === 1) {
    return values[mid];
  }
  return (values[mid - 1] + values[mid]) / 2;
}

function toFinite(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : NaN;
}
