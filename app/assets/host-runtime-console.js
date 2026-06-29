/**
 * mei-host-shell runtime management console: categorized left nav + detail panels.
 */
(function (global) {
  "use strict";

  const STATUS_URL = "/api/host/ops/status";
  const RELOAD_URL = "/api/host/ops/reload";
  const PREBUILD_URL = "/api/host/ops/prebuild";
  const SNAPSHOT_URL = "/api/runtime/snapshot";

  let pollTimer = null;
  let snapshotCache = null;
  let opsCache = null;

  function appIdFromShell() {
    const shell = document.querySelector("[data-runtime-node][data-app-path], .shell[data-app-path]");
    return shell ? String(shell.getAttribute("data-app-path") || "").trim() : "";
  }

  function activeNodeFromUrl() {
    try {
      return new URL(global.location.href).searchParams.get("node") || "";
    } catch (_error) {
      return "";
    }
  }

  function resolveActiveNode() {
    const fromUrl = activeNodeFromUrl();
    if (fromUrl) return fromUrl;
    const fromShell = activeNodeFromShell();
    if (fromShell && fromShell !== "overview-host") return fromShell;
    return "mgmt-app-state";
  }

  function syncShellNodeFromUrl() {
    const nodeId = activeNodeFromUrl();
    if (!nodeId) return;
    const shell = document.querySelector("[data-runtime-node]");
    if (shell) shell.setAttribute("data-runtime-node", nodeId);
  }

  function activeNodeFromShell() {
    const shell = document.querySelector("[data-runtime-node]");
    return shell ? String(shell.getAttribute("data-runtime-node") || "").trim() : "";
  }

  function runtimeHref(appId, nodeId) {
    const base = `/apps/runtime/${encodeURIComponent(appId)}`;
    if (!nodeId) return base;
    return `${base}?node=${encodeURIComponent(nodeId)}`;
  }

  function navigateToNode(nodeId, options) {
    const appId = appIdFromShell();
    if (!appId || !nodeId) return;
    const url = runtimeHref(appId, nodeId);
    const shell = document.querySelector("[data-runtime-node]");
    if (shell) shell.setAttribute("data-runtime-node", nodeId);
    if (options && options.replace) {
      global.history.replaceState({ hostRuntimeNode: nodeId }, "", url);
    } else {
      global.history.pushState({ hostRuntimeNode: nodeId }, "", url);
    }
    if (opsCache && snapshotCache) {
      paintConsole(appId, opsCache, snapshotCache);
    }
  }

  function phaseLabel(phase) {
    return (
      {
        starting: "启动中",
        bound: "已绑定（MCG）",
        ready: "就绪（含 MRG）",
      }[phase] || phase || "-"
    );
  }

  async function fetchOps() {
    const res = await fetch(STATUS_URL, { headers: { Accept: "application/json" } });
    if (!res.ok) return null;
    const data = await res.json();
    return data && data.hostShellOps ? data : null;
  }

  async function fetchSnapshot(appId) {
    const res = await fetch(`${SNAPSHOT_URL}?appId=${encodeURIComponent(appId)}`, {
      headers: { Accept: "application/json" },
    });
    if (!res.ok) return null;
    const data = await res.json();
    return data && data.hostShellMgmt ? data : null;
  }

  function navLink(appId, nodeId, label, meta, activeNode) {
    const cls =
      activeNode === nodeId
        ? "host-runtime-nav-link is-active"
        : "host-runtime-nav-link";
    const metaHtml = meta ? `<span class="host-runtime-nav-meta">${meta}</span>` : "";
    return `<a class="${cls}" href="${runtimeHref(appId, nodeId)}" data-runtime-node="${nodeId}"><span>${label}</span>${metaHtml}</a>`;
  }

  function renderNav(appId, snapshot, activeNode) {
    const slotCount = snapshot?.diagnostics?.mrg?.slotCount ?? snapshot?.slots?.length ?? 0;
    const routes = Array.isArray(snapshot?.scopeRoutes) ? snapshot.scopeRoutes : [];
    const scopeLinks = routes
      .map((route) =>
        navLink(
          appId,
          `mgmt-mrg-scope:${route.sceneId}`,
          `scene · ${route.sceneId}`,
          "入口",
          activeNode,
        ),
      )
      .join("");
    return `
<nav class="host-runtime-nav" aria-label="Host 管理导航">
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">运行与版本</div>
    ${navLink(appId, "mgmt-version", "运行版本", null, activeNode)}
    ${navLink(appId, "mgmt-app-state", "应用状态", null, activeNode)}
  </div>
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">MRG</div>
    ${navLink(appId, "mgmt-mrg", "MRG 总览", String(slotCount), activeNode)}
    ${scopeLinks}
    ${navLink(appId, "mgmt-mrg-slots", "求值 slots", null, activeNode)}
  </div>
</nav>`;
  }

  function renderVersionDetail(ops, snapshot) {
    const binary = ops?.binary || {};
    const env = ops?.env || snapshot?.diagnostics?.build || {};
    const version = ops?.version || {};
    const workspace = version.workspace || {};
    return `
<section class="host-runtime-detail-panel">
  <header class="host-runtime-detail-head">
    <h2 class="host-runtime-detail-title">运行版本</h2>
    <p class="host-runtime-detail-sub">${ops?.displayLabel || ""}</p>
  </header>
  <dl class="host-runtime-kv">
    <div><dt>Shell build</dt><dd>${binary.build_version || "-"}</dd></div>
    <div><dt>Cargo package</dt><dd>${binary.cargo_package_version || "-"}</dd></div>
    <div><dt>Toolchain</dt><dd>${ops?.toolchain?.active || env.toolchainVersion || "-"}</dd></div>
    <div><dt>Workspace</dt><dd>${ops?.workspaceVersion || env.workspaceVersion || "-"}</dd></div>
    <div><dt>Env active</dt><dd>${ops?.env?.active || env.envActive || "-"}</dd></div>
    <div><dt>Env candidate</dt><dd>${ops?.env?.candidate || "-"}</dd></div>
    <div><dt>Env previous</dt><dd>${ops?.env?.previous || "-"}</dd></div>
    <div><dt>Git</dt><dd>${binary.git?.commit_short || "-"} @ ${binary.git?.branch || "-"}</dd></div>
  </dl>
</section>`;
  }

  function jobLine(ops) {
    const job = ops?.job;
    const lastJob = ops?.lastJob;
    if (job) return `<p class="host-shell-ops-job">进行中：${job.kind} …</p>`;
    if (!lastJob) return "";
    return `<p class="host-shell-ops-job">最近：${lastJob.kind} · ${lastJob.status}${
      lastJob.message ? " · " + lastJob.message : ""
    }${lastJob.error ? " · " + lastJob.error : ""}</p>`;
  }

  function renderAppStateDetail(ops, snapshot) {
    const host = snapshot?.host || {};
    return `
<section class="host-runtime-detail-panel">
  <header class="host-runtime-detail-head">
    <h2 class="host-runtime-detail-title">应用状态</h2>
    <p class="host-runtime-detail-sub">phase · ${phaseLabel(ops?.phase || host.phase)}</p>
  </header>
  <div class="host-shell-ops-badges">
    <span class="host-shell-ops-badge ${ops?.accessReady ? "is-ready" : ""}">accessReady: ${ops?.accessReady ? "是" : "否"}</span>
    <span class="host-shell-ops-badge ${ops?.warmupReady ? "is-ready" : ""}">warmupReady: ${ops?.warmupReady ? "是" : "否"}</span>
  </div>
  ${jobLine(ops)}
  <div class="host-runtime-actions">
    <button type="button" class="mei-btn mei-btn--sm" data-host-ops="refresh">刷新状态</button>
    <button type="button" class="mei-btn mei-btn--sm" data-host-ops="reload">重新加载（compile + import）</button>
    <button type="button" class="mei-btn mei-btn--sm mei-btn--accent" data-host-ops="prebuild">完整预构建</button>
  </div>
  <p class="host-runtime-hint mei-font-1 mei-text-muted">停止宿主请在工作区执行 <code>./deploy/stop.sh</code>；host-shell 不提供进程内「关闭」API。</p>
</section>`;
  }

  function renderMrgOverview(snapshot) {
    const mrg = snapshot?.diagnostics?.mrg || {};
    const status = snapshot?.mrgStatus || {};
    const tiers = status.slotsByTier || {};
    return `
<section class="host-runtime-detail-panel">
  <header class="host-runtime-detail-head">
    <h2 class="host-runtime-detail-title">MRG 总览</h2>
  </header>
  <dl class="host-runtime-kv">
    <div><dt>slots</dt><dd>${mrg.slotCount ?? 0}</dd></div>
    <div><dt>ready</dt><dd>${mrg.readySlots ?? 0}</dd></div>
    <div><dt>stale</dt><dd>${mrg.staleSlots ?? 0}</dd></div>
    <div><dt>failed</dt><dd>${mrg.failedSlots ?? 0}</dd></div>
    <div><dt>diskReady</dt><dd>${tiers.diskReady ?? "-"}</dd></div>
    <div><dt>memoryResident</dt><dd>${tiers.memoryResident ?? "-"}</dd></div>
    <div><dt>clientEligible</dt><dd>${tiers.clientEligible ?? "-"}</dd></div>
    <div><dt>edges</dt><dd>${status.edgeCount ?? mrg.navigationNodeCount ?? 0}</dd></div>
  </dl>
</section>`;
  }

  function renderMrgScopeDetail(snapshot, sceneId, appId) {
    const slots = (snapshot?.slots || []).filter((slot) => slot.scopeKey === sceneId);
    const route = (snapshot?.scopeRoutes || []).find((item) => item.sceneId === sceneId);
    const accessUrl = route?.url || `/apps/app/${appId}/scene/${sceneId}`;
    const rows = slots
      .map(
        (slot) =>
          `<tr><td>${slot.nodeKey || "-"}</td><td>${slot.state || "-"}</td><td>${slot.residentTier || "-"}</td><td>${slot.clientEligible ? "是" : "否"}</td></tr>`,
      )
      .join("");
    return `
<section class="host-runtime-detail-panel">
  <header class="host-runtime-detail-head">
    <h2 class="host-runtime-detail-title">MRG · ${sceneId}</h2>
    <a class="build-toolbar-btn inline-flex w-fit" href="${accessUrl}">打开访问入口</a>
  </header>
  <p class="mei-font-1 mei-text-muted">scope 下 ${slots.length} 个 slot；下表为求值/预热状态摘要。</p>
  <div class="host-runtime-table-wrap">
    <table class="host-runtime-table">
      <thead><tr><th>节点</th><th>状态</th><th>Tier</th><th>Client</th></tr></thead>
      <tbody>${rows || `<tr><td colspan="4">暂无 slot</td></tr>`}</tbody>
    </table>
  </div>
</section>`;
  }

  function renderMrgSlotsDetail(snapshot) {
    const slots = Array.isArray(snapshot?.slots) ? snapshot.slots : [];
    const rows = slots
      .map(
        (slot) =>
          `<tr><td>${slot.scopeKey || "-"}</td><td>${slot.nodeKey || "-"}</td><td>${slot.state || "-"}</td><td>${slot.residentTier || "-"}</td><td>${slot.ownerResourceId || "-"}</td></tr>`,
      )
      .join("");
    return `
<section class="host-runtime-detail-panel">
  <header class="host-runtime-detail-head">
    <h2 class="host-runtime-detail-title">MRG 求值清单</h2>
    <p class="host-runtime-detail-sub">共 ${slots.length} 个 slot</p>
  </header>
  <div class="host-runtime-table-wrap">
    <table class="host-runtime-table">
      <thead><tr><th>Scope</th><th>节点</th><th>状态</th><th>Tier</th><th>Resource</th></tr></thead>
      <tbody>${rows || `<tr><td colspan="5">暂无 slot</td></tr>`}</tbody>
    </table>
  </div>
</section>`;
  }

  function renderDetail(activeNode, appId, ops, snapshot) {
    if (activeNode === "mgmt-version") return renderVersionDetail(ops, snapshot);
    if (activeNode === "mgmt-app-state") return renderAppStateDetail(ops, snapshot);
    if (activeNode === "mgmt-mrg") return renderMrgOverview(snapshot);
    if (activeNode === "mgmt-mrg-slots") return renderMrgSlotsDetail(snapshot);
    if (activeNode.startsWith("mgmt-mrg-scope:")) {
      return renderMrgScopeDetail(snapshot, activeNode.slice("mgmt-mrg-scope:".length), appId);
    }
    return renderAppStateDetail(ops, snapshot);
  }

  function setBusy(root, busy) {
    if (!root) return;
    root.querySelectorAll("[data-host-ops]").forEach((btn) => {
      if (btn.getAttribute("data-host-ops") === "refresh") return;
      btn.disabled = busy;
    });
  }

  async function postReload(root) {
    setBusy(root, true);
    try {
      const res = await fetch(RELOAD_URL, { method: "POST" });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body.error || res.statusText || "reload failed");
      global.location.reload();
    } catch (err) {
      global.alert(String(err && err.message ? err.message : err));
    } finally {
      setBusy(root, false);
    }
  }

  async function postPrebuild(root) {
    if (
      !global.confirm(
        "将执行完整 prebuild（compile + import + data + warmup + finalize），可能需要较长时间。继续？",
      )
    ) {
      return;
    }
    setBusy(root, true);
    try {
      const res = await fetch(PREBUILD_URL, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ policy: "standard" }),
      });
      const body = await res.json().catch(() => ({}));
      if (!res.ok && res.status !== 202) {
        throw new Error(body.error || res.statusText || "prebuild failed");
      }
      schedulePoll(root);
    } catch (err) {
      global.alert(String(err && err.message ? err.message : err));
    } finally {
      setBusy(root, false);
    }
  }

  function bindDetailActions(detailRoot, ops) {
    if (!detailRoot) return;
    detailRoot.querySelectorAll("[data-host-ops]").forEach((btn) => {
      if (btn.__hostRuntimeOpsBound) return;
      btn.__hostRuntimeOpsBound = true;
      btn.addEventListener("click", () => {
        const action = btn.getAttribute("data-host-ops");
        if (action === "reload") postReload(detailRoot);
        else if (action === "prebuild") postPrebuild(detailRoot);
        else if (action === "refresh") refreshConsole();
      });
    });
    if (ops?.job && ops.job.status === "running") {
      setBusy(detailRoot, true);
    }
  }

  function hideLegacyPanels() {
    document.querySelectorAll("[data-host-runtime-legacy-tree]").forEach((el) => {
      el.hidden = true;
    });
    document.querySelectorAll("[data-host-runtime-legacy-overview]").forEach((el) => {
      el.hidden = true;
    });
    const workspace = document.querySelector(".runtime-workspace");
    if (workspace) workspace.classList.add("host-runtime-console-active");
  }

  function paintConsole(appId, ops, snapshot) {
    const activeNode = resolveActiveNode();
    const navMount = document.getElementById("host-runtime-nav-mount");
    const detailMount = document.getElementById("host-runtime-detail-mount");
    if (!navMount || !detailMount) return;
    hideLegacyPanels();
    navMount.innerHTML = renderNav(appId, snapshot, activeNode);
    detailMount.innerHTML = renderDetail(activeNode, appId, ops, snapshot);
    bindDetailActions(detailMount, ops);
  }

  async function refreshConsole() {
    const appId = appIdFromShell();
    if (!appId) return false;
    const ops = await fetchOps();
    if (!ops) return false;
    opsCache = ops;
    const snapshot = await fetchSnapshot(appId);
    if (!snapshot) return false;
    snapshotCache = snapshot;
    paintConsole(appId, ops, snapshot);
    if (ops.job && ops.job.status === "running") {
      schedulePoll(document.getElementById("host-runtime-detail-mount"));
    } else if (pollTimer) {
      clearTimeout(pollTimer);
      pollTimer = null;
    }
    return true;
  }

  function schedulePoll(detailRoot) {
    if (pollTimer) return;
    pollTimer = global.setTimeout(async () => {
      pollTimer = null;
      const hadJob = opsCache?.job?.status === "running";
      await refreshConsole();
      if (hadJob && opsCache && !opsCache.job) {
        document.getElementById("runtime-refresh-btn")?.click();
        if (opsCache.lastJob?.status === "success") {
          global.setTimeout(() => global.location.reload(), 800);
        }
      }
      if (opsCache?.job?.status === "running") {
        schedulePoll(detailRoot);
      }
    }, 1500);
  }

  async function initHostRuntimeConsole() {
    if (!document.querySelector(".runtime-workspace")) return;
    syncShellNodeFromUrl();
    const ok = await refreshConsole();
    if (!ok) return;
    if (!activeNodeFromUrl()) {
      navigateToNode("mgmt-app-state", { replace: true });
    }
  }

  function bindHostRuntimeNavCapture() {
    if (document.__hostRuntimeNavCaptureBound) return;
    document.__hostRuntimeNavCaptureBound = true;
    document.addEventListener(
      "click",
      (event) => {
        const link = event.target.closest(".host-runtime-nav-link");
        if (!link || !document.getElementById("host-runtime-nav-mount")?.contains(link)) {
          return;
        }
        event.preventDefault();
        event.stopImmediatePropagation();
        const nodeId = link.getAttribute("data-runtime-node");
        if (nodeId) navigateToNode(nodeId);
      },
      true,
    );
  }

  bindHostRuntimeNavCapture();

  global.addEventListener("popstate", () => {
    if (!document.querySelector(".runtime-workspace")) return;
    syncShellNodeFromUrl();
    if (opsCache && snapshotCache) {
      paintConsole(appIdFromShell(), opsCache, snapshotCache);
    } else {
      void initHostRuntimeConsole();
    }
  });

  global.addEventListener("meilang:preview-updated", () => {
    if (document.getElementById("host-runtime-nav-mount")) {
      void initHostRuntimeConsole();
    }
  });

  global.MeiHostRuntimeConsole = { refreshConsole, initHostRuntimeConsole, navigateToNode };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initHostRuntimeConsole);
  } else {
    initHostRuntimeConsole();
  }
})(typeof window !== "undefined" ? window : globalThis);
