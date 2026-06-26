          if (Number.isFinite(contentLength) && contentLength > 0) {
            trackSession.api.bytes += contentLength;
          }
          const kind = requestUrl.includes("/api/datasets/metrics/") ? "metrics" : "query";
          let parsedJson = null;
          try {
            const clone = response.clone();
            parsedJson = await clone.json();
            if (!Number.isFinite(contentLength) || contentLength <= 0) {
              trackSession.api.bytes += new TextEncoder().encode(JSON.stringify(parsedJson)).length;
            }
            recordApiPerfFromJson(kind, parsedJson);
          } catch (_) {}
          trackSession.api.completed += 1;
          if (!response.ok) trackSession.api.failed += 1;
          trackSession.api.inflight = Math.max(0, trackSession.api.inflight - 1);
          const elapsed = Math.max(0, Date.now() - requestStarted);
          trackSession.api.evalWallMs = Math.max(Number(trackSession.api.evalWallMs) || 0, elapsed);
          if (!Array.isArray(trackSession.apiCalls)) trackSession.apiCalls = [];
          const clientHit =
            Number(parsedJson?.perf?.client_result_cache_hit) === 1 ||
            Number(parsedJson?.perf?.client_metric_scope_cache_hit) === 1;
          trackSession.apiCalls.push({
            url: requestUrl,
            kind: kind,
            status: response.status,
            ms: elapsed,
            ok: response.ok,
            clientHit: clientHit,
            responseCacheHit: Number(parsedJson?.perf?.response_cache_hit) === 1,
            resultArtifactHit: Number(parsedJson?.perf?.result_artifact_hit) === 1,
          });
          if (trackSession.apiCalls.length > 20) {
            trackSession.apiCalls = trackSession.apiCalls.slice(-20);
          }
          paint();
          scheduleReadyCheck();
        }
        return response;
      } catch (error) {
        if (trackSession && trackSession.api) {
          trackSession.api.failed += 1;
          trackSession.api.completed += 1;
          trackSession.api.inflight = Math.max(0, trackSession.api.inflight - 1);
          const elapsed = Math.max(0, Date.now() - requestStarted);
          trackSession.api.evalWallMs = Math.max(Number(trackSession.api.evalWallMs) || 0, elapsed);
          if (!Array.isArray(trackSession.apiCalls)) trackSession.apiCalls = [];
          trackSession.apiCalls.push({
            url: requestUrl,
            kind: requestUrl.includes("/api/datasets/metrics/") ? "metrics" : "query",
            status: 0,
            ms: elapsed,
            ok: false,
            clientHit: false,
          });
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
    const evalMs = Math.max(
      Number(state.api.evalMs) || 0,
      Number(state.api.evalWallMs) || 0,
      Number(state.phases.eval.durationMs) || 0,
    );
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
      apiFailed: state.api.failed || 0,
      apiCalls: Array.isArray(state.apiCalls) ? state.apiCalls.slice(0, 20) : [],
      handlerReadyMs: Number.isFinite(state.compile.handlerReadyMs) ? state.compile.handlerReadyMs : 0,
      readyReason: state.ready ? "ready" : "aborted",
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
