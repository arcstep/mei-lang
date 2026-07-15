(function initRuntimeObservatoryRefresh() {
  const shell = document.querySelector("[data-runtime-node]");
  if (!shell) return;
  const appPath = shell.getAttribute("data-app-path") || "";
  const refreshBtn = document.getElementById("runtime-refresh-btn");
  let timer = null;

  function formatBytes(bytes) {
    const value = Number(bytes) || 0;
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
    if (value < 1024 * 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MiB`;
    return `${(value / 1024 / 1024 / 1024).toFixed(2)} GiB`;
  }

  function msLabel(value) {
    return value == null ? "-" : `${value}ms`;
  }

  function gateCount(host, sweep) {
    return host == null ? String(sweep || 0) : String(host);
  }

  function metricRow(label, value) {
    return `<div class="flex justify-between gap-2"><dt class="mei-text-muted">${label}</dt><dd>${value}</dd></div>`;
  }

  function renderLayerMetrics(payload) {
    const host = payload.host || {};
    const prebuild = payload.prebuild || {};
    const diag = payload.diagnostics || {};
    const disk = diag.disk || {};
    const evalStats = diag.eval || {};
    const mcg = diag.mcg || {};
    const mrg = diag.mrg || {};
    const cache = diag.cache || {};
    const build = diag.build || {};
    const sweep = diag.scopeGateSweep || {};
    const contentStore = diag.contentStore || {};
    return `
      <section class="rounded-lg border border-white/10 bg-black/10 p-3">
        <h3 class="mb-2 mei-font-2 mei-text-primary">宿主 / 构建</h3>
        <dl class="grid gap-1 mei-font-1">
          ${metricRow("宿主 phase", host.phase || "-")}
          ${metricRow("App phase", host.appPhase || "-")}
          ${metricRow("access_ready", String(Boolean(host.accessReady)))}
          ${metricRow("wall", msLabel(prebuild.totalWallMs))}
          ${metricRow("compile", msLabel(prebuild.compileScopesMs))}
          ${metricRow("peak RSS", prebuild.peakRssBytes == null ? "-" : formatBytes(prebuild.peakRssBytes))}
          ${metricRow("current RSS", prebuild.currentRssBytes == null ? "-" : formatBytes(prebuild.currentRssBytes))}
        </dl>
      </section>
      <section class="rounded-lg border border-white/10 bg-black/10 p-3">
        <h3 class="mb-2 mei-font-2 mei-text-primary">L1 · Cache</h3>
        <dl class="grid gap-1 mei-font-1">
          ${metricRow("dedup", String(Boolean(cache.graphRegistryDedup)))}
          ${metricRow("compile_index", `${build.compileIndexEntries || 0} / ${build.compileIndexHits || 0} hit`)}
          ${metricRow("eval files", String(evalStats.evalTotalFiles || 0))}
          ${metricRow("eval bytes", formatBytes(evalStats.evalTotalBytes || 0))}
          ${metricRow("CAS store", formatBytes(contentStore.bytes || 0))}
        </dl>
      </section>
      <section class="rounded-lg border border-white/10 bg-black/10 p-3">
        <h3 class="mb-2 mei-font-2 mei-text-primary">L2 · Navigation</h3>
        <dl class="grid gap-1 mei-font-1">
          ${metricRow("L2 miss", gateCount(host.gateL2Miss, sweep.l2Miss))}
          ${metricRow("nav nodes", String(mrg.navigationNodeCount || 0))}
          ${metricRow("nav dup", String(mrg.navigationDuplicateKeys || 0))}
        </dl>
      </section>
      <section class="rounded-lg border border-white/10 bg-black/10 p-3">
        <h3 class="mb-2 mei-font-2 mei-text-primary">L3 · Assembly</h3>
        <dl class="grid gap-1 mei-font-1">
          ${metricRow("MCG nodes", String(mcg.nodeCount || 0))}
          ${metricRow("scene_payload", String(mcg.scenePayloadNodes || 0))}
          ${metricRow("L3 fail", gateCount(host.gateL3Fail, sweep.l3Fail))}
          ${metricRow("payload disk", formatBytes(disk.scenePayloadBytes || 0))}
        </dl>
      </section>
      <section class="rounded-lg border border-white/10 bg-black/10 p-3">
        <h3 class="mb-2 mei-font-2 mei-text-primary">L4 · Materialization</h3>
        <dl class="grid gap-1 mei-font-1">
          ${metricRow("slots", `${mrg.readySlots || 0} ready / ${mrg.failedSlots || 0} fail`)}
          ${metricRow("stale", `${mrg.staleSlots || 0} (${Math.round((mrg.staleRatio || 0) * 100)}%)`)}
          ${metricRow("L4 stale", gateCount(host.gateL4Stale, sweep.l4Stale))}
          ${metricRow("data snapshots", formatBytes(disk.dataSnapshotsBytes || 0))}
        </dl>
      </section>
      <section class="rounded-lg border border-white/10 bg-black/10 p-3">
        <h3 class="mb-2 mei-font-2 mei-text-primary">磁盘</h3>
        <dl class="grid gap-1 mei-font-1">
          ${metricRow("app_root", formatBytes(disk.appRootBytes || 0))}
          ${metricRow("compiled_app", `${disk.compiledAppFileCount || 0} / ${formatBytes(disk.compiledAppBytes || 0)}`)}
          ${metricRow("graph", formatBytes(disk.graphBytes || 0))}
          ${metricRow("prebuild", formatBytes(disk.prebuildBytes || 0))}
        </dl>
      </section>`;
  }

  function applySnapshot(payload) {
    if (!payload || typeof payload !== "object") return;
    const rootsScript = document.getElementById("mei-runtime-observability-tree");
    if (rootsScript && Array.isArray(payload.roots)) {
      rootsScript.textContent = JSON.stringify(payload.roots);
    }
    const snapshotScript = document.getElementById("mei-runtime-observability-snapshot");
    if (snapshotScript) {
      snapshotScript.textContent = JSON.stringify(payload);
    }
    const metricsHost = document.getElementById("runtime-layer-metrics");
    if (metricsHost) {
      metricsHost.innerHTML = renderLayerMetrics(payload);
    }
    const loading = document.getElementById("runtime-layer-metrics-loading");
    if (loading) loading.remove();
    const detail = document.getElementById("runtime-detail-json");
    if (detail) {
      detail.textContent = JSON.stringify(payload, null, 2);
    }
  }

  async function refreshSnapshot() {
    if (!appPath) return;
    const url = `/api/runtime/snapshot?appId=${encodeURIComponent(appPath)}`;
    try {
      const response = await fetch(url, {
        headers: { Accept: "application/json" },
        credentials: "same-origin",
      });
      if (!response.ok) return;
      const payload = await response.json();
      applySnapshot(payload);
    } catch (_error) {
      /* ignore transient refresh errors */
    }
  }

  if (refreshBtn) {
    refreshBtn.addEventListener("click", () => {
      refreshSnapshot();
    });
  }

  refreshSnapshot();
  timer = window.setInterval(refreshSnapshot, 5000);
  window.addEventListener(
    "pagehide",
    () => {
      if (timer) window.clearInterval(timer);
    },
    { once: true },
  );
})();
