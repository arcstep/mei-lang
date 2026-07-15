/**
 * host-shell runtime checkbench:
 * - grouped nav: Host / Navigation / Scope Gate / Warmup / MRG / Cache / Diagnostics
 * - contextual detail panel
 * - current-node JSON vs full-snapshot JSON
 */
(function (global) {
  "use strict";

  const STATUS_URL = "/api/host/ops/status";
  const RELOAD_URL = "/api/host/ops/reload";
  const PREBUILD_URL = "/api/host/ops/prebuild";
  const ACTIVATE_SCOPE_URL = "/api/host/mrg/activate";
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

  function activeNodeFromShell() {
    const shell = document.querySelector("[data-runtime-node]");
    return shell ? String(shell.getAttribute("data-runtime-node") || "").trim() : "";
  }

  function defaultNodeId(snapshot) {
    const scopeId = snapshot?.scopes?.[0]?.nodeId;
    return scopeId || "ops:overview";
  }

  function resolveActiveNode(snapshot) {
    const fromUrl = activeNodeFromUrl();
    if (fromUrl) return fromUrl;
    const fromShell = activeNodeFromShell();
    if (fromShell && fromShell !== "overview-host") return fromShell;
    return defaultNodeId(snapshot);
  }

  function syncShellNodeFromUrl() {
    const nodeId = activeNodeFromUrl();
    if (!nodeId) return;
    const shell = document.querySelector("[data-runtime-node]");
    if (shell) shell.setAttribute("data-runtime-node", nodeId);
  }

  function runtimeHref(appId, nodeId) {
    const base = `/runtime?app=${encodeURIComponent(appId)}`;
    if (!nodeId) return base;
    return `${base}&node=${encodeURIComponent(nodeId)}`;
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
        bound: "已绑定（compile/import 完成，warmup 未就绪）",
        ready: "就绪（MRG / warmup 可观测）",
      }[phase] || phase || "-"
    );
  }

  function boolText(value) {
    return value ? "是" : "否";
  }

  function escapeHtml(value) {
    return String(value == null ? "" : value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function prettyJson(value) {
    try {
      return JSON.stringify(value == null ? {} : value, null, 2);
    } catch (_error) {
      return String(value);
    }
  }

  function formatMaybe(value) {
    if (value == null || value === "") return "-";
    if (typeof value === "boolean") return boolText(value);
    if (typeof value === "number") return String(value);
    if (Array.isArray(value)) return value.length ? value.join(", ") : "-";
    if (typeof value === "object") return prettyJson(value);
    return String(value);
  }

  function formatBytes(bytes) {
    if (typeof bytes !== "number" || !Number.isFinite(bytes)) return "-";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GiB`;
  }

  function formatMs(ms) {
    return typeof ms === "number" && Number.isFinite(ms) ? `${ms}ms` : "-";
  }

  function statusBadge(label, ok) {
    return `<span class="host-shell-ops-badge ${ok ? "is-ready" : ""}">${escapeHtml(label)}</span>`;
  }

  function hintBlock(lines) {
    const items = (lines || [])
      .filter(Boolean)
      .map((line) => `<li>${escapeHtml(line)}</li>`)
      .join("");
    if (!items) return "";
    return `
<section class="rounded-lg border border-white/10 bg-black/10 p-3">
  <h3 class="mb-2 mei-font-2 mei-text-primary">排障提示</h3>
  <ul class="grid gap-1 mei-font-1 mei-text-muted">${items}</ul>
</section>`;
  }

  function kvTable(rows) {
    return `<dl class="host-runtime-kv">${rows
      .map(([label, value]) => `<div><dt>${escapeHtml(label)}</dt><dd>${value}</dd></div>`)
      .join("")}</dl>`;
  }

  function shellLink(nodeId, label, meta, activeNode, extraClass) {
    const appId = appIdFromShell();
    const cls = [extraClass || "host-runtime-nav-link", activeNode === nodeId ? "is-active" : ""]
      .filter(Boolean)
      .join(" ");
    const metaHtml = meta ? `<span class="host-runtime-nav-meta">${escapeHtml(meta)}</span>` : "";
    return `<a class="${cls}" href="${runtimeHref(appId, nodeId)}" data-runtime-node="${escapeHtml(
      nodeId,
    )}" data-runtime-node-link="1"><span>${escapeHtml(label)}</span>${metaHtml}</a>`;
  }

  function allScopes(snapshot) {
    return Array.isArray(snapshot?.scopes) ? snapshot.scopes : [];
  }

  function allRoutes(snapshot) {
    return Array.isArray(snapshot?.navigation?.routes) ? snapshot.navigation.routes : [];
  }

  function allSlots(snapshot) {
    return Array.isArray(snapshot?.mrg?.slots) ? snapshot.mrg.slots : Array.isArray(snapshot?.slots) ? snapshot.slots : [];
  }

  function findScope(snapshot, scopeKey) {
    return allScopes(snapshot).find((scope) => scope.scopeKey === scopeKey) || null;
  }

  function findRoute(snapshot, nodeId) {
    return allRoutes(snapshot).find((route) => route.nodeId === nodeId) || null;
  }

  function findSlot(snapshot, nodeId) {
    return allSlots(snapshot).find((slot) => slot.nodeId === nodeId) || null;
  }

  function routesOfScope(snapshot, scopeKey) {
    return allRoutes(snapshot).filter((route) => route.scopeKey === scopeKey);
  }

  function slotsOfScope(snapshot, scopeKey) {
    return allSlots(snapshot).filter((slot) => slot.scopeKey === scopeKey);
  }

  function scopeStatus(scope) {
    const blockers = Array.isArray(scope?.blockers) ? scope.blockers : [];
    if (!scope) return "unknown";
    if (blockers.length === 0) return "ready";
    if ((scope.failedSlots || 0) > 0 || (scope.staleSlots || 0) > 0) return "degraded";
    if ((scope.routeCount || 0) === 0) return "missing";
    return "warming";
  }

  function detailSection(title, bodyHtml, subtext) {
    return `
<section class="host-runtime-detail-panel">
  <header class="host-runtime-detail-head">
    <div>
      <h2 class="host-runtime-detail-title">${escapeHtml(title)}</h2>
      ${subtext ? `<p class="host-runtime-detail-sub">${escapeHtml(subtext)}</p>` : ""}
    </div>
  </header>
  ${bodyHtml}
</section>`;
  }

  function renderNav(appId, snapshot, activeNode) {
    const scopes = allScopes(snapshot);
    const warmupScopes = scopes
      .filter((scope) => (scope.dirtySlots || 0) > 0)
      .map((scope) =>
        shellLink(
          `warmup:scope:${scope.scopeKey}`,
          `scope · ${scope.scopeKey}`,
          `${scope.dirtySlots || 0} dirty`,
          activeNode,
        ),
      )
      .join("");
    const scopeLinks = scopes
      .map((scope) =>
        shellLink(
          `scope:${scope.scopeKey}`,
          `scope · ${scope.scopeKey}`,
          `${scope.routeCount || 0} routes · ${scope.slotCount || 0} slots`,
          activeNode,
        ),
      )
      .join("");
    return `
<nav class="host-runtime-nav" aria-label="Host 运行检查台导航">
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">Host / Ops</div>
    ${shellLink("ops:overview", "运行状态", null, activeNode)}
    ${shellLink("ops:versions", "运行版本", null, activeNode)}
  </div>
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">Navigation</div>
    ${shellLink("nav:summary", "入口总览", `${snapshot?.navigation?.routeCount || 0} routes`, activeNode)}
    ${scopeLinks}
  </div>
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">Scope Gate</div>
    ${shellLink("gate:summary", "就绪判定", `${snapshot?.scopeGate?.degradedScopes?.length || 0} degraded`, activeNode)}
  </div>
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">Warmup</div>
    ${shellLink("warmup:summary", "预热计划", `${snapshot?.warmup?.dirtySlotCount || 0} dirty`, activeNode)}
    ${warmupScopes}
  </div>
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">MRG</div>
    ${shellLink("mrg:summary", "MRG 总览", `${snapshot?.mrg?.slotCount || 0} slots`, activeNode)}
    ${shellLink("mrg:failed", "失败 slots", `${snapshot?.mrg?.failedSlots?.length || 0}`, activeNode)}
    ${shellLink("mrg:slots", "全部 slots", null, activeNode)}
  </div>
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">Cache / Artifact</div>
    ${shellLink("cache:summary", "缓存与产物", null, activeNode)}
  </div>
  <div class="host-runtime-nav-section">
    <div class="host-runtime-nav-heading">Diagnostics</div>
    ${shellLink("diag:summary", "诊断与告警", `${snapshot?.diagnostics?.alerts?.length || 0} alerts`, activeNode)}
  </div>
</nav>`;
  }

  function renderVersionDetail(ops, snapshot) {
    const binary = ops?.binary || {};
    const build = snapshot?.diagnostics?.build || {};
    const meilang =
      ops?.version?.meilangVersion ||
      ops?.version?.workspace?.meilangVersion ||
      binary.meilangVersion ||
      binary.cargo_package_version ||
      "-";
    const buildGeneration =
      ops?.version?.buildGeneration ||
      ops?.version?.workspace?.buildGeneration ||
      build.buildGeneration ||
      "-";
    const buildDisplayTag =
      ops?.version?.buildDisplayTag ||
      ops?.version?.workspace?.buildDisplayTag ||
      (buildGeneration !== "-" ? `Build ${buildGeneration}` : "-");
    return detailSection(
      "运行版本",
      kvTable([
        ["MeiLang", escapeHtml(meilang)],
        ["Build generation", escapeHtml(buildGeneration)],
        ["Shell build", escapeHtml(binary.build_version || "-")],
        ["Build tag", escapeHtml(buildDisplayTag)],
        ["Workspace date", escapeHtml(ops?.workspaceVersion || build.workspaceVersion || "-")],
        ["Env active (ops)", escapeHtml(ops?.env?.active || build.envActive || "-")],
        ["Env candidate", escapeHtml(ops?.env?.candidate || "-")],
        ["Env previous", escapeHtml(ops?.env?.previous || "-")],
        ["Git", escapeHtml(`${binary.git?.commit_short || "-"} @ ${binary.git?.branch || "-"}`)],
      ]) +
        hintBlock([
          "MeiLang 版本表示工具链；Build generation 表示当前工作区构建代。",
          "Env active 为内部目录指针，供运维诊断使用。",
        ]),
      ops?.version?.displayLabel || ops?.displayLabel || "",
    );
  }

  function jobLine(ops) {
    const job = ops?.job;
    const lastJob = ops?.lastJob;
    if (job) {
      return `<p class="host-shell-ops-job">进行中：${escapeHtml(job.kind)} · ${escapeHtml(
        job.status || "running",
      )}</p>`;
    }
    if (!lastJob) return "";
    const details = [lastJob.message, lastJob.error].filter(Boolean).join(" · ");
    return `<p class="host-shell-ops-job">最近：${escapeHtml(lastJob.kind || "-")} · ${escapeHtml(
      lastJob.status || "-",
    )}${details ? ` · ${escapeHtml(details)}` : ""}</p>`;
  }

  function renderOpsDetail(ops, snapshot) {
    const gate = snapshot?.scopeGate || {};
    const defaultScope = gate.defaultScope || "-";
    return detailSection(
      "运行状态",
      `<div class="host-shell-ops-badges">
        ${statusBadge(`phase: ${phaseLabel(ops?.phase || snapshot?.host?.phase)}`, true)}
        ${statusBadge(`accessReady: ${boolText(!!ops?.accessReady)}`, !!ops?.accessReady)}
        ${statusBadge(`warmupReady: ${boolText(!!ops?.warmupReady)}`, !!ops?.warmupReady)}
        ${statusBadge(`scopeGate: ${boolText(!!snapshot?.host?.scopeGateReady)}`, !!snapshot?.host?.scopeGateReady)}
      </div>
      ${jobLine(ops)}
      ${kvTable([
        ["默认 scope", escapeHtml(defaultScope)],
        ["scope blockers", escapeHtml((gate.blockers || []).join(" | ") || "-")],
        ["prebuild plan", escapeHtml(snapshot?.warmup?.planSource || "-")],
        ["dirty slots", escapeHtml(String(snapshot?.warmup?.dirtySlotCount || 0))],
      ])}
      <div class="host-runtime-actions">
        <button type="button" class="mei-btn mei-btn--sm" data-host-ops="refresh">刷新状态</button>
        <button type="button" class="mei-btn mei-btn--sm" data-host-ops="reload">重新加载（compile + import）</button>
        <button type="button" class="mei-btn mei-btn--sm mei-btn--accent" data-host-ops="prebuild">完整预构建</button>
      </div>
      ${hintBlock([
        "reload = compile + import，用于刷新当前 app 的编译与 registry。",
        "prebuild = standard policy，会推进数据、warmup 与 finalize。",
        "停止/重启进程、promote/rollback、verify-only 等细粒度运维仍保留在 CLI/脚本。",
      ])}`,
      "同页同时承载安全动作与运行证据，但不把所有脚本语义都伪装成按钮。",
    );
  }

  function renderNavigationSummary(snapshot) {
    const routes = allRoutes(snapshot);
    const scopes = allScopes(snapshot);
    const rows = scopes
      .map(
        (scope) =>
          `<tr>
            <td>${shellLink(`scope:${scope.scopeKey}`, `scope · ${scope.scopeKey}`, null, "", "build-toolbar-btn")}</td>
            <td>${escapeHtml(String(scope.routeCount || 0))}</td>
            <td>${escapeHtml(String(scope.slotCount || 0))}</td>
            <td>${escapeHtml(scope.accessUrl || "-")}</td>
          </tr>`,
      )
      .join("");
    return detailSection(
      "入口总览",
      kvTable([
        ["routes", escapeHtml(String(routes.length))],
        ["scopes", escapeHtml(String(scopes.length))],
        ["duplicate routes", escapeHtml(String(snapshot?.navigation?.duplicateRouteCount || 0))],
        ["orphan urls", escapeHtml(String(snapshot?.navigation?.orphanUrlCount || 0))],
      ]) +
        `<div class="host-runtime-table-wrap">
          <table class="host-runtime-table">
            <thead><tr><th>Scope</th><th>Routes</th><th>Slots</th><th>默认入口</th></tr></thead>
            <tbody>${rows || `<tr><td colspan="4">暂无入口</td></tr>`}</tbody>
          </table>
        </div>` +
        hintBlock([
          "这里的 route 是入口登记，不是 MRG scope 节点本身。",
          "同一 scope 对应多条 route 时会在 scope 详情里展开真实入口列表，不再重复伪装成多个 scene 节点。",
        ]),
      "从 route 看入口分布，再钻到 scope 看 gate / warmup / slot。",
    );
  }

  function renderRouteDetail(snapshot, route) {
    if (!route) return renderNavigationSummary(snapshot);
    const scope = findScope(snapshot, route.scopeKey);
    return detailSection(
      `Route · ${route.sceneId || route.scopeKey || "-"}`,
      kvTable([
        ["scope", shellLink(`scope:${route.scopeKey}`, `scope · ${route.scopeKey}`, null, "", "build-toolbar-btn")],
        ["url", escapeHtml(route.url || "-")],
        ["assemblyKey", escapeHtml(route.assemblyKey || "-")],
        ["sceneId", escapeHtml(route.sceneId || "-")],
        ["scope slots", escapeHtml(String(scope?.slotCount || 0))],
      ]) +
        `<div class="host-runtime-actions">
          ${route.url ? `<a class="mei-btn mei-btn--sm" href="${escapeHtml(route.url)}">打开访问入口</a>` : ""}
        </div>` +
        hintBlock([
          "route 只说明入口与装配归属，不直接表示 slot 是否已 warmup/命中缓存。",
          "判断运行就绪请继续看对应 scope 或 slot 详情。",
        ]),
      "route / scope / slot 是三种不同对象，不应混成同一列。",
    );
  }

  function renderScopeDetail(snapshot, scopeKey) {
    const scope = findScope(snapshot, scopeKey);
    if (!scope) return renderNavigationSummary(snapshot);
    const routes = routesOfScope(snapshot, scopeKey);
    const slots = slotsOfScope(snapshot, scopeKey);
    const status = scopeStatus(scope);
    const routeRows = routes
      .map(
        (route) =>
          `<tr>
            <td>${shellLink(route.nodeId, route.sceneId || route.scopeKey, null, "", "build-toolbar-btn")}</td>
            <td>${escapeHtml(route.url || "-")}</td>
            <td>${escapeHtml(route.assemblyKey || "-")}</td>
          </tr>`,
      )
      .join("");
    return detailSection(
      `Scope · ${scopeKey}`,
      `<div class="host-shell-ops-badges">
        ${statusBadge(`status: ${status}`, status === "ready")}
        ${statusBadge(`routes: ${scope.routeCount || 0}`, (scope.routeCount || 0) > 0)}
        ${statusBadge(`dirty: ${scope.dirtySlots || 0}`, (scope.dirtySlots || 0) === 0)}
      </div>
      <div class="host-runtime-actions">
        ${scope.accessUrl ? `<a class="mei-btn mei-btn--sm" href="${escapeHtml(scope.accessUrl)}">打开默认入口</a>` : ""}
        <button type="button" class="mei-btn mei-btn--sm" data-host-ops="activate-scope" data-host-scope="${escapeHtml(scopeKey)}">补热当前 scope（1-hop）</button>
        <button type="button" class="mei-btn mei-btn--sm" data-host-ops="refresh">刷新状态</button>
      </div>
      ${kvTable([
        ["routeCount", escapeHtml(String(scope.routeCount || 0))],
        ["slotCount", escapeHtml(String(scope.slotCount || 0))],
        ["readySlots", escapeHtml(String(scope.readySlots || 0))],
        ["staleSlots", escapeHtml(String(scope.staleSlots || 0))],
        ["failedSlots", escapeHtml(String(scope.failedSlots || 0))],
        ["dirtySlots", escapeHtml(String(scope.dirtySlots || 0))],
        ["clientEligibleSlots", escapeHtml(String(scope.clientEligibleSlots || 0))],
        ["worksetIds", escapeHtml((scope.worksetIds || []).join(", ") || "-")],
        ["blockers", escapeHtml((scope.blockers || []).join(" | ") || "-")],
      ])}
      <div class="host-runtime-table-wrap">
        <table class="host-runtime-table">
          <thead><tr><th>Route</th><th>URL</th><th>Assembly</th></tr></thead>
          <tbody>${routeRows || `<tr><td colspan="3">暂无 route</td></tr>`}</tbody>
        </table>
      </div>
      ${renderSlotsTable(slots, { title: `Scope ${scopeKey} 的 slots`, scopeFirst: false })}
      ${hintBlock([
        "scope 是 gate / warmup / materialization 的落点；多条 route 进入同一 scope 不代表存在多份 MRG scope。",
        "clientEligible 仅表示可写入 client tier 的资格，不代表浏览器当前 tab 已命中。",
        "若要进一步核对四层 readiness，请回到 readiness / scope-gate / block CLI。",
      ])}`,
      "这个面板应该能直接回答：入口有哪些、slot 状态怎样、为什么 ready / degraded。",
    );
  }

  function renderGateSummary(snapshot) {
    const gate = snapshot?.scopeGate || {};
    const degradedScopes = Array.isArray(gate.degradedScopes) ? gate.degradedScopes : [];
    const degradedLinks = degradedScopes
      .map((scopeKey) => shellLink(`scope:${scopeKey}`, `scope · ${scopeKey}`, null, "", "build-toolbar-btn"))
      .join(" ");
    return detailSection(
      "Scope Gate",
      kvTable([
        ["defaultScope", escapeHtml(gate.defaultScope || "-")],
        ["accessReady", escapeHtml(boolText(!!gate.accessReady))],
        ["shellReady", escapeHtml(boolText(!!gate.shellReady))],
        ["dataReady", escapeHtml(boolText(!!gate.dataReady))],
        ["blockers", escapeHtml((gate.blockers || []).join(" | ") || "-")],
        ["L2/L3/L4 sweep", escapeHtml(prettyJson(gate.scopeGateSweep || {}))],
      ]) +
        (degradedLinks
          ? `<section class="rounded-lg border border-white/10 bg-black/10 p-3">
              <h3 class="mb-2 mei-font-2 mei-text-primary">Degraded scopes</h3>
              <div class="flex flex-wrap gap-2">${degradedLinks}</div>
            </section>`
          : "") +
        hintBlock([
          "host-shell 当前给出的是轻量 gate 摘要，用于说明入口、装配和物化是否能继续观测。",
          "当你需要严格的 L2/L3/L4 parity 时，请结合 readiness / scope-gate CLI 与磁盘证据。",
        ]),
      "把“为什么不 ready”先压缩成 blockers，再决定是否回到 CLI。",
    );
  }

  function renderWarmupSummary(snapshot) {
    const warmup = snapshot?.warmup || {};
    const hotScopes = Array.isArray(warmup.hotScopes) ? warmup.hotScopes : [];
    const rows = hotScopes
      .map(
        (item) =>
          `<tr><td>${shellLink(`scope:${item.scope}`, item.scope || "-", null, "", "build-toolbar-btn")}</td><td>${escapeHtml(
            String(item.slots || 0),
          )}</td></tr>`,
      )
      .join("");
    return detailSection(
      "Warmup / Frontier",
      kvTable([
        ["planSource", escapeHtml(warmup.planSource || "-")],
        ["dirtySlotCount", escapeHtml(String(warmup.dirtySlotCount || 0))],
        ["dirtyScopes", escapeHtml((warmup.dirtyScopes || []).join(", ") || "-")],
        ["bootstrapManifests", escapeHtml(String(warmup.bootstrapManifestCount || 0))],
        ["mrgEvalSkips", escapeHtml(String(warmup.mrgEvalSkips || 0))],
      ]) +
        `<div class="host-runtime-table-wrap">
          <table class="host-runtime-table">
            <thead><tr><th>Hot scope</th><th>Slots</th></tr></thead>
            <tbody>${rows || `<tr><td colspan="2">暂无 hot scopes</td></tr>`}</tbody>
          </table>
        </div>` +
        hintBlock([
          "warmup 关心哪些 scope/workset/slot 需要推进，不等同于 compile/import 是否完成。",
          "dirty slot = 仍需推进或已 stale/failed 的物化单元，是 prebuild 与运行排障的重要交集。",
        ]),
      "预热计划、dirty frontier 与 bootstrap 资格在这里汇合。",
    );
  }

  function renderWarmupScopeDetail(snapshot, scopeKey) {
    const scope = findScope(snapshot, scopeKey);
    if (!scope) return renderWarmupSummary(snapshot);
    return detailSection(
      `Warmup · ${scopeKey}`,
      kvTable([
        ["dirtySlots", escapeHtml(String(scope.dirtySlots || 0))],
        ["readySlots", escapeHtml(String(scope.readySlots || 0))],
        ["failedSlots", escapeHtml(String(scope.failedSlots || 0))],
        ["worksetIds", escapeHtml((scope.worksetIds || []).join(", ") || "-")],
      ]) +
        renderSlotsTable(slotsOfScope(snapshot, scopeKey), { title: `Warmup 关注的 slots · ${scopeKey}` }) +
        hintBlock([
          "若 dirty slot 长时间不下降，优先核对该 scope 的 route / blockers / artifact 输入是否匹配。",
        ]),
      "按 scope 看 warmup frontier，而不是把 dirty slot 散落在全局列表里。",
    );
  }

  function renderMrgSummary(snapshot) {
    const mrg = snapshot?.mrg || {};
    const tiers = mrg.slotsByTier || {};
    return detailSection(
      "MRG 总览",
      kvTable([
        ["schemaVersion", escapeHtml(mrg.schemaVersion || "-")],
        ["registryRevision", escapeHtml(mrg.registryRevision || "-")],
        ["updatedAtMs", escapeHtml(String(mrg.updatedAtMs || 0))],
        ["slotCount", escapeHtml(String(mrg.slotCount || 0))],
        ["failedSlots", escapeHtml(String((mrg.failedSlots || []).length))],
        ["edgeCount", escapeHtml(String(mrg.edgeCount || 0))],
        ["diskReady", escapeHtml(String(tiers.diskReady || 0))],
        ["memoryResident", escapeHtml(String(tiers.memoryResident || 0))],
        ["clientEligible", escapeHtml(String(tiers.clientEligible || 0))],
      ]) +
        hintBlock([
          "MRG 是当前 materialization 真源；scope 详情会把同一 scope 下的 slot 汇总回来。",
          "“全部 slots”是排障索引，不是 scene 树的一一镜像。",
        ]),
      "用来回答：当前 materialization 到了哪一步、tier 状态如何。",
    );
  }

  function renderSlotsTable(slots, options) {
    const rows = (slots || [])
      .map(
        (slot) =>
          `<tr>
            <td>${options?.scopeFirst ? escapeHtml(slot.scopeKey || "-") : shellLink(slot.nodeId, slot.nodeKey || "-", null, "", "build-toolbar-btn")}</td>
            <td>${options?.scopeFirst ? shellLink(slot.nodeId, slot.nodeKey || "-", null, "", "build-toolbar-btn") : escapeHtml(slot.state || "-")}</td>
            <td>${options?.scopeFirst ? escapeHtml(slot.state || "-") : escapeHtml(slot.residentTier || "-")}</td>
            <td>${options?.scopeFirst ? escapeHtml(slot.residentTier || "-") : escapeHtml(slot.ownerResourceId || "-")}</td>
            ${
              options?.scopeFirst
                ? `<td>${escapeHtml(slot.ownerResourceId || "-")}</td>`
                : ""
            }
          </tr>`,
      )
      .join("");
    const headers = options?.scopeFirst
      ? "<th>Scope</th><th>Slot</th><th>状态</th><th>Tier</th><th>Owner</th>"
      : "<th>Slot</th><th>状态</th><th>Tier</th><th>Owner</th>";
    const span = options?.scopeFirst ? 5 : 4;
    return `
<section class="rounded-lg border border-white/10 bg-black/10 p-3">
  <h3 class="mb-2 mei-font-2 mei-text-primary">${escapeHtml(options?.title || "Slots")}</h3>
  <div class="host-runtime-table-wrap">
    <table class="host-runtime-table">
      <thead><tr>${headers}</tr></thead>
      <tbody>${rows || `<tr><td colspan="${span}">暂无 slot</td></tr>`}</tbody>
    </table>
  </div>
</section>`;
  }

  function renderAllSlots(snapshot) {
    return detailSection(
      "全部 slots",
      renderSlotsTable(allSlots(snapshot), { title: "MRG 全量 slot 索引", scopeFirst: true }) +
        hintBlock([
          "这是排障总表，便于按 scope / state / tier 观察；不再假装与左侧入口一一对应。",
        ]),
      `共 ${allSlots(snapshot).length} 个 slot`,
    );
  }

  function renderFailedSlots(snapshot) {
    const failed = Array.isArray(snapshot?.mrg?.failedSlots) ? snapshot.mrg.failedSlots : [];
    return detailSection(
      "失败 slots",
      renderSlotsTable(failed, { title: "当前 failed slots", scopeFirst: true }) +
        hintBlock([
          "failed slot 说明当前 materialization 已进入错误态，通常需要结合日志、layer/block CLI 或输入产物继续确认根因。",
        ]),
      `当前 ${failed.length} 个 failed slot`,
    );
  }

  function renderSlotDetail(snapshot, nodeId) {
    const slot = findSlot(snapshot, nodeId);
    if (!slot) return renderAllSlots(snapshot);
    const scope = findScope(snapshot, slot.scopeKey);
    return detailSection(
      `Slot · ${slot.nodeKey || "-"}`,
      kvTable([
        ["scope", shellLink(`scope:${slot.scopeKey}`, slot.scopeKey || "-", null, "", "build-toolbar-btn")],
        ["state", escapeHtml(slot.state || "-")],
        ["slotRevision", escapeHtml(slot.slotRevision || "-")],
        ["ownerResourceId", escapeHtml(slot.ownerResourceId || "-")],
        ["metricDefBundleRevision", escapeHtml(slot.metricDefBundleRevision || "-")],
        ["dataSourceRevision", escapeHtml(slot.dataSourceRevision || "-")],
        ["cachePolicy", escapeHtml(slot.cachePolicy || "-")],
        ["evalEngine", escapeHtml(slot.evalEngine || "-")],
        ["residentTier", escapeHtml(slot.residentTier || "-")],
        ["clientEligible", escapeHtml(boolText(!!slot.clientEligible))],
        ["clientRevision", escapeHtml(slot.clientRevision || "-")],
        ["payloadBytes", escapeHtml(formatBytes(slot.payloadBytes))],
        ["accessCount", escapeHtml(String(slot.accessCount || 0))],
        ["lastAccessMs", escapeHtml(formatMaybe(slot.lastAccessMs))],
        ["worksetId", escapeHtml(slot.worksetId || "-")],
      ]) +
        `<section class="rounded-lg border border-white/10 bg-black/10 p-3">
          <h3 class="mb-2 mei-font-2 mei-text-primary">最近一次求值</h3>
          ${kvTable([
            ["atMs", escapeHtml(formatMaybe(slot.lastEval?.atMs))],
            ["wallMs", escapeHtml(formatMs(slot.lastEval?.wallMs))],
            ["artifactHit", escapeHtml(boolText(!!slot.lastEval?.artifactHit))],
            ["cacheLayer", escapeHtml(slot.lastEval?.cacheLayer || "-")],
          ])}
        </section>
        <section class="rounded-lg border border-white/10 bg-black/10 p-3">
          <h3 class="mb-2 mei-font-2 mei-text-primary">Payload / Tier 备注</h3>
          <pre class="runtime-detail-json overflow-auto rounded bg-black/20 p-3 font-mono mei-font-1 leading-5 mei-text-body">${escapeHtml(
            prettyJson({
              payloadRef: slot.payloadRef || null,
              tiersReady: slot.tiersReady || null,
              scopeSummary: scope || null,
            }),
          )}</pre>
        </section>
        ${hintBlock([
          "artifactHit/cacheLayer 描述最近一次已记录求值如何命中，并非浏览器当前 tab 的实时命中轨迹。",
          "clientRevision 是 client tier 的失效键；是否被浏览器实际消费仍需结合 access 页 perf / 日志确认。",
        ])}`,
      "slot 是 (owner, scopeKey) 级别的求值单元，这里展示最接近原始 registry 的字段。",
    );
  }

  function renderCacheSummary(snapshot) {
    const cache = snapshot?.cache || {};
    const flags = cache.flags || {};
    const content = cache.contentStore || {};
    const evalInfo = cache.eval || {};
    const disk = cache.disk || {};
    const build = cache.build || {};
    return detailSection(
      "Cache / Artifact / Store",
      kvTable([
        ["dataGeneration", escapeHtml(cache.dataGeneration || "-")],
        ["accessSlimArtifacts", escapeHtml(boolText(!!flags.accessSlimArtifacts))],
        ["canonicalArtifactPersist", escapeHtml(boolText(!!flags.canonicalArtifactPersist))],
        ["graphRegistryDedup", escapeHtml(boolText(!!flags.graphRegistryDedup))],
        ["envOverrides", escapeHtml((flags.envOverrides || []).join(", ") || "-")],
        ["CAS store", escapeHtml(`${formatBytes(content.bytes)} / ${Object.keys(content.filesByKind || {}).length} kinds`)],
        ["eval response", escapeHtml(`${evalInfo.metricResponseFiles || 0} / ${formatBytes(evalInfo.metricResponseBytes)}`)],
        ["eval dataframe", escapeHtml(`${evalInfo.metricDataframeFiles || 0} / ${formatBytes(evalInfo.metricDataframeBytes)}`)],
        ["disk artifacts", escapeHtml(`${disk.evalArtifactFileCount || 0} / ${formatBytes(disk.evalArtifactBytes)}`)],
        ["compile index", escapeHtml(`${build.compileIndexEntries || 0} entries / ${build.compileIndexHits || 0} hit`)],
      ]) +
        hintBlock([
          "这里区分 sealed artifact / content store / client tier 资格，避免把资格误读成 live cache hit。",
          "浏览器当前 tab 的 runtime-query Map / sessionStorage 命中率不在 host-shell 直接真源内。",
        ]),
      "缓存面板回答：哪些东西已存在、哪些层只是资格或提示。",
    );
  }

  function renderDiagnosticsSummary(snapshot) {
    const diagnostics = snapshot?.diagnostics || {};
    const alerts = Array.isArray(diagnostics.alerts) ? diagnostics.alerts : [];
    const alertHtml = alerts.length
      ? `<section class="rounded-lg border border-amber-500/30 bg-amber-500/10 p-3">
          <h3 class="mb-2 mei-font-2 mei-text-primary">告警</h3>
          <ul class="grid gap-1 mei-font-1">${alerts
            .map((alert) => `<li>${escapeHtml(alert)}</li>`)
            .join("")}</ul>
        </section>`
      : "";
    return detailSection(
      "Diagnostics",
      kvTable([
        ["compile source", escapeHtml(diagnostics.build?.source || "-")],
        ["compile index stale", escapeHtml(String(diagnostics.build?.compileIndexStaleEntries || 0))],
        ["mrg stale ratio", escapeHtml(String(Math.round((diagnostics.mrg?.staleRatio || 0) * 100)) + "%")],
        ["L2/L3/L4 sweep", escapeHtml(prettyJson(diagnostics.scopeGateSweep || {}))],
      ]) +
        alertHtml +
        hintBlock([
          "诊断面板是对 disk / eval / build / gate 的聚合摘要，不等同于单个 scope/slot 的原始真源。",
          "当这里的摘要不足以解释根因时，请优先切回当前节点 JSON，再决定是否回到 CLI。",
        ]),
      "聚合视角，帮助决定从哪条证据链继续下钻。",
    );
  }

  function renderDetail(activeNode, appId, ops, snapshot) {
    if (activeNode === "ops:overview") return renderOpsDetail(ops, snapshot);
    if (activeNode === "ops:versions") return renderVersionDetail(ops, snapshot);
    if (activeNode === "nav:summary") return renderNavigationSummary(snapshot);
    if (activeNode === "gate:summary") return renderGateSummary(snapshot);
    if (activeNode === "warmup:summary") return renderWarmupSummary(snapshot);
    if (activeNode === "mrg:summary") return renderMrgSummary(snapshot);
    if (activeNode === "mrg:failed") return renderFailedSlots(snapshot);
    if (activeNode === "mrg:slots") return renderAllSlots(snapshot);
    if (activeNode === "cache:summary") return renderCacheSummary(snapshot);
    if (activeNode === "diag:summary") return renderDiagnosticsSummary(snapshot);
    if (activeNode.startsWith("scope:")) {
      return renderScopeDetail(snapshot, activeNode.slice("scope:".length));
    }
    if (activeNode.startsWith("nav:route:")) {
      return renderRouteDetail(snapshot, findRoute(snapshot, activeNode));
    }
    if (activeNode.startsWith("warmup:scope:")) {
      return renderWarmupScopeDetail(snapshot, activeNode.slice("warmup:scope:".length));
    }
    if (activeNode.startsWith("mrg:slot:")) {
      return renderSlotDetail(snapshot, activeNode);
    }
    return renderOpsDetail(ops, snapshot);
  }

  function selectNodeJson(activeNode, snapshot, ops) {
    if (activeNode === "ops:overview") {
      return {
        nodeId: activeNode,
        ops: ops || snapshot?.ops || {},
        host: snapshot?.host || {},
        scopeGate: snapshot?.scopeGate || {},
      };
    }
    if (activeNode === "ops:versions") {
      return {
        nodeId: activeNode,
        ops: ops || snapshot?.ops || {},
        build: snapshot?.diagnostics?.build || {},
      };
    }
    if (activeNode === "nav:summary") return snapshot?.navigation || {};
    if (activeNode === "gate:summary") return snapshot?.scopeGate || {};
    if (activeNode === "warmup:summary") return snapshot?.warmup || {};
    if (activeNode === "mrg:summary") return snapshot?.mrg || {};
    if (activeNode === "mrg:failed") return { failedSlots: snapshot?.mrg?.failedSlots || [] };
    if (activeNode === "mrg:slots") return { slots: allSlots(snapshot) };
    if (activeNode === "cache:summary") return snapshot?.cache || {};
    if (activeNode === "diag:summary") return snapshot?.diagnostics || {};
    if (activeNode.startsWith("scope:")) {
      const scopeKey = activeNode.slice("scope:".length);
      return {
        scope: findScope(snapshot, scopeKey),
        routes: routesOfScope(snapshot, scopeKey),
        slots: slotsOfScope(snapshot, scopeKey),
        gate: snapshot?.scopeGate || {},
        warmup: snapshot?.warmup || {},
      };
    }
    if (activeNode.startsWith("nav:route:")) return findRoute(snapshot, activeNode) || {};
    if (activeNode.startsWith("warmup:scope:")) {
      const scopeKey = activeNode.slice("warmup:scope:".length);
      return {
        scope: findScope(snapshot, scopeKey),
        warmup: snapshot?.warmup || {},
        slots: slotsOfScope(snapshot, scopeKey),
      };
    }
    if (activeNode.startsWith("mrg:slot:")) return findSlot(snapshot, activeNode) || {};
    return snapshot || {};
  }

  function setJsonPanels(activeNode, snapshot, ops) {
    const nodePanel = document.getElementById("host-runtime-node-json");
    if (nodePanel) {
      nodePanel.textContent = prettyJson(selectNodeJson(activeNode, snapshot, ops));
    }
    const snapshotPanel = document.getElementById("host-runtime-snapshot-json");
    if (snapshotPanel) {
      snapshotPanel.textContent = prettyJson(snapshot || {});
    }
  }

  function setBusy(root, busy) {
    if (!root) return;
    root.querySelectorAll("[data-host-ops]").forEach((btn) => {
      if (btn.getAttribute("data-host-ops") === "refresh") return;
      btn.disabled = busy;
    });
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
      await refreshConsole();
    } catch (err) {
      global.alert(String(err && err.message ? err.message : err));
    } finally {
      setBusy(root, false);
    }
  }

  async function postActivateScope(root, scopeKey) {
    setBusy(root, true);
    try {
      const appId = appIdFromShell();
      const appQuery = appId ? `&appId=${encodeURIComponent(appId)}` : "";
      const url = `${ACTIVATE_SCOPE_URL}?scope=${encodeURIComponent(scopeKey)}&hops=1${appQuery}`;
      const res = await fetch(url, { method: "POST" });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body.error || res.statusText || "scope activation failed");
      await refreshConsole();
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
        else if (action === "activate-scope") {
          const scopeKey = btn.getAttribute("data-host-scope");
          if (scopeKey) postActivateScope(detailRoot, scopeKey);
        } else if (action === "refresh") {
          void refreshConsole();
        }
      });
    });
    if (ops?.job && ops.job.status === "running") {
      setBusy(detailRoot, true);
    }
  }

  function bindToolbarActions() {
    const refreshBtn = document.getElementById("runtime-refresh-btn");
    if (!refreshBtn || refreshBtn.__hostRuntimeBound) return;
    refreshBtn.__hostRuntimeBound = true;
    refreshBtn.addEventListener("click", () => {
      void refreshConsole();
    });
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
    const activeNode = resolveActiveNode(snapshot);
    const navMount = document.getElementById("host-runtime-nav-mount");
    const detailMount = document.getElementById("host-runtime-detail-mount");
    if (!navMount || !detailMount) return;
    hideLegacyPanels();
    bindToolbarActions();
    navMount.innerHTML = renderNav(appId, snapshot, activeNode);
    detailMount.innerHTML = renderDetail(activeNode, appId, ops, snapshot);
    setJsonPanels(activeNode, snapshot, ops);
    bindDetailActions(detailMount, ops);
  }

  async function refreshConsole() {
    const appId = appIdFromShell();
    if (!appId) return false;
    try {
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
    } catch (error) {
      // Host restart / ACCESS not ready: keep polling quietly.
      console.debug?.("[host-runtime-console] refresh skipped", error);
      return false;
    }
  }

  function schedulePoll(detailRoot) {
    if (pollTimer) return;
    pollTimer = global.setTimeout(async () => {
      pollTimer = null;
      const hadJob = opsCache?.job?.status === "running";
      await refreshConsole();
      if (hadJob && opsCache && !opsCache.job && opsCache.lastJob?.status === "success") {
        global.setTimeout(() => global.location.reload(), 800);
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
      navigateToNode(defaultNodeId(snapshotCache), { replace: true });
    }
  }

  function bindHostRuntimeNavCapture() {
    if (document.__hostRuntimeNavCaptureBound) return;
    document.__hostRuntimeNavCaptureBound = true;
    document.addEventListener(
      "click",
      (event) => {
        const link = event.target.closest("[data-runtime-node-link='1']");
        if (!link) return;
        if (!document.querySelector(".runtime-workspace")?.contains(link)) return;
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
