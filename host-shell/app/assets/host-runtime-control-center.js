/**
 * `/runtime` workspace hub — per-app card console (0536/0537).
 * App-scoped `/runtime?app=` remains in host-runtime-console.js.
 */
(function (global) {
  "use strict";

  const OPS_STATUS_API = "/api/host/ops/status";
  const OPS_PREBUILD_API = "/api/host/ops/prebuild";
  const APPS_API = "/api/host/apps";
  const CLEANUP_PREVIEW_API = "/api/host/builds/cleanup-preview";
  const CLEANUP_API = "/api/host/builds/cleanup";
  const ACTIVATE_ENV_API = "/api/host/runtime/activate-env";

  const state = {
    appsOverview: null,
    ops: null,
    opsTimer: null,
    durationTimer: null,
    busy: false,
    cleanupPreview: null,
    cleanupAppId: null,
    selectedAppId: "",
    query: "",
    viewMode: "card",
    sort: { field: "name", dir: "asc" },
    navWidthPct: 50,
    shellReady: false,
    /** @type {Record<string, { kind: string, label: string, startedAt: number }>} */
    pendingByApp: {},
    /** @type {Record<string, string>} */
    modeSelections: {},
  };

  let root = null;
  let overflowModulePromise = null;

  function escapeHtml(value) {
    return String(value == null ? "" : value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function errorMessage(error) {
    if (!error) return "未知错误";
    if (typeof error === "string") return error;
    if (typeof error.error === "string") return error.error;
    if (error.error && typeof error.error === "object") {
      const details = error.error.details || {};
      const suffix =
        details.currentRevision != null
          ? `（服务器 revision: ${details.currentRevision || "无"}）`
          : details.parseError
            ? `：${details.parseError}`
            : "";
      return `${error.error.message || error.error.code || "请求失败"}${suffix}`;
    }
    return error.message || String(error);
  }

  async function requestJson(url, options) {
    const response = await fetch(url, {
      ...options,
      headers: { Accept: "application/json", ...(options && options.headers) },
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      const failure = new Error(errorMessage(body) || response.statusText);
      failure.status = response.status;
      failure.body = body;
      throw failure;
    }
    return body;
  }

  function announce(message, tone) {
    const live = root && root.querySelector("[data-runtime-live]");
    if (!live) return;
    live.innerHTML = overflowText(message, "展开完整状态信息");
    live.dataset.tone = tone || "neutral";
  }

  function overflowText(value, label) {
    const text = String(value == null || value === "" ? "—" : value);
    if (text.length < 44) return `<span>${escapeHtml(text)}</span>`;
    return `<span class="mei-runtime-overflow">
      <span class="mei-runtime-overflow__preview">${escapeHtml(text)}</span>
      <button type="button" class="mei-runtime-overflow__expand" data-runtime-expand data-runtime-full-text="${escapeHtml(text)}" aria-label="${escapeHtml(label || "查看全文")}">…</button>
    </span>`;
  }

  function overflowModule() {
    if (!overflowModulePromise) {
      overflowModulePromise = import("/workspace-components/mei/overflow-text.js");
    }
    return overflowModulePromise;
  }

  function formatBytes(value) {
    const bytes = Number(value) || 0;
    if (bytes < 1024) return `${bytes} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let size = bytes / 1024;
    let unit = units[0];
    for (let index = 1; index < units.length && size >= 1024; index += 1) {
      size /= 1024;
      unit = units[index];
    }
    return `${size.toFixed(size >= 10 ? 1 : 2)} ${unit}`;
  }

  function formatDuration(ms) {
    const total = Math.max(0, Math.floor(Number(ms) / 1000));
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const seconds = total % 60;
    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m ${seconds}s`;
    return `${seconds}s`;
  }

  function formatClock(ms) {
    if (!ms) return "—";
    try {
      return new Date(Number(ms)).toLocaleString();
    } catch (_error) {
      return String(ms);
    }
  }

  function validAppId(value) {
    return value === "*" || /^[A-Za-z0-9_.-]+$/.test(value);
  }

  function selectedLaunch(_appId) {
    return "launch";
  }

  function modeLabel(mode) {
    const value = String(mode || "").trim().toLowerCase();
    if (value === "hot") return "hot · 热加载";
    if (value === "lazy") return "lazy · 按需";
    if (value === "frozen") return "frozen · 冻结";
    return value || "—";
  }

  function modeOptionsHtml(selected, gitDefault) {
    const git = String(gitDefault || "lazy").trim().toLowerCase();
    const fallback = git === "hot" || git === "lazy" || git === "frozen" ? git : "lazy";
    const currentRaw = String(selected || "").trim().toLowerCase();
    const current =
      currentRaw === "hot" || currentRaw === "lazy" || currentRaw === "frozen"
        ? currentRaw
        : fallback;
    return ["hot", "lazy", "frozen"]
      .map((mode) => {
        const selectedAttr = current === mode ? " selected" : "";
        return `<option value="${mode}"${selectedAttr}>${escapeHtml(modeLabel(mode))}</option>`;
      })
      .join("");
  }

  function parseExplorerSort(raw, fallbackField = "name") {
    const text = String(raw || "").trim();
    const match = text.match(/^([a-zA-Z_]+):(asc|desc)$/i);
    if (!match) return { field: fallbackField, dir: "asc" };
    return { field: match[1].toLowerCase(), dir: match[2].toLowerCase() };
  }

  function formatExplorerSort(sort) {
    return `${sort.field}:${sort.dir}`;
  }

  function compareText(left, right) {
    return String(left || "").localeCompare(String(right || ""), "zh-CN", {
      sensitivity: "base",
      numeric: true,
    });
  }

  function compareNumber(left, right) {
    const a = Number(left);
    const b = Number(right);
    const aOk = Number.isFinite(a);
    const bOk = Number.isFinite(b);
    if (!aOk && !bOk) return 0;
    if (!aOk) return 1;
    if (!bOk) return -1;
    return a - b;
  }

  function appSortSize(app) {
    const generations = Array.isArray(app?.generations) ? app.generations : [];
    return generations.reduce((sum, gen) => sum + (Number(gen?.bytes) || 0), 0);
  }

  function appSortTime(app) {
    const generations = Array.isArray(app?.generations) ? app.generations : [];
    let latest = 0;
    generations.forEach((gen) => {
      const ms = Date.parse(String(gen?.createdAt || ""));
      if (Number.isFinite(ms) && ms > latest) latest = ms;
    });
    return latest || null;
  }

  function sortRuntimeApps(apps, sort) {
    const dir = sort?.dir === "desc" ? -1 : 1;
    const field = sort?.field || "name";
    return [...apps].sort((left, right) => {
      let cmp = 0;
      if (field === "size") {
        cmp = compareNumber(appSortSize(left), appSortSize(right));
        if (!cmp) cmp = compareText(left?.displayName || left?.appId, right?.displayName || right?.appId);
      } else if (field === "time") {
        cmp = compareNumber(appSortTime(left), appSortTime(right));
        if (!cmp) cmp = compareText(left?.displayName || left?.appId, right?.displayName || right?.appId);
      } else {
        cmp = compareText(left?.displayName || left?.appId, right?.displayName || right?.appId);
      }
      return cmp * dir;
    });
  }

  function syncExplorerUrl() {
    try {
      const url = new URL(global.location.href);
      if (state.selectedAppId) url.searchParams.set("sel", state.selectedAppId);
      else url.searchParams.delete("sel");
      if (state.query) url.searchParams.set("q", state.query);
      else url.searchParams.delete("q");
      if (state.viewMode && state.viewMode !== "card") url.searchParams.set("view", state.viewMode);
      else url.searchParams.delete("view");
      const sortText = formatExplorerSort(state.sort || { field: "name", dir: "asc" });
      if (sortText && sortText !== "name:asc") url.searchParams.set("sort", sortText);
      else url.searchParams.delete("sort");
      global.history.replaceState(global.history.state, "", url);
    } catch (_error) {
      /* ignore */
    }
  }

  function readExplorerUrlState() {
    try {
      const url = new URL(global.location.href);
      return {
        sel: String(url.searchParams.get("sel") || "").trim(),
        q: String(url.searchParams.get("q") || ""),
        view: String(url.searchParams.get("view") || "card").trim() || "card",
        sort: parseExplorerSort(url.searchParams.get("sort"), "name"),
      };
    } catch (_error) {
      return { sel: "", q: "", view: "card", sort: { field: "name", dir: "asc" } };
    }
  }

  function syncSelectedAppUrl(appId) {
    state.selectedAppId = appId || "";
    syncExplorerUrl();
  }

  function readSelectedAppFromUrl() {
    return readExplorerUrlState().sel;
  }

  function appMatchesQuery(app, needle) {
    if (!needle) return true;
    const haystack = [app.displayName, app.appId, app.launchPath, app.shortTitle];
    return haystack.some((value) =>
      String(value || "")
        .toLocaleLowerCase()
        .includes(needle),
    );
  }

  function resolveSelectedMode(app, modeSelections) {
    const appId = app.appId || "";
    if (modeSelections[appId] != null && modeSelections[appId] !== "") {
      return String(modeSelections[appId]).trim().toLowerCase();
    }
    const overlay = String(app.overlayDefaultMode || "").trim().toLowerCase();
    if (overlay === "hot" || overlay === "lazy" || overlay === "frozen") return overlay;
    const git = String(app.gitDefaultMode || "lazy").trim().toLowerCase();
    return git === "hot" || git === "lazy" || git === "frozen" ? git : "lazy";
  }

  function setAppPending(appId, kind, label) {
    if (!appId) return;
    state.pendingByApp[appId] = {
      kind: kind || "busy",
      label: label || "处理中…",
      startedAt: Date.now(),
    };
    patchNavItem(appId);
    if (appId === state.selectedAppId) paintDetail();
  }

  function clearAppPending(appId) {
    if (!appId || !state.pendingByApp[appId]) return;
    delete state.pendingByApp[appId];
    patchNavItem(appId);
    if (appId === state.selectedAppId) paintDetail();
  }

  function pendingProgressHtml(pending) {
    if (!pending) return "";
    return `<div class="mei-runtime-control__progress" role="status" aria-live="polite">
      <div class="mei-runtime-control__progress-track" aria-hidden="true"><span class="mei-runtime-control__progress-bar"></span></div>
      <p class="mei-runtime-control__progress-label">${escapeHtml(pending.label || "处理中…")}</p>
    </div>`;
  }

  function lockedAttr(locked) {
    return locked ? " disabled data-runtime-locked" : "";
  }

  function setBusy(busy) {
    state.busy = busy;
    const controlsBusy = busy || state.ops?.job?.status === "running";
    root.querySelectorAll("button, select").forEach((control) => {
      if (control.matches("[data-runtime-expand]")) return;
      if (control.matches("[data-runtime-cleanup-cancel]")) {
        control.disabled = false;
        return;
      }
      const locked = control.hasAttribute("data-runtime-locked");
      control.disabled = controlsBusy || locked;
    });
  }

  function renderGlobalOps() {
    const mount = root?.querySelector("[data-runtime-global-ops]");
    if (!mount) return;
    const job = state.ops?.job;
    if (!job || job.status !== "running") {
      mount.hidden = true;
      mount.innerHTML = "";
      return;
    }
    mount.hidden = false;
    mount.innerHTML = `<p class="mei-runtime-control__notice" role="status">
      <span class="mei-runtime-control__pulse" aria-hidden="true"></span>
      任务进行中：<code>${escapeHtml(job.kind || "ops")}</code> · ${escapeHtml(job.phase || "running")}
      ${job.generation ? ` · <code>${escapeHtml(job.generation)}</code>` : ""}
    </p>`;
  }

  function ensureShell(mount) {
    if (state.shellReady && mount.querySelector("[data-runtime-app-nav]") && mount.querySelector("[data-runtime-app-detail]")) {
      return;
    }
    mount.className = "mei-runtime-control__explorer";
    mount.style.setProperty("--nav-width", `${state.navWidthPct || 50}%`);
    mount.innerHTML = `<div class="mei-runtime-control__explorer-nav">
        <div class="mei-runtime-control__explorer-toolbar" data-runtime-toolbar></div>
        <div class="mei-runtime-control__explorer-nav-scroll" data-runtime-app-nav role="listbox" aria-label="应用清单"></div>
      </div>
      <div class="mei-runtime-control__explorer-splitter" data-runtime-splitter role="separator" aria-orientation="vertical" aria-label="调整左右宽度" tabindex="0"></div>
      <div class="mei-runtime-control__explorer-detail" data-runtime-app-detail></div>`;
    bindRuntimeSplitter(mount);
    state.shellReady = true;
  }

  function renderToolbar(mount) {
    const toolbar = mount.querySelector("[data-runtime-toolbar]");
    if (!toolbar) return;
    const sort = state.sort || { field: "name", dir: "asc" };
    const sortValue = formatExplorerSort(sort);
    const options = [
      ["name:asc", "名称升序"],
      ["name:desc", "名称降序"],
      ["size:asc", "大小升序"],
      ["size:desc", "大小降序"],
      ["time:asc", "时间升序"],
      ["time:desc", "时间降序"],
    ]
      .map(
        ([value, label]) =>
          `<option value="${value}"${value === sortValue ? " selected" : ""}>${label}</option>`,
      )
      .join("");
    toolbar.innerHTML = `<input class="mei-runtime-control__search" data-runtime-search type="search" placeholder="搜索应用名或 appId" aria-label="搜索应用" value="${escapeHtml(state.query || "")}" />
      <div class="mei-runtime-control__view-toggle" aria-label="应用展示方式">
        <button type="button" data-runtime-view="card" aria-pressed="${state.viewMode === "card" ? "true" : "false"}">卡片</button>
        <button type="button" data-runtime-view="list" aria-pressed="${state.viewMode === "list" ? "true" : "false"}">列表</button>
      </div>
      <select class="mei-runtime-control__sort" data-runtime-sort aria-label="应用排序">${options}</select>`;
  }

  function loadStateLabel(app, runningByApp) {
    const appId = app.appId || "";
    const run = runningByApp[appId];
    const phaseFromApi = run?.phase || null;
    const enabled = app.enabled === true;
    const loaded = app.loaded === true || phaseFromApi === "ready";
    const loading =
      app.loadState === "loading" ||
      phaseFromApi === "starting" ||
      Boolean(state.pendingByApp[appId]);
    const failed = app.loadState === "load_failed";
    if (!enabled) {
      return {
        label: "未启用",
        shortLabel: "未启用",
        tone: "is-disabled",
        detail: "不可访问 · 需先启用",
      };
    }
    if (failed) {
      return {
        label: "载入失败",
        shortLabel: "失败",
        tone: "is-failed",
        detail: "可重试启用或立即载入",
      };
    }
    if (loading) {
      return {
        label: "载入中",
        shortLabel: "载入中",
        tone: "is-pending",
        detail: `模式 ${String(app.effectiveDefaultMode || "lazy")}`,
      };
    }
    if (loaded) {
      return {
        label: "已载入",
        shortLabel: "已载入",
        tone: "is-running",
        detail: `模式 ${String(app.effectiveDefaultMode || "lazy")}`,
      };
    }
    return {
      label: "已启用 · 未载入",
      shortLabel: "待载入",
      tone: "is-enabled",
      detail: `已准入 · 首访或点「立即载入」`,
    };
  }

  function navCardHtml(app, runningByApp) {
    const appId = app.appId || "—";
    const pending = state.pendingByApp[appId] || null;
    const gitMode = String(app.gitDefaultMode || "lazy").trim().toLowerCase();
    const effectiveMode = String(
      app.effectiveDefaultMode || app.overlayDefaultMode || gitMode || "lazy",
    )
      .trim()
      .toLowerCase();
    const generations = Array.isArray(app.generations) ? app.generations : [];
    const hasCurrentBundle = generations.some((gen) => gen.isCurrent);
    const selected = state.selectedAppId === appId;
    const loadUi = loadStateLabel(app, runningByApp);
    let phaseLabel = loadUi.shortLabel || loadUi.label;
    let tone = loadUi.tone;
    if (pending) {
      phaseLabel = "处理中";
      tone = "is-pending";
    }
    const stoppedDetail = !app.hasLaunch
      ? "无 app.toml"
      : !hasCurrentBundle
        ? "无编译产物"
        : loadUi.detail;
    const meta = pending
      ? pending.label || "处理中…"
      : app.enabled
        ? stoppedDetail
        : stoppedDetail;
    const view = state.viewMode === "list" ? "list" : "card";
    return `<article class="mei-runtime-control__nav-card${selected ? " is-selected" : ""}${tone ? ` ${tone}` : ""}" data-runtime-select-app="${escapeHtml(appId)}" data-view="${view}" data-load-tone="${escapeHtml(tone || "is-disabled")}" role="option" aria-selected="${selected ? "true" : "false"}" tabindex="0">
      <div class="mei-runtime-control__nav-card-head">
        <div class="mei-runtime-control__nav-card-identity">
          <h3>${escapeHtml(app.displayName || appId)}</h3>
          <p><code>${escapeHtml(appId)}</code></p>
        </div>
        <span class="mei-runtime-control__status-chip${tone ? ` ${tone}` : ""}" data-runtime-nav-chip title="${escapeHtml(loadUi.label)}">
          <span class="mei-runtime-control__status-dot" aria-hidden="true"></span>
          <strong>${escapeHtml(phaseLabel)}</strong>
        </span>
      </div>
      <div class="mei-runtime-control__nav-card-foot">
        <span class="mei-runtime-control__mode-pill" data-mode="${escapeHtml(effectiveMode)}">${escapeHtml(modeLabel(effectiveMode))}</span>
        <p class="mei-runtime-control__nav-card-meta" data-runtime-nav-meta>${escapeHtml(meta)}</p>
      </div>
    </article>`;
  }

  function paintSelection() {
    const mount = root?.querySelector("[data-runtime-app-grid]");
    if (!mount) return;
    mount.querySelectorAll("[data-runtime-select-app]").forEach((card) => {
      const selected = card.getAttribute("data-runtime-select-app") === state.selectedAppId;
      card.classList.toggle("is-selected", selected);
      card.setAttribute("aria-selected", String(selected));
    });
  }

  function patchNavItem(appId) {
    const mount = root?.querySelector("[data-runtime-app-grid]");
    if (!mount) return;
    const apps = Array.isArray(state.appsOverview?.apps) ? state.appsOverview.apps : [];
    const running = Array.isArray(state.appsOverview?.running) ? state.appsOverview.running : [];
    const runningByApp = Object.fromEntries(running.map((row) => [row.appId, row]));
    const app = apps.find((row) => row.appId === appId);
    if (!app) return;
    const card = Array.from(mount.querySelectorAll("[data-runtime-select-app]")).find(
      (node) => node.getAttribute("data-runtime-select-app") === appId,
    );
    if (!card) return;
    const html = navCardHtml(app, runningByApp);
    const wrap = document.createElement("div");
    wrap.innerHTML = html;
    const next = wrap.firstElementChild;
    if (next) card.replaceWith(next);
  }

  function paintDetail() {
    const mount = root?.querySelector("[data-runtime-app-grid]");
    const detailMount = mount?.querySelector("[data-runtime-app-detail]");
    if (!detailMount) return;
    const apps = Array.isArray(state.appsOverview?.apps) ? state.appsOverview.apps : [];
    const running = Array.isArray(state.appsOverview?.running) ? state.appsOverview.running : [];
    const runningByApp = Object.fromEntries(running.map((row) => [row.appId, row]));
    const selectedApp = apps.find((app) => app.appId === state.selectedAppId) || apps[0];
    if (!selectedApp) {
      detailMount.innerHTML = `<div class="mei-runtime-control__detail-empty">选择左侧应用以操作</div>`;
      return;
    }
    const bundlesOpen = Boolean(detailMount.querySelector("details.mei-runtime-control__bundles[open]"));
    const modeSelections = { ...state.modeSelections };
    detailMount.querySelectorAll("[data-runtime-mode-select]").forEach((select) => {
      modeSelections[select.getAttribute("data-app")] = select.value;
    });
    Object.assign(state.modeSelections, modeSelections);
    const now = Date.now();
    const app = selectedApp;
    const appId = app.appId || "—";
    const run = runningByApp[appId];
    const phaseFromApi = run?.phase || null;
    const isReady = phaseFromApi === "ready" || app.loaded === true;
    const isStarting = phaseFromApi === "starting" || app.loadState === "loading";
    const isEnabled = app.enabled === true;
    const isLoaded = isReady;
    const isRunning = isReady || isStarting;
    const hasLaunch = Boolean(app.hasLaunch);
    const gitMode = String(app.gitDefaultMode || "lazy").trim().toLowerCase();
    const overlayMode = String(app.overlayDefaultMode || "").trim().toLowerCase();
    const effectiveMode = String(app.effectiveDefaultMode || overlayMode || gitMode || "lazy")
      .trim()
      .toLowerCase();
    const selectedMode = resolveSelectedMode(app, modeSelections);
    const generations = Array.isArray(app.generations) ? app.generations : [];
    const hasCurrentBundle = generations.some((gen) => gen.isCurrent);
    const hasAnyBundle = generations.length > 0;
    const canEnableExisting = hasLaunch && hasCurrentBundle;
    const pending = state.pendingByApp[appId] || null;
    const startedAt = run?.startedAtMs ? Number(run.startedAtMs) : null;
    const duration = isReady && startedAt ? formatDuration(now - startedAt) : null;
    const loadUi = loadStateLabel(app, runningByApp);
    const phaseLabel = pending ? pending.label || "处理中…" : loadUi.label;
    let stoppedDetail = loadUi.detail;
    if (!hasLaunch) stoppedDetail = "无 app.toml · 启用将自动创建";
    else if (!hasCurrentBundle) stoppedDetail = "无编译产物";
    const overlayHint = overlayMode
      ? `临时 ${modeLabel(overlayMode)}`
      : `默认 ${modeLabel(gitMode)}`;
    const statusTone = pending
      ? "is-pending"
      : loadUi.tone || "";
    const statusBlock = pending
      ? `<div class="mei-runtime-control__status-chip is-pending">
           <span class="mei-runtime-control__status-dot" aria-hidden="true"></span>
           <strong>${escapeHtml(phaseLabel)}</strong>
           <span>${escapeHtml(pending.label || "处理中…")}</span>
         </div>
         ${pendingProgressHtml(pending)}`
      : isLoaded
        ? `<div class="mei-runtime-control__status-chip is-running">
             <span class="mei-runtime-control__status-dot" aria-hidden="true"></span>
             <strong>${escapeHtml(phaseLabel)}</strong>
             <span data-runtime-uptime data-started-at="${startedAt || ""}">${escapeHtml(duration ? `已运行 ${duration}` : "—")}</span>
           </div>
           <dl class="mei-runtime-control__status-meta">
             <div><dt>载入</dt><dd>${escapeHtml(formatClock(startedAt))}</dd></div>
             <div><dt>模式</dt><dd><code>${escapeHtml(effectiveMode)}</code> · ${escapeHtml(overlayHint)}</dd></div>
             <div><dt>配置</dt><dd><code>${escapeHtml(app.launchPath || `apps/${appId}/app.toml`)}</code></dd></div>
           </dl>`
        : isStarting
          ? `<div class="mei-runtime-control__status-chip is-pending">
               <span class="mei-runtime-control__status-dot" aria-hidden="true"></span>
               <strong>${escapeHtml(phaseLabel)}</strong>
               <span>进程尚未就绪</span>
             </div>
             <dl class="mei-runtime-control__status-meta">
               <div><dt>模式</dt><dd><code>${escapeHtml(effectiveMode)}</code> · ${escapeHtml(overlayHint)}</dd></div>
             </dl>`
          : `<div class="mei-runtime-control__status-chip${statusTone ? ` ${statusTone}` : " is-disabled"}">
               <span class="mei-runtime-control__status-dot" aria-hidden="true"></span>
               <strong>${escapeHtml(phaseLabel)}</strong>
               <span>${escapeHtml(stoppedDetail)}</span>
             </div>
             <dl class="mei-runtime-control__status-meta">
               <div><dt>准入</dt><dd>${isEnabled ? "已启用" : "未启用"}</dd></div>
               <div><dt>模式</dt><dd><code>${escapeHtml(effectiveMode)}</code> · ${escapeHtml(overlayHint)}</dd></div>
               <div><dt>配置</dt><dd><code>${escapeHtml(app.launchPath || `apps/${appId}/app.toml`)}</code></dd></div>
             </dl>`;
    const genRows = generations.length
      ? generations
          .map((gen) => {
            const protectedReasons = Array.isArray(gen.protectedReasons) ? gen.protectedReasons : [];
            const isProtected = gen.isCurrent || protectedReasons.length > 0;
            const loadLocked = isRunning;
            return `<li class="mei-runtime-control__bundle-row${gen.isCurrent ? " is-current" : ""}">
              <div class="mei-runtime-control__bundle-main">
                <code>${escapeHtml(gen.id)}</code>
                ${gen.isCurrent ? '<span class="mei-runtime-control__badge is-clean">current</span>' : ""}
                ${isProtected && !gen.isCurrent ? `<span class="mei-runtime-control__badge">${escapeHtml(protectedReasons.join(", "))}</span>` : ""}
              </div>
              <div class="mei-runtime-control__bundle-meta">
                <span>${escapeHtml(gen.createdAt || "—")}</span>
                <span>${formatBytes(gen.bytes)}</span>
                <button class="mei-host-shell__btn mei-host-shell__btn--ghost mei-runtime-control__btn-compact" type="button" data-runtime-load-generation data-app="${escapeHtml(appId)}" data-generation="${escapeHtml(gen.id)}"${lockedAttr(loadLocked)} title="${loadLocked ? "请先停用或卸载应用" : "切换到该 Bundle 并用当前模式载入"}">载入 Bundle</button>
              </div>
            </li>`;
          })
          .join("")
      : `<li class="mei-runtime-control__bundle-empty">尚无历史 Bundle</li>`;
    const actions = pending
      ? `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" disabled data-runtime-locked>处理中…</button>`
      : isEnabled
        ? `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-app-stop data-app="${escapeHtml(appId)}">停用</button>
           ${
             isLoaded || isStarting
               ? `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-app-reload data-app="${escapeHtml(appId)}" title="卸载进程后立刻再载入（保持启用）">重载</button>`
               : `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-app-reload data-app="${escapeHtml(appId)}" title="立即载入进程（不等待首访）">立即载入</button>`
           }
           <button class="mei-host-shell__btn" type="button" data-runtime-app-compile-load data-app="${escapeHtml(appId)}" title="prebuild 后重载">编译并重载</button>`
        : `<button class="mei-host-shell__btn mei-host-shell__btn--primary" type="button" data-runtime-app-start data-app="${escapeHtml(appId)}"${lockedAttr(!canEnableExisting)} title="${!hasCurrentBundle ? "尚无 current 编译产物，请先编译并启用" : "启用：hot 立刻载入，lazy/frozen 仅准入"}">启用</button>
           <button class="mei-host-shell__btn" type="button" data-runtime-app-compile-load data-app="${escapeHtml(appId)}" title="先 prebuild（若无 app.toml 将自动创建），再启用">编译并启用</button>`;
    const enterLink =
      !pending && isEnabled && app.href
        ? `<a class="mei-runtime-control__enter" href="${escapeHtml(app.href)}">${isLoaded ? "进入" : "进入（将载入）"}</a>`
        : "";
    detailMount.innerHTML = `<div class="mei-runtime-control__detail-surface${statusTone ? ` ${statusTone}` : " is-disabled"}" data-app-card="${escapeHtml(appId)}">
      <header class="mei-runtime-control__app-card-head">
        <div class="mei-runtime-control__app-card-identity">
          <h3 class="mei-runtime-control__app-card-title">${escapeHtml(app.displayName || appId)}</h3>
          <p class="mei-runtime-control__app-card-id"><code>${escapeHtml(appId)}</code></p>
        </div>
        ${enterLink}
      </header>
      <div class="mei-runtime-control__app-card-status">${statusBlock}</div>
      <div class="mei-runtime-control__app-card-launch">
        <label class="mei-runtime-control__mode-field">
          <span>运行模式</span>
          <select data-runtime-mode-select data-app="${escapeHtml(appId)}" aria-label="${escapeHtml(appId)} 运行模式"${lockedAttr(Boolean(pending))}>${modeOptionsHtml(selectedMode, gitMode)}</select>
        </label>
      </div>
      <div class="mei-runtime-control__app-card-actions">${actions}</div>
      <details class="mei-runtime-control__bundles"${bundlesOpen ? " open" : ""}>
        <summary>
          <span>历史 Bundle</span>
          <span class="mei-runtime-control__bundles-count">${generations.length}</span>
        </summary>
        <ul class="mei-runtime-control__bundle-list">${genRows}</ul>
        <div class="mei-runtime-control__bundle-footer">
          <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-app-cleanup data-app="${escapeHtml(appId)}"${lockedAttr(!hasAnyBundle)} title="${hasAnyBundle ? "预览并清理未保护历史 Bundle" : "暂无历史 Bundle"}">清理历史…</button>
        </div>
      </details>
    </div>`;
    setBusy(state.busy);
  }

  function rebuildNavList() {
    const mount = root.querySelector("[data-runtime-app-grid]");
    if (!mount) return;
    ensureShell(mount);
    renderToolbar(mount);
    const nav = mount.querySelector("[data-runtime-app-nav]");
    if (!nav) return;
    const scrollTop = nav.scrollTop;
    const apps = Array.isArray(state.appsOverview?.apps) ? state.appsOverview.apps : [];
    const running = Array.isArray(state.appsOverview?.running) ? state.appsOverview.running : [];
    const runningByApp = Object.fromEntries(running.map((row) => [row.appId, row]));
    if (!apps.length) {
      state.shellReady = false;
      mount.innerHTML = `<p class="mei-host-shell__message">工作区暂无应用。可在 apps/ 下创建应用后刷新。</p>`;
      return;
    }
    const needle = String(state.query || "").trim().toLocaleLowerCase();
    const filtered = sortRuntimeApps(
      apps.filter((app) => appMatchesQuery(app, needle)),
      state.sort || { field: "name", dir: "asc" },
    );
    if (!state.selectedAppId) state.selectedAppId = readSelectedAppFromUrl();
    if (!filtered.some((app) => app.appId === state.selectedAppId)) {
      state.selectedAppId = filtered[0]?.appId || apps[0].appId || "";
    }
    syncExplorerUrl();
    nav.dataset.view = state.viewMode === "list" ? "list" : "card";
    nav.innerHTML = filtered.length
      ? filtered.map((app) => navCardHtml(app, runningByApp)).join("")
      : `<div class="mei-runtime-control__nav-empty">${needle ? "没有匹配的应用" : "暂无应用"}</div>`;
    nav.scrollTop = scrollTop;
    paintDetail();
  }

  function renderAppCards() {
    const mount = root.querySelector("[data-runtime-app-grid]");
    if (!mount) return;
    if (!state.query && !state.viewMode) {
      const urlState = readExplorerUrlState();
      state.query = urlState.q;
      state.viewMode = urlState.view === "list" ? "list" : "card";
      state.sort = urlState.sort || { field: "name", dir: "asc" };
      if (!state.selectedAppId) state.selectedAppId = urlState.sel;
    } else {
      const urlState = readExplorerUrlState();
      if (!state.selectedAppId) state.selectedAppId = urlState.sel;
      if (state.query === "" && urlState.q) state.query = urlState.q;
      if (!state._viewBootstrapped) {
        state.viewMode = urlState.view === "list" ? "list" : state.viewMode || "card";
        state.sort = urlState.sort || state.sort || { field: "name", dir: "asc" };
        state._viewBootstrapped = true;
      }
    }
    rebuildNavList();
  }

  function selectApp(appId) {
    if (!appId || appId === state.selectedAppId) return;
    const select = root.querySelector("[data-runtime-mode-select]");
    if (select) state.modeSelections[select.getAttribute("data-app")] = select.value;
    state.selectedAppId = appId;
    syncExplorerUrl();
    paintSelection();
    paintDetail();
  }

  function closeCleanupModal() {
    state.cleanupPreview = null;
    state.cleanupAppId = null;
    const modal = root?.querySelector("[data-runtime-cleanup-modal]");
    if (!modal) return;
    modal.hidden = true;
    modal.innerHTML = "";
    modal.classList.remove("is-open");
  }

  function renderCleanupModal(mode) {
    const modal = root?.querySelector("[data-runtime-cleanup-modal]");
    if (!modal) return;
    const appId = state.cleanupAppId;
    if (!appId) {
      closeCleanupModal();
      return;
    }
    modal.hidden = false;
    modal.classList.add("is-open");
    if (mode === "loading") {
      modal.innerHTML = `<div class="mei-runtime-cleanup-modal__backdrop" data-runtime-cleanup-cancel data-app="${escapeHtml(appId)}"></div>
        <div class="mei-runtime-cleanup-modal__panel" role="dialog" aria-modal="true" aria-labelledby="runtime-cleanup-title">
          <header class="mei-runtime-cleanup-modal__head">
            <div>
              <p class="mei-runtime-cleanup-modal__eyebrow">历史 Bundle</p>
              <h3 id="runtime-cleanup-title">清理 ${escapeHtml(appId)}</h3>
            </div>
            <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-cleanup-cancel data-app="${escapeHtml(appId)}" aria-label="关闭">关闭</button>
          </header>
          <p class="mei-host-shell__message">正在生成清理预览…</p>
        </div>`;
      return;
    }
    const preview = state.cleanupPreview;
    if (!preview) {
      closeCleanupModal();
      return;
    }
    const entries = (preview.report?.entries || []).filter((entry) => entry.appId === appId);
    const removable = entries.filter((entry) => !entry.protected);
    const retained = entries.filter((entry) => entry.protected);
    const totalBytes = removable.reduce((sum, entry) => sum + (Number(entry.bytes) || 0), 0);
    const body = !removable.length
      ? `<p class="mei-host-shell__message">没有可清理的未保护历史 Bundle。</p>
         ${
           retained.length
             ? `<ul class="mei-runtime-cleanup-modal__list">${retained
                 .map(
                   (entry) =>
                     `<li class="is-protected"><code>${escapeHtml(entry.generation)}</code> · ${formatBytes(entry.bytes)} · ${escapeHtml((entry.reasons || []).join("、") || "受保护")}</li>`,
                 )
                 .join("")}</ul>`
             : ""
         }`
      : `<p class="mei-runtime-control__validation is-invalid" role="alert">将永久删除 ${removable.length} 个目录（${formatBytes(totalBytes)}）。受保护代次不会删除。</p>
         <ul class="mei-runtime-cleanup-modal__list">${removable
           .map(
             (entry) =>
               `<li>
                  <div><code>${escapeHtml(entry.generation)}</code> · ${formatBytes(entry.bytes)}</div>
                  <div class="mei-runtime-cleanup-modal__path">${overflowText(entry.path, "查看完整路径")}</div>
                </li>`,
           )
           .join("")}</ul>
         <label class="mei-runtime-control__danger-confirm">
           <input type="checkbox" data-runtime-cleanup-confirm data-app="${escapeHtml(appId)}" />
           确认删除上述未保护目录
         </label>`;
    modal.innerHTML = `<div class="mei-runtime-cleanup-modal__backdrop" data-runtime-cleanup-cancel data-app="${escapeHtml(appId)}"></div>
      <div class="mei-runtime-cleanup-modal__panel" role="dialog" aria-modal="true" aria-labelledby="runtime-cleanup-title">
        <header class="mei-runtime-cleanup-modal__head">
          <div>
            <p class="mei-runtime-cleanup-modal__eyebrow">历史 Bundle</p>
            <h3 id="runtime-cleanup-title">清理 ${escapeHtml(appId)}</h3>
          </div>
          <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-cleanup-cancel data-app="${escapeHtml(appId)}" aria-label="关闭">关闭</button>
        </header>
        <div class="mei-runtime-cleanup-modal__body">${body}</div>
        <footer class="mei-runtime-cleanup-modal__foot">
          <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-cleanup-cancel data-app="${escapeHtml(appId)}">取消</button>
          <button class="mei-host-shell__btn" type="button" data-runtime-cleanup-execute data-app="${escapeHtml(appId)}" disabled>确认清理</button>
        </footer>
      </div>`;
  }

  function renderAll() {
    renderGlobalOps();
    renderAppCards();
    setBusy(state.busy);
  }

  function refreshDurations() {
    const now = Date.now();
    root?.querySelectorAll("[data-runtime-uptime]").forEach((node) => {
      const started = Number(node.getAttribute("data-started-at") || 0);
      if (!started) return;
      node.textContent = `已运行 ${formatDuration(now - started)}`;
    });
  }

  async function loadApps() {
    state.appsOverview = await requestJson(APPS_API);
    renderAppCards();
    setBusy(state.busy);
  }

  async function refreshOps() {
    try {
      state.ops = await requestJson(OPS_STATUS_API);
      renderGlobalOps();
      if (state.opsTimer) global.clearTimeout(state.opsTimer);
      state.opsTimer =
        state.ops?.job?.status === "running" ? global.setTimeout(refreshOps, 1500) : null;
      setBusy(state.busy);
    } catch (error) {
      announce(`任务状态读取失败：${error.message}`, "error");
    }
  }

  async function waitForOpsIdle(timeoutMs) {
    const deadline = Date.now() + (timeoutMs || 10 * 60 * 1000);
    while (Date.now() < deadline) {
      await refreshOps();
      if (!state.ops?.job || state.ops.job.status !== "running") {
        const last = state.ops?.lastJob;
        if (last && last.status === "failed") {
          throw new Error(last.message || last.error || "任务失败");
        }
        return last;
      }
      await new Promise((resolve) => global.setTimeout(resolve, 1200));
    }
    throw new Error("等待编译超时");
  }

  function selectedModeForApp(appId) {
    const select = root.querySelector(`[data-runtime-mode-select][data-app="${appId}"]`);
    return String(select?.value || "").trim().toLowerCase();
  }

  function startBodyForApp(appId) {
    const mode = selectedModeForApp(appId);
    if (mode === "hot" || mode === "lazy" || mode === "frozen") {
      return { mode };
    }
    const app = (state.appsOverview?.apps || []).find((row) => row.appId === appId);
    const git = String(app?.gitDefaultMode || "lazy").trim().toLowerCase();
    return {
      mode: git === "hot" || git === "lazy" || git === "frozen" ? git : "lazy",
    };
  }

  function modeAnnounceLabel(body) {
    if (body?.mode) return body.mode;
    return "lazy";
  }

  function bindRuntimeSplitter(mount) {
    const splitter = mount.querySelector("[data-runtime-splitter]");
    const explorer = mount;
    if (!splitter || !explorer) return;
    const applyWidth = (pct) => {
      const next = Math.min(72, Math.max(28, pct));
      state.navWidthPct = next;
      explorer.style.setProperty("--nav-width", `${next}%`);
    };
    applyWidth(state.navWidthPct || 50);
    const onPointerMove = (event) => {
      if (!state._resizing) return;
      const rect = explorer.getBoundingClientRect();
      if (!rect.width) return;
      applyWidth(((event.clientX - rect.left) / rect.width) * 100);
    };
    const onPointerUp = () => {
      if (!state._resizing) return;
      state._resizing = false;
      splitter.classList.remove("is-dragging");
      explorer.classList.remove("is-resizing");
      global.removeEventListener("pointermove", onPointerMove);
      global.removeEventListener("pointerup", onPointerUp);
    };
    splitter.onpointerdown = (event) => {
      if (event.button != null && event.button !== 0) return;
      event.preventDefault();
      state._resizing = true;
      splitter.classList.add("is-dragging");
      explorer.classList.add("is-resizing");
      global.addEventListener("pointermove", onPointerMove);
      global.addEventListener("pointerup", onPointerUp);
    };
  }

  async function startAppWithLaunch(appId, _configOverride) {
    if (!appId) return;
    // Capture mode before pending re-render rebuilds the select.
    const startBody = startBodyForApp(appId);
    setAppPending(
      appId,
      "starting",
      `正在启用（mode=${modeAnnounceLabel(startBody)}）…`,
    );
    setBusy(true);
    let keepPending = false;
    try {
      const payload = await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/enable`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(startBody),
      });
      const loaded = payload?.loaded === true;
      announce(
        loaded
          ? `已启用并载入 ${appId} · ${modeAnnounceLabel(startBody)}`
          : `已启用 ${appId}（未载入，等待首访）· ${modeAnnounceLabel(startBody)}`,
        "success",
      );
      await loadApps();
    } catch (error) {
      if (isStartInFlightError(error)) {
        keepPending = true;
        announce(`启用/载入进行中：${appId}（请勿重复点击）`, "neutral");
        setAppPending(appId, "starting", "载入进行中…");
        await loadApps();
      } else {
        announce(`启用失败：${error.message}`, "error");
      }
    } finally {
      if (!keepPending) clearAppPending(appId);
      setBusy(false);
    }
  }

  function isStartInFlightError(error) {
    return Boolean(
      error &&
        error.status === 409 &&
        (error.body?.kind === "app-start-in-flight" ||
          String(error.message || "").includes("app-start-in-flight")),
    );
  }

  async function stopAppRuntime(appId, { confirm = true } = {}) {
    if (!appId) return;
    if (confirm && !global.confirm(`确认停用应用 ${appId}？（将禁止访问并卸载进程）`)) return;
    setBusy(true);
    try {
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/disable`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      announce(`已停用 ${appId}`, "success");
      await loadApps();
    } catch (error) {
      announce(`停用失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function reloadApp(appId) {
    if (!appId) return;
    const startBody = startBodyForApp(appId);
    if (
      !global.confirm(
        `确认载入/重载 ${appId}（统一模式 ${modeAnnounceLabel(startBody)}）？`,
      )
    ) {
      return;
    }
    setAppPending(appId, "reloading", `正在载入（mode=${modeAnnounceLabel(startBody)}）…`);
    setBusy(true);
    try {
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/reload`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(startBody),
      });
      announce(`已载入 ${appId} · ${modeAnnounceLabel(startBody)}`, "success");
      await loadApps();
    } catch (error) {
      announce(`载入失败：${error.message}`, "error");
    } finally {
      clearAppPending(appId);
      setBusy(false);
    }
  }

  async function compileAndLoad(appId) {
    if (!appId) return;
    const config = "launch";
    const startBody = startBodyForApp(appId);
    const running = (state.appsOverview?.running || []).some((row) => row.appId === appId);
    const enabled = (state.appsOverview?.apps || []).some(
      (row) => row.appId === appId && row.enabled === true,
    );
    const actionLabel = running || enabled ? "编译并重载" : "编译并启用";
    if (
      !global.confirm(
        `${actionLabel} ${appId}（统一模式 ${modeAnnounceLabel(startBody)}）？`,
      )
    ) {
      return;
    }
    setAppPending(appId, "compiling", `正在编译 ${appId}…`);
    setBusy(true);
    try {
      if (running) {
        await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/unload`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({}),
        });
      }
      await requestJson(OPS_PREBUILD_API, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ app_id: appId, config }),
      });
      announce(`正在编译 ${appId}…`, "neutral");
      await waitForOpsIdle();
      setAppPending(
        appId,
        "starting",
        `编译完成，正在启用/载入（mode=${modeAnnounceLabel(startBody)}）…`,
      );
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/reload`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(startBody),
      });
      announce(
        `已${actionLabel} ${appId} · ${modeAnnounceLabel(startBody)}`,
        "success",
      );
      await loadApps();
    } catch (error) {
      announce(`${actionLabel}失败：${error.message}`, "error");
    } finally {
      clearAppPending(appId);
      setBusy(false);
    }
  }

  async function loadGenerationAndStart(appId, generation) {
    if (!appId || !generation) return;
    const startBody = startBodyForApp(appId);
    if (
      !global.confirm(
        `将 ${appId} 切换到 ${generation} 并以模式 ${modeAnnounceLabel(startBody)} 载入？`,
      )
    ) {
      return;
    }
    setBusy(true);
    try {
      const url = `${ACTIVATE_ENV_API}?appId=${encodeURIComponent(appId)}&envVersion=${encodeURIComponent(generation)}`;
      await requestJson(url, { method: "POST" });
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/reload`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(startBody),
      });
      announce(
        `已载入 Bundle ${generation} · ${appId} · ${modeAnnounceLabel(startBody)}`,
        "success",
      );
      await loadApps();
    } catch (error) {
      announce(`载入失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function previewCleanup(appId) {
    if (!appId) return;
    state.cleanupAppId = appId;
    renderCleanupModal("loading");
    try {
      state.cleanupPreview = await requestJson(
        `${CLEANUP_PREVIEW_API}?appId=${encodeURIComponent(appId)}`,
        { method: "POST" },
      );
      renderCleanupModal("ready");
      announce(`已打开 ${appId} 清理预览`, "neutral");
    } catch (error) {
      closeCleanupModal();
      announce(`清理预览失败：${error.message}`, "error");
    }
  }

  async function executeCleanup(appId) {
    if (!appId || !state.cleanupPreview || state.cleanupAppId !== appId) return;
    setBusy(true);
    try {
      await requestJson(CLEANUP_API, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          previewToken: state.cleanupPreview.previewToken,
          revision: state.cleanupPreview.revision,
        }),
      });
      closeCleanupModal();
      announce(`已开始清理 ${appId} 历史 Bundle`, "success");
      await refreshOps();
      await waitForOpsIdle(5 * 60 * 1000).catch(() => null);
      await loadApps();
    } catch (error) {
      announce(`清理失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  function handleHostEvent(event) {
    const detail = event.detail || {};
    if (detail.type === "host-resync") {
      void Promise.all([loadApps(), refreshOps()]);
    } else if (detail.type === "job-phase" && detail.payload) {
      const job = detail.payload;
      state.ops = state.ops || {};
      if (job.status === "running") {
        state.ops.job = job;
        const appId = job.appId || job.app_id;
        if (appId && state.pendingByApp[appId]) {
          setAppPending(
            appId,
            "compiling",
            `编译中：${job.phase || job.kind || "running"}${job.generation ? ` · ${job.generation}` : ""}`,
          );
        }
      } else {
        state.ops.job = null;
        state.ops.lastJob = job;
        void loadApps();
      }
      renderGlobalOps();
      setBusy(state.busy);
    } else if (detail.type === "app-starting" && detail.payload?.appId) {
      setAppPending(
        detail.payload.appId,
        "starting",
        detail.payload.message || `正在启动（${detail.payload.launchId || "default"}）…`,
      );
    } else if (
      detail.type === "app-started" ||
      detail.type === "app-stopped" ||
      detail.type === "app-config-switched" ||
      detail.type === "instance-ready" ||
      detail.type === "instance-failed" ||
      detail.type === "route-cutover" ||
      detail.type === "route-rollback" ||
      detail.type === "generation-activated" ||
      detail.type === "generation-rolled-back"
    ) {
      const appId = detail.payload?.appId;
      if (appId) clearAppPending(appId);
      void loadApps();
    }
  }

  function bindEvents() {
    root.addEventListener("click", (event) => {
      const viewBtn = event.target.closest("[data-runtime-view]");
      if (viewBtn && root.contains(viewBtn)) {
        const mode = viewBtn.getAttribute("data-runtime-view") === "list" ? "list" : "card";
        if (mode !== state.viewMode) {
          state.viewMode = mode;
          syncExplorerUrl();
          rebuildNavList();
        }
        return;
      }
      const selectCard = event.target.closest("[data-runtime-select-app]");
      if (selectCard && root.contains(selectCard) && !event.target.closest("button, a, select")) {
        selectApp(selectCard.getAttribute("data-runtime-select-app") || "");
        return;
      }
      const cancelEl = event.target.closest("[data-runtime-cleanup-cancel]");
      if (cancelEl && root.contains(cancelEl)) {
        closeCleanupModal();
        return;
      }
      const target = event.target.closest("button, a");
      if (!target || !root.contains(target)) return;
      if (target.matches("[data-runtime-expand]")) {
        event.preventDefault();
        const text = target.getAttribute("data-runtime-full-text") || "";
        void overflowModule().then((mod) => {
          if (mod?.showFloatingTextPopover) mod.showFloatingTextPopover(target, text);
        });
        return;
      }
      const appId = target.getAttribute("data-app");
      if (target.matches("[data-runtime-app-start]")) {
        void startAppWithLaunch(appId);
      } else if (target.matches("[data-runtime-app-stop]")) {
        void stopAppRuntime(appId);
      } else if (target.matches("[data-runtime-app-reload]")) {
        void reloadApp(appId);
      } else if (target.matches("[data-runtime-app-compile-load]")) {
        void compileAndLoad(appId);
      } else if (target.matches("[data-runtime-app-cleanup]")) {
        void previewCleanup(appId);
      } else if (target.matches("[data-runtime-cleanup-execute]")) {
        void executeCleanup(appId);
      } else if (target.matches("[data-runtime-load-generation]")) {
        void loadGenerationAndStart(appId, target.getAttribute("data-generation"));
      }
    });

    root.addEventListener("input", (event) => {
      const target = event.target;
      if (target.matches("[data-runtime-search]")) {
        if (event.isComposing) return;
        state.query = target.value;
        syncExplorerUrl();
        rebuildNavList();
        queueMicrotask(() => {
          const next = root.querySelector("[data-runtime-search]");
          if (next instanceof HTMLInputElement) {
            next.focus();
            const caret = next.value.length;
            next.setSelectionRange(caret, caret);
          }
        });
      }
    });

    root.addEventListener("change", (event) => {
      const target = event.target;
      if (target.matches("[data-runtime-sort]")) {
        state.sort = parseExplorerSort(target.value, "name");
        syncExplorerUrl();
        rebuildNavList();
        return;
      }
      if (target.matches("[data-runtime-mode-select]")) {
        state.modeSelections[target.getAttribute("data-app")] = target.value;
        return;
      }
      if (target.matches("[data-runtime-cleanup-confirm]")) {
        const execute = root.querySelector(
          `[data-runtime-cleanup-execute][data-app="${target.getAttribute("data-app") || ""}"]`,
        );
        if (execute) {
          const locked = !target.checked || state.busy || state.ops?.job?.status === "running";
          execute.disabled = locked;
          if (!target.checked) execute.setAttribute("data-runtime-locked", "");
          else execute.removeAttribute("data-runtime-locked");
        }
      }
    });

    global.document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && state.cleanupAppId) {
        closeCleanupModal();
      }
    });

    global.document.addEventListener("mei:host-event", handleHostEvent);
  }

  async function init() {
    root = document.querySelector("[data-host-runtime-control-center]");
    if (!root) return;
    const urlState = readExplorerUrlState();
    state.selectedAppId = urlState.sel;
    state.query = urlState.q;
    state.viewMode = urlState.view === "list" ? "list" : "card";
    state.sort = urlState.sort || { field: "name", dir: "asc" };
    state._viewBootstrapped = true;
    bindEvents();
    try {
      await Promise.all([loadApps(), refreshOps()]);
      announce("应用列表已就绪", "success");
    } catch (error) {
      announce(`初始化失败：${error.message}`, "error");
    }
    if (state.durationTimer) global.clearInterval(state.durationTimer);
    state.durationTimer = global.setInterval(refreshDurations, 1000);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => void init());
  } else {
    void init();
  }

  global.MeiHostRuntimeControlCenter = {
    validAppId,
    errorMessage,
    formatBytes,
    formatDuration,
    selectApp,
    paintSelection,
    paintDetail,
    rebuildNavList,
  };
})(typeof window !== "undefined" ? window : globalThis);
