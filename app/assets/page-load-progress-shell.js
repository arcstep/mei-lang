(function (global) {
  const STORAGE_KEY = "__mei_loading_handoff";
  const PHASES = ["render", "eval"];
  const PHASE_LABELS = { render: "渲染", eval: "求值" };
  const PHASE_WEIGHTS = { render: 0.55, eval: 0.45 };
  const READY_QUIET_MS = 320;
  const READY_POLL_MS = 48;
  const SHOW_DELAY_MS = 1000;
  const MIN_VISIBLE_MS = 1000;

  let state = null;
  let fetchHookInstalled = false;
  let quietSince = 0;
  let readyPollTimer = 0;
  let overlayShownAt = 0;
  let hideTimer = 0;
  let showDelayTimer = 0;

  function escapeHtml(value) {
    return String(value ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;");
  }

  function formatMs(value) {
    const ms = Number(value);
    if (!Number.isFinite(ms) || ms < 0) return "—";
    if (ms < 1000) return Math.round(ms) + "ms";
    return (ms / 1000).toFixed(2) + "s";
  }

  function reasonLabel(reason) {
    const map = {
      compile_inflight: "后台编译进行中",
      compile_started: "已启动后台编译",
      cache_hit: "编译缓存已就绪",
      bootstrap_disabled: "编译引导已禁用",
      recent_compile_failure: "编译失败后重试",
      network_retry: "网络重试",
    };
    return map[String(reason || "").trim()] || String(reason || "").trim() || "等待编译";
  }

  function overlayMarkup() {
    return (
      '<div class="spa-loading-inner">' +
      '<img class="spa-loading-icon" src="/app-assets/favicon.svg" alt="loading"/>' +
      '<div class="spa-loading-body">' +
      '<span class="spa-loading-text" data-mei-page-load-title="true">加载中…</span>' +
      '<div class="spa-loading-track">' +
      '<div class="spa-loading-bar"><div class="spa-loading-bar-fill"></div></div>' +
      "</div>" +
      '<div class="spa-loading-detail" data-mei-page-load-detail="true"></div>' +
      "</div>" +
      "</div>"
    );
  }

  function overlayVisible() {
    const overlay = document.getElementById("mei-page-load-progress");
    return Boolean(overlay && overlay.classList.contains("is-visible"));
  }

  function cancelShowDelay() {
    if (showDelayTimer) {
      global.clearTimeout(showDelayTimer);
      showDelayTimer = 0;
    }
  }

  function revealOverlay() {
    const overlay = ensureOverlay();
    if (!overlay.classList.contains("is-visible")) {
      overlay.classList.add("is-visible");
      overlayShownAt = Date.now();
    }
    return overlay;
  }

  function scheduleOverlayShow() {
    cancelShowDelay();
    if (!state) return;
    const elapsed = Date.now() - state.wallStartedAt;
    const delay = Math.max(0, SHOW_DELAY_MS - elapsed);
    showDelayTimer = global.setTimeout(function () {
      showDelayTimer = 0;
      if (!state || state.ready) return;
      revealOverlay();
      paint();
    }, delay);
  }

  function ensureOverlay() {
    var overlay = document.getElementById("mei-page-load-progress");
    if (overlay) return overlay;
    overlay = document.createElement("div");
    overlay.id = "mei-page-load-progress";
    overlay.className = "spa-loading-overlay";
    overlay.setAttribute("role", "status");
    overlay.setAttribute("aria-live", "polite");
    overlay.innerHTML = overlayMarkup();
    (document.body || document.documentElement).appendChild(overlay);
    return overlay;
  }

  function createState(mode) {
    return {
      mode: mode || "bootstrap",
      wallStartedAt: Date.now(),
      phases: {
        render: { status: "pending", startedAt: 0, durationMs: 0, detail: "" },
        eval: { status: "pending", startedAt: 0, durationMs: 0, detail: "" },
      },
      compile: {
        cacheHit: null,
        serverCompileMs: NaN,
        handlerReadyMs: NaN,
        bootstrapWaitMs: 0,
        dataPropsBytes: 0,
        dataPropsCount: 0,
        probeCount: 0,
        lastReason: "",
      },
      api: {
        total: 0,
        inflight: 0,
        completed: 0,
        failed: 0,
        bytes: 0,
        evalMs: 0,
        lastKind: "",
      },
      renderTraceCount: 0,
      ready: false,
      handoffLifecycle: false,
    };
  }

  function phaseDisplayMs(phase) {
    if (!state) return 0;
    const entry = state.phases[phase];
    if (!entry) return 0;
    if (phase === "render" && Number.isFinite(state.compile.handlerReadyMs)) {
      return Math.max(entry.durationMs, state.compile.handlerReadyMs);
    }
    return entry.durationMs;
  }

  function setPhase(phase, status, detail) {
    if (!state || !state.phases[phase]) return;
    const entry = state.phases[phase];
    if (entry.status === "done" && status !== "done") return;
    if (status === "active" && entry.status === "pending") {
      entry.startedAt = Date.now();
    }
    entry.status = status;
    if (detail) entry.detail = detail;
    if (status === "done") {
      entry.durationMs = entry.startedAt
        ? Math.max(0, Date.now() - entry.startedAt)
        : 0;
    }
    paint();
    scheduleReadyCheck();
  }

  function resolveActivePhase() {
    if (!state) return null;
    for (let i = PHASES.length - 1; i >= 0; i -= 1) {
      if (state.phases[PHASES[i]].status === "active") return PHASES[i];
    }
    return null;
  }

  function phaseProgress(phase) {
    if (!state) return 0;
    const entry = state.phases[phase];
    if (!entry) return 0;
    if (entry.status === "done") return 1;
    if (entry.status === "active") {
      if (phase === "eval" && state.api.total > 0) {
        return Math.min(0.95, 0.2 + (state.api.completed / Math.max(state.api.total, 1)) * 0.75);
      }
      return 0.45;
    }
    return 0;
  }

  function overallProgress() {
    let sum = 0;
    PHASES.forEach(function (phase) {
      sum += phaseProgress(phase) * PHASE_WEIGHTS[phase];
    });
    return Math.max(0, Math.min(1, sum));
  }

  function buildDetailLines() {
    if (!state) return [];
    const parts = [];
    if (state.phases.render.status !== "pending") {
      parts.push("渲染 " + formatMs(phaseDisplayMs("render")));
    }
    if (state.api.total > 0) {
      parts.push("求值 " + formatMs(state.api.evalMs));
    } else if (state.phases.eval.status === "done") {
      parts.push("求值 " + formatMs(state.phases.eval.durationMs));
    }
    parts.push("总计 " + formatMs(Date.now() - state.wallStartedAt));
    return [parts.join(" · ")];
  }

  function paint() {
    const overlay = document.getElementById("mei-page-load-progress");
    if (!overlay || !state) return;
    const fill = overlay.querySelector(".spa-loading-bar-fill");
    if (fill) {
      fill.style.width =
        state.ready || overallProgress() >= 0.99
          ? "100%"
          : Math.round(overallProgress() * 100) + "%";
    }
    const detailHost = overlay.querySelector("[data-mei-page-load-detail]");
    if (detailHost) {
      detailHost.innerHTML = buildDetailLines()
        .map(function (line) {
          return '<div class="spa-loading-detail-line">' + escapeHtml(line) + "</div>";
        })
        .join("");
    }
    const title = overlay.querySelector("[data-mei-page-load-title]");
    if (title) {
      const activePhase = resolveActivePhase();
      title.textContent = activePhase
        ? "正在" + PHASE_LABELS[activePhase] + "…"
        : state.ready
          ? "加载完成"
          : "加载中…";
    }
  }

  function isDatasetApiUrl(url) {
    const text = String(url || "");
    return text.includes("/api/datasets/metrics/") || text.includes("/api/datasets/query/");
  }

  function recordApiPerfFromJson(kind, json) {
    if (!json || typeof json !== "object" || !state) return;
    const perf = json.perf && typeof json.perf === "object" ? json.perf : null;
    if (!perf) return;
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
        state.api.evalMs += ms;
        break;
      }
    }
    state.api.lastKind = kind === "metrics" ? "指标求值" : "数据集查询";
  }

  function installFetchHook() {
    if (fetchHookInstalled || typeof global.fetch !== "function") return;
    fetchHookInstalled = true;
    const nativeFetch = global.fetch.bind(global);
    global.fetch = async function meiPageLoadFetch(input, init) {
      const requestUrl =
        typeof input === "string"
          ? input
          : input && typeof input.url === "string"
            ? input.url
            : "";
      const trackSession =
        state && state.handoffLifecycle && state.api && isDatasetApiUrl(requestUrl)
          ? state
          : null;
      if (trackSession) {
        if (trackSession.phases.eval.status === "pending") {
          setPhase("eval", "active");
        }
        trackSession.api.total += 1;
        trackSession.api.inflight += 1;
        paint();
      }
      try {
        const response = await nativeFetch(input, init);
        if (trackSession && trackSession.api) {
          const contentLength = Number(response.headers?.get?.("content-length"));
          if (Number.isFinite(contentLength) && contentLength > 0) {
            trackSession.api.bytes += contentLength;
          }
          const kind = requestUrl.includes("/api/datasets/metrics/") ? "metrics" : "query";
          try {
            const clone = response.clone();
            const json = await clone.json();
            if (!Number.isFinite(contentLength) || contentLength <= 0) {
              trackSession.api.bytes += new TextEncoder().encode(JSON.stringify(json)).length;
            }
            recordApiPerfFromJson(kind, json);
          } catch (_) {}
          trackSession.api.completed += 1;
          if (!response.ok) trackSession.api.failed += 1;
          trackSession.api.inflight = Math.max(0, trackSession.api.inflight - 1);
          paint();
          scheduleReadyCheck();
        }
        return response;
      } catch (error) {
        if (trackSession && trackSession.api) {
          trackSession.api.failed += 1;
          trackSession.api.completed += 1;
          trackSession.api.inflight = Math.max(0, trackSession.api.inflight - 1);
          paint();
          scheduleReadyCheck();
        }
        throw error;
      }
    };
  }

  function isReady() {
    if (!state || state.ready) return true;
    if (state.phases.render.status !== "done") return false;
    if (state.api.inflight > 0) return false;
    if (state.phases.eval.status === "pending" && state.api.total === 0) return true;
    if (state.phases.eval.status === "active" && state.api.inflight === 0) return true;
    return state.phases.eval.status === "done";
  }

  function scheduleHide() {
    if (hideTimer) return;
    const elapsed = overlayShownAt ? Date.now() - overlayShownAt : MIN_VISIBLE_MS;
    const delay = Math.max(0, MIN_VISIBLE_MS - elapsed);
    hideTimer = global.setTimeout(function () {
      hideTimer = 0;
      hide();
    }, delay);
  }

  function finishIfReady() {
    if (!state || state.ready) return;
    if (!isReady()) {
      quietSince = 0;
      return;
    }
    if (!quietSince) quietSince = Date.now();
    if (Date.now() - quietSince < READY_QUIET_MS) return;
    if (state.phases.eval.status === "active" || state.phases.eval.status === "pending") {
      setPhase("eval", "done");
    }
    state.ready = true;
    paint();
    if (!overlayVisible()) {
      cancelShowDelay();
      hide();
      return;
    }
    scheduleHide();
  }

  function scheduleReadyCheck() {
    if (readyPollTimer) return;
    readyPollTimer = global.setTimeout(function () {
      readyPollTimer = 0;
      finishIfReady();
      if (state && !state.ready) scheduleReadyCheck();
    }, READY_POLL_MS);
  }

  function applyBodyPerf() {
    if (!state) return;
    const perf = readBodyPerf();
    if (Number.isFinite(perf.compileMs)) state.compile.serverCompileMs = perf.compileMs;
    if (perf.compileCacheHit === "1") state.compile.cacheHit = true;
    if (perf.compileCacheHit === "0") state.compile.cacheHit = false;
    if (Number.isFinite(perf.handlerReadyMs)) state.compile.handlerReadyMs = perf.handlerReadyMs;
    if (Number.isFinite(perf.dataPropsBytes)) state.compile.dataPropsBytes = perf.dataPropsBytes;
    if (Number.isFinite(perf.dataPropsCount)) state.compile.dataPropsCount = perf.dataPropsCount;
    paint();
  }

  function onDomReady() {
    if (!state || !state.handoffLifecycle) return;
    applyBodyPerf();
    if (state.phases.eval.status === "pending") {
      setPhase("eval", "active");
    }
    scheduleReadyCheck();
  }

  function beginHandoffLifecycle() {
    if (!state) return;
    state.handoffLifecycle = true;
    installFetchHook();
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", onDomReady, { once: true });
    } else {
      onDomReady();
    }
    scheduleReadyCheck();
  }

  function writeHandoff() {
    if (!state) return;
    try {
      global.sessionStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          wallStartedAt: state.wallStartedAt,
          bootstrapWaitMs: Math.max(0, Date.now() - state.wallStartedAt),
          probeCount: state.compile.probeCount,
          lastReason: state.compile.lastReason,
        }),
      );
    } catch (_) {}
  }

  function peekHandoff() {
    try {
      const raw = global.sessionStorage.getItem(STORAGE_KEY);
      return raw ? JSON.parse(raw) : null;
    } catch (_) {
      return null;
    }
  }

  function mountEarlyHandoffOverlay() {
    const handoff = peekHandoff();
    if (!handoff || state) return;
    state = createState("handoff");
    state.wallStartedAt = Number(handoff.wallStartedAt) || Date.now();
    state.compile.probeCount = Number(handoff.probeCount) || 0;
    state.compile.lastReason = String(handoff.lastReason || "");
    state.compile.bootstrapWaitMs = Number(handoff.bootstrapWaitMs) || 0;
    setPhase("render", "active");
    scheduleOverlayShow();
  }

  function readHandoff() {
    try {
      const raw = global.sessionStorage.getItem(STORAGE_KEY);
      if (!raw) return null;
      global.sessionStorage.removeItem(STORAGE_KEY);
      return JSON.parse(raw);
    } catch (_) {
      return null;
    }
  }

  function readBodyPerf() {
    const body = document.body;
    if (!body || !body.dataset) return {};
    return {
      handlerReadyMs: Number(body.dataset.meiHandlerHtmlReadyMs),
      ssrBodyMs: Number(body.dataset.meiSsrHttpResponseBodyMs),
      compileMs: Number(body.dataset.meiCompileMs),
      compileCacheHit: body.dataset.meiCompileCacheHit,
      dataPropsBytes: Number(body.dataset.meiDataPropsBytes),
      dataPropsCount: Number(body.dataset.meiDataPropsCount),
    };
  }

  function mountBootstrap(titleText) {
    state = createState("bootstrap");
    state.wallStartedAt = Date.now();
    setPhase("render", "active");
    scheduleOverlayShow();
    global.setTimeout(function () {
      const title = document.querySelector("[data-mei-page-load-title]");
      if (title && titleText) title.textContent = titleText;
      paint();
    }, 0);
  }

  function mountFromHandoff() {
    const handoff = readHandoff() || peekHandoff();
    const perf = readBodyPerf();
    if (!handoff && !Number.isFinite(perf.handlerReadyMs)) return false;
    if (!state) {
      state = createState("handoff");
      state.wallStartedAt = Number(handoff?.wallStartedAt) || Date.now();
    }
    if (handoff) {
      state.wallStartedAt = Number(handoff.wallStartedAt) || state.wallStartedAt;
      state.compile.probeCount = Number(handoff.probeCount) || 0;
      state.compile.lastReason = String(handoff.lastReason || "");
      state.compile.bootstrapWaitMs = Number(handoff.bootstrapWaitMs) || 0;
    }
    applyBodyPerf();
    const renderMs =
      Number.isFinite(perf.handlerReadyMs) && perf.handlerReadyMs > 0
        ? perf.handlerReadyMs
        : 0;
    if (renderMs > 0) {
      state.phases.render.startedAt = Date.now() - renderMs;
    }
    setPhase("render", "done");
    setPhase("eval", "active");
    beginHandoffLifecycle();
    scheduleOverlayShow();
    return true;
  }

  function noteProbe(reason) {
    if (!state) return;
    state.compile.probeCount += 1;
    state.compile.lastReason = String(reason || "");
    if (state.phases.render.status !== "done") {
      setPhase("render", "active", reasonLabel(state.compile.lastReason));
    } else {
      paint();
    }
  }

  function noteCompileReady() {
    if (!state) return;
    setPhase("render", "done", reasonLabel(state.compile.lastReason));
    writeHandoff();
    paint();
  }

  function appendShellVisitHistory() {
    if (!state) return;
    const renderMs = phaseDisplayMs("render");
    const evalMs = state.api.evalMs || state.phases.eval.durationMs || 0;
    const record = {
      id: "shell-" + String(Date.now()),
      kind: "initial",
      at: Date.now(),
      label: (global.document && global.document.title) || "首屏",
      path: global.location.pathname + global.location.search,
      renderMs: Math.max(0, Math.round(renderMs)),
      evalMs: Math.max(0, Math.round(evalMs)),
      totalMs: Math.max(0, Date.now() - state.wallStartedAt),
      apiTotal: state.api.total || 0,
      uiShown: overlayVisible(),
      outcome: state.ready ? "ready" : "aborted",
    };
    if (global.MeiVisitHistoryStore && typeof global.MeiVisitHistoryStore.append === "function") {
      global.MeiVisitHistoryStore.append(record);
    }
  }

  function hide() {
    appendShellVisitHistory();
    cancelShowDelay();
    if (hideTimer) {
      global.clearTimeout(hideTimer);
      hideTimer = 0;
    }
    const overlay = document.getElementById("mei-page-load-progress");
    if (overlay) overlay.classList.remove("is-visible");
    state = null;
    quietSince = 0;
    overlayShownAt = 0;
  }

  function isTracking() {
    return Boolean(state && !state.ready);
  }

  global.MeiPageLoadProgress = {
    mountBootstrap: mountBootstrap,
    mountEarlyHandoffOverlay: mountEarlyHandoffOverlay,
    mountFromHandoff: mountFromHandoff,
    noteProbe: noteProbe,
    noteCompileReady: noteCompileReady,
    hide: hide,
    paint: paint,
    isTracking: isTracking,
    getState: function () {
      return state;
    },
    setPhase: setPhase,
    STORAGE_KEY: STORAGE_KEY,
  };

  mountEarlyHandoffOverlay();
})(window);
