#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";

const RED_THRESHOLDS = {
  html_ready_ratio: 0.15,
  stable_render_ratio: 0.15,
  interactive_ratio: 0.15,
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
  node ./scripts/host-perf-report.mjs --sample <jsonl> [options]

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
      row.scenario_id === currentEntry.scenario_id &&
      row.run_kind === currentEntry.run_kind &&
      String(row.environment || "") === String(currentEntry.environment || "")
    ) {
      return { source: "latest", entry: row };
    }
  }
  for (let index = list.length - 1; index >= 0; index -= 1) {
    const row = list[index];
    if (row.scenario_id === currentEntry.scenario_id && row.run_kind === currentEntry.run_kind) {
      return { source: "latest:any-env", entry: row };
    }
  }
  return null;
}

function compareEntry(prev, curr) {
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
  const compileStage = formatStageComparison(prev?.perf || {}, curr?.perf || {}, [
    "dependency_graph_build_ms",
    "active_payload_pick_or_compile_ms",
    "catalog_compile_ms",
    "world_finalize_ms",
  ]);
  return [compileStage].filter(Boolean);
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

function renderTextReport(rows, summary) {
  const lines = ["==> host perf report", ""];
  for (const row of rows) {
    lines.push(
      `- ${row.scenario_id}: ${row.status} [${row.baseline_source}] ${row.details.join("; ")}`
    );
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
    "| Scenario | Status | Baseline | Details |",
    "|---|---|---|---|",
  ];
  for (const row of rows) {
    lines.push(
      `| ${row.scenario_id} | ${row.status} | ${row.baseline_source} | ${row.details.join("<br>")} |`
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
