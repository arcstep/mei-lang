#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";
import { createHash } from "node:crypto";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const meiLangRoot = path.resolve(scriptDir, "..");
const projectsRoot = path.resolve(meiLangRoot, "..");

const defaultScenarioFile = path.join(scriptDir, "perf-scenarios", "template.app.json");
const defaultLedgerPath = path.join(
  projectsRoot,
  "docs",
  "mei-lang",
  "benchmarks",
  "template-app-perf-ledger.jsonl"
);

const args = parseArgs(process.argv.slice(2));
if (args.help) {
  printHelp();
  process.exit(0);
}

const scenarioFile = path.resolve(
  args.scenarioFile || process.env.MEI_SCENARIO_FILE || defaultScenarioFile
);
const scenarioPayload = await readJson(scenarioFile);
const scenarioDir = path.dirname(scenarioFile);
const serverUrl = String(args.serverUrl || process.env.MEI_SERVER_URL || "http://127.0.0.1:9527")
  .trim()
  .replace(/\/+$/, "");
const explicitOutput = String(args.output || process.env.MEI_OUTPUT_JSONL || "").trim();
const outputPath = explicitOutput
  ? path.resolve(explicitOutput)
  : resolveScenarioPath(scenarioPayload.ledger_path || defaultLedgerPath, scenarioDir);
const append = args.append ?? process.env.MEI_APPEND !== "0";
const environmentName = String(
  args.environment || process.env.MEI_ENV || scenarioPayload.environment || "local_release_noauth"
).trim();
const authBearer = String(args.authBearer || process.env.MEI_AUTH_BEARER || "").trim();
const cookieHeader = String(args.cookie || process.env.MEI_COOKIE || "").trim();
const scenarioIdFilter = String(args.scenarioId || process.env.MEI_SCENARIO_ID || "").trim();
const scenarioFamilyFilter = String(
  args.scenarioFamily || process.env.MEI_SCENARIO_FAMILY || ""
).trim();
const browserWindowMsDefault = toFinite(
  args.browserWindowMs || process.env.MEI_BROWSER_WINDOW_MS || 0
);
const repeatCount = Math.max(
  1,
  Math.trunc(
    toFinite(args.repeat || process.env.MEI_REPEAT || scenarioPayload.default_repeat || 1)
  ) || 1
);
const requestHeaders = buildRequestHeaders({ authBearer, cookieHeader });
const revision = currentRevision();
const measuredAt = new Date().toISOString();
let playwrightChromiumPromise = null;

const workspaceId = String(
  scenarioPayload.workspace_id || process.env.MEI_WORKSPACE_ID || "unknown_workspace"
).trim();
const appId = String(scenarioPayload.app_id || process.env.MEI_APP_ID || "your-app").trim();
const rawScenarios = Array.isArray(scenarioPayload.scenarios) ? scenarioPayload.scenarios : [];
const scenarios = rawScenarios
  .map(normalizeScenario)
  .filter((scenario) => {
    if (scenarioIdFilter && scenario.scenario_id !== scenarioIdFilter) {
      return false;
    }
    if (scenarioFamilyFilter && scenario.scenario_family !== scenarioFamilyFilter) {
      return false;
    }
    return true;
  });

if (scenarios.length === 0) {
  throw new Error(
    `scenario file has no matching scenarios: ${scenarioFile} id=${scenarioIdFilter || "-"} family=${scenarioFamilyFilter || "-"}`
  );
}

const records = [];
for (const scenario of scenarios) {
  for (let repeatIndex = 1; repeatIndex <= repeatCount; repeatIndex += 1) {
    records.push(
      await sampleScenario({
        scenario,
        serverUrl,
        appId,
        workspaceId,
        environmentName,
        revision,
        measuredAt,
        requestHeaders,
        repeatIndex,
        repeatCount,
      })
    );
  }
}

await writeJsonl(outputPath, records, append);
printSummary({ outputPath, append, records });

async function sampleScenario(context) {
  const {
    scenario,
    serverUrl: baseUrl,
    appId: targetAppId,
    workspaceId: targetWorkspaceId,
    environmentName: envName,
    revision: currentRev,
    measuredAt: now,
    requestHeaders: headers,
    repeatIndex,
    repeatCount: totalRepeats,
  } = context;
  const perf = {};
  const notes = [];
  const browserWindowMs = Number.isFinite(scenario.browser_window_ms)
    ? scenario.browser_window_ms
    : browserWindowMsDefault;
  const needsDataset =
    scenario.clear_before_sample ||
    scenario.route_mode === "metric_probe" ||
    scenario.sample_metric_api ||
    scenario.sample_dataset_query;
  let datasetId = scenario.dataset_id;

  if (!datasetId && needsDataset && scenario.scene_id) {
    datasetId = await discoverDatasetId(baseUrl, targetAppId, scenario, headers);
    if (datasetId) {
      notes.push(`dataset_id=${datasetId}`);
    } else {
      notes.push("dataset_id_unavailable=1");
    }
  } else if (datasetId) {
    notes.push(`dataset_id=${datasetId}`);
  }

  if (scenario.clear_before_sample) {
    const clearDatasetId = datasetId || scenario.dataset_id || "__scenario_clear__";
    try {
      const recompute = await clearScenarioCaches(
        baseUrl,
        targetAppId,
        scenario,
        clearDatasetId,
        headers
      );
      perf.clear_total_ms = recompute.total_ms;
      perf.clear_ms = recompute.clear_ms;
      perf.clear_compile_cache_cleared = recompute.compile_cache_cleared;
      perf.clear_file_cache_cleared = recompute.file_cache_cleared;
      perf.clear_metric_response_cache_cleared = recompute.metric_response_cache_cleared;
      perf.clear_metric_dataframe_cache_cleared = recompute.metric_dataframe_cache_cleared;
      notes.push(`clear_mode=${scenario.clear_mode}`);
    } catch (error) {
      notes.push(`clear_error=${sanitizeNote(error)}`);
    }
  }

  if (scenario.run_kind === "switch_back" && scenario.switch_from_scene) {
    const switchUrl = buildPageUrl(baseUrl, targetAppId, {
      route_mode: scenario.route_mode,
      scene_id: scenario.switch_from_scene,
      target_file: scenario.target_file,
      chrome: scenario.chrome,
    });
    await fetchPage(switchUrl, headers);
    notes.push(`switch_from_scene=${scenario.switch_from_scene}`);
  }

  if (scenario.run_kind === "warm" && isPageScenario(scenario.route_mode)) {
    const warmUrl = buildPageUrl(baseUrl, targetAppId, scenario);
    await fetchPage(warmUrl, headers);
    notes.push("warmup_page_prefetch=1");
  }

  if (scenario.bootstrap_mode === "disabled") {
    notes.push("bootstrap_mode=disabled");
  }

  if (isPageScenario(scenario.route_mode)) {
    const pageUrl = buildPageUrl(baseUrl, targetAppId, scenario);
    if (browserWindowMs > 0) {
      try {
        const browserPerf = await captureBrowserWindowMetrics(pageUrl, headers, browserWindowMs, {
          browserWarmup: scenario.run_kind === "warm",
        });
        mergePerf(perf, browserPerf);
      } catch (error) {
        notes.push(`browser_window_error=${sanitizeNote(error)}`);
      }
    }
    const page = await fetchPageUntilReady(pageUrl, headers, {
      maxBootstrapWaitMs: Number.isFinite(scenario.bootstrap_max_wait_ms)
        ? scenario.bootstrap_max_wait_ms
        : 60000,
    });
    perf.page_http_elapsed_ms = page.elapsed_ms;
    perf.page_request_roundtrips = page.request_roundtrips;
    perf.bootstrap_shell_roundtrips = page.bootstrap_shell_roundtrips;
    perf.bootstrap_total_wall_ms = page.bootstrap_total_wall_ms;
    perf.bootstrap_waited_ms = page.bootstrap_waited_ms;
    perf.bootstrap_timed_out = page.bootstrap_timed_out;
    applyPagePerf(perf, page.headers);
    if (scenario.route_mode === "manage") {
      const pipeline = extractManagePipeline(page.body);
      if (pipeline) {
        applyManagePipelinePerf(perf, pipeline);
      } else {
        notes.push("manage_pipeline_missing=1");
      }
    }
    if (page.bootstrap_shell_roundtrips > 0) {
      notes.push(`bootstrap_shell_roundtrips=${page.bootstrap_shell_roundtrips}`);
    }
    if (page.bootstrap_timed_out >= 1) {
      notes.push("bootstrap_probe_timeout=1");
    }
  }

  if ((scenario.route_mode === "metric_probe" || scenario.sample_metric_api) && datasetId) {
    try {
      const metricPerf = await collectMetricPerf(
        baseUrl,
        targetAppId,
        scenario.scene_id,
        datasetId,
        scenario.metric_ids,
        headers
      );
      mergePerf(perf, metricPerf);
      if (scenario.metric_ids.length > 0) {
        notes.push(`metric_ids=${scenario.metric_ids.join(",")}`);
      }
    } catch (error) {
      notes.push(`metric_perf_error=${sanitizeNote(error)}`);
    }
  }

  if (scenario.sample_dataset_query && datasetId) {
    try {
      const datasetPerf = await collectDatasetPerf(
        baseUrl,
        targetAppId,
        scenario.scene_id,
        datasetId,
        scenario.dataset_query_metric_id,
        headers
      );
      mergePerf(perf, datasetPerf);
      if (scenario.dataset_query_metric_id) {
        notes.push(`dataset_query_metric_id=${scenario.dataset_query_metric_id}`);
      }
    } catch (error) {
      notes.push(`dataset_perf_error=${sanitizeNote(error)}`);
    }
  }

  try {
    mergePerf(perf, await collectHostReadinessSnapshot(baseUrl, headers));
  } catch (error) {
    notes.push(`host_readiness_error=${sanitizeNote(error)}`);
  }
  applyAcceptancePerf(perf, scenario);

  return {
    workspace_id: targetWorkspaceId,
    app_id: targetAppId,
    scenario_id: scenario.scenario_id,
    scenario_family: scenario.scenario_family || undefined,
    route_mode: scenario.route_mode,
    entry_url_or_locator: buildScenarioLocator(baseUrl, targetAppId, scenario),
    run_kind: scenario.run_kind,
    environment: envName,
    revision: currentRev,
    measured_at: now,
    sample_repeat_index: repeatIndex,
    sample_repeat_total: totalRepeats,
    perf,
    notes,
  };
}

function applyAcceptancePerf(perf, scenario) {
  const accessFirstVisible = bestFinite(perf.first_stable_render_ms, perf.handler_html_ready_ms);
  const accessFirstInteractive = bestFinite(perf.first_interactive_ms, accessFirstVisible);
  const accessCriticalMetricsReady = bestFinite(
    perf.critical_metrics_ready_ms,
    perf.first_metric_ready_ms,
    perf.metric_total_ms
  );
  const localEditFeedback = bestFinite(
    perf.bootstrap_total_wall_ms,
    perf.page_http_elapsed_ms,
    perf.handler_html_ready_ms,
    perf.compile_ms
  );
  const metricProbeReady = bestFinite(perf.metric_total_ms, perf.total_ms, perf.metric_elapsed_ms);

  if (scenario.route_mode === "access") {
    setNumeric(perf, "access_first_visible_ms", accessFirstVisible);
    setNumeric(perf, "access_first_interactive_ms", accessFirstInteractive);
    setNumeric(perf, "access_critical_metrics_ready_ms", accessCriticalMetricsReady);
    setNumeric(perf, "hot_start_ready_ms", accessFirstInteractive);
  }
  if (scenario.route_mode === "manage") {
    setNumeric(perf, "local_edit_feedback_ms", localEditFeedback);
  }
  if (scenario.route_mode === "metric_probe") {
    setNumeric(perf, "metric_probe_ready_ms", metricProbeReady);
  }
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--help" || token === "-h") {
      parsed.help = true;
      continue;
    }
    if (token === "--no-append") {
      parsed.append = false;
      continue;
    }
    if (token === "--append") {
      parsed.append = true;
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
  console.log(`host-perf-sample.mjs

Usage:
  node ./scripts/host-perf-sample.mjs [options]

Options:
  --server-url <url>        Host URL (default: http://127.0.0.1:9527)
  --scenario-file <path>    Scenario matrix JSON path
  --output <path>           JSONL output path
  --scenario-id <id>        Sample only one scenario id
  --scenario-family <name>  Sample only one scenario family
  --browser-window-ms <n>   Optional browser request window in ms
  --repeat <n>              Repeat each scenario n times
  --environment <name>      Environment label
  --auth-bearer <token>     Optional bearer token
  --cookie <header>         Optional cookie header
  --append                  Append output (default)
  --no-append               Overwrite output
  --help                    Show help

Scenario extensions:
  bootstrap_mode            default | disabled (disabled adds diag_filter sentinel)
  bootstrap_max_wait_ms     Max shell follow-up wait before timeout
  query_params              Extra URL query params for A/B experiments
`);
}

function currentRevision() {
  try {
    return execSync("git rev-parse --short HEAD", {
      cwd: meiLangRoot,
      stdio: ["ignore", "pipe", "ignore"],
      encoding: "utf8",
    }).trim();
  } catch {
    return "unknown_revision";
  }
}

async function readJson(filePath) {
  return JSON.parse(await fs.readFile(filePath, "utf8"));
}

function resolveScenarioPath(rawPath, scenarioDir) {
  const value = String(rawPath || "").trim();
  if (!value) {
    return path.resolve(defaultLedgerPath);
  }
  return path.isAbsolute(value) ? value : path.resolve(scenarioDir, value);
}

function normalizeScenario(raw) {
  const metricIds = Array.isArray(raw.metric_ids)
    ? raw.metric_ids.map((value) => String(value || "").trim()).filter(Boolean)
    : [];
  const queryParams = normalizeQueryParams(raw.query_params);
  const scenario = {
    scenario_id: String(raw.scenario_id || "").trim(),
    scenario_family: String(raw.scenario_family || "").trim(),
    route_mode: String(raw.route_mode || "").trim(),
    scene_id: raw.scene_id ? String(raw.scene_id).trim() : "",
    target_file: raw.target_file ? String(raw.target_file).trim() : "",
    run_kind: String(raw.run_kind || "cold").trim(),
    switch_from_scene: raw.switch_from_scene ? String(raw.switch_from_scene).trim() : "",
    dataset_id: raw.dataset_id ? String(raw.dataset_id).trim() : "",
    dataset_query_metric_id: raw.dataset_query_metric_id
      ? String(raw.dataset_query_metric_id).trim()
      : "",
    chrome: raw.chrome ? String(raw.chrome).trim() : "",
    browser_window_ms: toFinite(raw.browser_window_ms),
    bootstrap_max_wait_ms: toFinite(raw.bootstrap_max_wait_ms),
    bootstrap_mode: String(raw.bootstrap_mode || "default").trim().toLowerCase() || "default",
    clear_before_sample: raw.clear_before_sample === true,
    clear_mode: String(raw.clear_mode || "clear_only").trim().toLowerCase() || "clear_only",
    sample_metric_api: raw.sample_metric_api === true,
    sample_dataset_query: raw.sample_dataset_query === true,
    metric_ids: metricIds,
    query_params: queryParams,
  };
  if (!scenario.scenario_id) {
    throw new Error(`scenario_id is required: ${JSON.stringify(raw)}`);
  }
  if (!["access", "manage", "metric_probe"].includes(scenario.route_mode)) {
    throw new Error(`unsupported route_mode for ${scenario.scenario_id}: ${scenario.route_mode}`);
  }
  if (!["cold", "warm", "switch_back"].includes(scenario.run_kind)) {
    throw new Error(`unsupported run_kind for ${scenario.scenario_id}: ${scenario.run_kind}`);
  }
  if (!["default", "disabled"].includes(scenario.bootstrap_mode)) {
    throw new Error(
      `unsupported bootstrap_mode for ${scenario.scenario_id}: ${scenario.bootstrap_mode}`
    );
  }
  if ((scenario.route_mode === "access" || scenario.route_mode === "metric_probe") && !scenario.scene_id) {
    throw new Error(`scene_id is required for scenario: ${scenario.scenario_id}`);
  }
  if (scenario.route_mode === "manage" && !scenario.target_file) {
    throw new Error(`target_file is required for manage scenario: ${scenario.scenario_id}`);
  }
  if (scenario.route_mode === "metric_probe" && scenario.metric_ids.length === 0) {
    throw new Error(`metric_ids is required for metric_probe scenario: ${scenario.scenario_id}`);
  }
  return scenario;
}

function normalizeQueryParams(raw) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return {};
  }
  const out = {};
  for (const [key, value] of Object.entries(raw)) {
    const normalizedKey = String(key || "").trim();
    const normalizedValue = String(value ?? "").trim();
    if (!normalizedKey || !normalizedValue) {
      continue;
    }
    out[normalizedKey] = normalizedValue;
  }
  return out;
}

function isPageScenario(routeMode) {
  return routeMode === "access" || routeMode === "manage";
}

function buildScenarioLocator(baseUrl, appId, scenario) {
  if (scenario.route_mode === "metric_probe") {
    const query = new URLSearchParams();
    query.set("scene_id", scenario.scene_id);
    if (scenario.target_file) {
      query.set("target_file", scenario.target_file);
    }
    if (scenario.dataset_id) {
      query.set("dataset_id", scenario.dataset_id);
    }
    if (scenario.metric_ids.length > 0) {
      query.set("metric_ids", scenario.metric_ids.join(","));
    }
    return `/api/datasets/metrics/${encodeURI(appId)}?${query.toString()}`;
  }
  return buildPageUrl(baseUrl, appId, scenario).replace(baseUrl, "");
}

function buildPageUrl(baseUrl, appId, scenario) {
  const appPath = encodeURI(appId);
  const params = new URLSearchParams();
  if (scenario.scene_id && scenario.route_mode !== "access") {
    params.set("scene", scenario.scene_id);
  }
  if (scenario.target_file) {
    params.set("file", scenario.target_file);
  }
  if (scenario.chrome) {
    params.set("chrome", scenario.chrome);
  } else if (scenario.route_mode === "access") {
    params.set("chrome", "none");
  }
  if (scenario.bootstrap_mode === "disabled") {
    params.set("diag_filter", "__mei_compile_no_bootstrap__");
  }
  for (const [key, value] of Object.entries(scenario.query_params || {})) {
    params.set(key, value);
  }
  if (scenario.route_mode === "access") {
    const pathScene = scenario.scene_id ? `/scene/${encodeURIComponent(scenario.scene_id)}` : "";
    const query = params.toString();
    return `${baseUrl}/apps/app/${appPath}${pathScene}${query ? `?${query}` : ""}`;
  }
  const query = params.toString();
  return `${baseUrl}/apps/build/${appPath}${query ? `?${query}` : ""}`;
}

async function fetchPage(url, extraHeaders = {}) {
  const started = Date.now();
  const response = await fetch(url, {
    method: "GET",
    redirect: "follow",
    headers: extraHeaders,
  });
  const body = await response.text();
  if (!response.ok) {
    throw new Error(`page request failed: ${url} -> ${response.status}\n${body.slice(0, 500)}`);
  }
  if (isAuthPage(response.url, body)) {
    throw new Error(
      `page request redirected to auth flow: ${response.url}. Set MEI_COOKIE or MEI_AUTH_BEARER.`
    );
  }
  return {
    headers: response.headers,
    body,
    elapsed_ms: Date.now() - started,
  };
}

async function fetchPageUntilReady(url, extraHeaders = {}, options = {}) {
  const startedAt = Date.now();
  const maxWaitMs = Number.isFinite(options.maxBootstrapWaitMs)
    ? Math.max(2000, options.maxBootstrapWaitMs)
    : 60000;
  const maxAttempts = 120;
  let attempts = 0;
  let shellRounds = 0;
  let lastPage = null;
  while (attempts < maxAttempts) {
    attempts += 1;
    const page = await fetchPage(url, extraHeaders);
    lastPage = page;
    if (!isCompileBootstrapShell(page)) {
      return {
        ...page,
        request_roundtrips: attempts,
        bootstrap_shell_roundtrips: shellRounds,
        bootstrap_total_wall_ms: Date.now() - startedAt,
        bootstrap_waited_ms: Math.max(Date.now() - startedAt - page.elapsed_ms, 0),
        bootstrap_timed_out: 0,
      };
    }
    shellRounds += 1;
    if (Date.now() - startedAt >= maxWaitMs) {
      break;
    }
    await delay(bootstrapProbeDelay(shellRounds));
  }
  return {
    ...(lastPage || { headers: new Headers(), body: "", elapsed_ms: NaN }),
    request_roundtrips: attempts,
    bootstrap_shell_roundtrips: shellRounds,
    bootstrap_total_wall_ms: Date.now() - startedAt,
    bootstrap_waited_ms: Math.max(Date.now() - startedAt - toFinite(lastPage?.elapsed_ms), 0),
    bootstrap_timed_out: 1,
  };
}

function isCompileBootstrapShell(page) {
  const body = String(page?.body || "");
  if (body.includes("data-mei-compile-shell=\"true\"")) {
    return true;
  }
  if (body.includes("MeiLang 编译引导页")) {
    return true;
  }
  const ready = Number(page?.headers?.get?.("x-mei-handler-html-ready-ms"));
  if (Number.isFinite(ready)) {
    return false;
  }
  return false;
}

function bootstrapProbeDelay(shellRounds) {
  const rounds = Math.max(1, Number(shellRounds) || 1);
  if (rounds <= 3) return 220;
  if (rounds <= 8) return 320;
  if (rounds <= 16) return 460;
  return 720;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, Math.max(0, Number(ms) || 0)));
}

async function captureBrowserWindowMetrics(url, extraHeaders, windowMs, options = {}) {
  const chromium = await loadPlaywrightChromium();
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    extraHTTPHeaders: sanitizeBrowserHeaders(extraHeaders),
  });
  await context.addInitScript(installBrowserReadinessProbe);
  const page = await context.newPage();
  if (options.browserWarmup === true) {
    await page.goto(url, {
      waitUntil: "domcontentloaded",
      timeout: Math.max(windowMs + 15000, 30000),
    });
    await page.waitForTimeout(400);
    await page.evaluate(() => {
      window.__MEI_BROWSER_READINESS_RESET__?.();
      if (typeof performance?.clearResourceTimings === "function") {
        performance.clearResourceTimings();
      }
    });
  }
  let navigationStartedAt = Date.now();
  const requestStates = new Map();
  const seenMetricPayloads = new Set();
  const seenQueryPayloads = new Set();
  const inflightBySignature = new Map();
  const finished = {
    metric: [],
    query: [],
  };
  const maxInflight = {
    metric: 0,
    query: 0,
  };
  const firstRequestStart = {
    metric: NaN,
    query: NaN,
  };
  const firstRequestReady = {
    metric: NaN,
    query: NaN,
  };
  let criticalMetricsReadyMs = NaN;
  let resourceSummary = {
    metric: [],
    query: [],
  };
  let navigationSummary = {
    domcontentloaded_ms: NaN,
    load_ms: NaN,
  };
  let readinessSummary = {};

  const onRequest = (request) => {
    const kind = requestKindForUrl(request.url());
    if (!kind) {
      return;
    }
    const signature = requestSignature(request);
    const startedAt = Date.now();
    requestStates.set(request, {
      kind,
      signature,
      startedAt,
    });
    if (kind === "metric") {
      seenMetricPayloads.add(signature);
      if (!Number.isFinite(firstRequestStart.metric)) {
        firstRequestStart.metric = startedAt - navigationStartedAt;
      }
    } else {
      seenQueryPayloads.add(signature);
      if (!Number.isFinite(firstRequestStart.query)) {
        firstRequestStart.query = startedAt - navigationStartedAt;
      }
    }
    const inflight = (inflightBySignature.get(signature) || 0) + 1;
    inflightBySignature.set(signature, inflight);
    maxInflight[kind] = Math.max(maxInflight[kind], inflight);
  };

  const onFinished = (request, failed = false) => {
    const state = requestStates.get(request);
    if (!state) {
      return;
    }
    requestStates.delete(request);
    const inflight = Math.max((inflightBySignature.get(state.signature) || 1) - 1, 0);
    if (inflight === 0) {
      inflightBySignature.delete(state.signature);
    } else {
      inflightBySignature.set(state.signature, inflight);
    }
    finished[state.kind].push({
      duration: Date.now() - state.startedAt,
      failed,
    });
    if (!failed) {
      const readyAt = Date.now() - navigationStartedAt;
      if (!Number.isFinite(firstRequestReady[state.kind])) {
        firstRequestReady[state.kind] = readyAt;
      }
      if (state.kind === "metric" && !Number.isFinite(criticalMetricsReadyMs)) {
        criticalMetricsReadyMs = readyAt;
      }
    }
  };

  page.on("request", onRequest);
  page.on("requestfinished", (request) => onFinished(request, false));
  page.on("requestfailed", (request) => onFinished(request, true));

  try {
    navigationStartedAt = Date.now();
    await page.goto(url, {
      waitUntil: "domcontentloaded",
      timeout: Math.max(windowMs + 15000, 30000),
    });
    await page.waitForTimeout(windowMs);
    resourceSummary = await page.evaluate(() => {
      const grouped = {
        metric: [],
        query: [],
      };
      for (const entry of performance.getEntriesByType("resource")) {
        const name = String(entry?.name || "");
        if (name.includes("/api/datasets/metrics/")) {
          grouped.metric.push(Number(entry.duration) || 0);
          continue;
        }
        if (name.includes("/api/datasets/query/")) {
          grouped.query.push(Number(entry.duration) || 0);
        }
      }
      return grouped;
    });
    navigationSummary = await page.evaluate(() => {
      const entry = performance.getEntriesByType("navigation")[0];
      return {
        domcontentloaded_ms: Number(entry?.domContentLoadedEventEnd) || NaN,
        load_ms: Number(entry?.loadEventEnd) || NaN,
      };
    });
    readinessSummary = await page.evaluate(() => {
      const snapshot = window.__MEI_BROWSER_READINESS__ || {};
      return {
        first_trace_entry_ms: Number(snapshot.first_trace_entry_ms),
        first_stable_render_ms: Number(snapshot.first_stable_render_ms),
        first_interactive_ms: Number(snapshot.first_interactive_ms),
        first_busy_clear_ms: Number(snapshot.first_busy_clear_ms),
        stable_render_within_window: Number(snapshot.stable_render_within_window),
        interactive_within_window: Number(snapshot.interactive_within_window),
        render_trace_entry_count: Number(snapshot.render_trace_entry_count),
        render_trace_component_count: Number(snapshot.render_trace_component_count),
        stable_render_event_count: Number(snapshot.stable_render_event_count),
        busy_sample_count: Number(snapshot.busy_sample_count),
        longtask_count: Number(snapshot.longtask_count),
        longtask_total_ms: Number(snapshot.longtask_total_ms),
        longtask_max_ms: Number(snapshot.longtask_max_ms),
      };
    });
  } finally {
    const cutoff = Date.now();
    for (const [request, state] of requestStates.entries()) {
      onFinished(request, true);
      const list = finished[state.kind];
      if (list.length > 0) {
        list[list.length - 1].duration = cutoff - state.startedAt;
      }
    }
    await context.close();
    await browser.close();
  }

  return {
    browser_window_ms: windowMs,
    browser_domcontentloaded_ms: toFinite(navigationSummary.domcontentloaded_ms),
    browser_load_event_ms: toFinite(navigationSummary.load_ms),
    first_metric_request_start_ms: toFinite(firstRequestStart.metric),
    first_query_request_start_ms: toFinite(firstRequestStart.query),
    first_metric_ready_ms: toFinite(firstRequestReady.metric),
    first_query_ready_ms: toFinite(firstRequestReady.query),
    critical_metrics_ready_ms: toFinite(
      Number.isFinite(criticalMetricsReadyMs)
        ? criticalMetricsReadyMs
        : firstRequestReady.query
    ),
    metrics_request_count: Math.max(finished.metric.length, resourceSummary.metric.length),
    query_request_count: Math.max(finished.query.length, resourceSummary.query.length),
    unique_metrics_payload_count: Math.max(seenMetricPayloads.size, resourceSummary.metric.length),
    unique_query_payload_count: Math.max(seenQueryPayloads.size, resourceSummary.query.length),
    metric_request_max_ms: Math.max(
      maxDuration(finished.metric),
      maxNumber(resourceSummary.metric)
    ),
    query_request_max_ms: Math.max(maxDuration(finished.query), maxNumber(resourceSummary.query)),
    metric_request_total_ms: Math.max(
      totalDuration(finished.metric),
      totalNumber(resourceSummary.metric)
    ),
    query_request_total_ms: Math.max(
      totalDuration(finished.query),
      totalNumber(resourceSummary.query)
    ),
    metric_duplicate_signature_max_inflight: maxInflight.metric,
    query_duplicate_signature_max_inflight: maxInflight.query,
    metric_failed_request_count: finished.metric.filter((entry) => entry.failed).length,
    query_failed_request_count: finished.query.filter((entry) => entry.failed).length,
    first_trace_entry_ms: toFinite(readinessSummary.first_trace_entry_ms),
    first_stable_render_ms: toFinite(readinessSummary.first_stable_render_ms),
    first_interactive_ms: toFinite(readinessSummary.first_interactive_ms),
    first_busy_clear_ms: toFinite(readinessSummary.first_busy_clear_ms),
    stable_render_within_window: toFinite(readinessSummary.stable_render_within_window),
    interactive_within_window: toFinite(readinessSummary.interactive_within_window),
    render_trace_entry_count: toFinite(readinessSummary.render_trace_entry_count),
    render_trace_component_count: toFinite(readinessSummary.render_trace_component_count),
    stable_render_event_count: toFinite(readinessSummary.stable_render_event_count),
    busy_sample_count: toFinite(readinessSummary.busy_sample_count),
    longtask_count: toFinite(readinessSummary.longtask_count),
    longtask_total_ms: toFinite(readinessSummary.longtask_total_ms),
    longtask_max_ms: toFinite(readinessSummary.longtask_max_ms),
  };
}

function installBrowserReadinessProbe() {
  const TRACE_STORE_KEY = "__MEI_RENDER_TRACE__";
  const READINESS_KEY = "__MEI_BROWSER_READINESS__";
  const STABLE_RENDER_PHASES = new Set(["render_done", "layer_ready", "sync_layers_done"]);
  const TRACE_COMPONENT_SELECTOR = [
    "mei-cockpit-donut-trio",
    "mei-chart-bar",
    "mei-chart-line",
    "mei-chart-area",
    "mei-chart-pie",
    "mei-chart-ranking",
    "mei-chart-geo",
    "mei-chart-scatter",
    "mei-map",
    "mei-maplibre-map",
  ].join(", ");
  const state = {
    origin_ms: typeof performance?.now === "function" ? performance.now() : Date.now(),
    first_trace_entry_ms: NaN,
    first_stable_render_ms: NaN,
    first_interactive_ms: NaN,
    first_busy_clear_ms: NaN,
    stable_render_within_window: 0,
    interactive_within_window: 0,
    render_trace_entry_count: 0,
    render_trace_component_count: 0,
    stable_render_event_count: 0,
    busy_sample_count: 0,
    longtask_count: 0,
    longtask_total_ms: 0,
    longtask_max_ms: 0,
  };
  window[READINESS_KEY] = state;

  function resetState() {
    state.origin_ms = nowMs();
    state.first_trace_entry_ms = NaN;
    state.first_stable_render_ms = NaN;
    state.first_interactive_ms = NaN;
    state.first_busy_clear_ms = NaN;
    state.stable_render_within_window = 0;
    state.interactive_within_window = 0;
    state.render_trace_entry_count = 0;
    state.render_trace_component_count = 0;
    state.stable_render_event_count = 0;
    state.busy_sample_count = 0;
    state.longtask_count = 0;
    state.longtask_total_ms = 0;
    state.longtask_max_ms = 0;
    quietFrames = 0;
  }
  window.__MEI_BROWSER_READINESS_RESET__ = resetState;

  let quietFrames = 0;
  let animationFrameId = 0;
  let longTaskObserver = null;

  function nowMs() {
    return typeof performance?.now === "function" ? performance.now() : Date.now();
  }

  function elapsedMs() {
    return nowMs() - state.origin_ms;
  }

  function mainElement() {
    return (
      document.querySelector("#workspace-root main.main") ||
      document.querySelector("main.main") ||
      document.querySelector("main")
    );
  }

  function isVisible(element) {
    if (!element || !element.isConnected) {
      return false;
    }
    const style = window.getComputedStyle(element);
    if (
      style.display === "none" ||
      style.visibility === "hidden" ||
      Number(style.opacity || 1) === 0
    ) {
      return false;
    }
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }

  function isBusy() {
    const isManageRoute = String(window.location?.pathname || "").includes("/apps/build/");
    const main = mainElement();
    const manageOverlay = document.querySelector('[data-mei-manage-nav-loading="true"]');
    const globalOverlay = document.getElementById("mei-spa-loading");
    const globalOverlayVisible = !!(
      globalOverlay && globalOverlay.classList?.contains("is-visible")
    );
    // Access route can keep aria-busy for long periods; rely on explicit overlays/inflight there.
    const mainBusy = isManageRoute && main?.getAttribute("aria-busy") === "true";
    const inFlight = Number(window.__meiLangBoot?._spaInFlight || 0) > 0;
    return Boolean(manageOverlay) || globalOverlayVisible || mainBusy || inFlight;
  }

  function hasLikelyInteractiveUi() {
    const scope = mainElement() || document.body;
    if (!scope) {
      return false;
    }
    const candidates = scope.querySelectorAll(
      'a[href], button, input, select, textarea, [role="button"], [tabindex]:not([tabindex="-1"]), sl-button'
    );
    for (const node of candidates) {
      if (node.disabled || node.getAttribute?.("aria-disabled") === "true") {
        continue;
      }
      if (isVisible(node)) {
        return true;
      }
    }
    return false;
  }

  function ingestRenderTrace() {
    const entries = Array.isArray(window[TRACE_STORE_KEY]) ? window[TRACE_STORE_KEY] : [];
    state.render_trace_entry_count = entries.length;
    if (entries.length === 0) {
      return;
    }
    const components = new Set();
    let firstTrace = Number.POSITIVE_INFINITY;
    let firstStable = Number.POSITIVE_INFINITY;
    let stableCount = 0;
    for (const entry of entries) {
      if (!entry || typeof entry !== "object") {
        continue;
      }
      const component = String(entry.component || "").trim();
      if (component) {
        components.add(component);
      }
      const sincePage = Number(entry.since_page_ms);
      if (!Number.isFinite(sincePage)) {
        continue;
      }
      firstTrace = Math.min(firstTrace, sincePage);
      if (STABLE_RENDER_PHASES.has(String(entry.phase || "").trim())) {
        stableCount += 1;
        firstStable = Math.min(firstStable, sincePage);
      }
    }
    state.render_trace_component_count = components.size;
    state.stable_render_event_count = stableCount;
    if (!Number.isFinite(state.first_trace_entry_ms) && Number.isFinite(firstTrace)) {
      state.first_trace_entry_ms = firstTrace;
    }
    if (!Number.isFinite(state.first_stable_render_ms) && Number.isFinite(firstStable)) {
      state.first_stable_render_ms = firstStable;
      state.stable_render_within_window = 1;
    }
  }

  function expectsTraceDrivenRender() {
    if (state.render_trace_entry_count > 0 || state.render_trace_component_count > 0) {
      return true;
    }
    return Boolean(document.querySelector(TRACE_COMPONENT_SELECTOR));
  }

  function tick() {
    ingestRenderTrace();
    const traceDriven = expectsTraceDrivenRender();
    const busy = isBusy();
    state.busy_sample_count += 1;
    if (!busy) {
      if (!Number.isFinite(state.first_busy_clear_ms)) {
        state.first_busy_clear_ms = elapsedMs();
      }
      quietFrames += 1;
    } else {
      quietFrames = 0;
    }
    const interactiveEligible = traceDriven
      ? Number.isFinite(state.first_stable_render_ms)
      : hasLikelyInteractiveUi();
    if (
      !Number.isFinite(state.first_stable_render_ms) &&
      !traceDriven &&
      !busy &&
      quietFrames >= 2 &&
      hasLikelyInteractiveUi()
    ) {
      state.first_stable_render_ms = elapsedMs();
      state.stable_render_within_window = 1;
    }
    if (
      !Number.isFinite(state.first_interactive_ms) &&
      !busy &&
      quietFrames >= 2 &&
      interactiveEligible
    ) {
      state.first_interactive_ms = elapsedMs();
      state.interactive_within_window = 1;
    }
    animationFrameId = window.requestAnimationFrame(tick);
  }

  try {
    if (typeof PerformanceObserver === "function") {
      longTaskObserver = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          const duration = Number(entry?.duration) || 0;
          state.longtask_count += 1;
          state.longtask_total_ms += duration;
          state.longtask_max_ms = Math.max(state.longtask_max_ms, duration);
        }
      });
      longTaskObserver.observe({ type: "longtask", buffered: true });
    }
  } catch {}

  window.addEventListener(
    "pagehide",
    () => {
      if (animationFrameId) {
        window.cancelAnimationFrame(animationFrameId);
      }
      try {
        longTaskObserver?.disconnect?.();
      } catch {}
    },
    { once: true }
  );
  animationFrameId = window.requestAnimationFrame(tick);
}

async function loadPlaywrightChromium() {
  if (!playwrightChromiumPromise) {
    playwrightChromiumPromise = import("playwright").then((mod) => mod.chromium);
  }
  return playwrightChromiumPromise;
}

function sanitizeBrowserHeaders(headers) {
  const sanitized = {};
  for (const [key, value] of Object.entries(headers || {})) {
    if (!value) {
      continue;
    }
    sanitized[key] = value;
  }
  return sanitized;
}

function requestKindForUrl(url) {
  const raw = String(url || "");
  if (raw.includes("/api/datasets/metrics/")) {
    return "metric";
  }
  if (raw.includes("/api/datasets/query/")) {
    return "query";
  }
  return "";
}

function requestSignature(request) {
  const kind = requestKindForUrl(request.url()) || request.method().toLowerCase();
  let raw = "";
  try {
    raw = request.postData() || "";
  } catch {
    raw = request.url();
  }
  const digest = createHash("sha1").update(raw).digest("hex").slice(0, 12);
  return `${kind}:${digest}`;
}

function maxDuration(entries) {
  return entries.reduce((max, entry) => Math.max(max, toFinite(entry.duration)), 0);
}

function totalDuration(entries) {
  return entries.reduce((sum, entry) => sum + Math.max(toFinite(entry.duration), 0), 0);
}

function maxNumber(values) {
  return values.reduce((max, value) => Math.max(max, toFinite(value)), 0);
}

function totalNumber(values) {
  return values.reduce((sum, value) => sum + Math.max(toFinite(value), 0), 0);
}

function bestFinite(...values) {
  for (const value of values) {
    const parsed = toFinite(value);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return NaN;
}

function applyPagePerf(target, headers) {
  setNumeric(target, "handler_html_ready_ms", headers.get("x-mei-handler-html-ready-ms"));
  setNumeric(target, "ssr_http_response_body_ms", headers.get("x-mei-ssr-http-response-body-ms"));
  setNumeric(target, "compile_ms", headers.get("x-mei-compile-ms"));
  setNumeric(target, "compile_cache_hit", headers.get("x-mei-compile-cache-hit"));
  setNumeric(target, "artifact_cache_hit", headers.get("x-mei-artifact-cache-hit"));
  setNumeric(target, "artifact_load_ms", headers.get("x-mei-artifact-load-ms"));
  setNumeric(target, "compile_cache_lookup_ms", headers.get("x-mei-compile-cache-lookup-ms"));
  setNumeric(target, "page_render_cache_hit", headers.get("x-mei-page-render-cache-hit"));
  setNumeric(target, "scene_bundle_probe_ms", headers.get("x-mei-scene-bundle-probe-ms"));
  setNumeric(
    target,
    "scene_bundle_build_scheduled",
    headers.get("x-mei-scene-bundle-build-scheduled")
  );
  applyKeyValueHeaderPerf(target, headers.get("x-mei-compile-stage-timing"));
  applyKeyValueHeaderPerf(target, headers.get("x-mei-compile-cache-stats"));
  applyKeyValueHeaderPerf(target, headers.get("x-mei-dependency-graph-stats"));
  applyKeyValueHeaderPerf(target, headers.get("x-mei-catalog-compile-stats"));
  const sceneBundleStatus = String(headers.get("x-mei-scene-bundle-status") || "").trim();
  if (sceneBundleStatus) {
    target.scene_bundle_status = sceneBundleStatus;
    target.scene_bundle_ready = Number(sceneBundleStatus === "ready");
    target.scene_bundle_scheduled = Number(sceneBundleStatus === "scheduled");
    target.scene_bundle_disabled = Number(sceneBundleStatus === "disabled");
    target.scene_bundle_fallback = Number(sceneBundleStatus === "fallback");
    target.scene_bundle_empty = Number(sceneBundleStatus === "empty");
  }
  const revisionScope = headers.get("x-mei-compile-revision-scope");
  if (revisionScope) {
    target.compile_revision_scope = revisionScope;
  }
  const cacheValidation = headers.get("x-mei-compile-cache-validation");
  if (cacheValidation) {
    target.compile_cache_validation = cacheValidation;
  }
  const feedbackPath = String(headers.get("x-mei-compile-feedback-path") || "").trim();
  if (feedbackPath) {
    target.compile_feedback_path = feedbackPath;
  }
  const feedbackReason = String(headers.get("x-mei-compile-feedback-reason") || "").trim();
  if (feedbackReason) {
    target.compile_feedback_reason = feedbackReason;
  }
  const feedbackScopeKind = String(headers.get("x-mei-compile-feedback-scope-kind") || "").trim();
  if (feedbackScopeKind) {
    target.compile_feedback_scope_kind = feedbackScopeKind;
    target.compile_feedback_scoped = Number(feedbackScopeKind !== "full_app");
    target.compile_feedback_full_app = Number(feedbackScopeKind === "full_app");
    target.compile_feedback_scene_target = Number(feedbackScopeKind === "scene_target");
  }
  setNumeric(
    target,
    "compile_feedback_diagnostic_errors",
    headers.get("x-mei-compile-feedback-diagnostic-errors")
  );
}

function applyKeyValueHeaderPerf(target, raw) {
  for (const [key, value] of Object.entries(parseKeyValueStats(raw))) {
    setNumeric(target, key, value);
  }
}

function parseKeyValueStats(raw) {
  const out = {};
  const text = String(raw || "").trim();
  if (!text) {
    return out;
  }
  for (const part of text.split(",")) {
    const [keyRaw, valueRaw] = part.split("=");
    const key = String(keyRaw || "").trim();
    const value = toFinite(valueRaw);
    if (!key || !Number.isFinite(value)) {
      continue;
    }
    out[key] = value;
  }
  return out;
}

function applyManagePipelinePerf(target, pipeline) {
  setNumeric(target, "manage_resources", pipeline?.artifact_stats?.resources);
  setNumeric(target, "manage_dataset_resources", pipeline?.artifact_stats?.dataset_resources);
  setNumeric(
    target,
    "handler_html_ready_ms",
    pipeline?.request_timing?.handler_html_ready_ms
  );
  setNumeric(
    target,
    "ssr_http_response_body_ms",
    pipeline?.request_timing?.ssr_http_response_body_ms
  );
  const compileStage = (pipeline?.stages || []).find((entry) => entry?.id === "compile_app");
  if (!("compile_ms" in target) && Number.isFinite(Number(compileStage?.ms))) {
    target.compile_ms = Number(compileStage.ms);
  }
}

function extractManagePipeline(html) {
  const match = html.match(/data-manage-pipeline-json=(?:"([^"]*)"|'([^']*)')/);
  const raw = match?.[1] || match?.[2];
  if (!raw) {
    return null;
  }
  try {
    return JSON.parse(decodeHtmlAttr(raw));
  } catch {
    return null;
  }
}

function decodeHtmlAttr(raw) {
  return raw
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"')
    .replace(/&#34;/g, '"')
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&#39;/g, "'");
}

async function discoverDatasetId(baseUrl, appId, scenario, extraHeaders = {}) {
  const appPath = encodeURI(appId);
  const query = new URLSearchParams({
    scene_id: scenario.scene_id,
  });
  if (scenario.target_file) {
    query.set("target_file", scenario.target_file);
  }
  const response = await fetch(`${baseUrl}/api/world/context/${appPath}?${query.toString()}`, {
    method: "GET",
    headers: extraHeaders,
  });
  const text = await response.text();
  if (!response.ok) {
    return "";
  }
  let payload = null;
  try {
    payload = text ? JSON.parse(text) : null;
  } catch {
    return "";
  }
  const items = Array.isArray(payload?.resource_inventory?.items)
    ? payload.resource_inventory.items
    : [];
  const preferred = items.find((item) => {
    const type = String(item?.resource_type || "").trim().toLowerCase();
    const summary = String(item?.summary || "").trim().toLowerCase();
    return (
      type === "loaded_resource" &&
      summary.includes("kind=dataset") &&
      typeof item?.id === "string" &&
      item.id
    );
  });
  if (preferred?.id) {
    return String(preferred.id);
  }
  return "";
}

async function clearScenarioCaches(baseUrl, appId, scenario, datasetId, extraHeaders = {}) {
  const response = await postJson(
    baseUrl,
    `/api/datasets/recompute/${encodeURI(appId)}`,
    {
      scene_id: scenario.scene_id || undefined,
      target: scenario.target_file || undefined,
      dataset_id: datasetId,
      metric_id: scenario.metric_ids[0] || undefined,
      mode: scenario.clear_mode,
    },
    extraHeaders
  );
  const perf = response?.perf || {};
  return {
    total_ms: toFinite(perf.total_ms),
    clear_ms: toFinite(perf.clear_ms),
    compile_cache_cleared: toFinite(response?.compile_cache_cleared),
    file_cache_cleared: toFinite(response?.file_cache_cleared),
    metric_response_cache_cleared: toFinite(perf.metric_response_cache_cleared),
    metric_dataframe_cache_cleared: toFinite(perf.metric_dataframe_cache_cleared),
  };
}

async function collectMetricPerf(
  baseUrl,
  appId,
  sceneId,
  datasetId,
  metricIds = [],
  extraHeaders = {}
) {
  const started = Date.now();
  const payload = await postJson(
    baseUrl,
    `/api/datasets/metrics/${encodeURI(appId)}`,
    {
      scene_id: sceneId,
      dataset_id: datasetId,
      metric_ids: metricIds,
    },
    extraHeaders
  );
  const perf = payload?.perf || {};
  return buildMetricPerfObject(payload, perf, Date.now() - started);
}

async function collectDatasetPerf(
  baseUrl,
  appId,
  sceneId,
  datasetId,
  metricId,
  extraHeaders = {}
) {
  const started = Date.now();
  const payload = await postJson(
    baseUrl,
    `/api/datasets/query/${encodeURI(appId)}`,
    {
      scene_id: sceneId,
      dataset_id: datasetId,
      metric_id: metricId || undefined,
      page: 1,
      page_size: 1,
    },
    extraHeaders
  );
  const perf = payload?.perf || {};
  return {
    dataset_elapsed_ms: Date.now() - started,
    dataset_total_rows: toFinite(payload?.total),
    dataset_compile_ms: toFinite(perf.compile_ms),
    dataset_compile_cache_hit: toFinite(perf.compile_cache_hit),
    dataset_artifact_cache_hit: toFinite(perf.artifact_cache_hit),
    dataset_artifact_load_ms: toFinite(perf.artifact_load_ms),
    dataset_query_api_ms: toFinite(perf.query_api_ms),
    dataset_total_ms: toFinite(perf.total_ms),
    dataset_compile_cache_lookup_ms: toFinite(perf.compile_cache_lookup_ms),
    dataset_compile_cache_lock_wait_ms: toFinite(perf.compile_cache_lock_wait_ms),
    dependency_graph_build_ms: toFinite(perf.dependency_graph_build_ms),
    official_results_all_routes_ms: toFinite(perf.official_results_all_routes_ms),
    active_payload_pick_or_compile_ms: toFinite(perf.active_payload_pick_or_compile_ms),
    catalog_compile_ms: toFinite(perf.catalog_compile_ms),
    resource_merge_ms: toFinite(perf.resource_merge_ms),
    world_metric_ledger_ms: toFinite(perf.world_metric_ledger_ms),
    scene_projection_assembly_ms: toFinite(perf.scene_projection_assembly_ms),
    source_tree_ms: toFinite(perf.source_tree_ms),
    world_finalize_ms: toFinite(perf.world_finalize_ms),
    dataset_file_cache_hit: toFinite(perf.file_cache_hit),
    dataset_file_cache_load_ms: toFinite(perf.file_cache_load_ms),
    dataset_import_load_ms: toFinite(perf.dataset_import_load_ms),
    dataset_import_artifact_hit: toFinite(perf.dataset_import_artifact_hit),
    dataset_rows_cache_hit: toFinite(perf.dataset_rows_cache_hit),
    dataset_rows_cache_rows: toFinite(perf.dataset_rows_cache_rows),
    dataset_materialized_cache_hit: toFinite(perf.materialized_cache_hit),
    dataset_default_board_bundle_hit: toFinite(perf.default_board_bundle_hit),
    dataset_metric_eval_ms: toFinite(perf.metric_eval_ms),
    dataset_hydrate_datasets_ms: toFinite(perf.hydrate_datasets_ms),
    dataset_base_query_ms: toFinite(perf.base_query_ms),
  };
}

async function collectHostReadinessSnapshot(baseUrl, extraHeaders = {}) {
  const response = await fetch(`${baseUrl}/api/host/heartbeat`, {
    method: "GET",
    headers: extraHeaders,
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`/api/host/heartbeat failed: ${response.status}\n${text}`);
  }
  const payload = text ? JSON.parse(text) : {};
  return {
    host_access_ready: Number(payload?.accessReady === true),
    host_full_warmup_ready: Number(payload?.fullWarmupReady === true),
    host_deferred_warmup_pending: Number(payload?.deferredWarmupPending === true),
    host_last_build_total_ms: toFinite(payload?.lastBuildTotalMs),
    host_last_build_compile_ms: toFinite(payload?.lastBuildCompileMs),
    host_last_build_warmup_ms: toFinite(payload?.lastBuildWarmupMs),
    host_last_critical_warmup_ms: toFinite(payload?.lastCriticalWarmupMs),
    host_last_deferred_warmup_ms: toFinite(payload?.lastDeferredWarmupMs),
    host_last_critical_warmup_request_count: toFinite(payload?.lastCriticalWarmupRequestCount),
    host_last_deferred_warmup_request_count: toFinite(payload?.lastDeferredWarmupRequestCount),
  };
}

function buildMetricPerfObject(payload, perf, elapsedMs) {
  const metricPerf = {
    metric_elapsed_ms: elapsedMs,
    metric_result_count: Array.isArray(payload?.metrics) ? payload.metrics.length : 0,
    metric_total_rows: toFinite(payload?.total_rows),
    metric_compile_ms: toFinite(perf.compile_ms),
    metric_compile_cache_hit: toFinite(perf.compile_cache_hit),
    metric_artifact_cache_hit: toFinite(perf.artifact_cache_hit),
    metric_artifact_load_ms: toFinite(perf.artifact_load_ms),
    metric_query_api_ms: toFinite(perf.query_api_ms),
    metric_hydrate_datasets_ms: toFinite(perf.hydrate_datasets_ms),
    metric_eval_ms: toFinite(perf.metric_eval_ms),
    metric_total_ms: toFinite(perf.total_ms),
    metric_response_cache_hit: toFinite(perf.response_cache_hit),
    metric_response_cache_lookup_ms: toFinite(perf.response_cache_lookup_ms),
    metric_compile_cache_lookup_ms: toFinite(perf.compile_cache_lookup_ms),
    metric_compile_cache_lock_wait_ms: toFinite(perf.compile_cache_lock_wait_ms),
    dependency_graph_build_ms: toFinite(perf.dependency_graph_build_ms),
    official_results_all_routes_ms: toFinite(perf.official_results_all_routes_ms),
    active_payload_pick_or_compile_ms: toFinite(perf.active_payload_pick_or_compile_ms),
    catalog_compile_ms: toFinite(perf.catalog_compile_ms),
    resource_merge_ms: toFinite(perf.resource_merge_ms),
    world_metric_ledger_ms: toFinite(perf.world_metric_ledger_ms),
    scene_projection_assembly_ms: toFinite(perf.scene_projection_assembly_ms),
    source_tree_ms: toFinite(perf.source_tree_ms),
    world_finalize_ms: toFinite(perf.world_finalize_ms),
    file_cache_hit: toFinite(perf.file_cache_hit),
    file_cache_load_ms: toFinite(perf.file_cache_load_ms),
    dataset_import_load_ms: toFinite(perf.dataset_import_load_ms),
    dataset_import_artifact_hit: toFinite(perf.dataset_import_artifact_hit),
    file_cache_paginate_ms: toFinite(perf.file_cache_paginate_ms),
    eval_plan_nodes: toFinite(perf.eval_plan_nodes),
    eval_plan_rowset_nodes: toFinite(perf.eval_plan_rowset_nodes),
    eval_plan_metric_nodes: toFinite(perf.eval_plan_metric_nodes),
    eval_plan_edges: toFinite(perf.eval_plan_edges),
    request_dag_nodes: toFinite(perf.request_dag_nodes),
    request_dag_hits: toFinite(perf.request_dag_hits),
    request_dag_misses: toFinite(perf.request_dag_misses),
    response_cache_hit: toFinite(perf.response_cache_hit),
    response_cache_lookup_ms: toFinite(perf.response_cache_lookup_ms),
    query_api_ms: toFinite(perf.query_api_ms),
    hydrate_datasets_ms: toFinite(perf.hydrate_datasets_ms),
    base_rowset_materialize_ms: toFinite(perf.base_rowset_materialize_ms),
    default_board_bundle_hit: toFinite(perf.default_board_bundle_hit),
    eval_artifact_load_ms: toFinite(perf.eval_artifact_load_ms),
    metric_eval_total_ms: toFinite(perf.metric_eval_ms),
    total_ms: toFinite(perf.total_ms),
  };
  if (!Number.isFinite(metricPerf.compile_ms) && Number.isFinite(metricPerf.metric_compile_ms)) {
    metricPerf.compile_ms = metricPerf.metric_compile_ms;
  }
  if (
    !Number.isFinite(metricPerf.compile_cache_hit) &&
    Number.isFinite(metricPerf.metric_compile_cache_hit)
  ) {
    metricPerf.compile_cache_hit = metricPerf.metric_compile_cache_hit;
  }
  if (
    !Number.isFinite(metricPerf.compile_cache_lookup_ms) &&
    Number.isFinite(metricPerf.metric_compile_cache_lookup_ms)
  ) {
    metricPerf.compile_cache_lookup_ms = metricPerf.metric_compile_cache_lookup_ms;
  }
  if (
    !Number.isFinite(metricPerf.compile_cache_lock_wait_ms) &&
    Number.isFinite(metricPerf.metric_compile_cache_lock_wait_ms)
  ) {
    metricPerf.compile_cache_lock_wait_ms = metricPerf.metric_compile_cache_lock_wait_ms;
  }
  if (!Number.isFinite(metricPerf.metric_eval_ms) && Number.isFinite(metricPerf.metric_eval_total_ms)) {
    metricPerf.metric_eval_ms = metricPerf.metric_eval_total_ms;
  }
  return metricPerf;
}

async function postJson(baseUrl, pathName, body, extraHeaders = {}) {
  const response = await fetch(`${baseUrl}${pathName}`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      accept: "application/json",
      ...extraHeaders,
    },
    body: JSON.stringify(body),
  });
  const text = await response.text();
  let json = null;
  if (text) {
    try {
      json = JSON.parse(text);
    } catch (error) {
      throw new Error(`invalid JSON from ${pathName}: ${error}`);
    }
  }
  if (!response.ok) {
    throw new Error(`${pathName} failed: ${response.status}\n${text}`);
  }
  return json;
}

function buildRequestHeaders({ authBearer, cookieHeader }) {
  const headers = {};
  if (authBearer) {
    headers.authorization = `Bearer ${authBearer}`;
  }
  if (cookieHeader) {
    headers.cookie = cookieHeader;
  }
  return headers;
}

function isAuthPage(finalUrl, body) {
  if (
    typeof finalUrl === "string" &&
    (finalUrl.includes("/auth/login") || finalUrl.includes("/login"))
  ) {
    return true;
  }
  const lowered = String(body || "").toLowerCase();
  return lowered.includes("authentication required") || lowered.includes("<title>登录");
}

function sanitizeNote(error) {
  const message = error instanceof Error ? error.message : String(error);
  return message.replace(/\s+/g, "_").slice(0, 180);
}

function mergePerf(target, patch) {
  for (const [key, value] of Object.entries(patch || {})) {
    if (value === undefined || value === null) {
      continue;
    }
    if (typeof value === "number" && !Number.isFinite(value)) {
      continue;
    }
    target[key] = value;
  }
}

function setNumeric(target, key, value) {
  const parsed = toFinite(value);
  if (Number.isFinite(parsed)) {
    target[key] = parsed;
  }
}

function toFinite(value) {
  if (value === null || value === undefined || value === "") {
    return NaN;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : NaN;
}

async function writeJsonl(filePath, rows, shouldAppend) {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  const body = rows.map((row) => JSON.stringify(row)).join("\n");
  const text = body ? `${body}\n` : "";
  if (shouldAppend) {
    await fs.appendFile(filePath, text, "utf8");
  } else {
    await fs.writeFile(filePath, text, "utf8");
  }
}

function printSummary({ outputPath, append: appendMode, records }) {
  console.log("# host perf sample");
  console.log(`server_url=${serverUrl}`);
  console.log(`scenario_file=${scenarioFile}`);
  console.log(`output=${outputPath}`);
  console.log(`append=${appendMode ? "1" : "0"}`);
  console.log(`records=${records.length}`);
  if (scenarioFamilyFilter) {
    console.log(`scenario_family=${scenarioFamilyFilter}`);
  }
  for (const row of records) {
    const perf = row.perf || {};
    const preview = {
      access_first_visible_ms: perf.access_first_visible_ms,
      access_first_interactive_ms: perf.access_first_interactive_ms,
      access_critical_metrics_ready_ms: perf.access_critical_metrics_ready_ms,
      hot_start_ready_ms: perf.hot_start_ready_ms,
      local_edit_feedback_ms: perf.local_edit_feedback_ms,
      metric_probe_ready_ms: perf.metric_probe_ready_ms,
      host_full_warmup_ready: perf.host_full_warmup_ready,
      host_deferred_warmup_pending: perf.host_deferred_warmup_pending,
      handler_html_ready_ms: perf.handler_html_ready_ms,
      bootstrap_total_wall_ms: perf.bootstrap_total_wall_ms,
      bootstrap_shell_roundtrips: perf.bootstrap_shell_roundtrips,
      page_request_roundtrips: perf.page_request_roundtrips,
      first_stable_render_ms: perf.first_stable_render_ms,
      first_interactive_ms: perf.first_interactive_ms,
      stable_render_within_window: perf.stable_render_within_window,
      interactive_within_window: perf.interactive_within_window,
      compile_ms: perf.compile_ms,
      artifact_load_ms: perf.artifact_load_ms,
      total_ms: perf.total_ms,
      metric_total_ms: perf.metric_total_ms,
      metric_eval_ms: perf.metric_eval_ms,
      hydrate_datasets_ms: perf.hydrate_datasets_ms,
      dataset_import_load_ms: perf.dataset_import_load_ms,
      base_rowset_materialize_ms: perf.base_rowset_materialize_ms,
      default_board_bundle_hit: perf.default_board_bundle_hit,
      response_cache_hit: perf.response_cache_hit,
      metrics_request_count: perf.metrics_request_count,
      metric_request_max_ms: perf.metric_request_max_ms,
      query_request_count: perf.query_request_count,
    };
    console.log(
      `- ${row.scenario_id} ${row.run_kind} ${JSON.stringify(preview)}`
    );
  }
}
