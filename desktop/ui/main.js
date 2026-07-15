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
  copyLog: document.getElementById("btn-copy-log"),
  revealLog: document.getElementById("btn-reveal-log"),
  hint: document.getElementById("hint"),
  recent: document.getElementById("recent-list"),
  openWs: document.getElementById("btn-open-workspace"),
  importSnap: document.getElementById("btn-import-snapshot"),
  exportSnap: document.getElementById("btn-export-snapshot"),
  exportApp: document.getElementById("export-app"),
  exportIncludeData: document.getElementById("export-include-data"),
  openHost: document.getElementById("btn-open-host"),
  stop: document.getElementById("btn-stop"),
  resourcesList: document.getElementById("resources-list"),
  refreshResources: document.getElementById("btn-refresh-resources"),
  martinStatus: document.getElementById("martin-status"),
  martinVersion: document.getElementById("martin-version"),
  martinMbtiles: document.getElementById("martin-mbtiles"),
  martinCatalog: document.getElementById("martin-catalog"),
  martinDownload: document.getElementById("btn-martin-download"),
  martinPick: document.getElementById("btn-martin-pick"),
  martinStart: document.getElementById("btn-martin-start"),
  martinStop: document.getElementById("btn-martin-stop"),
  martinOpenCatalog: document.getElementById("btn-martin-catalog"),
  martinReveal: document.getElementById("btn-martin-reveal"),
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

function hasLogSelection() {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || !sel.rangeCount) return false;
  return el.logView.contains(sel.anchorNode);
}

async function refreshLogs({ force = false } = {}) {
  try {
    // Avoid wiping user selection / scroll while they copy manually.
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

let lastWorkspaceForApps = null;

function selectedExportApps() {
  return Array.from(el.exportApp.selectedOptions || [])
    .map((o) => o.value)
    .filter(Boolean);
}

async function refreshExportApps(workspace) {
  const hasWorkspace = Boolean(workspace);
  if (workspace === lastWorkspaceForApps && hasWorkspace) {
    el.exportSnap.disabled = el.exportApp.disabled || selectedExportApps().length === 0;
    return;
  }
  lastWorkspaceForApps = workspace;
  const prev = selectedExportApps();
  el.exportApp.innerHTML = "";
  if (!hasWorkspace) {
    const opt = document.createElement("option");
    opt.value = "";
    opt.textContent = "（未打开工作区）";
    el.exportApp.appendChild(opt);
    el.exportApp.disabled = true;
    el.exportSnap.disabled = true;
    return;
  }
  try {
    const apps = await invoke("list_workspace_apps");
    if (!apps.length) {
      const opt = document.createElement("option");
      opt.value = "";
      opt.textContent = "（工作区无 app）";
      el.exportApp.appendChild(opt);
      el.exportApp.disabled = true;
      el.exportSnap.disabled = true;
      return;
    }
    for (const id of apps) {
      const opt = document.createElement("option");
      opt.value = id;
      opt.textContent = id;
      if (prev.includes(id) || (prev.length === 0 && apps.length === 1)) {
        opt.selected = true;
      }
      el.exportApp.appendChild(opt);
    }
    el.exportApp.disabled = false;
    el.exportSnap.disabled = selectedExportApps().length === 0;
  } catch (e) {
    const opt = document.createElement("option");
    opt.value = "";
    opt.textContent = "（无法列出 app）";
    el.exportApp.appendChild(opt);
    el.exportApp.disabled = true;
    el.exportSnap.disabled = true;
    setHint(String(e), true);
  }
}

async function refreshResources() {
  if (!el.resourcesList) return;
  try {
    const doc = await invoke("list_snapshot_resources");
    const resources = Array.isArray(doc.resources) ? doc.resources : [];
    const pending = resources.filter(
      (r) => r.state === "external" || r.state === "missing",
    );
    if (!resources.length) {
      el.resourcesList.textContent = "当前工作区无 resources.json（打开普通工作区或 v1 快照时正常）。";
      return;
    }
    if (!pending.length) {
      el.resourcesList.textContent = `全部 ${resources.length} 项资源已就绪。`;
      return;
    }
    el.resourcesList.innerHTML = "";
    for (const r of pending) {
      const row = document.createElement("div");
      row.className = "resource-row";
      const meta = document.createElement("div");
      meta.className = "resource-meta";
      meta.innerHTML = `<strong>${r.kind}</strong> · ${r.id}<br/><code>${r.targetPath || ""}</code><br/><span class="muted">${r.hint || ""}</span>`;
      const actions = document.createElement("div");
      actions.className = "resource-actions";
      const pick = document.createElement("button");
      pick.type = "button";
      pick.textContent = "选择文件…";
      pick.addEventListener("click", async () => {
        try {
          let selected = await open({ multiple: false });
          if (!selected) return;
          if (Array.isArray(selected)) selected = selected[0];
          const msg = await invoke("replenish_snapshot_resource", {
            resourceId: r.id,
            sourceFile: String(selected),
          });
          setHint(msg);
          await refreshResources();
        } catch (e) {
          setHint(String(e), true);
        }
      });
      const reveal = document.createElement("button");
      reveal.type = "button";
      reveal.className = "secondary compact";
      reveal.textContent = "打开目录";
      reveal.addEventListener("click", async () => {
        try {
          const dir = await invoke("reveal_snapshot_resource_dir", { resourceId: r.id });
          setHint(`已打开：${dir}`);
        } catch (e) {
          setHint(String(e), true);
        }
      });
      actions.appendChild(pick);
      actions.appendChild(reveal);
      row.appendChild(meta);
      row.appendChild(actions);
      el.resourcesList.appendChild(row);
    }
  } catch (_) {
    el.resourcesList.textContent = "（无法读取资源清单）";
  }
}

async function refreshStatus() {
  const s = await invoke("host_status");
  // Prefer live readiness so the open button unlocks as soon as control plane is up.
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
  el.port.textContent = s.port != null ? String(s.port) : "—";
  el.workspace.textContent = s.workspace || "—";
  el.logPath.textContent = s.logPath || "—";
  el.openHost.disabled = !ready;
  el.stop.disabled = !s.running;
  if (s.viewerVersion) applyViewerVersion(s.viewerVersion);
  await refreshExportApps(s.workspace || null);
  await refreshResources();
  if (s.autoOpened && ready) {
    setHint(
      "已从当前工作区自动启动（含 --launch）。菜单「查看 → 显示启动器与运行日志」或 ⌘/Ctrl+L 可回看日志。"
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
        await invoke("start_workspace", { path });
        await waitReady("正在打开工作区");
        await refreshLogs();
        await invoke("open_host_ui");
        setHint("已启动；宿主 UI 已在系统浏览器打开。");
      } catch (e) {
        await refreshLogs({ force: true });
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
          await refreshStatus();
          return readiness;
        }
      }
      if (i % 2 === 0) {
        await refreshLogs({ force: i % 6 === 0 });
      }
      await new Promise((r) => setTimeout(r, 500));
    }
    throw new Error("等待 host 就绪超时（约 4 分钟）");
  } finally {
    setStartupVisible(false);
  }
}

el.openWs.addEventListener("click", async () => {
  setHint("");
  try {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;
    setHint("正在启动工作区…");
    await invoke("start_workspace", { path: selected });
    await waitReady("正在打开工作区");
    await refreshRecent();
    await invoke("open_host_ui");
    setHint("工作区已启动（已带 --launch）。");
  } catch (e) {
    await refreshLogs();
    setHint(String(e), true);
  }
});

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
    setHint("正在导入快照…");
    await invoke("import_snapshot", { archive: selected });
    await waitReady("正在导入并启动快照");
    await refreshRecent();
    await invoke("open_host_ui");
    setHint("快照已导入并启动。");
  } catch (e) {
    await refreshLogs({ force: true });
    setHint(String(e), true);
  }
});

el.exportSnap.addEventListener("click", async () => {
  setHint("");
  const appIds = selectedExportApps();
  if (!appIds.length) {
    setHint("请先选择要导出的 app（可多选）。", true);
    return;
  }
  if (typeof save !== "function") {
    setHint("对话框插件未就绪（__TAURI__.dialog.save）。请重启 mei-viewer。", true);
    return;
  }
  try {
    const defaultName =
      appIds.length === 1 ? `${appIds[0]}.mei-snapshot.zip` : `multi-${appIds[0]}.mei-snapshot.zip`;
    let outPath = await save({
      defaultPath: defaultName,
      filters: [{ name: "Mei Snapshot", extensions: ["zip"] }],
    });
    if (!outPath) return;
    if (typeof outPath !== "string") {
      outPath = String(outPath);
    }
    setHint(`正在导出 ${appIds.join(", ")}…`);
    const msg = await invoke("export_snapshot", {
      appIds,
      outPath,
      includeData: Boolean(el.exportIncludeData.checked),
    });
    setHint(msg);
  } catch (e) {
    setHint(String(e), true);
  }
});

el.exportApp.addEventListener("change", () => {
  el.exportSnap.disabled = el.exportApp.disabled || selectedExportApps().length === 0;
});

el.refreshResources?.addEventListener("click", async () => {
  await refreshResources();
  setHint("已重新检测资源清单。");
});

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

async function refreshMartinStatus() {
  try {
    const s = await invoke("martin_status");
    const label = !s.installed
      ? "未安装"
      : s.ready
        ? "就绪"
        : s.running
          ? "启动中…"
          : "已安装 · 未运行";
    if (el.martinStatus) el.martinStatus.textContent = label;
    if (el.martinVersion) el.martinVersion.textContent = s.version || "—";
    if (el.martinMbtiles) el.martinMbtiles.textContent = s.mbtilesPath || "（未选择）";
    if (el.martinCatalog) el.martinCatalog.textContent = s.catalogUrl || "—";
    if (el.martinStop) el.martinStop.disabled = !s.running;
    if (el.martinOpenCatalog) el.martinOpenCatalog.disabled = !s.running;
    return s;
  } catch (_) {
    return null;
  }
}

el.martinDownload?.addEventListener("click", async () => {
  setHint("正在从 GitHub 下载 Martin…");
  setStartupVisible(true, "下载 Martin");
  updateStartupProgress(20, "拉取官方二进制…");
  try {
    await invoke("martin_ensure_installed");
    await refreshMartinStatus();
    setHint("Martin 已安装到本机 Application Support。");
  } catch (e) {
    setHint(String(e), true);
  } finally {
    setStartupVisible(false);
  }
});

el.martinPick?.addEventListener("click", async () => {
  setHint("");
  if (typeof open !== "function") {
    setHint("对话框插件未就绪。请重启 mei-viewer。", true);
    return;
  }
  try {
    let selected = await open({
      multiple: false,
      filters: [{ name: "MBTiles", extensions: ["mbtiles"] }],
    });
    if (!selected) return;
    if (Array.isArray(selected)) selected = selected[0];
    await invoke("martin_pick_mbtiles", { path: String(selected) });
    await refreshMartinStatus();
    setHint("已记住 MBTiles 路径。");
  } catch (e) {
    setHint(String(e), true);
  }
});

el.martinStart?.addEventListener("click", async () => {
  setHint("正在启动 Martin…");
  setStartupVisible(true, "启动瓦片服务");
  updateStartupProgress(30, "下载（如需）并监听 :8080…");
  try {
    await invoke("martin_start");
    await refreshMartinStatus();
    setHint("Martin 已就绪（http://127.0.0.1:8080/catalog）。打开工作区时会自动注入 GIS 上游。");
  } catch (e) {
    setHint(String(e), true);
  } finally {
    setStartupVisible(false);
  }
});

el.martinStop?.addEventListener("click", async () => {
  try {
    await invoke("martin_stop");
    await refreshMartinStatus();
    setHint("Martin 已停止。");
  } catch (e) {
    setHint(String(e), true);
  }
});

el.martinOpenCatalog?.addEventListener("click", async () => {
  try {
    await invoke("martin_open_catalog");
  } catch (e) {
    setHint(String(e), true);
  }
});

el.martinReveal?.addEventListener("click", async () => {
  try {
    const path = await invoke("martin_reveal");
    setHint(`已打开：${path}`);
  } catch (e) {
    setHint(String(e), true);
  }
});

el.stop.addEventListener("click", async () => {
  try {
    await invoke("stop_host");
    lastWorkspaceForApps = null;
    await refreshStatus();
    await refreshLogs();
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
    const s = await refreshStatus();
    await refreshMartinStatus();
    await refreshRecent();
    await refreshLogs();
    if (s.autoOpened && s.ready) {
      // Host window already opened by Rust setup; keep launcher hidden unless user reveals it.
    }
  } catch (_) {}
  setInterval(() => {
    refreshStatus().catch(() => {});
    refreshMartinStatus().catch(() => {});
    if (el.logFollow.checked) {
      refreshLogs().catch(() => {});
    }
  }, 1500);
})();
