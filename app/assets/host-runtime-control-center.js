/**
 * `/runtime` workspace hub. The app-scoped `/runtime?app=` console remains owned
 * by host-runtime-console.js.
 */
(function (global) {
  "use strict";

  const PROFILE_API = "/api/host/workspace-profiles";
  const CONTROL_PROFILE_API = "/api/host/runtime/profile";
  const OPS_STATUS_API = "/api/host/ops/status";
  const APPLY_PROFILE_API = "/api/host/runtime/apply-profile";
  const BUILDS_API = "/api/host/builds";
  const BUILDS_REQUEST_API = "/api/host/builds/request";
  const CLEANUP_PREVIEW_API = "/api/host/builds/cleanup-preview";
  const CLEANUP_API = "/api/host/builds/cleanup";
  const LAUNCH_MANIFEST_API = "/api/host/launch-manifest";
  const INSTANCES_API = "/api/host/instances";
  const MODES = ["hot", "lazy", "frozen"];
  const state = {
    profiles: [],
    control: null,
    document: null,
    draft: null,
    rawValid: true,
    dirty: false,
    validation: null,
    dryRun: null,
    applyPreview: null,
    busy: false,
    previewToken: 0,
    previewTimer: null,
    ops: null,
    opsTimer: null,
    builds: null,
    cleanupPreview: null,
    launchManifest: null,
    instances: null,
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

  function modeOptions(selected) {
    return MODES.map(
      (mode) => `<option value="${mode}"${mode === selected ? " selected" : ""}>${mode}</option>`,
    ).join("");
  }

  function errorMessage(error) {
    if (!error) return "未知错误";
    if (typeof error === "string") return error;
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

  function profileUrl(id, suffix) {
    return `${PROFILE_API}/${encodeURIComponent(id)}${suffix || ""}`;
  }

  function announce(message, tone) {
    const live = root && root.querySelector("[data-runtime-live]");
    if (!live) return;
    live.innerHTML = overflowText(message, "展开完整状态信息");
    live.dataset.tone = tone || "neutral";
  }

  function renderControlStatus() {
    const status = state.control?.status || "unconfigured";
    const access = state.control?.access?.state || "unconfigured";
    const statusMount = root?.querySelector("[data-runtime-control-status]");
    const accessMount = root?.querySelector("[data-runtime-access-status]");
    if (statusMount) {
      statusMount.innerHTML = `状态 <code>${escapeHtml(status)}</code>`;
    }
    if (accessMount) {
      accessMount.innerHTML = `Access <code>${escapeHtml(access)}</code>`;
    }
  }

  async function loadControlStatus() {
    state.control = await requestJson(CONTROL_PROFILE_API);
    renderControlStatus();
    return state.control;
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

  function getRuntimePlan() {
    return state.draft?.deploy?.runtimePlan || null;
  }

  function ensureRuntimePlan() {
    if (!state.draft || typeof state.draft !== "object" || Array.isArray(state.draft)) return null;
    if (!state.draft.deploy || typeof state.draft.deploy !== "object" || Array.isArray(state.draft.deploy)) {
      state.draft.deploy = {};
    }
    if (
      !state.draft.deploy.runtimePlan ||
      typeof state.draft.deploy.runtimePlan !== "object" ||
      Array.isArray(state.draft.deploy.runtimePlan)
    ) {
      state.draft.deploy.runtimePlan = { defaultMode: "hot", apps: {} };
    }
    const plan = state.draft.deploy.runtimePlan;
    if (!MODES.includes(plan.defaultMode)) plan.defaultMode = "hot";
    if (!plan.apps || typeof plan.apps !== "object" || Array.isArray(plan.apps)) plan.apps = {};
    return plan;
  }

  function ensureAppPlan(appId) {
    const plan = ensureRuntimePlan();
    if (!plan) return null;
    if (!plan.apps[appId] || typeof plan.apps[appId] !== "object") {
      plan.apps[appId] = { targets: [], metricOverrides: {} };
    }
    const app = plan.apps[appId];
    if (!Array.isArray(app.targets)) app.targets = [];
    if (!app.metricOverrides || typeof app.metricOverrides !== "object") app.metricOverrides = {};
    return app;
  }

  function currentRaw() {
    const editor = root && root.querySelector("[data-runtime-json-editor]");
    return editor ? editor.value : "";
  }

  function syncRawFromDraft() {
    const editor = root && root.querySelector("[data-runtime-json-editor]");
    if (editor && state.draft) editor.value = `${JSON.stringify(state.draft, null, 2)}\n`;
    markDirty();
  }

  function markDirty() {
    state.dirty = true;
    state.validation = null;
    state.dryRun = null;
    state.applyPreview = null;
    renderProfile();
    renderValidation();
    renderDryRun();
    schedulePreview();
    setBusy(state.busy);
  }

  function setBusy(busy) {
    state.busy = busy;
    const controlsBusy = busy || state.ops?.job?.status === "running";
    root.querySelectorAll("button, select").forEach((control) => {
      if (control.matches("[data-runtime-expand]")) return;
      control.disabled = controlsBusy;
    });
    const editor = root.querySelector("[data-runtime-json-editor]");
    if (editor) editor.readOnly = busy;
    const apply = root.querySelector("[data-runtime-apply-profile]");
    if (apply) {
      apply.disabled =
        controlsBusy || state.dirty || !state.document || !state.document.validation?.valid;
    }
    const cleanupExecute = root.querySelector("[data-runtime-cleanup-execute]");
    if (cleanupExecute) {
      cleanupExecute.disabled =
        controlsBusy || !root.querySelector("[data-runtime-cleanup-confirm]")?.checked;
    }
  }

  function renderProfileActions() {
    const mount = root.querySelector("[data-runtime-profile-actions]");
    if (!mount) return;
    const selected = state.document?.id || "";
    mount.innerHTML = `<div class="mei-runtime-control__toolbar">
      <label>配置档
        <select data-runtime-profile-select aria-label="选择配置档">
          ${state.profiles
            .map(
              (profile) =>
                `<option value="${escapeHtml(profile.id)}"${profile.id === selected ? " selected" : ""}>${escapeHtml(profile.label || profile.id)}</option>`,
            )
            .join("")}
        </select>
      </label>
      <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-reload-profiles>刷新</button>
      <button class="mei-host-shell__btn" type="button" data-runtime-apply-profile${state.dirty || !state.document?.validation?.valid ? " disabled" : ""}>应用配置档</button>
    </div>`;
  }

  function validationIssuesHtml(validation) {
    const issues = Array.isArray(validation?.issues) ? validation.issues : [];
    if (!issues.length) {
      return `<p class="mei-runtime-control__validation is-valid">校验通过</p>`;
    }
    return `<div class="mei-runtime-control__validation is-invalid" role="alert">
      <strong>发现 ${issues.length} 个问题</strong>
      <ul>${issues
        .map(
          (issue) =>
            `<li><code>${escapeHtml(issue.path || "$")}</code> ${overflowText(issue.message, "展开校验错误")}</li>`,
        )
        .join("")}</ul>
    </div>`;
  }

  function renderProfile() {
    const mount = root.querySelector("[data-runtime-profile-mount]");
    if (!mount) return;
    if (!state.document) {
      mount.innerHTML = `<p class="mei-host-shell__message">没有可读取的配置档。</p>`;
      return;
    }
    const revision = state.document.revision || "—";
    mount.innerHTML = `<div class="mei-runtime-control__profile-meta">
      <dl>
        <div><dt>文件</dt><dd>${overflowText(state.document.file, "展开配置文件路径")}</dd></div>
        <div><dt>Revision</dt><dd>${overflowText(revision, "展开 revision")}</dd></div>
        <div><dt>草稿状态</dt><dd><span class="mei-runtime-control__badge ${state.dirty ? "is-dirty" : "is-clean"}">${state.dirty ? "未保存" : "已同步"}</span></dd></div>
      </dl>
      <form class="mei-runtime-control__copy" data-runtime-copy-form>
        <label for="runtime-copy-id">复制为新配置档</label>
        <div><input id="runtime-copy-id" name="profileId" required pattern="[A-Za-z0-9_.-]+" placeholder="例如 staging-hot" />
        <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="submit">复制</button></div>
      </form>
    </div>`;
  }

  function renderJsonEditor() {
    const mount = root.querySelector("[data-runtime-json-mount]");
    if (!mount || !state.document) return;
    mount.innerHTML = `<details class="mei-runtime-control__raw" open>
      <summary>JSON 原始编辑</summary>
      <textarea data-runtime-json-editor aria-label="配置档 JSON 原始内容" spellcheck="false"></textarea>
      <div class="mei-runtime-control__raw-actions">
        <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-validate>校验并 dry-run</button>
        <button class="mei-host-shell__btn" type="button" data-runtime-save>保存配置档</button>
      </div>
      <div data-runtime-validation></div>
    </details>`;
    mount.querySelector("[data-runtime-json-editor]").value = `${JSON.stringify(state.draft, null, 2)}\n`;
    renderValidation();
  }

  function renderValidation() {
    const mount = root && root.querySelector("[data-runtime-validation]");
    if (!mount) return;
    if (!state.rawValid) {
      mount.innerHTML = `<p class="mei-runtime-control__validation is-invalid" role="alert">JSON 语法无效；结构化编辑与保存已暂停。</p>`;
    } else if (!state.validation) {
      mount.innerHTML = `<p class="mei-runtime-control__validation">草稿待校验。</p>`;
    } else {
      mount.innerHTML = validationIssuesHtml(state.validation);
    }
    const save = root.querySelector("[data-runtime-save]");
    if (save) save.disabled = state.busy || !state.rawValid || !state.dirty;
  }

  function planAppIds(plan) {
    const ids = new Set(Object.keys(plan?.apps || {}));
    (state.dryRun?.discoveredApps || []).forEach((id) => ids.add(id));
    return [...ids].sort((left, right) => {
      if (left === "*") return -1;
      if (right === "*") return 1;
      return left.localeCompare(right);
    });
  }

  function renderTargetRows(appId, app) {
    const targets = Array.isArray(app?.targets) ? app.targets : [];
    return targets.length
      ? targets
          .map(
            (target, index) => `<div class="mei-runtime-control__rule-row">
              <label>Scope
                <input value="${escapeHtml(target.scope || "")}" data-runtime-target-scope data-app="${escapeHtml(appId)}" data-index="${index}" placeholder="home/t1" />
              </label>
              <label>Mode
                <select data-runtime-target-mode data-app="${escapeHtml(appId)}" data-index="${index}">${modeOptions(target.mode)}</select>
              </label>
              <button type="button" class="mei-runtime-control__icon-btn" data-runtime-remove-target data-app="${escapeHtml(appId)}" data-index="${index}" aria-label="删除 scope 规则">删除</button>
            </div>`,
          )
          .join("")
      : `<p class="mei-runtime-control__empty">没有 target 规则，将使用 defaultMode。</p>`;
  }

  function renderMetricRows(appId, app) {
    const entries = Object.entries(app?.metricOverrides || {});
    return entries.length
      ? entries
          .map(
            ([metricId, mode], index) => `<div class="mei-runtime-control__rule-row">
              <label>Metric ID
                <input value="${escapeHtml(metricId)}" data-runtime-metric-id data-app="${escapeHtml(appId)}" data-key="${escapeHtml(metricId)}" />
              </label>
              <label>Mode
                <select data-runtime-metric-mode data-app="${escapeHtml(appId)}" data-key="${escapeHtml(metricId)}">${modeOptions(mode)}</select>
              </label>
              <button type="button" class="mei-runtime-control__icon-btn" data-runtime-remove-metric data-app="${escapeHtml(appId)}" data-key="${escapeHtml(metricId)}" aria-label="删除 metric override ${index + 1}">删除</button>
            </div>`,
          )
          .join("")
      : `<p class="mei-runtime-control__empty">没有 metric override。</p>`;
  }

  function renderPlan() {
    const mount = root.querySelector("[data-runtime-plan-mount]");
    const summary = root.querySelector("[data-runtime-plan-summary]");
    if (!mount) return;
    if (!state.rawValid || !state.draft) {
      mount.innerHTML = `<p class="mei-host-shell__message">修复 JSON 后可继续编辑执行计划。</p>`;
      if (summary) summary.innerHTML = "";
      return;
    }
    const plan = getRuntimePlan();
    const appIds = planAppIds(plan);
    const configuredCount = Object.keys(plan?.apps || {}).length;
    if (summary) {
      summary.innerHTML = `<span class="mei-runtime-control__badge">${configuredCount} 个已配置 app</span>`;
    }
    mount.innerHTML = `<div class="mei-runtime-control__plan-top">
      <label>Default mode
        <select data-runtime-default-mode>${modeOptions(plan?.defaultMode || "hot")}</select>
      </label>
      <form data-runtime-add-app-form>
        <label for="runtime-plan-app">新增 app 规则</label>
        <div><input id="runtime-plan-app" name="appId" list="runtime-discovered-apps" placeholder="app id 或 *" required />
        <button type="submit" class="mei-host-shell__btn mei-host-shell__btn--ghost">新增</button></div>
        <datalist id="runtime-discovered-apps">${(state.dryRun?.discoveredApps || [])
          .map((id) => `<option value="${escapeHtml(id)}"></option>`)
          .join("")}</datalist>
      </form>
    </div>
    ${!plan ? `<p class="mei-runtime-control__notice">当前 JSON 未显式配置 deploy.runtimePlan；首次修改会创建该字段。</p>` : ""}
    <div class="mei-runtime-control__apps">
      ${appIds
        .map((appId) => {
          const configured = !!plan?.apps?.[appId];
          const app = configured ? plan.apps[appId] : { targets: [], metricOverrides: {} };
          return `<article class="mei-runtime-control__app-plan">
            <header><div><h3>${escapeHtml(appId === "*" ? "所有应用（*）" : appId)}</h3>
              <span class="mei-runtime-control__badge ${configured ? "" : "is-inherited"}">${configured ? "已配置" : "继承默认"}</span></div>
              ${configured ? `<button type="button" class="mei-runtime-control__icon-btn" data-runtime-remove-app data-app="${escapeHtml(appId)}">移除 app 规则</button>` : `<button type="button" class="mei-runtime-control__icon-btn" data-runtime-add-app data-app="${escapeHtml(appId)}">配置此 app</button>`}
            </header>
            ${configured ? `<div class="mei-runtime-control__rules">
              <h4>Target scopes</h4>
              ${renderTargetRows(appId, app)}
              <button type="button" class="mei-runtime-control__add-btn" data-runtime-add-target data-app="${escapeHtml(appId)}">+ 添加 target</button>
              <details class="mei-runtime-control__advanced">
                <summary>高级：metricOverrides（${Object.keys(app.metricOverrides || {}).length}）</summary>
                ${renderMetricRows(appId, app)}
                <button type="button" class="mei-runtime-control__add-btn" data-runtime-add-metric data-app="${escapeHtml(appId)}">+ 添加 metric override</button>
              </details>
            </div>` : ""}
          </article>`;
        })
        .join("") || `<p class="mei-runtime-control__empty">dry-run 尚未发现应用，也没有显式 app 规则。</p>`}
    </div>`;
  }

  function checksHtml(items, emptyText) {
    if (!items?.length) return `<p class="mei-runtime-control__empty">${emptyText}</p>`;
    return `<ul>${items
      .map(
        (item) =>
          `<li><code>${escapeHtml(item.appId || "—")}</code> · ${escapeHtml(item.kind || "—")} · ${overflowText(item.value, "展开引用")} <span>${escapeHtml(item.reason || "")}</span></li>`,
      )
      .join("")}</ul>`;
  }

  function renderDryRun() {
    const mount = root.querySelector("[data-runtime-dry-run-mount]");
    if (!mount) return;
    if (!state.dryRun) {
      mount.innerHTML = `<section class="mei-runtime-control__preview"><h3>Dry-run</h3><p class="mei-runtime-control__empty">${state.rawValid ? "等待草稿预检…" : "JSON 无效，无法 dry-run。"}</p></section>`;
      return;
    }
    const targetRules = (state.dryRun.apps || []).reduce((sum, app) => sum + (app.targetRuleCount || 0), 0);
    const metricRules = (state.dryRun.apps || []).reduce(
      (sum, app) => sum + (app.metricOverrideCount || 0),
      0,
    );
    const applyPreview = state.applyPreview;
    mount.innerHTML = `<section class="mei-runtime-control__preview">
      <header><h3>Dry-run</h3><span class="mei-runtime-control__badge is-clean">草稿预检</span></header>
      <dl class="mei-runtime-control__stats">
        <div><dt>Discover apps</dt><dd>${state.dryRun.discoveredApps?.length || 0}</dd></div>
        <div><dt>Target rules</dt><dd>${targetRules}</dd></div>
        <div><dt>Metric overrides</dt><dd>${metricRules}</dd></div>
        <div><dt>Unresolved</dt><dd>${(state.dryRun.unresolvedScopes?.length || 0) + (state.dryRun.unresolvedMetrics?.length || 0)}</dd></div>
        <div><dt>Deferred</dt><dd>${state.dryRun.deferred?.length || 0}</dd></div>
      </dl>
      <details><summary>Unresolved scopes（${state.dryRun.unresolvedScopes?.length || 0}）</summary>${checksHtml(state.dryRun.unresolvedScopes, "无 unresolved scope。")}</details>
      <details><summary>Unresolved metrics（${state.dryRun.unresolvedMetrics?.length || 0}）</summary>${checksHtml(state.dryRun.unresolvedMetrics, "无 unresolved metric。")}</details>
      <details><summary>Deferred（${state.dryRun.deferred?.length || 0}）</summary>${checksHtml(state.dryRun.deferred, "无 deferred 引用。")}</details>
    </section>
    ${
      applyPreview
        ? `<section class="mei-runtime-control__preview" data-runtime-apply-preview>
          <header><h3>应用影响确认</h3><span class="mei-runtime-control__badge">${applyPreview.apps?.length || 0} 个目标 app</span></header>
          <p class="mei-runtime-control__notice">流水线：${escapeHtml(applyPreview.pipeline || "build worker → launch instances → cutover route")}。将复用或构建 bundle，启动 candidate 实例，再切流 route。</p>
          <ul>${(applyPreview.apps || [])
            .map((app) => {
              const bundle =
                app.bundleAction === "reuse"
                  ? `复用 bundle${app.generation ? `（${app.generation}）` : ""}`
                  : "构建 bundle（Build Worker）";
              const launch = app.launchInstance === false ? "不启动实例" : "启动 candidate 实例";
              const cutover = app.cutoverRoute === false ? "不切流" : "cutover route";
              const warmHint = app.warm
                ? `；hot readiness：targets ${app.hotTargets?.length || 0} / metrics ${app.hotMetrics?.length || 0}`
                : "；不预热 hot";
              return `<li><code>${escapeHtml(app.appId)}</code> · ${escapeHtml(bundle)} → ${escapeHtml(launch)} → ${escapeHtml(cutover)}${escapeHtml(warmHint)}</li>`;
            })
            .join("")}</ul>
          <div class="mei-host-shell__actions">
            <button class="mei-host-shell__btn" type="button" data-runtime-confirm-apply>确认应用</button>
            <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-cancel-apply>取消</button>
          </div>
        </section>`
        : ""
    }`;
  }

  function renderManifest() {
    const mount = root.querySelector("[data-runtime-manifest-mount]");
    if (!mount) return;
    const manifest = state.launchManifest?.manifest || state.launchManifest;
    if (!manifest) {
      mount.innerHTML = `<p class="mei-host-shell__message">尚未读取 LaunchManifest。</p>`;
      return;
    }
    const routes = Object.entries(manifest.routes || {});
    const desired = Object.entries(manifest.instances || {});
    const apply = manifest.lastSuccessfulApply;
    mount.innerHTML = `<section class="mei-runtime-control__preview">
      <header><h3>LaunchManifest</h3><span class="mei-runtime-control__badge">${overflowText(manifest.revision || state.launchManifest?.revision || "—", "展开 manifest revision")}</span></header>
      <dl class="mei-runtime-control__stats">
        <div><dt>Desired instances</dt><dd>${desired.length}</dd></div>
        <div><dt>Active routes</dt><dd>${routes.filter(([, route]) => route.active).length}</dd></div>
        <div><dt>Last apply</dt><dd>${overflowText(apply ? `${apply.profileId}@${apply.profileRevision || "—"}` : "—", "展开 lastSuccessfulApply")}</dd></div>
      </dl>
      <details open><summary>Active routes（${routes.length}）</summary>
        ${
          routes.length
            ? `<ul>${routes
                .map(
                  ([appId, route]) =>
                    `<li><code>${escapeHtml(appId)}</code> · active ${overflowText(route.active || "—", "查看 active")} · candidate ${escapeHtml(route.candidate || "—")} · previous ${escapeHtml(route.previous || "—")}</li>`,
                )
                .join("")}</ul>`
            : `<p class="mei-runtime-control__empty">尚无 route binding。</p>`
        }
      </details>
      <details><summary>Desired instances（${desired.length}）</summary>
        ${
          desired.length
            ? `<ul>${desired
                .map(
                  ([id, entry]) =>
                    `<li><code>${escapeHtml(id)}</code> · ${escapeHtml(entry.desiredState || "—")} · ${overflowText(entry.specRef || "—", "展开 specRef")}</li>`,
                )
                .join("")}</ul>`
            : `<p class="mei-runtime-control__empty">尚无 desired instance。</p>`
        }
      </details>
    </section>`;
  }

  function phaseLabel(phase) {
    if (!phase) return "—";
    return String(phase);
  }

  function renderInstances() {
    const mount = root.querySelector("[data-runtime-instances-mount]");
    if (!mount) return;
    const instances = Array.isArray(state.instances?.instances)
      ? state.instances.instances
      : Array.isArray(state.instances)
        ? state.instances
        : [];
    if (!instances.length) {
      mount.innerHTML = `<p class="mei-host-shell__message">当前没有 ObservedInstance；apply 或 reconcile 后会出现在此。</p>`;
      return;
    }
    const revision = state.instances?.revision || state.launchManifest?.revision || "";
    mount.innerHTML = `<div class="mei-runtime-control__generation-list" role="list" aria-label="实例与路由">
      <p class="mei-runtime-control__notice">Manifest revision：${overflowText(revision || "—", "展开 revision")}</p>
      ${instances
        .map((item) => {
          const instanceId = item.instanceId || "—";
          const appId = item.appId || "—";
          const role = item.routeRole || "—";
          const generation = item.resource?.generation || item.revisions?.dataGeneration || "—";
          const phase = phaseLabel(item.phase);
          const endpoint = item.endpoint || "—";
          const canPromote = role === "candidate";
          const canRollback = role === "active" || role === "previous";
          const canStop = role !== "active";
          return `<article class="mei-runtime-control__generation${role === "active" ? " is-active" : ""}" role="listitem">
            <header>
              <div>
                <code>${escapeHtml(instanceId)}</code>
                <span class="mei-runtime-control__badge">${escapeHtml(appId)}</span>
                <span class="mei-runtime-control__badge ${role === "active" ? "is-clean" : ""}">${escapeHtml(role)}</span>
                <span class="mei-runtime-control__badge">${escapeHtml(phase)}</span>
              </div>
              <div class="mei-host-shell__actions">
                ${canPromote ? `<button class="mei-host-shell__btn" type="button" data-runtime-cutover-instance data-app="${escapeHtml(appId)}" data-instance="${escapeHtml(instanceId)}">Promote / Cutover</button>` : ""}
                ${canRollback && role === "active" ? `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-rollback-route data-app="${escapeHtml(appId)}">Rollback</button>` : ""}
                ${canStop ? `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-stop-instance data-instance="${escapeHtml(instanceId)}">Stop</button>` : ""}
                <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-restart-instance data-instance="${escapeHtml(instanceId)}">Restart</button>
              </div>
            </header>
            <dl>
              <div><dt>Generation</dt><dd>${overflowText(generation, "查看 generation")}</dd></div>
              <div><dt>Endpoint</dt><dd>${overflowText(endpoint, "查看 endpoint")}</dd></div>
              <div><dt>Desired</dt><dd>${escapeHtml(item.desiredState || "—")}</dd></div>
              <div><dt>Reachable</dt><dd>${item.reachable ? "yes" : "no"}</dd></div>
              <div><dt>Last error</dt><dd>${overflowText(item.lastError || "—", "展开错误")}</dd></div>
            </dl>
          </article>`;
        })
        .join("")}
    </div>`;
  }

  function renderTask() {
    const mount = root.querySelector("[data-runtime-task-mount]");
    if (!mount) return;
    const job = state.ops?.job;
    const last = state.ops?.lastJob;
    if (job) {
      mount.innerHTML = `<article class="mei-runtime-control__task is-running">
        <div><span class="mei-runtime-control__pulse" aria-hidden="true"></span><strong>${job.status === "running" ? "Builder / ops 进行中" : "任务完成"}</strong></div>
        <dl><div><dt>类型</dt><dd>${escapeHtml(job.kind || "—")}</dd></div>
        <div><dt>阶段</dt><dd>${escapeHtml(job.phase || "queued")}</dd></div>
        <div><dt>配置档</dt><dd>${overflowText(job.profileId ? `${job.profileId}@${job.profileRevision || "—"}` : "—", "展开配置档 revision")}</dd></div>
        <div><dt>Generation</dt><dd>${overflowText(job.generation || "—", "展开 generation")}</dd></div></dl>
        ${
          Array.isArray(job.apps) && job.apps.length
            ? `<ul>${job.apps
                .map(
                  (app) =>
                    `<li><code>${escapeHtml(app.appId)}</code> · ${escapeHtml(app.phase || "queued")} · ${overflowText(app.message || "等待中", "展开 app 进度")}</li>`,
                )
                .join("")}</ul>`
            : ""
        }
        ${
          Array.isArray(job.logSummary) && job.logSummary.length
            ? `<details open><summary>阶段日志（${job.logSummary.length}）</summary><ul>${job.logSummary
                .map((line) => `<li>${overflowText(line, "展开日志摘要")}</li>`)
                .join("")}</ul></details>`
            : ""
        }
      </article>`;
      return;
    }
    if (last) {
      mount.innerHTML = `<article class="mei-runtime-control__task">
        <div><strong>最近 Builder / ops</strong><span class="mei-runtime-control__badge">${escapeHtml(last.status || "—")}</span></div>
        <dl><div><dt>类型</dt><dd>${escapeHtml(last.kind || "—")}</dd></div>
        <div><dt>阶段</dt><dd>${escapeHtml(last.phase || last.status || "—")}</dd></div>
        <div><dt>配置档</dt><dd>${overflowText(last.profileId ? `${last.profileId}@${last.profileRevision || "—"}` : "—", "展开配置档 revision")}</dd></div>
        <div><dt>结果</dt><dd>${overflowText(last.message || last.error || "—", "展开任务结果")}</dd></div></dl>
        ${
          Array.isArray(last.logSummary) && last.logSummary.length
            ? `<details><summary>阶段日志（${last.logSummary.length}）</summary><ul>${last.logSummary
                .map((line) => `<li>${overflowText(line, "展开日志摘要")}</li>`)
                .join("")}</ul></details>`
            : ""
        }
      </article>`;
      return;
    }
    mount.innerHTML = `<p class="mei-host-shell__message">当前没有 Builder / ops job。可请求 Build Worker，或通过应用配置档触发 build → launch → cutover。</p>`;
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

  function renderBuilds() {
    const mount = root.querySelector("[data-runtime-builds-mount]");
    const diagnostic = root.querySelector("[data-runtime-single-app-mount]");
    if (!mount) return;
    const builds = state.builds;
    const generations = Array.isArray(builds?.generations) ? builds.generations : [];
    if (!generations.length) {
      mount.innerHTML = `<p class="mei-host-shell__message">尚未发现 WS-* generation。</p>`;
      if (diagnostic) diagnostic.innerHTML = "";
      return;
    }
    mount.innerHTML = `<div class="mei-runtime-control__generation-list" role="list" aria-label="工作区 generation">
      ${generations
        .map((generation) => {
          const reasons = (generation.protectedReasons || []).join("、") || "—";
          const appSummary = (generation.apps || [])
            .map(
              (app) =>
                `${app.appId}: ${app.current ? "current" : app.valid ? "available" : app.error || "missing"}`,
            )
            .join("；");
          const canActivate = generation.coherent && !generation.active;
          const canRollback = generation.previous && generation.coherent && !generation.active;
          return `<article class="mei-runtime-control__generation${generation.active ? " is-active" : ""}" role="listitem">
            <header>
              <div>
                <code>${escapeHtml(generation.generation)}</code>
                <span class="mei-runtime-control__badge ${generation.coherent ? "is-clean" : "is-dirty"}">${generation.coherent ? "coherent" : "incomplete"}</span>
                ${generation.active ? '<span class="mei-runtime-control__badge is-clean">active</span>' : ""}
                ${generation.candidate ? '<span class="mei-runtime-control__badge">candidate</span>' : ""}
                ${generation.previous ? '<span class="mei-runtime-control__badge">previous</span>' : ""}
              </div>
              <div class="mei-host-shell__actions">
                ${canActivate ? `<button class="mei-host-shell__btn" type="button" data-runtime-activate-generation="${escapeHtml(generation.generation)}">激活整个工作区</button>` : ""}
                ${canRollback ? `<button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-rollback-generation="${escapeHtml(generation.generation)}">回滚到此代次</button>` : ""}
              </div>
            </header>
            <dl>
              <div><dt>大小</dt><dd>${formatBytes(generation.bytes)}</dd></div>
              <div><dt>创建时间</dt><dd>${overflowText(generation.createdAt || "—", "查看完整创建时间")}</dd></div>
              <div><dt>Toolchain</dt><dd>${overflowText(generation.toolchainDigest || "—", "查看完整 toolchain digest")}</dd></div>
              <div><dt>Config digest</dt><dd>${overflowText(generation.configDigest || "—", "查看完整 config digest")}</dd></div>
              <div><dt>保护原因</dt><dd>${overflowText(reasons, "查看全部保护原因")}</dd></div>
              <div><dt>App 状态</dt><dd>${overflowText(appSummary, "查看全部 app generation 状态")}</dd></div>
            </dl>
          </article>`;
        })
        .join("")}
    </div>`;

    if (diagnostic) {
      diagnostic.innerHTML = (builds.apps || [])
        .map((appId) => {
          const options = generations
            .filter((generation) =>
              (generation.apps || []).some((app) => app.appId === appId && app.available),
            )
            .map(
              (generation) =>
                `<option value="${escapeHtml(generation.generation)}">${escapeHtml(generation.generation)}</option>`,
            )
            .join("");
          return `<div class="mei-runtime-control__diagnostic-row">
            <code>${escapeHtml(appId)}</code>
            <select data-runtime-diagnostic-select="${escapeHtml(appId)}" aria-label="${escapeHtml(appId)} 单 app generation">${options}</select>
            <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-diagnostic-activate="${escapeHtml(appId)}">仅切换此 app</button>
          </div>`;
        })
        .join("");
    }
  }

  function renderCleanupPreview() {
    const mount = root.querySelector("[data-runtime-cleanup-mount]");
    if (!mount) return;
    const preview = state.cleanupPreview;
    if (!preview) {
      mount.innerHTML = "";
      return;
    }
    const entries = preview.report?.entries || [];
    const removable = entries.filter((entry) => !entry.protected);
    const totalBytes = removable.reduce((sum, entry) => sum + (Number(entry.bytes) || 0), 0);
    mount.innerHTML = `<section class="mei-runtime-control__preview mei-runtime-control__cleanup" aria-labelledby="runtime-cleanup-heading">
      <header><h3 id="runtime-cleanup-heading">清理预览</h3><span class="mei-runtime-control__badge is-dirty">${removable.length} 个目录 / ${formatBytes(totalBytes)}</span></header>
      <p class="mei-runtime-control__validation is-invalid" role="alert">危险操作：执行后会永久删除下列未保护 env 目录。请核对完整路径与保护原因。</p>
      <ul>${entries
        .map(
          (entry) =>
            `<li class="${entry.protected ? "is-protected" : "is-removable"}">
              <span class="mei-runtime-control__badge ${entry.protected ? "is-clean" : "is-dirty"}">${entry.protected ? "保留" : "删除"}</span>
              ${overflowText(entry.path, "查看完整 env 目录路径")}
              <span>${formatBytes(entry.bytes)} · ${escapeHtml((entry.reasons || []).join("、") || "未保护")}</span>
            </li>`,
        )
        .join("")}</ul>
      <label class="mei-runtime-control__danger-confirm">
        <input type="checkbox" data-runtime-cleanup-confirm />
        我已核对预览，确认永久删除 ${removable.length} 个未保护目录。
      </label>
      <div class="mei-host-shell__actions">
        <button class="mei-host-shell__btn" type="button" data-runtime-cleanup-execute disabled>确认执行清理</button>
        <button class="mei-host-shell__btn mei-host-shell__btn--ghost" type="button" data-runtime-cleanup-cancel>取消</button>
      </div>
    </section>`;
  }

  async function loadBuilds() {
    state.builds = await requestJson(BUILDS_API);
    renderBuilds();
    setBusy(state.busy);
  }

  async function loadLaunchManifest() {
    state.launchManifest = await requestJson(LAUNCH_MANIFEST_API);
    renderManifest();
  }

  async function loadInstances() {
    state.instances = await requestJson(INSTANCES_API);
    renderInstances();
    setBusy(state.busy);
  }

  async function refreshTopology() {
    await Promise.all([loadLaunchManifest(), loadInstances()]);
  }

  async function requestBuildWorker() {
    if (!state.document) {
      announce("请先选择配置档后再请求 Build Worker。", "warning");
      return;
    }
    const apps = (state.dryRun?.discoveredApps || []).filter(Boolean);
    if (!apps.length) {
      announce("dry-run 尚未发现 app，无法请求 Build Worker。", "warning");
      return;
    }
    if (!global.confirm(`将为 ${apps.length} 个 app 请求 Build Worker（异步）。继续？`)) return;
    setBusy(true);
    try {
      await requestJson(BUILDS_REQUEST_API, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          schemaVersion: "mei-build-request-v1",
          profileId: state.document.id,
          profileRevision: state.document.revision,
          profileFile: state.document.file,
          apps,
          wait: false,
        }),
      });
      announce("Build Worker 请求已接受。", "success");
      await refreshOps();
    } catch (error) {
      announce(`Build Worker 请求失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function cutoverInstance(appId, instanceId) {
    const revision = state.instances?.revision || state.launchManifest?.revision;
    if (!appId || !instanceId || !revision) {
      announce("缺少 app / instance / manifest revision，无法 cutover。", "error");
      return;
    }
    if (!global.confirm(`确认将 ${appId} 的 active route cutover 到 ${instanceId}？`)) return;
    setBusy(true);
    try {
      await requestJson(`/api/host/routes/${encodeURIComponent(appId)}/cutover`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          instanceId,
          expectedManifestRevision: revision,
        }),
      });
      announce(`已 cutover ${appId} → ${instanceId}。`, "success");
      await refreshTopology();
    } catch (error) {
      announce(`Cutover 失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function rollbackRoute(appId) {
    const revision = state.instances?.revision || state.launchManifest?.revision;
    if (!appId) return;
    if (!global.confirm(`确认将 ${appId} 回滚到 previous 实例？`)) return;
    setBusy(true);
    try {
      await requestJson(`/api/host/routes/${encodeURIComponent(appId)}/rollback`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          expectedManifestRevision: revision || null,
        }),
      });
      announce(`已 rollback ${appId}。`, "success");
      await refreshTopology();
    } catch (error) {
      announce(`Rollback 失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function stopInstance(instanceId) {
    if (!instanceId) return;
    if (!global.confirm(`确认停止实例 ${instanceId}？`)) return;
    setBusy(true);
    try {
      await requestJson(`/api/host/instances/${encodeURIComponent(instanceId)}/stop`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: "{}",
      });
      announce(`已停止 ${instanceId}。`, "success");
      await refreshTopology();
    } catch (error) {
      announce(`Stop 失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function restartInstance(instanceId) {
    if (!instanceId) return;
    if (!global.confirm(`确认重启实例 ${instanceId}？`)) return;
    setBusy(true);
    try {
      await requestJson(`/api/host/instances/${encodeURIComponent(instanceId)}/restart`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: "{}",
      });
      announce(`已重启 ${instanceId}。`, "success");
      await refreshTopology();
    } catch (error) {
      announce(`Restart 失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function previewCleanup() {
    setBusy(true);
    try {
      state.cleanupPreview = await requestJson(CLEANUP_PREVIEW_API, { method: "POST" });
      renderCleanupPreview();
      announce("已生成清理预览；执行前必须明确确认。", "warning");
    } catch (error) {
      announce(`清理预览失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function executeCleanup() {
    if (!state.cleanupPreview) return;
    const confirmed = root.querySelector("[data-runtime-cleanup-confirm]")?.checked;
    if (!confirmed) {
      announce("请先勾选永久删除确认。", "warning");
      return;
    }
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
      state.cleanupPreview = null;
      renderCleanupPreview();
      announce("清理任务已接受。", "success");
      await refreshOps();
    } catch (error) {
      announce(`清理启动失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function loadProfiles(preferredId) {
    const data = await requestJson(PROFILE_API);
    state.profiles = Array.isArray(data.profiles) ? data.profiles : [];
    const next =
      state.profiles.find((profile) => profile.id === preferredId) ||
      state.profiles.find((profile) => profile.id === state.document?.id) ||
      state.profiles[0];
    renderProfileActions();
    if (next) await loadProfile(next.id);
    else {
      state.document = null;
      renderProfile();
      announce("未找到 workspace.json 或 configs/*.json。", "warning");
    }
  }

  async function loadProfile(id) {
    setBusy(true);
    try {
      const document = await requestJson(profileUrl(id));
      state.document = document;
      state.draft = JSON.parse(JSON.stringify(document.config));
      state.rawValid = true;
      state.dirty = false;
      state.validation = document.validation;
      state.dryRun = null;
      state.applyPreview = null;
      renderProfileActions();
      renderProfile();
      renderJsonEditor();
      renderPlan();
      renderDryRun();
      announce(`已读取 ${document.file}。`);
      await previewDraft({ announceResult: false });
    } catch (error) {
      announce(`读取配置档失败：${error.message}`, "error");
    } finally {
      setBusy(false);
      renderValidation();
    }
  }

  async function previewDraft(options) {
    if (!state.document || !state.draft || !state.rawValid) return false;
    const token = ++state.previewToken;
    const body = JSON.stringify({ config: state.draft });
    try {
      const validated = await requestJson(profileUrl(state.document.id, "/validate"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
      });
      if (token !== state.previewToken) return false;
      state.validation = validated.validation;
      renderValidation();
      if (!state.validation?.valid) {
        state.dryRun = null;
        renderDryRun();
        if (options?.announceResult) announce("草稿校验未通过，未执行 dry-run。", "error");
        return false;
      }
      state.dryRun = await requestJson(profileUrl(state.document.id, "/dry-run"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
      });
      if (token !== state.previewToken) return false;
      renderPlan();
      renderDryRun();
      if (options?.announceResult) announce("草稿校验与 dry-run 已通过。", "success");
      return true;
    } catch (error) {
      if (token !== state.previewToken) return false;
      state.dryRun = null;
      renderDryRun();
      announce(`草稿预检失败：${error.message}`, "error");
      return false;
    }
  }

  function schedulePreview() {
    if (state.previewTimer) global.clearTimeout(state.previewTimer);
    state.previewTimer = global.setTimeout(() => {
      state.previewTimer = null;
      void previewDraft({ announceResult: false });
    }, 450);
  }

  async function saveProfile() {
    if (!state.document || !state.dirty || !state.rawValid) return;
    setBusy(true);
    try {
      const valid = await previewDraft({ announceResult: true });
      if (!valid) return;
      const saved = await requestJson(profileUrl(state.document.id), {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          expectedRevision: state.document.revision,
          config: state.draft,
        }),
      });
      state.document = saved;
      state.draft = JSON.parse(JSON.stringify(saved.config));
      state.validation = saved.validation;
      state.dirty = false;
      state.applyPreview = null;
      renderProfile();
      renderJsonEditor();
      renderPlan();
      announce(`已保存 ${saved.file}。`, "success");
      await loadProfiles(saved.id);
    } catch (error) {
      if (error.status === 409) {
        announce(`保存冲突：文件已被其他进程修改。草稿仍保留，未覆盖服务器内容。${error.message}`, "error");
      } else {
        announce(`保存失败：${error.message}`, "error");
      }
    } finally {
      setBusy(false);
      renderValidation();
    }
  }

  async function copyProfile(profileId) {
    const id = String(profileId || "").trim();
    if (!id || !state.draft) return;
    setBusy(true);
    try {
      await requestJson(profileUrl(id), {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ expectedRevision: null, config: state.draft }),
      });
      announce(`已复制为 configs/${id}.json。`, "success");
      await loadProfiles(id);
    } catch (error) {
      announce(
        error.status === 409
          ? `复制失败：配置档 ${id} 已存在，未覆盖。`
          : `复制失败：${error.message}`,
        "error",
      );
    } finally {
      setBusy(false);
    }
  }

  function applyVisualChange(render) {
    state.rawValid = true;
    syncRawFromDraft();
    if (render !== false) renderPlan();
  }

  function validAppId(value) {
    return value === "*" || /^[A-Za-z0-9_.-]+$/.test(value);
  }

  async function refreshOps() {
    try {
      state.ops = await requestJson(OPS_STATUS_API);
      renderTask();
      const running = state.ops?.job?.status === "running";
      root.querySelectorAll("[data-mei-ops-reload], [data-mei-ops-prebuild], [data-mei-prebuild-app]").forEach(
        (button) => {
          button.disabled = running || state.busy;
        },
      );
      if (state.opsTimer) global.clearTimeout(state.opsTimer);
      state.opsTimer = running ? global.setTimeout(refreshOps, 1500) : null;
    } catch (error) {
      announce(`任务状态读取失败：${error.message}`, "error");
    }
  }

  async function previewProfileApply() {
    if (!state.document || state.dirty || !state.document.validation?.valid) {
      announce("请先保存并确保配置档校验通过；保存不会自动应用。", "warning");
      return;
    }
    setBusy(true);
    try {
      const response = await requestJson(APPLY_PROFILE_API, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          profileId: state.document.id,
          expectedRevision: state.document.revision,
          dryRun: true,
        }),
      });
      state.applyPreview = response.plan || null;
      renderDryRun();
      announce("已生成应用影响预览，请确认后执行。", "success");
    } catch (error) {
      announce(`应用预览失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  async function confirmProfileApply() {
    if (!state.document || !state.applyPreview) return;
    setBusy(true);
    try {
      await requestJson(APPLY_PROFILE_API, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          profileId: state.document.id,
          expectedRevision: state.document.revision,
        }),
      });
      state.applyPreview = null;
      renderDryRun();
      announce("应用配置档任务已进入队列。", "success");
      await refreshOps();
    } catch (error) {
      announce(`应用任务启动失败：${error.message}`, "error");
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
      } else {
        state.ops.job = null;
        state.ops.lastJob = job;
        void loadControlStatus();
        if (String(job.kind || "").startsWith("generation-") || job.kind === "build" || job.kind === "apply-profile") {
          void loadBuilds();
          void refreshTopology();
        }
      }
      renderTask();
      setBusy(state.busy);
    } else if (detail.type === "builder-phase") {
      const payload = detail.payload || {};
      if (payload.job) {
        state.ops = state.ops || {};
        if (payload.job.status === "running") state.ops.job = payload.job;
        else {
          state.ops.job = null;
          state.ops.lastJob = payload.job;
        }
        renderTask();
      }
      if (payload.message) announce(`Builder：${payload.message}`);
    } else if (
      detail.type === "instance-phase" ||
      detail.type === "instance-ready" ||
      detail.type === "instance-failed"
    ) {
      void refreshTopology();
      const payload = detail.payload || {};
      if (detail.type === "instance-failed") {
        announce(`实例失败：${payload.instanceId || "—"} ${payload.lastError || ""}`, "error");
      } else if (detail.type === "instance-ready") {
        announce(`实例就绪：${payload.instanceId || "—"}`, "success");
      }
    } else if (detail.type === "route-cutover" || detail.type === "route-rollback") {
      void refreshTopology();
      void loadBuilds();
      const payload = detail.payload || {};
      announce(
        `${detail.type === "route-cutover" ? "Cutover" : "Rollback"} ${payload.appId || ""} → ${payload.active || "—"}`,
        "success",
      );
    } else if (detail.type === "profile-applied") {
      const payload = detail.payload || {};
      if (payload.profileId === state.document?.id) {
        announce(`配置档 ${payload.profileId} 已应用。`, "success");
      }
      void loadControlStatus();
      void loadBuilds();
      void refreshTopology();
    } else if (
      detail.type === "generation-activated" ||
      detail.type === "generation-rolled-back"
    ) {
      state.cleanupPreview = null;
      renderCleanupPreview();
      void loadBuilds();
      void refreshTopology();
    }
  }

  async function runOps(url, body, reloadAfter) {
    setBusy(true);
    try {
      await requestJson(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body || {}),
      });
      announce("任务已接受。", "success");
      await refreshOps();
      if (reloadAfter) await loadBuilds();
    } catch (error) {
      announce(`任务启动失败：${error.message}`, "error");
    } finally {
      setBusy(false);
    }
  }

  function bindEvents() {
    root.addEventListener("submit", (event) => {
      if (event.target.matches("[data-runtime-copy-form]")) {
        event.preventDefault();
        void copyProfile(new FormData(event.target).get("profileId"));
      } else if (event.target.matches("[data-runtime-add-app-form]")) {
        event.preventDefault();
        const appId = String(new FormData(event.target).get("appId") || "").trim();
        if (!validAppId(appId)) {
          announce("app id 仅允许字母、数字、点、下划线、短横线或 *。", "error");
          return;
        }
        ensureAppPlan(appId);
        applyVisualChange();
      }
    });

    root.addEventListener("input", (event) => {
      const target = event.target;
      if (target.matches("[data-runtime-json-editor]")) {
        try {
          state.draft = JSON.parse(target.value);
          state.rawValid = true;
          markDirty();
          renderPlan();
        } catch (_error) {
          state.rawValid = false;
          state.dirty = true;
          state.validation = null;
          state.dryRun = null;
          renderProfile();
          renderValidation();
          renderPlan();
          renderDryRun();
        }
      } else if (target.matches("[data-runtime-target-scope]")) {
        const app = ensureAppPlan(target.dataset.app);
        const rule = app?.targets?.[Number(target.dataset.index)];
        if (rule) {
          rule.scope = target.value;
          applyVisualChange(false);
        }
      }
    });

    root.addEventListener("change", (event) => {
      const target = event.target;
      if (target.matches("[data-runtime-profile-select]")) {
        void loadProfile(target.value);
      } else if (target.matches("[data-runtime-default-mode]")) {
        ensureRuntimePlan().defaultMode = target.value;
        applyVisualChange();
      } else if (target.matches("[data-runtime-target-mode]")) {
        const app = ensureAppPlan(target.dataset.app);
        const rule = app?.targets?.[Number(target.dataset.index)];
        if (rule) rule.mode = target.value;
        applyVisualChange();
      } else if (target.matches("[data-runtime-metric-mode]")) {
        const app = ensureAppPlan(target.dataset.app);
        if (app) app.metricOverrides[target.dataset.key] = target.value;
        applyVisualChange();
      } else if (target.matches("[data-runtime-metric-id]")) {
        const app = ensureAppPlan(target.dataset.app);
        const oldKey = target.dataset.key;
        const newKey = target.value.trim();
        if (!app || !newKey || newKey === oldKey) return;
        const mode = app.metricOverrides[oldKey];
        delete app.metricOverrides[oldKey];
        app.metricOverrides[newKey] = mode;
        applyVisualChange();
      } else if (target.matches("[data-runtime-cleanup-confirm]")) {
        const execute = root.querySelector("[data-runtime-cleanup-execute]");
        if (execute) execute.disabled = !target.checked || state.busy;
      }
    });

    root.addEventListener("click", (event) => {
      const button = event.target.closest("button");
      if (!button) return;
      if (button.matches("[data-runtime-expand]")) {
        const full = button.getAttribute("data-runtime-full-text") || "";
        void overflowModule()
          .then((module) =>
            module.openOverflowTextPopover(state, full, button, {
              title: button.getAttribute("aria-label") || "查看全文",
              variant: "large",
            }),
          )
          .catch((error) => announce(`全文弹窗加载失败：${error.message}`, "error"));
      } else if (button.matches("[data-runtime-reload-profiles]")) {
        void loadProfiles(state.document?.id);
      } else if (button.matches("[data-runtime-validate]")) {
        void previewDraft({ announceResult: true });
      } else if (button.matches("[data-runtime-save]")) {
        void saveProfile();
      } else if (button.matches("[data-runtime-apply-profile]")) {
        void previewProfileApply();
      } else if (button.matches("[data-runtime-confirm-apply]")) {
        void confirmProfileApply();
      } else if (button.matches("[data-runtime-cancel-apply]")) {
        state.applyPreview = null;
        renderDryRun();
      } else if (button.matches("[data-runtime-refresh-builds]")) {
        void loadBuilds().catch((error) => announce(`generation 刷新失败：${error.message}`, "error"));
      } else if (button.matches("[data-runtime-refresh-instances]")) {
        void refreshTopology().catch((error) => announce(`实例刷新失败：${error.message}`, "error"));
      } else if (button.matches("[data-runtime-builds-request]")) {
        void requestBuildWorker();
      } else if (button.matches("[data-runtime-cutover-instance]")) {
        void cutoverInstance(button.getAttribute("data-app"), button.getAttribute("data-instance"));
      } else if (button.matches("[data-runtime-rollback-route]")) {
        void rollbackRoute(button.getAttribute("data-app"));
      } else if (button.matches("[data-runtime-stop-instance]")) {
        void stopInstance(button.getAttribute("data-instance"));
      } else if (button.matches("[data-runtime-restart-instance]")) {
        void restartInstance(button.getAttribute("data-instance"));
      } else if (button.matches("[data-runtime-activate-generation]")) {
        const generation = button.getAttribute("data-runtime-activate-generation");
        if (
          generation &&
          global.confirm(`确认将全部活动 app 原子切换到 ${generation} 并刷新 registry/cache/bootstrap？`)
        ) {
          void runOps(`/api/host/builds/${encodeURIComponent(generation)}/activate`, {}, false);
        }
      } else if (button.matches("[data-runtime-rollback-generation]")) {
        const generation = button.getAttribute("data-runtime-rollback-generation");
        if (
          generation &&
          global.confirm(`确认使用 links.previous 回滚全部活动 app 到 ${generation}？`)
        ) {
          void runOps(`/api/host/builds/${encodeURIComponent(generation)}/rollback`, {}, false);
        }
      } else if (button.matches("[data-runtime-cleanup-preview]")) {
        void previewCleanup();
      } else if (button.matches("[data-runtime-cleanup-execute]")) {
        void executeCleanup();
      } else if (button.matches("[data-runtime-cleanup-cancel]")) {
        state.cleanupPreview = null;
        renderCleanupPreview();
      } else if (button.matches("[data-runtime-diagnostic-activate]")) {
        const appId = button.getAttribute("data-runtime-diagnostic-activate");
        const select = [...root.querySelectorAll("[data-runtime-diagnostic-select]")].find(
          (entry) => entry.getAttribute("data-runtime-diagnostic-select") === appId,
        );
        const envVersion = select?.value;
        if (
          appId &&
          envVersion &&
          global.confirm(
            `高级诊断警告：仅切换 ${appId} 到 ${envVersion} 会造成工作区不一致。仍要继续？`,
          )
        ) {
          const url = `/api/host/runtime/activate-env?appId=${encodeURIComponent(appId)}&envVersion=${encodeURIComponent(envVersion)}`;
          void runOps(url, {}, true);
        }
      } else if (button.matches("[data-runtime-add-app]")) {
        ensureAppPlan(button.dataset.app);
        applyVisualChange();
      } else if (button.matches("[data-runtime-remove-app]")) {
        delete ensureRuntimePlan().apps[button.dataset.app];
        applyVisualChange();
      } else if (button.matches("[data-runtime-add-target]")) {
        ensureAppPlan(button.dataset.app).targets.push({ scope: "", mode: "hot" });
        applyVisualChange();
      } else if (button.matches("[data-runtime-remove-target]")) {
        ensureAppPlan(button.dataset.app).targets.splice(Number(button.dataset.index), 1);
        applyVisualChange();
      } else if (button.matches("[data-runtime-add-metric]")) {
        const app = ensureAppPlan(button.dataset.app);
        let index = 1;
        while (Object.prototype.hasOwnProperty.call(app.metricOverrides, `metric_${index}`)) index += 1;
        app.metricOverrides[`metric_${index}`] = "hot";
        applyVisualChange();
      } else if (button.matches("[data-runtime-remove-metric]")) {
        delete ensureAppPlan(button.dataset.app).metricOverrides[button.dataset.key];
        applyVisualChange();
      } else if (button.matches("[data-mei-ops-reload]")) {
        void runOps("/api/host/ops/reload", {}, true);
      } else if (button.matches("[data-mei-ops-prebuild]")) {
        if (global.confirm("将执行全工作区 prebuild，可能需要较长时间。继续？")) {
          void runOps("/api/host/ops/prebuild", { policy: "home" }, false);
        }
      } else if (button.matches("[data-mei-prebuild-app]")) {
        void runOps(
          "/api/host/ops/prebuild",
          { policy: "home", app_id: button.getAttribute("data-mei-prebuild-app") },
          false,
        );
      }
    });
  }

  async function init() {
    root = document.querySelector("[data-host-runtime-control-center]");
    if (!root || new URL(global.location.href).searchParams.has("app")) return;
    bindEvents();
    global.addEventListener("mei:host-event", handleHostEvent);
    try {
      const control = await loadControlStatus();
      await Promise.all([
        loadProfiles(control?.selectedProfile?.id || "default"),
        refreshOps(),
        loadBuilds(),
        refreshTopology(),
      ]);
    } catch (error) {
      announce(`运行控制中心载入失败：${error.message}`, "error");
    }
  }

  global.MeiHostRuntimeControlCenter = {
    ensureRuntimePlan,
    errorMessage,
    handleHostEvent,
    validAppId,
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    void init();
  }
})(typeof window !== "undefined" ? window : globalThis);
