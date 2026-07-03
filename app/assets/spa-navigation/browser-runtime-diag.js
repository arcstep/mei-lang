// 浏览器运行时诊断：GIS 瓦片 / 布局反馈环 / 长任务 / FPS → sessionStorage
(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.browserRuntimeDiagInstalled) return;
  boot.browserRuntimeDiagInstalled = true;

  const STORAGE_KEY = "mei.browser_runtime_diag";
  const SCHEMA_VERSION = 2;
  const MAX_EVENTS = 96;
  const MAX_GIS = 48;
  const MAX_ALERTS = 32;
  const FLUSH_MS = 2000;
  const SLOW_GIS_MS = 800;
  const SLOW_LONG_TASK_MS = 50;
  const LAYOUT_BURST_WINDOW_MS = 3000;
  const LAYOUT_BURST_THRESHOLD = 120;
  const LAYOUT_SYNC_STORM_THRESHOLD = 40;
  const SNAPSHOT_INTERVAL_MS = 60000;
  const SCENE_SHELL_BLOAT_BYTES = 5 * 1024 * 1024;
  const SCENE_SHELL_DB = "mei-scene-shell-cache-v1";
  const SCENE_SHELL_STORE = "snapshots";

  function nowMs() {
    return typeof performance !== "undefined" && performance.now
      ? performance.now()
      : Date.now();
  }

  function pageOriginMs() {
    if (typeof performance !== "undefined" && performance.timing?.navigationStart) {
      return performance.timing.navigationStart;
    }
    return Date.now();
  }

  function isEnabled() {
    try {
      const qs = new URLSearchParams(window.location.search || "");
      if (qs.get("mei_runtime_diag") === "0") return false;
      if (qs.get("mei_runtime_diag") === "1") return true;
    } catch (_) {
      /* ignore */
    }
    try {
      const stored = window.sessionStorage?.getItem("mei.runtime_diag.enabled");
      if (stored === "0") return false;
      if (stored === "1") return true;
    } catch (_) {
      /* ignore */
    }
    return true;
  }

  function emptyState() {
    const t = Date.now();
    return {
      schemaVersion: SCHEMA_VERSION,
      startedAt: new Date(t).toISOString(),
      pageUrl: String(window.location.href || ""),
      enabled: isEnabled(),
      summary: {
        gis: {
          total: 0,
          ok: 0,
          fail: 0,
          slow: 0,
          doubleGis: 0,
          pendingPeak: 0,
          tilejson: 0,
          tiles: 0,
          glyphs: 0,
        },
        layout: {
          viewportStageLayout: 0,
          previewUpdated: 0,
          mapResizeObserver: 0,
          cockpitMapToolsSync: 0,
          cockpitStageLayoutSync: 0,
        },
        map: {
          fullRender: 0,
          renderStart: 0,
          runtimeError: 0,
          instancesPeak: 0,
        },
        perf: {
          longTaskCount: 0,
          longTaskTotalMs: 0,
          longTaskMaxMs: 0,
          fpsMin: null,
          fpsAvg: null,
          fpsSamples: 0,
        },
        gpu: {
          canvasCount: 0,
          canvasPeak: 0,
          worldRendererActive: false,
          mapPausedCount: 0,
          dualWebGLAlerts: 0,
          jsHeapMb: null,
        },
        world: {
          enterCount: 0,
          exitCount: 0,
          sceneBootstrapCount: 0,
          sceneDisposeCount: 0,
        },
        storage: {
          sceneShellIdbEntries: 0,
          sceneShellIdbBytesEst: 0,
          sessionStorageBytes: 0,
          runtimeDiagBytes: 0,
        },
      },
      alerts: [],
      recentEvents: [],
      recentGisRequests: [],
    };
  }

  let state = emptyState();
  let gisPending = 0;
  let layoutBurst = { windowStart: nowMs(), count: 0 };
  let flushTimer = 0;
  let fpsFrames = 0;
  let fpsWindowStart = nowMs();
  let fpsSum = 0;
  let fpsSamples = 0;
  let layoutSyncBurst = { windowStart: nowMs(), count: 0, baseline: 0 };
  let canvasGrowthStreak = 0;
  let lastCanvasCount = 0;
  let snapshotTimer = 0;
  const alertCooldown = {};

  function ensureSummaryShape() {
    const s = state.summary;
    if (!s.gpu) {
      s.gpu = emptyState().summary.gpu;
    }
    if (!s.world) {
      s.world = emptyState().summary.world;
    }
    if (!s.storage) {
      s.storage = emptyState().summary.storage;
    }
  }

  function loadState() {
    try {
      const raw = window.sessionStorage?.getItem(STORAGE_KEY);
      if (!raw) return;
      const parsed = JSON.parse(raw);
      if (parsed && (parsed.schemaVersion === SCHEMA_VERSION || parsed.schemaVersion === 1)) {
        state = parsed;
        state.schemaVersion = SCHEMA_VERSION;
        state.enabled = isEnabled();
        ensureSummaryShape();
      }
    } catch (_) {
      /* ignore corrupt snapshot */
    }
  }

  function trimLists() {
    if (state.recentEvents.length > MAX_EVENTS) {
      state.recentEvents.length = MAX_EVENTS;
    }
    if (state.recentGisRequests.length > MAX_GIS) {
      state.recentGisRequests.length = MAX_GIS;
    }
    if (state.alerts.length > MAX_ALERTS) {
      state.alerts.length = MAX_ALERTS;
    }
  }

  function flush() {
    if (!isEnabled()) return;
    trimLists();
    state.lastFlushAt = new Date().toISOString();
    state.pageUrl = String(window.location.href || "");
    try {
      window.sessionStorage?.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch (error) {
      try {
        state.recentGisRequests = state.recentGisRequests.slice(0, 16);
        state.recentEvents = state.recentEvents.slice(0, 32);
        window.sessionStorage?.setItem(STORAGE_KEY, JSON.stringify(state));
      } catch (_) {
        console.warn("[mei-runtime-diag] sessionStorage flush failed", error);
      }
    }
  }

  function scheduleFlush() {
    if (flushTimer) return;
    flushTimer = window.setTimeout(() => {
      flushTimer = 0;
      flush();
    }, FLUSH_MS);
  }

  function pushEvent(kind, detail = {}) {
    if (!isEnabled()) return;
    const eventKind = String(kind || "event");
    if (eventKind === "world_scene_disposed") {
      ensureSummaryShape();
      state.summary.world.sceneDisposeCount += 1;
    } else if (eventKind === "world_scene_bootstrapped") {
      ensureSummaryShape();
      state.summary.world.sceneBootstrapCount += 1;
    }
    state.recentEvents.unshift({
      t: Math.round(nowMs()),
      sinceNavMs: Math.round(Date.now() - pageOriginMs()),
      kind: eventKind,
      detail:
        detail && typeof detail === "object" && !Array.isArray(detail)
          ? { ...detail }
          : { value: String(detail ?? "") },
    });
    trimLists();
    scheduleFlush();
  }

  function pushAlert(kind, detail = {}) {
    if (!isEnabled()) return;
    state.alerts.unshift({
      t: Math.round(nowMs()),
      kind: String(kind || "alert"),
      detail:
        detail && typeof detail === "object" && !Array.isArray(detail)
          ? { ...detail }
          : { value: String(detail ?? "") },
    });
    trimLists();
    pushEvent(`alert:${kind}`, detail);
  }

  function classifyGisUrl(url) {
    const text = String(url || "");
    if (!text.includes("/gis")) {
      return { kind: "other", doubleGis: false };
    }
    const doubleGis = /\/gis\/gis(?:\/|$)/.test(text);
    let kind = "gis";
    if (/\/fonts\//.test(text) || text.includes("{fontstack}")) {
      kind = "glyphs";
    } else if (/\/\d+\/\d+\/\d+/.test(text)) {
      kind = "tile";
    } else if (text.includes("shapingba") || text.endsWith("/gis") || /\/gis\/[^/]+$/.test(text)) {
      kind = "tilejson";
    }
    return { kind, doubleGis };
  }

  function recordGisFinish(url, status, durationMs, errorMessage = "") {
    if (!isEnabled()) return;
    gisPending = Math.max(0, gisPending - 1);
    const { kind, doubleGis } = classifyGisUrl(url);
    const ok = status >= 200 && status < 400 && !errorMessage;
    const slow = durationMs >= SLOW_GIS_MS;

    const bucket = state.summary.gis;
    bucket.total += 1;
    if (ok) bucket.ok += 1;
    else bucket.fail += 1;
    if (slow) bucket.slow += 1;
    if (doubleGis) bucket.doubleGis += 1;
    if (kind === "tilejson") bucket.tilejson += 1;
    else if (kind === "tile") bucket.tiles += 1;
    else if (kind === "glyphs") bucket.glyphs += 1;
    bucket.pendingPeak = Math.max(bucket.pendingPeak, gisPending);

    const entry = {
      t: Math.round(nowMs()),
      url: String(url).slice(0, 240),
      status: Number(status) || 0,
      ms: Math.round(durationMs),
      kind,
      ok,
      doubleGis,
      error: errorMessage ? String(errorMessage).slice(0, 160) : "",
    };
    state.recentGisRequests.unshift(entry);
    trimLists();

    if (doubleGis) {
      pushAlert("gis_double_proxy_path", { url: entry.url });
    }
    if (!ok) {
      pushEvent("gis_request_fail", entry);
    } else if (slow) {
      pushEvent("gis_request_slow", entry);
    }
    scheduleFlush();
  }

  function recordGisStart(url) {
    if (!isEnabled()) return;
    gisPending += 1;
    state.summary.gis.pendingPeak = Math.max(state.summary.gis.pendingPeak, gisPending);
    const { doubleGis } = classifyGisUrl(url);
    if (doubleGis) {
      pushAlert("gis_double_proxy_path_pending", { url: String(url).slice(0, 240) });
    }
  }

  function recordLayout(kind, detail = {}) {
    if (!isEnabled()) return;
    const key = String(kind || "").trim();
    const map = {
      viewport_stage_layout: "viewportStageLayout",
      preview_updated: "previewUpdated",
      map_resize_observer: "mapResizeObserver",
      cockpit_map_tools_sync: "cockpitMapToolsSync",
      cockpit_stage_layout_sync: "cockpitStageLayoutSync",
    };
    const field = map[key] || null;
    if (field && state.summary.layout[field] != null) {
      state.summary.layout[field] += 1;
    }
    if (key === "cockpit_map_tools_sync") {
      const nowSync = nowMs();
      if (nowSync - layoutSyncBurst.windowStart > LAYOUT_BURST_WINDOW_MS) {
        layoutSyncBurst = {
          windowStart: nowSync,
          count: 0,
          baseline: state.summary.layout.cockpitMapToolsSync,
        };
      }
      layoutSyncBurst.count =
        state.summary.layout.cockpitMapToolsSync - layoutSyncBurst.baseline;
      if (layoutSyncBurst.count >= LAYOUT_SYNC_STORM_THRESHOLD) {
        pushAlert("layout_sync_storm", {
          count: layoutSyncBurst.count,
          windowMs: LAYOUT_BURST_WINDOW_MS,
        });
        layoutSyncBurst.baseline = state.summary.layout.cockpitMapToolsSync;
      }
    }
    const now = nowMs();
    if (now - layoutBurst.windowStart > LAYOUT_BURST_WINDOW_MS) {
      layoutBurst = { windowStart: now, count: 0 };
    }
    layoutBurst.count += 1;
    if (layoutBurst.count === LAYOUT_BURST_THRESHOLD) {
      pushAlert("layout_burst", {
        kind: key,
        count: layoutBurst.count,
        windowMs: LAYOUT_BURST_WINDOW_MS,
      });
    }
    if (
      key === "map_resize_observer" ||
      key === "cockpit_map_tools_sync" ||
      key === "viewport_stage_layout"
    ) {
      pushEvent(`layout:${key}`, detail);
    }
    scheduleFlush();
  }

  function recordMap(kind, detail = {}) {
    if (!isEnabled()) return;
    const key = String(kind || "").trim();
    const map = {
      full_render: "fullRender",
      render_start: "renderStart",
      runtime_error: "runtimeError",
    };
    const field = map[key];
    if (field) state.summary.map[field] += 1;
    if (typeof detail.instances === "number") {
      state.summary.map.instancesPeak = Math.max(
        state.summary.map.instancesPeak,
        detail.instances,
      );
    }
    pushEvent(`map:${key}`, detail);
    scheduleFlush();
  }

  function recordLongTask(durationMs, detail = {}) {
    if (!isEnabled()) return;
    const ms = Number(durationMs) || 0;
    if (ms < SLOW_LONG_TASK_MS) return;
    const perf = state.summary.perf;
    perf.longTaskCount += 1;
    perf.longTaskTotalMs += ms;
    perf.longTaskMaxMs = Math.max(perf.longTaskMaxMs, ms);
    if (ms >= 200) {
      pushAlert("long_task", { ms: Math.round(ms), ...detail });
    } else {
      pushEvent("long_task", { ms: Math.round(ms), ...detail });
    }
    scheduleFlush();
  }

  function sampleFps() {
    if (!isEnabled()) return;
    fpsFrames += 1;
    const elapsed = nowMs() - fpsWindowStart;
    if (elapsed < 1000) {
      window.requestAnimationFrame(sampleFps);
      return;
    }
    const fps = Math.round((fpsFrames * 1000) / Math.max(1, elapsed));
    fpsSum += fps;
    fpsSamples += 1;
    const perf = state.summary.perf;
    perf.fpsSamples = fpsSamples;
    perf.fpsAvg = Math.round(fpsSum / fpsSamples);
    perf.fpsMin = perf.fpsMin == null ? fps : Math.min(perf.fpsMin, fps);
    if (fps <= 10) {
      pushAlert("low_fps", { fps, gisPending, layout: { ...state.summary.layout } });
    }
    fpsFrames = 0;
    fpsWindowStart = nowMs();
    scheduleFlush();
    window.requestAnimationFrame(sampleFps);
  }

  function patchFetch() {
    if (typeof window.fetch !== "function" || window.fetch.__meiRuntimeDiagPatched) return;
    const nativeFetch = window.fetch.bind(window);
    function wrappedFetch(input, init) {
      const url =
        typeof input === "string"
          ? input
          : input && typeof input.url === "string"
            ? input.url
            : "";
      const trackGis =
        url.includes("/gis/") ||
        url.endsWith("/gis") ||
        url.includes("/workspace-components/vendor/maplibre/fonts/");
      if (trackGis) recordGisStart(url);
      const started = nowMs();
      return nativeFetch(input, init)
        .then((response) => {
          if (trackGis) {
            recordGisFinish(url, response.status, nowMs() - started);
          }
          return response;
        })
        .catch((error) => {
          if (trackGis) {
            recordGisFinish(url, 0, nowMs() - started, String(error?.message || error));
          }
          throw error;
        });
    }
    wrappedFetch.__meiRuntimeDiagPatched = true;
    window.fetch = wrappedFetch;
  }

  function installObservers() {
    if (typeof PerformanceObserver !== "function") return;
    try {
      const longTaskObserver = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          recordLongTask(entry.duration, {
            name: entry.name || "longtask",
            startTime: Math.round(entry.startTime || 0),
          });
        }
      });
      longTaskObserver.observe({ entryTypes: ["longtask"] });
    } catch (_) {
      /* longtask not supported */
    }
  }

  function countMapPausedInstances() {
    const bootApi = window.__meiLangBoot || {};
    const instances = bootApi.worldMapInstances;
    if (!instances || typeof instances.forEach !== "function") {
      return 0;
    }
    let paused = 0;
    instances.forEach((instance) => {
      if (instance?._mapPausedForWorldStage) paused += 1;
    });
    return paused;
  }

  function worldRendererActive() {
    const world = document.querySelector("mei-world-stage");
    return Boolean(world?._renderer);
  }

  function measureSessionStorageBytes() {
    let total = 0;
    try {
      for (let i = 0; i < sessionStorage.length; i += 1) {
        const key = sessionStorage.key(i);
        if (!key) continue;
        total += key.length + String(sessionStorage.getItem(key) || "").length;
      }
    } catch (_) {
      /* ignore */
    }
    return total;
  }

  async function auditSceneShellIdb() {
    if (typeof indexedDB === "undefined") {
      return { entries: 0, totalBytesEstimate: 0 };
    }
    return new Promise((resolve) => {
      try {
        const request = indexedDB.open(SCENE_SHELL_DB, 1);
        request.onerror = () => resolve({ entries: 0, totalBytesEstimate: 0 });
        request.onsuccess = () => {
          const db = request.result;
          if (!db.objectStoreNames.contains(SCENE_SHELL_STORE)) {
            try {
              db.close();
            } catch (_) {}
            resolve({ entries: 0, totalBytesEstimate: 0 });
            return;
          }
          const tx = db.transaction(SCENE_SHELL_STORE, "readonly");
          const getAll = tx.objectStore(SCENE_SHELL_STORE).getAll();
          getAll.onsuccess = () => {
            const rows = getAll.result || [];
            let totalBytesEstimate = 0;
            for (const row of rows) {
              try {
                totalBytesEstimate += JSON.stringify(row).length;
              } catch (_) {
                /* ignore */
              }
            }
            try {
              db.close();
            } catch (_) {}
            resolve({ entries: rows.length, totalBytesEstimate });
          };
          getAll.onerror = () => {
            try {
              db.close();
            } catch (_) {}
            resolve({ entries: 0, totalBytesEstimate: 0 });
          };
        };
      } catch (_) {
        resolve({ entries: 0, totalBytesEstimate: 0 });
      }
    });
  }

  async function auditStorage() {
    const idb = await auditSceneShellIdb();
    const sessionStorageBytes = measureSessionStorageBytes();
    let runtimeDiagBytes = 0;
    try {
      runtimeDiagBytes = String(sessionStorage.getItem(STORAGE_KEY) || "").length;
    } catch (_) {
      /* ignore */
    }
    return {
      sceneShellIdbEntries: idb.entries,
      sceneShellIdbBytesEst: idb.totalBytesEstimate,
      sessionStorageBytes,
      runtimeDiagBytes,
    };
  }

  function shouldAlert(kind, cooldownMs = 60000) {
    const now = Date.now();
    if (alertCooldown[kind] && now - alertCooldown[kind] < cooldownMs) {
      return false;
    }
    alertCooldown[kind] = now;
    return true;
  }

  function evaluateGpuAlerts(gpu) {
    const worldActive = document.documentElement.classList.contains("mei-world-stage-active");
    if (gpu.worldRendererActive && !worldActive && gpu.mapPausedCount === 0) {
      gpu.dualWebGLAlerts += 1;
      if (shouldAlert("dual_webgl")) {
        pushAlert("dual_webgl", {
          canvasCount: gpu.canvasCount,
          mapPausedCount: gpu.mapPausedCount,
        });
      }
    }
    if (gpu.canvasCount > Math.max(gpu.canvasPeak - 1, lastCanvasCount) + 1 && lastCanvasCount > 0) {
      canvasGrowthStreak += 1;
      if (canvasGrowthStreak >= 2 && shouldAlert("canvas_growth")) {
        pushAlert("canvas_growth", {
          canvasCount: gpu.canvasCount,
          canvasPeak: gpu.canvasPeak,
        });
        canvasGrowthStreak = 0;
      }
    } else {
      canvasGrowthStreak = 0;
    }
    lastCanvasCount = gpu.canvasCount;
    ensureSummaryShape();
    const w = state.summary.world;
    const m = state.summary.map;
    if (w.enterCount >= 5 && m.fullRender / w.enterCount > 2 && shouldAlert("full_render_burst")) {
      pushAlert("full_render_burst", {
        fullRender: m.fullRender,
        enterCount: w.enterCount,
      });
    }
    ensureSummaryShape();
    const st = state.summary.storage;
    if (st.sceneShellIdbBytesEst > SCENE_SHELL_BLOAT_BYTES && shouldAlert("scene_shell_bloat")) {
      pushAlert("scene_shell_bloat", {
        bytes: st.sceneShellIdbBytesEst,
      });
    }
  }

  async function collectSnapshot() {
    if (!isEnabled()) return null;
    ensureSummaryShape();
    const canvasCount = document.querySelectorAll("canvas").length;
    const gpu = state.summary.gpu;
    gpu.canvasCount = canvasCount;
    gpu.canvasPeak = Math.max(gpu.canvasPeak || 0, canvasCount);
    gpu.worldRendererActive = worldRendererActive();
    gpu.mapPausedCount = countMapPausedInstances();
    if (performance?.memory?.usedJSHeapSize) {
      gpu.jsHeapMb = Math.round(performance.memory.usedJSHeapSize / 1048576);
    }
    const storageAudit = await auditStorage();
    Object.assign(state.summary.storage, storageAudit);
    evaluateGpuAlerts(gpu);
    scheduleFlush();
    return {
      gpu: { ...gpu },
      world: { ...state.summary.world },
      storage: { ...state.summary.storage },
    };
  }

  function snapshot() {
    return collectSnapshot();
  }

  function exportReport() {
    flush();
    return {
      ...state,
      exportedAt: new Date().toISOString(),
      gisPending,
      hints: buildHints(),
    };
  }

  function buildHints() {
    const hints = [];
    const g = state.summary.gis;
    const l = state.summary.layout;
    const m = state.summary.map;
    const p = state.summary.perf;
    ensureSummaryShape();
    const gpu = state.summary.gpu;
    const w = state.summary.world;
    const st = state.summary.storage;
    if (g.doubleGis > 0) {
      hints.push(
        "检测到 /gis/gis 双前缀瓦片 URL，请检查 MEI_GIS_PROXY_UPSTREAM 是否指回 9527/gis。",
      );
    }
    if (g.fail > 12 && g.ok < g.fail) {
      hints.push("GIS 瓦片失败居多，Martin 可能未启动或代理上游不可达。");
    }
    if (g.pendingPeak > 80) {
      hints.push("GIS 并发峰值过高，可能引发连接池阻塞与页面假死。");
    }
    if (l.mapResizeObserver > 400 || l.cockpitMapToolsSync > 400) {
      hints.push("布局同步/地图 resize 次数异常，疑似 ResizeObserver 反馈环。");
    }
    if (m.fullRender > 10) {
      hints.push("地图多次全量重建，检查 preview-updated 是否导致 props 签名抖动。");
    }
    if (p.longTaskMaxMs >= 200) {
      hints.push("主线程长任务过多，可能是布局环或 MapLibre 样式同步阻塞。");
    }
    if (p.fpsMin != null && p.fpsMin <= 10) {
      hints.push("FPS 过低，可能是 GPU 压力（建筑挤出/高 pitch）或主线程阻塞。");
    }
    if (gpu.dualWebGLAlerts > 0) {
      hints.push("检测到双 WebGL 共存（3D 已退出但 Three.js 未释放或地图未 pause）。");
    }
    if (gpu.canvasPeak > 2) {
      hints.push(`页面 canvas 峰值 ${gpu.canvasPeak}，可能存在 WebGL 上下文泄漏。`);
    }
    if (w.enterCount >= 5 && m.fullRender / w.enterCount > 2) {
      hints.push("3D 切换导致地图全量重建过多，检查 props 签名或布局反馈环。");
    }
    if (st.sceneShellIdbBytesEst > SCENE_SHELL_BLOAT_BYTES) {
      hints.push("scene-shell IndexedDB 缓存超过 5MB，建议清理或降低保留条数。");
    }
    hints.push(
      "快检：(() => { const d = window.__meiBrowserRuntimeDiag; d?.snapshot?.(); return d?.dump?.(); })()",
    );
    return hints;
  }

  function dump() {
    const report = exportReport();
    console.group("[mei-runtime-diag] browser runtime report");
    console.log("hints:", report.hints);
    console.log("summary:", report.summary);
    console.log("alerts:", report.alerts);
    console.log("recentGisRequests:", report.recentGisRequests);
    console.log("full:", report);
    console.groupEnd();
    return report;
  }

  async function copy() {
    const report = exportReport();
    const text = JSON.stringify(report, null, 2);
    try {
      await navigator.clipboard.writeText(text);
      return { ok: true, bytes: text.length };
    } catch (error) {
      return { ok: false, error: String(error?.message || error), text };
    }
  }

  function reset() {
    state = emptyState();
    gisPending = 0;
    layoutBurst = { windowStart: nowMs(), count: 0 };
    layoutSyncBurst = { windowStart: nowMs(), count: 0, baseline: 0 };
    canvasGrowthStreak = 0;
    lastCanvasCount = 0;
    Object.keys(alertCooldown).forEach((key) => delete alertCooldown[key]);
    flush();
  }

  function installEventHooks() {
    window.addEventListener("meilang:viewport-stage-layout", () => {
      recordLayout("viewport_stage_layout");
    });
    window.addEventListener("meilang:preview-updated", () => {
      recordLayout("preview_updated");
    });
    window.addEventListener("mei:world-stage-entered", () => {
      ensureSummaryShape();
      state.summary.world.enterCount += 1;
      void collectSnapshot();
    });
    window.addEventListener("mei:world-stage-exited", () => {
      ensureSummaryShape();
      state.summary.world.exitCount += 1;
      void collectSnapshot();
    });
    window.addEventListener("beforeunload", () => flush());
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") flush();
    });
    if (!snapshotTimer) {
      snapshotTimer = window.setInterval(() => {
        if (document.visibilityState === "visible") {
          void collectSnapshot();
        }
      }, SNAPSHOT_INTERVAL_MS);
    }
  }

  loadState();
  ensureSummaryShape();
  if (isEnabled()) {
    patchFetch();
    installObservers();
    installEventHooks();
    window.requestAnimationFrame(sampleFps);
    pushEvent("diag_boot", { href: window.location.href });
    void collectSnapshot();
    scheduleFlush();
  }

  const api = {
    enabled: isEnabled,
    record: pushEvent,
    recordLayout,
    recordMap,
    recordGisStart,
    recordGisFinish,
    exportReport,
    dump,
    copy,
    reset,
    flush,
    snapshot,
    collectSnapshot,
    auditStorage,
    getState: () => state,
    storageKey: STORAGE_KEY,
  };

  window.__meiBrowserRuntimeDiag = api;
  boot.browserRuntimeDiag = api;
})();
