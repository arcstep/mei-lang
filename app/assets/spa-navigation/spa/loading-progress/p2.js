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
