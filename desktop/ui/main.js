const invoke = window.__TAURI__.core.invoke;
const dialogApi = window.__TAURI__?.dialog || {};
const open = typeof dialogApi.open === "function" ? dialogApi.open.bind(dialogApi) : null;
const save = typeof dialogApi.save === "function" ? dialogApi.save.bind(dialogApi) : null;

const el = {
  status: document.getElementById("status-text"),
  port: document.getElementById("status-port"),
  workspace: document.getElementById("status-workspace"),
  logPath: document.getElementById("status-log"),
  versionBadge: document.getElementById("viewer-version"),
  statusVersion: document.getElementById("status-version"),
  logView: document.getElementById("log-view"),
  logFollow: document.getElementById("log-follow"),
  logsFold: document.getElementById("logs-fold"),
  copyLog: document.getElementById("btn-copy-log"),
  revealLog: document.getElementById("btn-reveal-log"),
  hint: document.getElementById("hint"),
  recent: document.getElementById("recent-list"),
  openWs: document.getElementById("btn-open-workspace"),
  startHome: document.getElementById("btn-start-home"),
  importSnap: document.getElementById("btn-import-snapshot"),
  exportSnap: document.getElementById("btn-export-snapshot"),
  exportScopeSummary: document.getElementById("export-scope-summary"),
  exportTreeStock: document.getElementById("export-tree-stock"),
  exportTreeApps: document.getElementById("export-tree-apps"),
  btnStockAll: document.getElementById("btn-stock-all"),
  btnStockNone: document.getElementById("btn-stock-none"),
  btnAppsAll: document.getElementById("btn-apps-all"),
  btnAppsNone: document.getElementById("btn-apps-none"),
  openHost: document.getElementById("btn-open-host"),
  stop: document.getElementById("btn-stop"),
  homePath: document.getElementById("status-home"),
  startupOverlay: document.getElementById("startup-overlay"),
  startupLabel: document.getElementById("startup-label"),
  startupBar: document.getElementById("startup-bar"),
  startupTrack: document.querySelector(".progress-track"),
};

function applyViewerVersion(ver) {
  if (!ver) return;
  if (el.versionBadge) el.versionBadge.textContent = ver;
  if (el.statusVersion) el.statusVersion.textContent = ver;
  document.title = `mei-viewer ${ver}`;
}

let lastLogText = "";
let startupDepth = 0;

function logsExpanded() {
  return Boolean(el.logsFold?.open);
}

function setStartupVisible(visible, title = "正在启动") {
  if (visible) {
    startupDepth += 1;
    document.getElementById("startup-title").textContent = title;
    el.startupOverlay.classList.remove("hidden");
    document.body.classList.add("startup-busy");
  } else {
    startupDepth = Math.max(0, startupDepth - 1);
    if (startupDepth === 0) {
      el.startupOverlay.classList.add("hidden");
      document.body.classList.remove("startup-busy");
    }
  }
}

function updateStartupProgress(percent, label) {
  const pct = Math.max(4, Math.min(100, Number(percent) || 4));
  el.startupBar.style.width = `${pct}%`;
  el.startupTrack?.setAttribute("aria-valuenow", String(Math.round(pct)));
  if (label) el.startupLabel.textContent = label;
}

function setHint(text, isError = false) {
  el.hint.textContent = text || "";
  el.hint.classList.toggle("error", Boolean(isError && text));
}

function pathText(codeId) {
  const node = document.getElementById(codeId);
  const raw = (node?.textContent || "").trim();
  if (!raw || raw === "—") return null;
  return raw;
}

function hasLogSelection() {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || !sel.rangeCount) return false;
  return el.logView.contains(sel.anchorNode);
}

async function refreshLogs({ force = false } = {}) {
  if (!force && !logsExpanded()) return;
  try {
    if (!force && hasLogSelection()) return;
    const text = await invoke("host_log_tail", { maxBytes: 256 * 1024 });
    const next = text && text.trim() ? text : "（日志为空）";
    if (next === lastLogText && !force) return;
    const atBottom =
      el.logView.scrollTop + el.logView.clientHeight >= el.logView.scrollHeight - 24;
    lastLogText = next;
    el.logView.textContent = next;
    if (atBottom || el.logFollow.checked) {
      el.logView.scrollTop = el.logView.scrollHeight;
    }
  } catch (_) {
    /* ignore while idle */
  }
}

let lastWorkspaceForScope = null;
/** @type {Map<string, HTMLInputElement>} */
const exportChecks = new Map();

function selectedExportPaths() {
  const paths = [];
  for (const [path, input] of exportChecks) {
    if (input.checked && !input.indeterminate) paths.push(path);
  }
  // Also include partially-checked? No — partial means some children; only fully checked folders.
  // But wait: if parent is indeterminate, children may be checked individually — those are already in map.
  // If parent is fully checked, parent path is enough (covers descendants). We can send all checked paths.
  return paths;
}

function selectedExportApps() {
  const ids = new Set();
  for (const path of selectedExportPaths()) {
    if (!path.startsWith("apps/")) continue;
    const id = path.slice("apps/".length).split("/")[0];
    if (id) ids.add(id);
  }
  return Array.from(ids).sort();
}

function selectionCoversUploadMedia() {
  return selectedExportPaths().some((p) => {
    if (p === "apps") return true;
    if (/^apps\/[^/]+$/.test(p)) return true;
    return p.includes("/upload");
  });
}

function updateScopeSummary() {
  const apps = selectedExportApps();
  const paths = selectedExportPaths();
  const stockPaths = paths.filter((p) => p === "stock" || p.startsWith("stock/"));
  if (!el.exportScopeSummary) return;
  if (!lastWorkspaceForScope) {
    el.exportScopeSummary.textContent = "（未打开工作区）";
    return;
  }
  const appLabel = apps.length ? apps.join(", ") : "未选应用";
  const stockLabel = stockPaths.length
    ? stockPaths.length <= 2
      ? stockPaths.join(", ")
      : `${stockPaths.length} 项 stock`
    : "stock 未选";
  el.exportScopeSummary.textContent = `${appLabel} · ${stockLabel}`;
  el.exportSnap.disabled = apps.length === 0;
}

function setSubtreeChecked(rootInput, checked) {
  const node = rootInput.closest(".tree-node");
  if (!node) return;
  node.querySelectorAll('input[type="checkbox"]').forEach((input) => {
    input.checked = checked;
    input.indeterminate = false;
  });
}

function syncAncestors(input) {
  let node = input.closest(".tree-node")?.parentElement?.closest(".tree-node");
  while (node) {
    const rowInput = node.querySelector(":scope > .tree-node-row input[type='checkbox']");
    const childInputs = Array.from(
      node.querySelectorAll(":scope > .tree-children > .tree-node > .tree-node-row input[type='checkbox']")
    );
    if (rowInput && childInputs.length) {
      const allChecked = childInputs.every((c) => c.checked && !c.indeterminate);
      const noneChecked = childInputs.every((c) => !c.checked && !c.indeterminate);
      rowInput.checked = allChecked;
      rowInput.indeterminate = !allChecked && !noneChecked;
    }
    node = node.parentElement?.closest(".tree-node");
  }
}

function onExportCheckChange(input) {
  if (!input.indeterminate) {
    setSubtreeChecked(input, input.checked);
  }
  syncAncestors(input);
  updateScopeSummary();
}

function setTreeAll(container, checked) {
  if (!container) return;
  container.querySelectorAll('input[type="checkbox"]').forEach((input) => {
    input.checked = checked;
    input.indeterminate = false;
  });
  updateScopeSummary();
}

function renderTreeNode(node, depth = 0) {
  const wrap = document.createElement("div");
  wrap.className = "tree-node";
  wrap.dataset.path = node.path;

  const row = document.createElement("div");
  row.className = "tree-node-row";

  const hasChildren = Array.isArray(node.children) && node.children.length > 0;
  const toggle = document.createElement("button");
  toggle.type = "button";
  toggle.className = "tree-toggle" + (hasChildren ? "" : " is-leaf");
  toggle.textContent = "▸";
  toggle.setAttribute("aria-expanded", depth < 1 ? "true" : "false");
  toggle.tabIndex = hasChildren ? 0 : -1;

  const label = document.createElement("label");
  const input = document.createElement("input");
  input.type = "checkbox";
  input.checked = Boolean(node.defaultChecked);
  input.dataset.path = node.path;
  exportChecks.set(node.path, input);
  input.addEventListener("change", () => onExportCheckChange(input));

  const name = document.createElement("span");
  name.className = "tree-node-name";
  name.textContent = node.name;
  name.title = node.path;

  label.appendChild(input);
  label.appendChild(name);
  row.appendChild(toggle);
  row.appendChild(label);
  wrap.appendChild(row);

  if (hasChildren) {
    const kids = document.createElement("div");
    kids.className = "tree-children";
    if (depth >= 1) kids.hidden = true;
    for (const child of node.children) {
      kids.appendChild(renderTreeNode(child, depth + 1));
    }
    wrap.appendChild(kids);
    toggle.addEventListener("click", (e) => {
      e.preventDefault();
      const open = kids.hidden;
      kids.hidden = !open;
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
    });
  }

  return wrap;
}

function renderTree(container, nodes, emptyText) {
  if (!container) return;
  container.innerHTML = "";
  container.classList.toggle("is-empty", !nodes?.length);
  if (!nodes?.length) {
    container.textContent = emptyText;
    return;
  }
  for (const node of nodes) {
    container.appendChild(renderTreeNode(node, 0));
  }
}

async function refreshExportScope(workspace, options = {}) {
  const force = Boolean(options.force);
  const hasWorkspace = Boolean(workspace);
  if (
    !force &&
    workspace === lastWorkspaceForScope &&
    hasWorkspace &&
    exportChecks.size > 0
  ) {
    updateScopeSummary();
    return;
  }
  lastWorkspaceForScope = workspace || null;
  exportChecks.clear();

  if (!hasWorkspace) {
    renderTree(el.exportTreeStock, [], "未打开工作区");
    renderTree(el.exportTreeApps, [], "未打开工作区");
    updateScopeSummary();
    return;
  }

  try {
    const tree = await invoke("list_export_scope");
    renderTree(el.exportTreeStock, tree.stock || [], "（无 stock 目录）");
    renderTree(el.exportTreeApps, tree.apps || [], "（无 apps 目录）");
    // Sync parent indeterminate states after defaults applied.
    for (const input of exportChecks.values()) {
      syncAncestors(input);
    }
    updateScopeSummary();
  } catch (e) {
    renderTree(el.exportTreeStock, [], "无法加载");
    renderTree(el.exportTreeApps, [], "无法加载");
    el.exportSnap.disabled = true;
    if (el.exportScopeSummary) el.exportScopeSummary.textContent = "（加载失败）";
    setHint(String(e), true);
  }
}

async function refreshStatus(options = {}) {
  const s = await invoke("host_status");
  let ready = Boolean(s.ready);
  if (s.running && !ready) {
    try {
      const r = await invoke("host_readiness");
      ready = Boolean(r.hostReady && r.controlReady);
    } catch (_) {
      /* still booting */
    }
  }
  el.status.textContent = s.running ? (ready ? "就绪" : "启动中…") : "未启动";
  el.status.dataset.state = s.running ? (ready ? "ready" : "booting") : "idle";
  el.port.textContent = s.port != null ? String(s.port) : "—";
  el.workspace.textContent = s.workspace || "—";
  if (el.homePath) el.homePath.textContent = s.homeWorkspace || el.homePath.textContent || "—";
  el.logPath.textContent = s.logPath || "—";
  el.openHost.disabled = !ready;
  el.stop.disabled = !s.running;
  if (s.viewerVersion) applyViewerVersion(s.viewerVersion);
  await refreshExportScope(s.workspace || null, {
    force: Boolean(options.forceExportScope),
  });
  if (s.autoOpened && ready) {
    setHint(
      "已从当前工作区自动启动（含 --launch）。菜单「查看 → 显示启动器」或 ⌘/Ctrl+L 可回到本窗口。"
    );
  }
  return { ...s, ready };
}

async function refreshRecent() {
  const items = await invoke("list_recent");
  el.recent.innerHTML = "";
  if (!items.length) {
    const li = document.createElement("li");
    li.innerHTML = `<button type="button" disabled>暂无记录</button>`;
    el.recent.appendChild(li);
    return;
  }
  for (const path of items) {
    const li = document.createElement("li");
    const btn = document.createElement("button");
    btn.type = "button";
    btn.textContent = path;
    btn.addEventListener("click", async () => {
      setHint("正在启动…");
      try {
        await openWorkspacePath(path);
      } catch (e) {
        setHint(String(e), true);
      }
    });
    li.appendChild(btn);
    el.recent.appendChild(li);
  }
}

async function waitReady(title = "正在启动工作区") {
  setStartupVisible(true, title);
  updateStartupProgress(6, "正在拉起 mei-host-shell…");
  try {
    for (let i = 0; i < 480; i++) {
      let readiness = null;
      try {
        readiness = await invoke("host_readiness");
      } catch (_) {
        /* host process still booting */
      }
      if (readiness) {
        updateStartupProgress(readiness.progressPercent, readiness.progressLabel);
        if (readiness.startupError) {
          throw new Error(readiness.startupError);
        }
        if (readiness.hostReady && readiness.controlReady) {
          updateStartupProgress(100, "控制面已就绪");
          // 打开/导入完成后强制重拉导出范围（路径可能不变但 apps/stock 已变）
          await refreshStatus({ forceExportScope: true });
          return readiness;
        }
      }
      await new Promise((r) => setTimeout(r, 500));
    }
    throw new Error("等待 host 就绪超时（约 4 分钟）");
  } finally {
    setStartupVisible(false);
  }
}

document.querySelectorAll("[data-copy-path]").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const id = btn.getAttribute("data-copy-path");
    const path = pathText(id);
    if (!path) {
      setHint("路径为空，无法复制。", true);
      return;
    }
    try {
      await navigator.clipboard.writeText(path);
      setHint("已复制路径。");
    } catch (e) {
      setHint(String(e), true);
    }
  });
});

document.querySelectorAll("[data-reveal-path]").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const id = btn.getAttribute("data-reveal-path");
    const path = pathText(id);
    if (!path) {
      setHint("路径为空，无法打开。", true);
      return;
    }
    try {
      const shown = await invoke("reveal_path", { path });
      setHint(`已在访达中显示：${shown}`);
    } catch (e) {
      setHint(String(e), true);
    }
  });
});

el.logsFold?.addEventListener("toggle", () => {
  if (logsExpanded()) {
    refreshLogs({ force: true }).catch(() => {});
  }
});

el.startHome?.addEventListener("click", async () => {
  setHint("");
  try {
    setHint("正在打开默认工作区…");
    await invoke("start_home_workspace");
    await waitReady("正在打开默认工作区");
    await refreshRecent();
    await invoke("open_host_ui");
    setHint("默认工作区已打开。可将快照导入到此目录，或手工拷贝 apps/。");
  } catch (e) {
    setHint(String(e), true);
  }
});

async function openWorkspacePath(selected) {
  const probe = await invoke("probe_workspace", { path: selected });
  if (!probe?.isWorkspace) {
    const ok = window.confirm(
      `该文件夹还不是 Mei 工作区，是否创建？\n\n${selected}\n\n将写入 workspace.json 并物化平台 stock（components/templates）。`
    );
    if (!ok) {
      setHint("已取消打开。");
      return;
    }
    setHint("正在初始化工作区…");
    await invoke("init_workspace", { path: selected });
  }
  setHint("正在启动工作区…");
  await invoke("start_workspace", { path: selected });
  await waitReady("正在打开工作区");
  await refreshRecent();
  await invoke("open_host_ui");
  setHint("工作区已启动（已带 --launch）。");
}

el.openWs.addEventListener("click", async () => {
  setHint("");
  try {
    if (typeof open !== "function") {
      setHint("对话框插件未就绪（__TAURI__.dialog.open）。请重启 mei-viewer。", true);
      return;
    }
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;
    await openWorkspacePath(selected);
  } catch (e) {
    setHint(String(e), true);
  }
});

function snapshotTimestamp() {
  const d = new Date();
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

function defaultSnapshotFileName(appIds) {
  const stamp = snapshotTimestamp();
  if (appIds.length === 1) {
    return `${appIds[0]}-${stamp}.mei-snapshot.zip`;
  }
  return `multi-${appIds[0]}-${stamp}.mei-snapshot.zip`;
}

el.importSnap.addEventListener("click", async () => {
  setHint("");
  if (typeof open !== "function") {
    setHint("对话框插件未就绪（__TAURI__.dialog.open）。请重启 mei-viewer。", true);
    return;
  }
  try {
    let selected = await open({
      multiple: false,
      filters: [{ name: "Mei Snapshot", extensions: ["zip"] }],
    });
    if (!selected) return;
    if (Array.isArray(selected)) selected = selected[0];
    if (typeof selected !== "string") selected = String(selected);
    // Always merge into the default workspace (even when no host workspace is open).
    let home = "";
    try {
      home = await invoke("home_workspace_path");
    } catch (_) {}
    setHint(
      home
        ? `正在导入快照到默认工作区：${home}`
        : "正在导入快照到默认工作区…"
    );
    await invoke("import_snapshot", { archive: selected });
    await waitReady("正在导入并启动默认工作区");
    await refreshRecent();
    await invoke("open_host_ui");
    setHint(
      home
        ? `快照已合并导入到默认工作区并启动：${home}`
        : "快照已合并导入到默认工作区并启动（未删除其中其它应用）。"
    );
  } catch (e) {
    setHint(String(e), true);
  }
});

el.exportSnap.addEventListener("click", async () => {
  setHint("");
  const appIds = selectedExportApps();
  const includePaths = selectedExportPaths();
  if (!appIds.length) {
    setHint("请先在「选择导出范围」中勾选至少一个应用目录。", true);
    return;
  }
  if (typeof save !== "function") {
    setHint("对话框插件未就绪（__TAURI__.dialog.save）。请重启 mei-viewer。", true);
    return;
  }
  try {
    const defaultName = defaultSnapshotFileName(appIds);
    let outPath = await save({
      defaultPath: defaultName,
      filters: [{ name: "Mei Snapshot", extensions: ["zip"] }],
    });
    if (!outPath) return;
    if (typeof outPath !== "string") {
      outPath = String(outPath);
    }
    const okPrep = window.confirm(
      "导出会先对所选 app 执行完整预构建（compile/import/data/warmup），可能需要数分钟，然后按勾选目录打包，并自动补齐运行必需配置。确定继续？"
    );
    if (!okPrep) {
      setHint("已取消导出。");
      return;
    }
    if (selectionCoversUploadMedia()) {
      const ok = window.confirm(
        "当前范围包含 upload（或整个应用目录），可能打入视频等大媒体，体积可达数 GB。若只需图表数据请取消后展开范围并去掉 upload。确定继续？"
      );
      if (!ok) {
        setHint("已取消导出。");
        return;
      }
    }
    setHint(`正在预构建并导出 ${appIds.join(", ")}…`);
    const msg = await invoke("export_snapshot", {
      appIds,
      outPath,
      includePaths,
    });
    setHint(msg);
  } catch (e) {
    setHint(String(e), true);
  }
});

el.btnStockAll?.addEventListener("click", () => setTreeAll(el.exportTreeStock, true));
el.btnStockNone?.addEventListener("click", () => setTreeAll(el.exportTreeStock, false));
el.btnAppsAll?.addEventListener("click", () => setTreeAll(el.exportTreeApps, true));
el.btnAppsNone?.addEventListener("click", () => setTreeAll(el.exportTreeApps, false));

el.openHost.addEventListener("click", async () => {
  try {
    await invoke("open_host_ui");
  } catch (e) {
    setHint(String(e), true);
  }
});

el.copyLog.addEventListener("click", async () => {
  try {
    const text = await invoke("host_log_tail", { maxBytes: 512 * 1024 });
    await navigator.clipboard.writeText(text || "");
    setHint("已复制运行日志到剪贴板。");
  } catch (e) {
    setHint(String(e), true);
  }
});

el.revealLog.addEventListener("click", async () => {
  try {
    const path = await invoke("reveal_host_log");
    setHint(`已在访达中显示：${path}`);
  } catch (e) {
    setHint(String(e), true);
  }
});

el.stop.addEventListener("click", async () => {
  try {
    await invoke("stop_host");
    lastWorkspaceForScope = null;
    exportChecks.clear();
    await refreshStatus({ forceExportScope: true });
    setHint("已停止。");
  } catch (e) {
    setHint(String(e), true);
  }
});

(async () => {
  try {
    try {
      applyViewerVersion(await invoke("viewer_version"));
    } catch (_) {}
    await refreshStatus();
    await refreshRecent();
  } catch (_) {}
  setInterval(() => {
    (async () => {
      try {
        const home = await invoke("home_workspace_path");
        if (el.homePath) el.homePath.textContent = home || "—";
      } catch (_) {}
      return refreshStatus();
    })().catch(() => {});
    if (logsExpanded() && el.logFollow.checked) {
      refreshLogs().catch(() => {});
    }
  }, 1500);
})();
