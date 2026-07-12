/**
 * Client render pipeline timeline: request → cache → assembly → surface-ready.
 *
 * Always records the latest run to window.__meiRenderPipeline.last
 * Verbose console + server log: ?mei_render_pipeline=1 or localStorage mei:render-pipeline=1
 *   (also follows mei_cache_diag=1)
 */
(function initRenderPipelineTimeline(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const PIPELINE_LS = "mei:render-pipeline";
  const PIPELINE_REPORT_API = "/api/host/client-trace";
  const HOST_API_RE =
    /\/api\/host\/(view-revision|layer-batch|scene-manifest|scene-bootstrap|scene-drilldown-context)/;
  const BUNDLE_RE = /\/(access|manage)\.bundle\.js(\?|$)/;

  const state = {
    runId: "",
    url: "",
    surface: "",
    startedAt: 0,
    runBeganAt: 0,
    marks: [],
    fetches: [],
    reported: false,
    visibleReadyScheduled: false,
    lastSummary: null,
  };

  function nowMs() {
    if (typeof performance !== "undefined" && typeof performance.now === "function") {
      return performance.now();
    }
    return Date.now();
  }

  function pipelineEnabled() {
    try {
      if (global.localStorage?.getItem(PIPELINE_LS) === "1") return true;
      if (new URL(global.location.href).searchParams.get("mei_render_pipeline") === "1") {
        return true;
      }
      if (typeof boot.cacheDiagEnabled === "function" && boot.cacheDiagEnabled()) return true;
    } catch (_) {}
    return false;
  }

  function beginRun(meta) {
    const detail = meta && typeof meta === "object" ? meta : {};
    state.runId = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    state.url = String(detail.url || global.location?.href || "");
    state.surface = String(detail.surface || "").trim().toLowerCase();
    state.runBeganAt = nowMs();
    state.startedAt = detail.fromNavigationStart === true ? 0 : state.runBeganAt;
    state.marks = [];
    state.fetches = [];
    state.reported = false;
    state.visibleReadyScheduled = false;
    mark("run_begin", detail);
    if (detail.fromNavigationStart === true) {
      const nav = readNavigationTiming();
      if (nav) {
        markAt("document:response_start", nav.responseStart, {});
        markAt("document:response_end", nav.responseEnd, {});
        if (nav.domInteractive > 0) markAt("document:interactive", nav.domInteractive, {});
        if (nav.domContentLoadedEventEnd > 0) {
          markAt("document:dom_content_loaded", nav.domContentLoadedEventEnd, {});
        }
        if (nav.loadEventEnd > 0) markAt("document:load", nav.loadEventEnd, {});
      }
    }
  }

  function mark(name, detail) {
    const entry = {
      name: String(name || "mark"),
      ms: Math.round(nowMs() - (state.startedAt || 0)),
      at: new Date().toISOString(),
      detail: detail && typeof detail === "object" ? { ...detail } : {},
    };
    state.marks.push(entry);
    if (state.marks.length > 96) state.marks.shift();
    return entry;
  }

  function markAt(name, offsetMs, detail) {
    const entry = {
      name: String(name || "mark"),
      ms: Math.round(Number(offsetMs) || 0),
      at: new Date().toISOString(),
      detail: detail && typeof detail === "object" ? { ...detail } : {},
    };
    state.marks.push(entry);
    return entry;
  }

  function classifyFetchUrl(url) {
    const text = String(url || "");
    if (BUNDLE_RE.test(text)) return "bundle";
    if (text.includes("/api/host/view-revision")) return "view-revision";
    if (text.includes("/api/host/layer-batch")) return "layer-batch";
    if (text.includes("/api/host/scene-bootstrap")) return "scene-bootstrap";
    if (text.includes("/api/host/scene-drilldown-context")) return "scene-drilldown";
    if (text.includes("/api/host/scene-manifest")) return "scene-manifest";
    if (text.includes("/api/dataset") || text.includes("/api/metric")) return "metric-api";
    if (text.includes("/api/host/")) return "host-other";
    if (/\/view(\?|$)/.test(text) && !text.includes("/api/")) return "document";
    return "other";
  }

  function resourceIdentity(url) {
    try {
      const parsed = new URL(String(url || ""), global.location?.href || "http://localhost/");
      return `${parsed.pathname}${parsed.search}`;
    } catch (_) {
      return String(url || "");
    }
  }

  function ingestResourceTiming() {
    if (typeof performance === "undefined" || typeof performance.getEntriesByType !== "function") {
      return [];
    }
    const rows = [];
    for (const entry of performance.getEntriesByType("resource")) {
      const name = String(entry.name || "");
      if (!HOST_API_RE.test(name) && !BUNDLE_RE.test(name) && !name.includes("/styles.bundle.css")) {
        continue;
      }
      const kind = classifyFetchUrl(name);
      const identity = resourceIdentity(name);
      const duplicate = state.fetches.some(
        (row) =>
          row.kind === kind &&
          resourceIdentity(row.name) === identity &&
          Math.abs(Number(row.startMs || 0) - Number(entry.startTime || 0)) <= 8,
      );
      if (duplicate) continue;
      rows.push({
        kind,
        name,
        ms: Math.round(entry.duration || 0),
        startMs: Math.round(entry.startTime || 0),
        transferSize: Number(entry.transferSize) || 0,
        fromCache: Number(entry.transferSize) === 0 && Number(entry.decodedBodySize) > 0,
      });
    }
    state.fetches.push(...rows);
    return rows;
  }

  function readNavigationTiming() {
    if (typeof performance === "undefined" || typeof performance.getEntriesByType !== "function") {
      return null;
    }
    const nav = performance.getEntriesByType("navigation")[0];
    if (!nav) return null;
    return {
      responseStart: Math.round(Number(nav.responseStart) || 0),
      responseEnd: Math.round(Number(nav.responseEnd) || 0),
      domInteractive: Math.round(Number(nav.domInteractive) || 0),
      domContentLoadedEventEnd: Math.round(Number(nav.domContentLoadedEventEnd) || 0),
      loadEventEnd: Math.round(Number(nav.loadEventEnd) || 0),
      transferSize: Number(nav.transferSize) || 0,
      encodedBodySize: Number(nav.encodedBodySize) || 0,
      decodedBodySize: Number(nav.decodedBodySize) || 0,
    };
  }

  function readBodyPerf() {
    const body = global.document?.body;
    if (!body?.dataset) return {};
    return {
      handlerReadyMs: Number(body.dataset.meiHandlerHtmlReadyMs),
      ssrBodyMs: Number(body.dataset.meiSsrHttpResponseBodyMs),
      htmlBytes: Number(body.dataset.meiHtmlBytes),
    };
  }

  function phaseSpan(prefix) {
    const hits = state.marks.filter((row) => row.name === prefix || row.name.startsWith(`${prefix}:`));
    if (!hits.length) return null;
    const first = hits[0].ms;
    const last = hits[hits.length - 1].ms;
    return { startMs: first, endMs: last, durationMs: Math.max(0, last - first), count: hits.length };
  }

  function buildSummary(options) {
    const opts = options || {};
    ingestResourceTiming();
    const bodyPerf = readBodyPerf();
    const navigation = readNavigationTiming();
    const wallMs = Math.round(nowMs() - (state.startedAt || 0));
    const lastMark = state.marks[state.marks.length - 1];
    const surfaceReady = state.marks.find((row) => row.name === "surface_ready");
    const coldStart = phaseSpan("cold_start");
    const assembly = phaseSpan("assembly");
    const compose = phaseSpan("preview_compose");
    const phaseNames = [
      "revision_store_parse",
      "layer_restore",
      "idb_open",
      "idb_transaction",
      "compose_structure",
      "bind_eval_slots",
      "apply_chrome",
      "component_wake",
    ];
    const phases = Object.fromEntries(
      phaseNames.map((name) => [name, phaseSpan(name)]).filter(([, value]) => value),
    );
    const byKind = {};
    for (const row of state.fetches) {
      const bucket = byKind[row.kind] || {
        count: 0,
        ms: 0,
        maxMs: 0,
        bytes: 0,
        cached: 0,
      };
      bucket.count += 1;
      bucket.ms += row.ms || 0;
      bucket.maxMs = Math.max(bucket.maxMs, row.ms || 0);
      bucket.bytes += row.transferSize || 0;
      if (row.fromCache) bucket.cached += 1;
      byKind[row.kind] = bucket;
    }
    const documentMs = navigation?.responseEnd || byKind.document?.maxMs || 0;
    const clientAfterDocumentMs = Math.max(0, wallMs - documentMs);
    const summary = {
      runId: state.runId,
      url: state.url,
      surface: state.surface,
      wallMs,
      documentMs,
      clientAfterDocumentMs,
      surfaceReadyMs: surfaceReady?.ms ?? null,
      coldStartMs: coldStart?.durationMs ?? null,
      assemblyMs: assembly?.durationMs ?? null,
      previewComposeMs: compose?.durationMs ?? null,
      phases,
      bodyPerf,
      navigation,
      fetchByKind: byKind,
      marks: state.marks.slice(-32),
      fetches: state.fetches.slice(-24),
      flags: {
        restored: opts.restored,
        source: opts.source || "",
        ssrPreview: opts.ssrPreview === true,
        viewRevisionOutcome: boot.lastViewRevisionOutcome || null,
      },
      endedAt: lastMark?.name || "",
    };
    state.lastSummary = summary;
    return summary;
  }

  function formatBytes(bytes) {
    const n = Number(bytes) || 0;
    if (n >= 1_048_576) return `${(n / 1_048_576).toFixed(1)}MB`;
    if (n >= 1024) return `${Math.round(n / 1024)}KB`;
    return `${n}B`;
  }

  function logSummaryToConsole(summary) {
    const lines = [
      `[mei-render-pipeline] wall=${summary.wallMs}ms`,
      `  document≈${summary.documentMs}ms`,
      `  client_after_doc≈${summary.clientAfterDocumentMs}ms`,
    ];
    if (summary.previewComposeMs != null) {
      lines.push(`  preview_compose≈${summary.previewComposeMs}ms`);
    }
    if (summary.assemblyMs != null) lines.push(`  assembly≈${summary.assemblyMs}ms`);
    if (summary.surfaceReadyMs != null) lines.push(`  surface_ready@${summary.surfaceReadyMs}ms`);
    Object.entries(summary.fetchByKind || {}).forEach(([kind, bucket]) => {
      lines.push(
        `  fetch ${kind}: n=${bucket.count} sum=${bucket.ms}ms max=${bucket.maxMs}ms bytes=${formatBytes(bucket.bytes)} cached=${bucket.cached}`,
      );
    });
    try {
      console.info(lines.join("\n"));
      console.table(summary.marks.map((row) => ({ ms: row.ms, event: row.name, ...row.detail })));
    } catch (_) {}
  }

  function reportSummary(summary) {
    if (state.reported) return;
    state.reported = true;
    state.lastSummary = summary;
    if (typeof boot.cacheDiagTrace === "function") {
      boot.cacheDiagTrace("render-pipeline", summary);
    }
    if (!pipelineEnabled()) return;
    logSummaryToConsole(summary);
    try {
      const body = JSON.stringify({
        id: summary.runId || `pipe-${Date.now()}`,
        kind: "RENDER_PIPELINE",
        label: `${summary.surface || "app"} wall=${summary.wallMs}ms`,
        pipeline: summary,
      });
      if (typeof navigator !== "undefined" && typeof navigator.sendBeacon === "function") {
        const blob = new Blob([body], { type: "application/json" });
        navigator.sendBeacon(PIPELINE_REPORT_API, blob);
        return;
      }
      void fetch(PIPELINE_REPORT_API, {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body,
        keepalive: true,
      });
    } catch (_) {}
  }

  function finalizeRun(options) {
    const summary = buildSummary(options);
    reportSummary(summary);
    return summary;
  }

  function installFetchTap() {
    if (global.__meiRenderPipelineFetchTap || typeof global.fetch !== "function") return;
    global.__meiRenderPipelineFetchTap = true;
    const nativeFetch = global.fetch.bind(global);
    global.fetch = function meiRenderPipelineFetch(input, init) {
      const url = String(input?.url || input || "");
      const track = HOST_API_RE.test(url) || url.includes("/api/dataset") || url.includes("/api/metric");
      const started = nowMs();
      const kind = classifyFetchUrl(url);
      if (track) mark(`fetch_start:${kind}`, { url: url.slice(-96) });
      return nativeFetch(input, init).then(
        (response) => {
          if (track) {
            const ms = Math.round(nowMs() - started);
            state.fetches.push({
              kind,
              name: url,
              ms,
              startMs: Math.round(started - (state.startedAt || 0)),
              transferSize: 0,
              fromCache: false,
              status: response.status,
            });
            mark(`fetch_done:${kind}`, { ms, status: response.status, url: url.slice(-96) });
          }
          return response;
        },
        (error) => {
          if (track) {
            mark(`fetch_fail:${kind}`, {
              ms: Math.round(nowMs() - started),
              message: String(error?.message || error),
            });
          }
          throw error;
        },
      );
    };
  }

  function hookCacheDiag() {
    if (hookCacheDiag._done || typeof boot.cacheDiagTrace !== "function") return;
    hookCacheDiag._done = true;
    const original = boot.cacheDiagTrace.bind(boot);
    boot.cacheDiagTrace = function wrappedCacheDiagTrace(event, detail) {
      const name = String(event || "");
      if (
        name === "assembly-phase" ||
        name === "view-cold-start" ||
        name === "view-revision-outcome" ||
        name === "preview-fragment-hydrate-miss" ||
        name === "missing-layers" ||
        name === "render-pipeline"
      ) {
        mark(name, detail || {});
      }
      return original(event, detail);
    };
  }

  function hookAssemblyCoordinator() {
    if (hookAssemblyCoordinator._done || !boot.viewAssembly) return;
    hookAssemblyCoordinator._done = true;
    const original = boot.viewAssembly.assemble?.bind(boot.viewAssembly);
    if (typeof original !== "function") return;
    boot.viewAssembly.assemble = async function wrappedAssemble(intent, options) {
      mark("assembly:begin", { kind: intent?.kind || "" });
      try {
        const result = await original(intent, options);
        mark("assembly:end", { ok: !!result?.ok, reason: result?.reason || "" });
        return result;
      } catch (error) {
        mark("assembly:error", { message: String(error?.message || error) });
        throw error;
      }
    };
  }

  function readSurfaceFromDom() {
    const body = global.document?.body;
    return String(
      body?.getAttribute("data-surface") || body?.getAttribute("data-mei-view") || "app",
    )
      .trim()
      .toLowerCase();
  }

  function tryMarkSurfaceReady() {
    const ctx =
      typeof boot.parseViewContext === "function"
        ? boot.parseViewContext(global.location.href)
        : { surface: readSurfaceFromDom() };
    if (typeof boot.isSurfaceMaterialized === "function" && boot.isSurfaceMaterialized(ctx)) {
      mark("surface_ready", boot.surfaceSnapshot?.(ctx) || {});
      scheduleUserVisibleReady();
      return true;
    }
    return false;
  }

  function scheduleUserVisibleReady() {
    if (state.visibleReadyScheduled || state.reported) return;
    state.visibleReadyScheduled = true;
    const finish = () => {
      mark("user_visible_ready");
      finalizeRun({ source: "user_visible_ready" });
    };
    if (typeof global.requestAnimationFrame !== "function") {
      global.setTimeout(finish, 0);
      return;
    }
    global.requestAnimationFrame(() => {
      global.requestAnimationFrame(finish);
    });
  }

  function installLifecycleHooks() {
    beginRun({
      url: global.location?.href,
      surface: readSurfaceFromDom(),
      fromNavigationStart: true,
    });
    mark("bundle_boot");
    installFetchTap();
    hookCacheDiag();
    hookAssemblyCoordinator();

    const onSpaNavComplete = () => {
      mark("spa_navigation_complete");
      if (tryMarkSurfaceReady()) return;
      let attempts = 0;
      const iv = global.setInterval(() => {
        attempts += 1;
        if (tryMarkSurfaceReady()) {
          global.clearInterval(iv);
          return;
        }
        if (attempts >= 40) {
          global.clearInterval(iv);
          if (!state.reported) {
            finalizeRun({ source: "surface_ready_timeout" });
          }
        }
      }, 50);
    };
    // Coordinator dispatches on document; window listeners miss non-bubbling CustomEvents.
    global.document?.addEventListener("mei:spa-navigation-complete", onSpaNavComplete);
    global.addEventListener("mei:spa-navigation-complete", onSpaNavComplete);

    if (global.document?.readyState === "loading") {
      global.document.addEventListener("DOMContentLoaded", () => mark("dom_ready"), { once: true });
    } else {
      mark("dom_ready");
    }

    global.addEventListener("load", () => {
      mark("window_load");
      setTimeout(() => {
        if (!state.reported) finalizeRun({ source: "window_load_timeout" });
      }, 8000);
    });
  }

  boot.renderPipelineMark = mark;
  boot.renderPipelineFinalize = finalizeRun;
  boot.renderPipelineEnabled = pipelineEnabled;
  global.__meiRenderPipeline = {
    mark,
    finalize: finalizeRun,
    summary: () => buildSummary(),
    enabled: pipelineEnabled,
    get marks() {
      return state.marks.slice();
    },
    get last() {
      return state.lastSummary;
    },
  };

  installLifecycleHooks();
})(typeof window !== "undefined" ? window : globalThis);
