  const PHASES = ["render", "eval"];
  const PHASE_LABELS = {
    render: "渲染",
    eval: "求值",
  };
  const PHASE_WEIGHTS = {
    render: 0.55,
    eval: 0.45,
  };
  const READY_QUIET_MS = 360;
  const READY_MAX_WAIT_MS = 45000;
  const READY_MAX_WAIT_INITIAL_MS = 120000;
  const READY_POLL_MS = 48;
  const INITIAL_LOAD_NAVIGATION_ID = -1;

  let activeSession = null;
  let fetchHookInstalled = false;

  function nowMs() {
    if (typeof performance !== "undefined" && typeof performance.now === "function") {
      return performance.now();
    }
    return Date.now();
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

  function formatMs(value) {
    const ms = Number(value);
    if (!Number.isFinite(ms) || ms < 0) return "—";
    if (ms < 1000) return `${Math.round(ms)}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
  }

  function createPhaseState() {
    return {
      status: "pending",
      startedAt: 0,
      endedAt: 0,
      durationMs: 0,
      detail: "",
    };
  }

  function createSession(navigationId, url) {
    return {
      navigationId,
      url: String(url || ""),
      startedAt: nowMs(),
      wallStartedAt: Date.now(),
      phases: {
        render: createPhaseState(),
        eval: createPhaseState(),
      },
      compile: {
        cacheHit: null,
        serverCompileMs: NaN,
        handlerReadyMs: NaN,
        htmlBytes: 0,
        probeCount: 0,
        dataPropsBytes: 0,
        dataPropsCount: 0,
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
      postSpaDone: false,
      swapDone: false,
      ready: false,
      readyReason: "",
    };
  }

  function getSession(navigationId) {
    if (!activeSession) return null;
    if (navigationId != null && activeSession.navigationId !== navigationId) return null;
    return activeSession;
  }

  function setPhaseStatus(session, phase, status, detail) {
    if (!session || !session.phases[phase]) return;
    const entry = session.phases[phase];
    if (entry.status === "done" && status !== "done") return;
    if (status === "active" && entry.status === "pending") {
      entry.startedAt = nowMs();
    }
    if (status === "done") {
      if (!entry.startedAt) entry.startedAt = session.startedAt;
      entry.endedAt = nowMs();
      entry.durationMs = Math.max(0, Math.round(entry.endedAt - entry.startedAt));
      if (detail) entry.detail = detail;
    }
    entry.status = status;
    updateLoadingProgressDom(session);
  }

  function phaseProgress(session, phase) {
    const entry = session.phases[phase];
    if (!entry) return 0;
    if (entry.status === "done") return 1;
    if (entry.status === "active") {
      if (phase === "eval" && session.api.total > 0) {
        const ratio = session.api.completed / Math.max(session.api.total, 1);
        return Math.min(0.92, 0.2 + ratio * 0.72);
      }
      if (phase === "render" && session.swapDone) {
        return session.postSpaDone ? 0.95 : 0.55;
      }
      return 0.35;
    }
    return 0;
  }

  function overallProgress(session) {
    let sum = 0;
    for (const phase of PHASES) {
      sum += phaseProgress(session, phase) * PHASE_WEIGHTS[phase];
    }
    return Math.max(0, Math.min(1, sum));
  }

  function buildDetailLines(session) {
    const parts = [];
    const render = session.phases.render;
    if (render.status !== "pending") {
      let renderMs = render.durationMs;
      if (Number.isFinite(session.compile.handlerReadyMs)) {
        renderMs = Math.max(renderMs, session.compile.handlerReadyMs);
      }
      parts.push(`渲染 ${formatMs(renderMs)}`);
    }
    const evalMs =
      session.api.evalMs > 0 ? session.api.evalMs : session.phases.eval.durationMs;
    if (session.api.total > 0) {
      parts.push(`求值 ${formatMs(evalMs)}`);
    } else if (session.phases.eval.status === "done" && session.phases.eval.detail !== "无运行时 API") {
      parts.push(`求值 ${formatMs(evalMs)}`);
    }
    parts.push(`总计 ${formatMs(Date.now() - session.wallStartedAt)}`);
    return [parts.join(" · ")];
  }

  function resolveActivePhase(session) {
    for (let i = PHASES.length - 1; i >= 0; i -= 1) {
      if (session.phases[PHASES[i]].status === "active") return PHASES[i];
    }
    return null;
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
      title.textContent = activePhase
        ? `正在${PHASE_LABELS[activePhase]}…`
        : session.ready
          ? "加载完成"
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

  function recordApiPerfFromJson(session, kind, json) {
    if (!json || typeof json !== "object") return;
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
        session.api.evalMs += ms;
        break;
      }
    }
    if (kind === "metrics") session.api.lastKind = "指标求值";
    else if (kind === "query") session.api.lastKind = "数据集查询";
  }

  function installLoadingProgressFetchHook() {
    if (fetchHookInstalled || typeof window === "undefined") return;
    fetchHookInstalled = true;
    const nativeFetch = window.fetch.bind(window);
    window.fetch = async function meiLoadingProgressFetch(input, init) {
      const requestUrl =
        typeof input === "string"
          ? input
          : input && typeof input.url === "string"
            ? input.url
            : "";
      const session = activeSession;
      const track = session && isDatasetApiUrl(requestUrl);
      if (track) {
        if (session.phases.eval.status === "pending") {
          setPhaseStatus(session, "eval", "active");
        }
        session.api.total += 1;
        session.api.inflight += 1;
        updateLoadingProgressDom(session);
      }
      const started = nowMs();
      try {
        const response = await nativeFetch(input, init);
        if (track) {
          const contentLength = Number(response.headers?.get?.("content-length"));
          if (Number.isFinite(contentLength) && contentLength > 0) {
            session.api.bytes += contentLength;
          }
          const kind = resolveApiKind(requestUrl);
          try {
            const clone = response.clone();
            const json = await clone.json();
            if (!Number.isFinite(contentLength) || contentLength <= 0) {
              session.api.bytes += new TextEncoder().encode(JSON.stringify(json)).length;
            }
            recordApiPerfFromJson(session, kind, json);
          } catch (_) {
            /* ignore non-json */
          }
          session.api.completed += 1;
          if (!response.ok) session.api.failed += 1;
          session.api.inflight = Math.max(0, session.api.inflight - 1);
          const elapsed = nowMs() - started;
          if (!Number.isFinite(session.phases.eval.durationMs) || session.phases.eval.durationMs < elapsed) {
            session.phases.eval.durationMs = Math.round(elapsed);
          }
          updateLoadingProgressDom(session);
        }
        return response;
      } catch (error) {
        if (track) {
          session.api.failed += 1;
          session.api.completed += 1;
          session.api.inflight = Math.max(0, session.api.inflight - 1);
          updateLoadingProgressDom(session);
        }
        throw error;
      }
    };
  }

  function beginLoadingProgressSession(navigationId, url) {
    installLoadingProgressFetchHook();
    activeSession = createSession(navigationId, url);
    setPhaseStatus(activeSession, "render", "active");
    updateLoadingProgressDom(activeSession);
    return activeSession;
  }

  function recordLoadingNavigationResponse(response, navigationId, htmlByteLength) {
    const session = getSession(navigationId);
    if (!session) return;
    session.compile.serverCompileMs = headerMs(response, "x-mei-compile-ms");
    session.compile.handlerReadyMs = headerMs(response, "x-mei-handler-html-ready-ms");
    const cacheHit = headerText(response, "x-mei-compile-cache-hit");
    if (cacheHit === "1" || cacheHit === "true") session.compile.cacheHit = true;
    else if (cacheHit === "0" || cacheHit === "false") session.compile.cacheHit = false;
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
    const session = activeSession;
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
        if (!activeSession || activeSession.navigationId !== navigationId) {
          resolve();
          return;
        }
        const verdict = loadingProgressReady(activeSession);
        const elapsed = Date.now() - started;
        if (verdict.ready) {
          if (!quietSince) quietSince = Date.now();
          if (Date.now() - quietSince >= READY_QUIET_MS) {
            if (activeSession.phases.eval.status === "active") {
              setPhaseStatus(activeSession, "eval", "done");
            } else if (activeSession.phases.eval.status === "pending") {
              setPhaseStatus(activeSession, "eval", "done", "无运行时 API");
            }
            activeSession.ready = true;
            activeSession.readyReason = verdict.reason;
            updateLoadingProgressDom(activeSession);
            resolve();
            return;
          }
        } else {
          quietSince = 0;
        }
        if (elapsed >= (navigationId === INITIAL_LOAD_NAVIGATION_ID ? READY_MAX_WAIT_INITIAL_MS : READY_MAX_WAIT_MS)) {
          activeSession.ready = true;
          activeSession.readyReason = "timeout";
          updateLoadingProgressDom(activeSession);
          resolve();
          return;
        }
        window.setTimeout(tick, READY_POLL_MS);
      };
      tick();
    });
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
    if (!activeSession) return;
    if (navigationId != null && activeSession.navigationId !== navigationId) return;
    activeSession = null;
  }

  boot.INITIAL_LOAD_NAVIGATION_ID = INITIAL_LOAD_NAVIGATION_ID;
  boot.beginLoadingProgressSession = beginLoadingProgressSession;
  boot.recordLoadingNavigationResponse = recordLoadingNavigationResponse;
  boot.markLoadingRenderSwapDone = markLoadingRenderSwapDone;
  boot.markLoadingPostSpaDone = markLoadingPostSpaDone;
  boot.waitForLoadingProgressReady = waitForLoadingProgressReady;
  boot.abortLoadingProgressSession = abortLoadingProgressSession;
  boot.clearLoadingProgressSession = clearLoadingProgressSession;
  boot.refreshLoadingProgressUi = function refreshLoadingProgressUi() {
    if (activeSession) updateLoadingProgressDom(activeSession);
  };
  boot.getLoadingProgressSession = function getLoadingProgressSession() {
    return activeSession;
  };
  if (typeof window !== "undefined") {
    window.__meiLoadingProgressMarkRender = markLoadingRenderTrace;
  }
