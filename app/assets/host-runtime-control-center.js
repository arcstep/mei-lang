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
    /** @type {Record<string, { kind: string, label: string, startedAt: number }>} */
    pendingByApp: {},
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

  function selectedLaunch(appId) {
    const select = root.querySelector(`[data-runtime-launch-select][data-app="${appId}"]`);
    return select?.value || "";
  }

  function setAppPending(appId, kind, label) {
    if (!appId) return;
    state.pendingByApp[appId] = {
      kind: kind || "busy",
      label: label || "处理中…",
      startedAt: Date.now(),
    };
    renderAppCards();
  }

  function clearAppPending(appId) {
    if (!appId || !state.pendingByApp[appId]) return;
    delete state.pendingByApp[appId];
    renderAppCards();
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

  function renderAppCards() {
    const mount = root.querySelector("[data-runtime-app-grid]");
    if (!mount) return;
    const openBundles = new Set(
      Array.from(mount.querySelectorAll("details.mei-runtime-control__bundles[open]")).map(
        (node) => node.closest("[data-app-card]")?.getAttribute("data-app-card"),
      ).filter(Boolean),
    );
    const launchSelections = {};
    mount.querySelectorAll("[data-runtime-launch-select]").forEach((select) => {
      launchSelections[select.getAttribute("data-app")] = select.value;
    });
    const apps = Array.isArray(state.appsOverview?.apps) ? state.appsOverview.apps : [];
    const running = Array.isArray(state.appsOverview?.running) ? state.appsOverview.running : [];
    const runningByApp = Object.fromEntries(running.map((row) => [row.appId, row]));
    if (!apps.length) {
      mount.innerHTML = `<p class="mei-host-shell__message">工作区暂无应用。可在 apps/ 下创建应用后刷新。</p>`;
      return;
    }
    const now = Date.now();
    mount.innerHTML = apps
      .map((app) => {
        const appId = app.appId || "—";
        const run = runningByApp[appId];
        const isRunning = Boolean(run);
        const launches = Array.isArray(app.launches) ? app.launches : [];
        const hasLaunches = launches.length > 0;
        const preferred =
          launchSelections[appId] ||
          (isRunning && run.launchId) ||
          app.defaultLaunch ||
          launches.find((item) => item.isDefault)?.id ||
          launches[0]?.id ||
          "default";
        const selected = hasLaunches
          ? launches.some((item) => (item.id || "") === preferred)
            ? preferred
            : launches[0]?.id || "default"
          : "default";
        const options = hasLaunches
          ? launches
              .map((launch) => {
                const id = launch.id || "";
                const label = launch.displayName || id;
                const mark = launch.isDefault ? " · default" : "";
                return `<option value="${escapeHtml(id)}"${id === selected ? " selected" : ""}>${escapeHtml(label)}${mark}</option>`;
              })
              .join("")
          : `<option value="default" selected>default（启动时自动创建）</option>`;
        const generations = Array.isArray(app.generations) ? app.generations : [];
        const hasCurrentBundle = generations.some((gen) => gen.isCurrent);
        const hasAnyBundle = generations.length > 0;
        const canUseLaunch = Boolean(selected);
        const canStartExisting = canUseLaunch && hasCurrentBundle;
        const currentLaunchId = run?.launchId || "";
        const launchChanged = isRunning && canUseLaunch && currentLaunchId && selected !== currentLaunchId;
        const pending = state.pendingByApp[appId] || null;
        const startedAt = run?.startedAtMs ? Number(run.startedAtMs) : null;
        const duration =
          isRunning && startedAt ? formatDuration(now - startedAt) : null;
        const phase = pending
          ? pending.kind
          : run?.phase || (isRunning ? "ready" : "stopped");
        let stoppedDetail = "未载入";
        if (!hasLaunches) stoppedDetail = "无 launch · 启动将创建 default";
        else if (!hasCurrentBundle) stoppedDetail = "无编译产物";
        const statusBlock = pending
          ? `<div class="mei-runtime-control__status-chip is-pending">
               <span class="mei-runtime-control__status-dot" aria-hidden="true"></span>
               <strong>${escapeHtml(phase)}</strong>
               <span>${escapeHtml(pending.label || "处理中…")}</span>
             </div>
             ${pendingProgressHtml(pending)}`
          : isRunning
          ? `<div class="mei-runtime-control__status-chip is-running">
               <span class="mei-runtime-control__status-dot" aria-hidden="true"></span>
               <strong>${escapeHtml(phase)}</strong>
               <span data-runtime-uptime data-started-at="${startedAt || ""}">${escapeHtml(duration || "—")}</span>
             </div>
             <dl class="mei-runtime-control__status-meta">
               <div><dt>启动</dt><dd>${escapeHtml(formatClock(startedAt))}</dd></div>
               <div><dt>Launch</dt><dd><code>${escapeHtml(run.launchId || selected || "—")}</code></dd></div>
             </dl>`
          : `<div class="mei-runtime-control__status-chip">
               <span class="mei-runtime-control__status-dot" aria-hidden="true"></span>
               <strong>已停止</strong>
               <span>${escapeHtml(stoppedDetail)}</span>
             </div>`;
        const genRows = generations.length
          ? generations
              .map((gen) => {
                const protectedReasons = Array.isArray(gen.protectedReasons)
                  ? gen.protectedReasons
                  : [];
                const isProtected = gen.isCurrent || protectedReasons.length > 0;
                const loadLocked = isRunning || !canUseLaunch;
                return `<li class="mei-runtime-control__bundle-row${gen.isCurrent ? " is-current" : ""}">
                  <div class="mei-runtime-control__bundle-main">
                    <code>${escapeHtml(gen.id)}</code>
                    ${gen.isCurrent ? '<span class="mei-runtime-control__badge is-clean">current</span>' : ""}
                    ${isProtected && !gen.isCurrent ? `<span class="mei-runtime-control__badge">${escapeHtml(protectedReasons.join(", "))}</span>` : ""}
                  </div>
                  <div class="mei-runtime-control__bundle-meta">
                    <span>${escapeHtml(gen.createdAt || "—")}</span>
                    <span>${formatBytes(gen.bytes)}</span>
                    <button class="mei-host-shell__btn mei-host-shell__btn--ghost mei-runtime-control__btn-compact" type="button" data-runtime-load-generation data-app="${escapeHtml(appId)}" data-generation="${escapeHtml(gen.id)}"${lockedAttr(loadLocked)} title="${loadLocked ? (isRunning ? "请先停止应用" : "需要可用的 launch 配置") : "切换到该 Bundle 并用所选 launch 启动"}">载入并启动</button>
                  </div>
                </li>`;
              })
              .join("")
          : `<li class="mei-runtime-control__bundle-empty">尚无历史 Bundle</li>`;
        const bundlesOpen = openBundles.has(appId) ? " open" : "";
        const actions = pending
          ? `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" disabled data-runtime-locked>处理中…</button>`
          : isRunning
          ? `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-app-stop data-app="${escapeHtml(appId)}">停止</button>
             <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-app-reload data-app="${escapeHtml(appId)}"${lockedAttr(!canUseLaunch || launchChanged)} title="${launchChanged ? "当前所选与运行中 launch 不同，请用「按所选配置重启」" : "按所选 launch 停止后立刻再启动"}">重载</button>
             <button class="mei-host-shell__btn" type="button" data-runtime-app-start data-app="${escapeHtml(appId)}"${lockedAttr(!canUseLaunch || !launchChanged)} title="${!launchChanged ? "所选 launch 与当前相同" : "停止后按所选 launch 重启"}">按所选配置重启</button>
             <button class="mei-host-shell__btn" type="button" data-runtime-app-compile-load data-app="${escapeHtml(appId)}"${lockedAttr(!canUseLaunch)} title="prebuild 后按所选 launch 重启">编译并重启</button>`
          : `<button class="mei-host-shell__btn mei-host-shell__btn--primary" type="button" data-runtime-app-start data-app="${escapeHtml(appId)}"${lockedAttr(!canStartExisting)} title="${!hasCurrentBundle ? "尚无 current 编译产物，请先编译并启动" : "用已有编译产物 + 所选 launch 启动"}">启动</button>
             <button class="mei-host-shell__btn" type="button" data-runtime-app-compile-load data-app="${escapeHtml(appId)}" title="先 prebuild（若无 launch 将自动创建 default），再启动">编译并启动</button>`;
        return `<article class="mei-runtime-control__app-card${isRunning ? " is-running" : ""}${pending ? " is-pending" : ""}" data-app-card="${escapeHtml(appId)}" role="listitem">
          <header class="mei-runtime-control__app-card-head">
            <div class="mei-runtime-control__app-card-identity">
              <h3 class="mei-runtime-control__app-card-title">${escapeHtml(app.displayName || appId)}</h3>
              <p class="mei-runtime-control__app-card-id"><code>${escapeHtml(appId)}</code></p>
            </div>
            ${
              !pending && isRunning && phase === "ready" && app.href
                ? `<a class="mei-runtime-control__enter" href="${escapeHtml(app.href)}">进入</a>`
                : ""
            }
          </header>
          <div class="mei-runtime-control__app-card-status">${statusBlock}</div>
          <label class="mei-runtime-control__app-card-launch">
            <span>启动配置</span>
            <select data-runtime-launch-select data-app="${escapeHtml(appId)}" aria-label="${escapeHtml(appId)} launch"${lockedAttr(Boolean(pending))}>${options}</select>
          </label>
          <div class="mei-runtime-control__app-card-actions">${actions}</div>
          <details class="mei-runtime-control__bundles"${bundlesOpen}>
            <summary>
              <span>历史 Bundle</span>
              <span class="mei-runtime-control__bundles-count">${generations.length}</span>
            </summary>
            <ul class="mei-runtime-control__bundle-list">${genRows}</ul>
            <div class="mei-runtime-control__bundle-footer">
              <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-app-cleanup data-app="${escapeHtml(appId)}"${lockedAttr(!hasAnyBundle)} title="${hasAnyBundle ? "预览并清理未保护历史 Bundle" : "暂无历史 Bundle"}">清理历史…</button>
            </div>
          </details>
        </article>`;
      })
      .join("");
    setBusy(state.busy);
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

  async function startAppWithLaunch(appId, configOverride) {
    if (!appId) return;
    const config = configOverride || selectedLaunch(appId) || "default";
    setAppPending(appId, "starting", `正在启动（${config}）…`);
    setBusy(true);
    try {
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ config }),
      });
      announce(`已启动 ${appId}（${config}）`, "success");
      await loadApps();
    } catch (error) {
      announce(`启动失败：${error.message}`, "error");
    } finally {
      clearAppPending(appId);
      setBusy(false);
    }
  }

  async function stopAppRuntime(appId, { confirm = true } = {}) {
    if (!appId) return;
    if (confirm && !global.confirm(`确认停止应用 ${appId}？`)) return;
    setBusy(true);
    try {
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/stop`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      announce(`已停止 ${appId}`, "success");
      await loadApps();
    } catch (error) {
      announce(`停止失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function reloadApp(appId) {
    if (!appId) return;
    const config = selectedLaunch(appId) || "default";
    if (!global.confirm(`确认重载 ${appId}（${config}）？`)) return;
    setAppPending(appId, "reloading", `正在重载（${config}）…`);
    setBusy(true);
    try {
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/stop`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ config }),
      });
      announce(`已重载 ${appId}`, "success");
      await loadApps();
    } catch (error) {
      announce(`重载失败：${error.message}`, "error");
    } finally {
      clearAppPending(appId);
      setBusy(false);
    }
  }

  async function compileAndLoad(appId) {
    if (!appId) return;
    const config = selectedLaunch(appId) || "default";
    const running = (state.appsOverview?.running || []).some((row) => row.appId === appId);
    const actionLabel = running ? "编译并重启" : "编译并启动";
    if (!global.confirm(`${actionLabel} ${appId}（launch=${config}）？`)) return;
    setAppPending(appId, "compiling", `正在编译 ${appId}…`);
    setBusy(true);
    try {
      if (running) {
        await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/stop`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({}),
        });
      }
      await requestJson(OPS_PREBUILD_API, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ app_id: appId }),
      });
      announce(`正在编译 ${appId}…`, "neutral");
      await waitForOpsIdle();
      setAppPending(appId, "starting", `编译完成，正在启动（${config}）…`);
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ config }),
      });
      announce(`已${actionLabel} ${appId}`, "success");
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
    const config = selectedLaunch(appId) || "default";
    if (!global.confirm(`将 ${appId} 切换到 ${generation} 并用 ${config} 启动？`)) return;
    setBusy(true);
    try {
      const url = `${ACTIVATE_ENV_API}?appId=${encodeURIComponent(appId)}&envVersion=${encodeURIComponent(generation)}`;
      await requestJson(url, { method: "POST" });
      await requestJson(`${APPS_API}/${encodeURIComponent(appId)}/start`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ config }),
      });
      announce(`已载入 ${generation} 并启动 ${appId}`, "success");
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
    if (detail.type === "job-phase" && detail.payload) {
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
      if (target.matches("[data-runtime-refresh-instances]")) {
        void loadApps().then(() => announce("已刷新应用列表", "success"));
      } else if (target.matches("[data-runtime-app-start]")) {
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

    root.addEventListener("change", (event) => {
      const target = event.target;
      if (target.matches("[data-runtime-launch-select]")) {
        renderAppCards();
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
  };
})(typeof window !== "undefined" ? window : globalThis);
