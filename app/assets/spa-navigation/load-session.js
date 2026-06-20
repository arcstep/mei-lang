  const LOAD_PHASES = ["render", "eval"];
  const LOAD_PHASE_LABELS = {
    render: "渲染",
    eval: "求值",
  };
  const LOAD_PHASE_WEIGHTS = {
    render: 0.55,
    eval: 0.45,
  };

  let activeLoadSession = null;

  function loadNowMs() {
    if (typeof performance !== "undefined" && typeof performance.now === "function") {
      return performance.now();
    }
    return Date.now();
  }

  function formatLoadMs(value) {
    const ms = Number(value);
    if (!Number.isFinite(ms) || ms < 0) return "—";
    if (ms < 1000) return `${Math.round(ms)}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
  }

  function createLoadId() {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      return crypto.randomUUID();
    }
    return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
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

  function createLoadSession(options) {
    const opts = options && typeof options === "object" ? options : {};
    const session = {
      id: createLoadId(),
      kind: String(opts.kind || "navigation"),
      label: String(opts.label || ""),
      path: String(opts.path || opts.url || ""),
      navigationId: opts.navigationId ?? null,
      url: String(opts.url || ""),
      startedAt: loadNowMs(),
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
      apiCalls: [],
      renderTraceCount: 0,
      postSpaDone: false,
      swapDone: false,
      contentReady: false,
      ready: false,
      readyReason: "",
      uiShown: false,
      finalized: false,
    };
    activeLoadSession = session;
    return session;
  }

  function getActiveLoadSession() {
    return activeLoadSession;
  }

  function getLoadSession(navigationId) {
    if (!activeLoadSession) return null;
    if (navigationId != null && activeLoadSession.navigationId !== navigationId) return null;
    return activeLoadSession;
  }

  function clearActiveLoadSession(navigationId) {
    if (!activeLoadSession) return;
    if (navigationId != null && activeLoadSession.navigationId !== navigationId) return;
    activeLoadSession = null;
  }

  function setLoadPhaseStatus(session, phase, status, detail) {
    if (!session || !session.phases[phase]) return;
    const entry = session.phases[phase];
    if (entry.status === "done" && status !== "done") return;
    if (status === "active" && entry.status === "pending") {
      entry.startedAt = loadNowMs();
    }
    if (status === "done") {
      if (!entry.startedAt) entry.startedAt = session.startedAt;
      entry.endedAt = loadNowMs();
      entry.durationMs = Math.max(0, Math.round(entry.endedAt - entry.startedAt));
      if (detail) entry.detail = detail;
    }
    entry.status = status;
  }

  function computeRenderMs(session) {
    if (!session) return 0;
    let renderMs = session.phases.render.durationMs;
    if (Number.isFinite(session.compile.handlerReadyMs)) {
      renderMs = Math.max(renderMs, session.compile.handlerReadyMs);
    }
    return Math.max(0, Math.round(renderMs));
  }

  function computeEvalMs(session) {
    if (!session) return 0;
    const evalMs =
      session.api.evalMs > 0 ? session.api.evalMs : session.phases.eval.durationMs;
    return Math.max(0, Math.round(evalMs));
  }

  function buildLoadDetailLines(session) {
    if (!session) return [];
    const parts = [];
    const render = session.phases.render;
    if (render.status !== "pending") {
      parts.push(`渲染 ${formatLoadMs(computeRenderMs(session))}`);
    }
    const evalMs = computeEvalMs(session);
    if (session.api.total > 0) {
      parts.push(`求值 ${formatLoadMs(evalMs)}`);
    } else if (session.phases.eval.status === "done" && session.phases.eval.detail !== "无运行时 API") {
      parts.push(`求值 ${formatLoadMs(evalMs)}`);
    }
    parts.push(`总计 ${formatLoadMs(Date.now() - session.wallStartedAt)}`);
    return [parts.join(" · ")];
  }

  function loadPhaseProgress(session, phase) {
    const entry = session.phases[phase];
    if (!entry) return 0;
    if (entry.status === "done") return 1;
    if (entry.status === "active") {
      if (phase === "eval" && session.api.total > 0) {
        const ratio = session.api.completed / Math.max(session.api.total, 1);
        return Math.min(0.92, 0.2 + ratio * 0.72);
      }
      if (phase === "render") {
        if (session.kind === "navigation" && session.swapDone) {
          return session.postSpaDone ? 0.95 : 0.55;
        }
        if (session.kind === "drilldown" && session.contentReady) {
          return 0.9;
        }
      }
      return 0.35;
    }
    return 0;
  }

  function overallLoadProgress(session) {
    let sum = 0;
    for (const phase of LOAD_PHASES) {
      sum += loadPhaseProgress(session, phase) * LOAD_PHASE_WEIGHTS[phase];
    }
    return Math.max(0, Math.min(1, sum));
  }

  function resolveActiveLoadPhase(session) {
    for (let i = LOAD_PHASES.length - 1; i >= 0; i -= 1) {
      if (session.phases[LOAD_PHASES[i]].status === "active") return LOAD_PHASES[i];
    }
    return null;
  }

  function mapOutcome(session, outcome) {
    if (outcome) return outcome;
    if (session.readyReason === "timeout") return "timeout";
    if (session.readyReason === "navigation_error" || session.readyReason === "aborted") {
      return "error";
    }
    if (session.ready && session.readyReason !== "aborted") return "ready";
    return "aborted";
  }

  function finalizeLoadSession(session, options) {
    if (!session || session.finalized) return null;
    session.finalized = true;
    const opts = options && typeof options === "object" ? options : {};
    const uiShown = Boolean(opts.uiShown || session.uiShown);
    const outcome = mapOutcome(session, opts.outcome);
    const record = {
      id: session.id,
      kind: session.kind,
      at: Date.now(),
      label: session.label || session.path || session.url || "访问",
      path: session.path || session.url || "",
      renderMs: computeRenderMs(session),
      evalMs: computeEvalMs(session),
      totalMs: Math.max(0, Date.now() - session.wallStartedAt),
      apiTotal: session.api.total,
      apiFailed: session.api.failed,
      apiCalls: Array.isArray(session.apiCalls) ? session.apiCalls.slice(0, 20) : [],
      handlerReadyMs: Number.isFinite(session.compile.handlerReadyMs)
        ? session.compile.handlerReadyMs
        : 0,
      readyReason: session.readyReason || "",
      uiShown,
      outcome,
    };
    const enriched =
      typeof boot.enrichVisitHistoryRecord === "function"
        ? boot.enrichVisitHistoryRecord(record, {
            url: session.url || session.path,
            scene: session.kind === "drilldown" ? session.path : "",
            apiCalls: record.apiCalls,
            apiFailed: record.apiFailed,
            handlerReadyMs: record.handlerReadyMs,
            readyReason: record.readyReason,
          })
        : record;
    if (typeof boot.appendVisitHistory === "function") {
      boot.appendVisitHistory(enriched);
    } else if (
      typeof window !== "undefined" &&
      window.MeiVisitHistoryStore &&
      typeof window.MeiVisitHistoryStore.append === "function"
    ) {
      window.MeiVisitHistoryStore.append(enriched);
    }
    return enriched;
  }

  boot.LOAD_PHASES = LOAD_PHASES;
  boot.LOAD_PHASE_LABELS = LOAD_PHASE_LABELS;
  boot.LOAD_PHASE_WEIGHTS = LOAD_PHASE_WEIGHTS;
  boot.createLoadSession = createLoadSession;
  boot.getActiveLoadSession = getActiveLoadSession;
  boot.getLoadSession = getLoadSession;
  boot.clearActiveLoadSession = clearActiveLoadSession;
  boot.setLoadPhaseStatus = setLoadPhaseStatus;
  boot.finalizeLoadSession = finalizeLoadSession;
  boot.buildLoadDetailLines = buildLoadDetailLines;
  boot.formatLoadMs = formatLoadMs;
  boot.overallLoadProgress = overallLoadProgress;
  boot.resolveActiveLoadPhase = resolveActiveLoadPhase;
  boot.computeRenderMs = computeRenderMs;
  boot.computeEvalMs = computeEvalMs;
  boot.loadNowMs = loadNowMs;
  boot.loadPhaseProgress = loadPhaseProgress;
