  const PHASES = boot.LOAD_PHASES || ["render", "eval"];
  const PHASE_LABELS = boot.LOAD_PHASE_LABELS || { render: "渲染", eval: "求值" };
  const PHASE_WEIGHTS = boot.LOAD_PHASE_WEIGHTS || { render: 0.55, eval: 0.45 };
  const READY_QUIET_MS = 360;
  const READY_MAX_WAIT_MS = 45000;
  const READY_MAX_WAIT_INITIAL_MS = 120000;
  const READY_POLL_MS = 48;
  const INITIAL_LOAD_NAVIGATION_ID = -1;

  let fetchHookInstalled = false;

  function activeSession() {
    return typeof boot.getActiveLoadSession === "function" ? boot.getActiveLoadSession() : null;
  }

  function formatMs(value) {
    return typeof boot.formatLoadMs === "function" ? boot.formatLoadMs(value) : String(value);
  }

  function sessionLabelFromUrl(url) {
    try {
      const parsed = new URL(url, window.location.href);
      const file = String(parsed.searchParams.get("file") || "").trim();
      if (file) return file;
      const scene = String(parsed.searchParams.get("scene") || "").trim();
      if (scene) return `scene:${scene}`;
      return parsed.pathname;
    } catch (_) {}
    return String(url || "访问");
  }

  function getSession(navigationId) {
    return typeof boot.getLoadSession === "function" ? boot.getLoadSession(navigationId) : null;
  }

  function setPhaseStatus(session, phase, status, detail) {
    if (!session || typeof boot.setLoadPhaseStatus !== "function") return;
    boot.setLoadPhaseStatus(session, phase, status, detail);
    updateLoadingProgressDom(session);
  }

  function phaseProgress(session, phase) {
    return typeof boot.loadPhaseProgress === "function"
      ? boot.loadPhaseProgress(session, phase)
      : 0;
  }

  function overallProgress(session) {
    return typeof boot.overallLoadProgress === "function"
      ? boot.overallLoadProgress(session)
      : 0;
  }

  function buildDetailLines(session) {
    return typeof boot.buildLoadDetailLines === "function"
      ? boot.buildLoadDetailLines(session)
      : [];
  }

  function resolveActivePhase(session) {
    return typeof boot.resolveActiveLoadPhase === "function"
      ? boot.resolveActiveLoadPhase(session)
      : null;
  }

  function paintProgressOverlay(overlay, session) {
    if (!(overlay instanceof HTMLElement)) return;
    const fill = overlay.querySelector(".spa-loading-bar-fill");
    if (fill) {
      fill.style.width =
        session.ready || overallProgress(session) >= 0.99
          ? "100%"
          : `${Math.round(overallProgress(session) * 100)}%`;
    }
    const detailHost =
      overlay.querySelector(".spa-loading-detail") ||
      overlay.querySelector("[data-mei-page-load-detail]");
    if (detailHost) {
      detailHost.innerHTML = buildDetailLines(session)
        .map((line) => `<div class="spa-loading-detail-line">${escapeHtml(line)}</div>`)
        .join("");
    }
    const title =
      overlay.querySelector(".spa-loading-text") ||
      overlay.querySelector("[data-mei-page-load-title]");
    if (title) {
      const activePhase = resolveActivePhase(session);
      const drilldown = overlay.closest("[data-mei-drilldown-load-progress]");
      title.textContent = activePhase
        ? drilldown
          ? `下钻${PHASE_LABELS[activePhase]}…`
          : `正在${PHASE_LABELS[activePhase]}…`
        : session.ready
          ? "加载完成"
          : drilldown
            ? "下钻加载中…"
            : "加载中…";
    }
  }

  function updateLoadingProgressDom(session) {
    if (!session) return;
    const shellTracking =
      typeof window !== "undefined" &&
      window.MeiPageLoadProgress &&
      typeof window.MeiPageLoadProgress.isTracking === "function" &&
      window.MeiPageLoadProgress.isTracking();
    paintProgressOverlay(document.getElementById("mei-spa-loading"), session);
    if (!shellTracking) {
      paintProgressOverlay(document.getElementById("mei-page-load-progress"), session);
    }
    document.querySelectorAll("[data-mei-drilldown-load-progress]").forEach((node) => {
      paintProgressOverlay(node, session);
    });

    const manageOverlay = document.querySelector('[data-mei-manage-nav-loading="true"]');
    if (manageOverlay) {
      const detail = manageOverlay.querySelector("[data-mei-manage-loading-detail]");
      if (detail) {
        detail.innerHTML = buildDetailLines(session)
          .map((line) => `<div style="font-size:11px;line-height:1.45;color:#cbd5e1;">${escapeHtml(line)}</div>`)
          .join("");
      }
      const bar = manageOverlay.querySelector("[data-mei-manage-loading-bar-fill]");
      if (bar) bar.style.width = `${Math.round(overallProgress(session) * 100)}%`;
    }
  }

  function escapeHtml(value) {
    return String(value ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;");
  }

  function isDatasetApiUrl(url) {
    const text = String(url || "");
    return text.includes("/api/datasets/metrics/") || text.includes("/api/datasets/query/");
  }

  function resolveApiKind(url) {
    const text = String(url || "");
    if (text.includes("/api/datasets/metrics/")) return "metrics";
    if (text.includes("/api/datasets/query/")) return "query";
    return "api";
  }

  function lookupApiKindSummary(session, kind) {
    return typeof boot.ensureApiKindSummary === "function"
      ? boot.ensureApiKindSummary(session, kind)
      : null;
  }

  function estimateResponseItemCount(json) {
    if (!json || typeof json !== "object") return 0;
    const arrayCandidates = [
      json.items,
      json.rows,
      json.records,
      json.results,
      json.data,
      json.list,
      json.values,
    ];
    for (const candidate of arrayCandidates) {
      if (Array.isArray(candidate)) return candidate.length;
    }
    const perf = json.perf && typeof json.perf === "object" ? json.perf : null;
    const numericCandidates = [
      json.row_count,
      json.total_count,
      json.count,
      perf?.row_count,
      perf?.result_count,
      perf?.rows,
    ];
    for (const candidate of numericCandidates) {
      const value = Number(candidate);
      if (Number.isFinite(value) && value >= 0) return Math.round(value);
    }
    return 0;
  }

  function recordApiPerfFromJson(session, kind, json) {
    if (!json || typeof json !== "object") return;
    const perf = json.perf && typeof json.perf === "object" ? json.perf : null;
    const kindSummary = lookupApiKindSummary(session, kind);
    const itemCount = estimateResponseItemCount(json);
    if (itemCount > 0) {
      session.api.items += itemCount;
      if (kindSummary) kindSummary.items += itemCount;
    }
    if (!perf) return;
    if (
      Number(perf.client_result_cache_hit) === 1 ||
      Number(perf.client_metric_scope_cache_hit) === 1
    ) {
      return;
    }
    const candidates = [
      perf.metric_eval_total_ms,
      perf.metric_eval_ms,
      perf.query_api_ms,
      perf.query_total_ms,
      perf.total_ms,
      perf.server_handler_total_ms,
    ];
    for (const value of candidates) {
      const ms = Number(value);
      if (Number.isFinite(ms) && ms > 0) {
        session.api.evalMs += ms;
        if (kindSummary) kindSummary.evalMs += ms;
        break;
      }
    }
    if (kind === "metrics") session.api.lastKind = "指标求值";
    else if (kind === "query") session.api.lastKind = "数据集查询";
  }

  function headerMs(response, name) {
    if (!response || typeof response.headers?.get !== "function") return NaN;
    const value = Number(response.headers.get(name));
    return Number.isFinite(value) && value >= 0 ? value : NaN;
  }

  function headerText(response, name) {
    if (!response || typeof response.headers?.get !== "function") return "";
    return String(response.headers.get(name) || "").trim();
  }

  function nowMs() {
    return typeof boot.loadNowMs === "function" ? boot.loadNowMs() : Date.now();
  }

  function recordClientRuntimeQueryCacheHit(session, kind) {
    if (!session) return;
    if (!Array.isArray(session.apiCalls)) session.apiCalls = [];
    const normalized = String(kind || "dataset").trim() || "dataset";
    const apiKind =
      normalized === "dataset" ? "query" : normalized === "metric_scope" ? "metrics" : "metrics";
    const kindSummary = lookupApiKindSummary(session, apiKind);
    if (kindSummary) {
      kindSummary.total += 1;
      kindSummary.completed += 1;
    }
    session.apiCalls.push({
      url: `/client-cache/${normalized}`,
      kind: apiKind,
      status: 200,
      ms: 0,
      ok: true,
      clientHit: true,
    });
    if (session.apiCalls.length > 20) {
      session.apiCalls = session.apiCalls.slice(-20);
    }
    if (session.phases.eval.status === "pending") {
      setPhaseStatus(session, "eval", "active");
    }
    updateLoadingProgressDom(session);
  }

  function installClientRuntimeQueryCacheHitListener() {
    if (typeof window === "undefined" || installClientRuntimeQueryCacheHitListener._installed) {
      return;
    }
    installClientRuntimeQueryCacheHitListener._installed = true;
    window.addEventListener("mei:runtime-query-client-cache-hit", (event) => {
      const session = activeSession();
      if (!session) return;
      recordClientRuntimeQueryCacheHit(session, event?.detail?.kind);
    });
  }

  function installLoadingProgressFetchHook() {
    if (fetchHookInstalled || typeof window === "undefined") return;
    fetchHookInstalled = true;
    installClientRuntimeQueryCacheHitListener();
    const nativeFetch = window.fetch.bind(window);
    window.fetch = async function meiLoadingProgressFetch(input, init) {
      const requestUrl =
        typeof input === "string"
          ? input
          : input && typeof input.url === "string"
            ? input.url
            : "";
      const session = activeSession();
      const track = session && isDatasetApiUrl(requestUrl);
      if (track) {
        if (session.phases.eval.status === "pending") {
          setPhaseStatus(session, "eval", "active");
        }
        session.api.total += 1;
        session.api.inflight += 1;
        const kindSummary = lookupApiKindSummary(session, resolveApiKind(requestUrl));
        if (kindSummary) kindSummary.total += 1;
        updateLoadingProgressDom(session);
      }
      const started = nowMs();
      try {
        const response = await nativeFetch(input, init);
        if (track) {
          const kind = resolveApiKind(requestUrl);
          const kindSummary = lookupApiKindSummary(session, kind);
          const contentLength = Number(response.headers?.get?.("content-length"));
          if (Number.isFinite(contentLength) && contentLength > 0) {
            session.api.bytes += contentLength;
            if (kindSummary) kindSummary.bytes += contentLength;
          }
          let parsedJson = null;
          try {
            const clone = response.clone();
            parsedJson = await clone.json();
            if (!Number.isFinite(contentLength) || contentLength <= 0) {
              const payloadBytes = new TextEncoder().encode(JSON.stringify(parsedJson)).length;
              session.api.bytes += payloadBytes;
              if (kindSummary) kindSummary.bytes += payloadBytes;
            }
            recordApiPerfFromJson(session, kind, parsedJson);
          } catch (_) {
            /* ignore non-json */
          }
          session.api.completed += 1;
          if (kindSummary) kindSummary.completed += 1;
          if (!response.ok) session.api.failed += 1;
          if (!response.ok && kindSummary) kindSummary.failed += 1;
          session.api.inflight = Math.max(0, session.api.inflight - 1);
          const elapsed = nowMs() - started;
          if (!Number.isFinite(session.phases.eval.durationMs) || session.phases.eval.durationMs < elapsed) {
            session.phases.eval.durationMs = Math.round(elapsed);
          }
          if (kindSummary) {
            kindSummary.maxMs = Math.max(Number(kindSummary.maxMs) || 0, Math.round(elapsed));
          }
          if (!Array.isArray(session.apiCalls)) session.apiCalls = [];
          const clientHit =
            Number(parsedJson?.perf?.client_result_cache_hit) === 1 ||
            Number(parsedJson?.perf?.client_metric_scope_cache_hit) === 1;
          session.apiCalls.push({
            url: requestUrl,
            kind: resolveApiKind(requestUrl),
            status: response.status,
            ms: Math.round(elapsed),
            ok: response.ok,
            clientHit,
          });
          if (session.apiCalls.length > 20) {
            session.apiCalls = session.apiCalls.slice(-20);
          }
          updateLoadingProgressDom(session);
        }
        return response;
      } catch (error) {
        if (track) {
          session.api.failed += 1;
          session.api.completed += 1;
          session.api.inflight = Math.max(0, session.api.inflight - 1);
          const kindSummary = lookupApiKindSummary(session, resolveApiKind(requestUrl));
          if (kindSummary) {
            kindSummary.failed += 1;
            kindSummary.completed += 1;
            kindSummary.maxMs = Math.max(Number(kindSummary.maxMs) || 0, Math.round(nowMs() - started));
          }
          if (!Array.isArray(session.apiCalls)) session.apiCalls = [];
          session.apiCalls.push({
            url: requestUrl,
            kind: resolveApiKind(requestUrl),
            status: 0,
            ms: Math.round(nowMs() - started),
            ok: false,
          });
          updateLoadingProgressDom(session);
        }
        throw error;
      }
    };
  }

  function beginLoadingProgressSession(navigationId, url) {
    installLoadingProgressFetchHook();
    const kind = navigationId === INITIAL_LOAD_NAVIGATION_ID ? "initial" : "navigation";
    const session = boot.createLoadSession({
      kind,
      label: sessionLabelFromUrl(url),
      path: url,
      navigationId,
      url,
    });
    setPhaseStatus(session, "render", "active");
    return session;
  }

  function beginDrilldownLoadSession(options) {
    installLoadingProgressFetchHook();
    const opts = options && typeof options === "object" ? options : {};
    const session = boot.createLoadSession({
      kind: "drilldown",
      label: String(opts.label || "下钻看板"),
      path: String(opts.path || ""),
    });
    setPhaseStatus(session, "render", "active");
    return session;
  }

  function recordLoadingNavigationResponse(response, navigationId, htmlByteLength) {
    const session = getSession(navigationId);
    if (!session) return;
    session.compile.serverCompileMs = headerMs(response, "x-mei-compile-ms");
    session.compile.handlerReadyMs = headerMs(response, "x-mei-handler-html-ready-ms");
    const cacheHit = headerText(response, "x-mei-compile-cache-hit");
    if (cacheHit === "1" || cacheHit === "true") session.compile.cacheHit = true;
    else if (cacheHit === "0" || cacheHit === "false") session.compile.cacheHit = false;
    const pageRenderCacheHit = headerText(response, "x-mei-page-render-cache-hit");
    if (pageRenderCacheHit === "1" || pageRenderCacheHit === "true") {
      session.compile.pageRenderCacheHit = true;
    } else if (pageRenderCacheHit === "0" || pageRenderCacheHit === "false") {
      session.compile.pageRenderCacheHit = false;
    }
    const htmlBytes = Number(htmlByteLength);
    const headerHtmlBytes = headerMs(response, "x-mei-html-bytes");
    session.compile.htmlBytes = Number.isFinite(htmlBytes)
      ? htmlBytes
      : Number.isFinite(headerHtmlBytes)
        ? headerHtmlBytes
        : 0;
    session.compile.dataPropsBytes = headerMs(response, "x-mei-data-props-bytes");
    session.compile.dataPropsCount = headerMs(response, "x-mei-data-props-count");
    updateLoadingProgressDom(session);
  }

  function markLoadingRenderSwapDone(navigationId) {
    const session = getSession(navigationId);
    if (!session) return;
    session.swapDone = true;
    updateLoadingProgressDom(session);
  }

  function markLoadingPostSpaDone(navigationId) {
    const session = getSession(navigationId);
    if (!session) return;
    session.postSpaDone = true;
    if (session.phases.render.status !== "done") {
      setPhaseStatus(session, "render", "done");
    }
    if (session.api.total === 0 && session.phases.eval.status === "pending") {
      setPhaseStatus(session, "eval", "active");
    } else if (session.api.inflight === 0 && session.api.completed > 0 && session.phases.eval.status === "active") {
      setPhaseStatus(session, "eval", "done");
    }
    updateLoadingProgressDom(session);
  }

  function markLoadingRenderTrace(entry) {
    const session = activeSession();
    if (!session || !entry) return;
    session.renderTraceCount += 1;
    const phase = String(entry.phase || "");
    if (
      phase === "render_done" ||
      phase === "layer_ready" ||
      phase === "sync_layers_done" ||
      phase === "runtime_query_done"
    ) {
      if (session.phases.render.status === "active") {
        setPhaseStatus(session, "render", "done");
      }
      if (phase === "runtime_query_done" && session.phases.eval.status === "pending") {
        setPhaseStatus(session, "eval", "active");
      }
    }
    updateLoadingProgressDom(session);
  }

  function loadingProgressReady(session) {
    if (!session) return { ready: true, reason: "no_session" };
    if (session.kind === "drilldown") {
      if (!session.contentReady) {
        return { ready: false, reason: "content" };
      }
      if (session.phases.render.status !== "done") {
        return { ready: false, reason: "render" };
      }
      if (session.api.inflight > 0) {
        return { ready: false, reason: "api_inflight" };
      }
      if (session.phases.eval.status === "pending" && session.api.total === 0) {
        return { ready: true, reason: "no_runtime_api" };
      }
      if (session.phases.eval.status === "active" && session.api.total > 0) {
        return { ready: true, reason: "eval_done" };
      }
      if (session.phases.eval.status === "done") {
        return { ready: true, reason: "eval_done" };
      }
      return { ready: true, reason: "stable" };
    }
    if (!session.swapDone) {
      return { ready: false, reason: "swap" };
    }
    if (!session.postSpaDone && session.phases.render.status !== "done") {
      return { ready: false, reason: "render" };
    }
    if (session.api.inflight > 0) {
      return { ready: false, reason: "api_inflight" };
    }
    if (session.phases.eval.status === "active" && session.api.total > 0) {
      return { ready: true, reason: "eval_done" };
    }
    if (session.phases.eval.status === "pending" && session.api.total === 0) {
      return { ready: true, reason: "no_runtime_api" };
    }
    if (session.phases.eval.status === "done") {
      return { ready: true, reason: "eval_done" };
    }
    return { ready: true, reason: "stable" };
  }

  function waitForLoadingProgressReady(navigationId) {
    const session = getSession(navigationId);
    if (!session) return Promise.resolve();
    const started = Date.now();
    let quietSince = 0;
    return new Promise((resolve) => {
      const tick = () => {
        const current = getSession(navigationId);
        if (!current) {
          resolve();
          return;
        }
        const verdict = loadingProgressReady(current);
        const elapsed = Date.now() - started;
        if (verdict.ready) {
          if (!quietSince) quietSince = Date.now();
          if (Date.now() - quietSince >= READY_QUIET_MS) {
            if (current.phases.eval.status === "active") {
              setPhaseStatus(current, "eval", "done");
            } else if (current.phases.eval.status === "pending") {
              setPhaseStatus(current, "eval", "done", "无运行时 API");
            }
            current.ready = true;
            current.readyReason = verdict.reason;
            updateLoadingProgressDom(current);
            resolve();
            return;
          }
        } else {
          quietSince = 0;
        }
        if (elapsed >= (navigationId === INITIAL_LOAD_NAVIGATION_ID ? READY_MAX_WAIT_INITIAL_MS : READY_MAX_WAIT_MS)) {
          current.ready = true;
          current.readyReason = "timeout";
          updateLoadingProgressDom(current);
          resolve();
          return;
        }
        window.setTimeout(tick, READY_POLL_MS);
      };
      tick();
    });
  }

  function waitForDrilldownLoadReady() {
    const session = activeSession();
    if (!session || session.kind !== "drilldown") return Promise.resolve();
    const started = Date.now();
    let quietSince = 0;
    return new Promise((resolve) => {
      const tick = () => {
        const current = activeSession();
        if (!current || current.kind !== "drilldown") {
          resolve();
          return;
        }
        const verdict = loadingProgressReady(current);
        if (verdict.ready) {
          if (!quietSince) quietSince = Date.now();
          if (Date.now() - quietSince >= READY_QUIET_MS) {
            if (current.phases.eval.status === "active") {
              setPhaseStatus(current, "eval", "done");
            } else if (current.phases.eval.status === "pending") {
              setPhaseStatus(current, "eval", "done", "无运行时 API");
            }
            current.ready = true;
            current.readyReason = verdict.reason;
            updateLoadingProgressDom(current);
            resolve();
            return;
          }
        } else {
          quietSince = 0;
        }
        if (Date.now() - started >= READY_MAX_WAIT_MS) {
          current.ready = true;
          current.readyReason = "timeout";
          updateLoadingProgressDom(current);
          resolve();
          return;
        }
        window.setTimeout(tick, READY_POLL_MS);
      };
      tick();
    });
  }

  async function completeDrilldownLoadSession(options) {
    const opts = options && typeof options === "object" ? options : {};
    const session = activeSession();
    if (!session || session.kind !== "drilldown" || session.finalized) return;
    if (opts.outcome === "ready") {
      session.contentReady = true;
      if (session.phases.render.status !== "done") {
        setPhaseStatus(session, "render", "done");
      }
    } else {
      session.contentReady = true;
      for (const phase of PHASES) {
        if (session.phases[phase].status !== "done") {
          setPhaseStatus(session, phase, "done", "error");
        }
      }
      session.ready = true;
      session.readyReason = "error";
      updateLoadingProgressDom(session);
      boot.finalizeLoadSession(session, {
        uiShown: Boolean(session.uiShown),
        outcome: "error",
      });
      boot.clearActiveLoadSession(null);
      return;
    }
    await waitForDrilldownLoadReady();
    const current = activeSession();
    if (!current || current.finalized) return;
    boot.finalizeLoadSession(current, {
      uiShown: Boolean(current.uiShown),
      outcome: "ready",
    });
    boot.clearActiveLoadSession(null);
  }

  function abortDrilldownLoadSession() {
    const session = activeSession();
    if (!session || session.kind !== "drilldown" || session.finalized) return;
    for (const phase of PHASES) {
      if (session.phases[phase].status !== "done") {
        setPhaseStatus(session, phase, "done", "aborted");
      }
    }
    session.ready = true;
    session.readyReason = "aborted";
    boot.finalizeLoadSession(session, {
      uiShown: Boolean(session.uiShown),
      outcome: "aborted",
    });
    boot.clearActiveLoadSession(null);
  }

  function abortLoadingProgressSession(navigationId, reason) {
    const session = getSession(navigationId);
    if (!session) return;
    for (const phase of PHASES) {
      if (session.phases[phase].status !== "done") {
        setPhaseStatus(session, phase, "done", reason || "aborted");
      }
    }
    session.ready = true;
    session.readyReason = reason || "aborted";
    updateLoadingProgressDom(session);
  }

  function clearLoadingProgressSession(navigationId) {
    if (typeof boot.clearActiveLoadSession === "function") {
      boot.clearActiveLoadSession(navigationId);
    }
  }

  boot.INITIAL_LOAD_NAVIGATION_ID = INITIAL_LOAD_NAVIGATION_ID;
  boot.beginLoadingProgressSession = beginLoadingProgressSession;
  boot.beginDrilldownLoadSession = beginDrilldownLoadSession;
  boot.completeDrilldownLoadSession = completeDrilldownLoadSession;
  boot.abortDrilldownLoadSession = abortDrilldownLoadSession;
  boot.recordLoadingNavigationResponse = recordLoadingNavigationResponse;
  boot.markLoadingRenderSwapDone = markLoadingRenderSwapDone;
  boot.markLoadingPostSpaDone = markLoadingPostSpaDone;
  boot.waitForLoadingProgressReady = waitForLoadingProgressReady;
  boot.abortLoadingProgressSession = abortLoadingProgressSession;
  boot.clearLoadingProgressSession = clearLoadingProgressSession;
  boot.refreshLoadingProgressUi = function refreshLoadingProgressUi() {
    const session = activeSession();
    if (session) updateLoadingProgressDom(session);
  };
  boot.getLoadingProgressSession = function getLoadingProgressSession() {
    return activeSession();
  };
  if (typeof window !== "undefined") {
    window.__meiLoadingProgressMarkRender = markLoadingRenderTrace;
  }
