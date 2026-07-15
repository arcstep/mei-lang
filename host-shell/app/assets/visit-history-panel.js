(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.visitHistoryPanelMounted) return;
  boot.visitHistoryPanelMounted = true;

  const POPOVER_ID = "mei-visit-history-popover";
  const TRIGGER_ID = "mei-visit-history-trigger";

  function store() {
    return window.MeiVisitHistoryStore || null;
  }

  function formatMs(value) {
    const ms = Number(value);
    if (!Number.isFinite(ms) || ms < 0) return "—";
    if (ms < 1000) return `${Math.round(ms)}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
  }

  function formatTime(at) {
    const date = new Date(Number(at) || 0);
    if (Number.isNaN(date.getTime())) return "—";
    return date.toLocaleTimeString("zh-CN", { hour12: false });
  }

  function kindLabel(kind) {
    const api = store();
    if (api && typeof api.kindLabel === "function") {
      return api.kindLabel(kind);
    }
    return String(kind || "访问");
  }

  function escapeHtml(value) {
    return String(value ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;");
  }

  function truncate(text, max) {
    const value = String(text || "");
    if (value.length <= max) return value;
    return value.slice(0, Math.max(0, max - 1)) + "…";
  }

  function buildPerfLine(item) {
    const normalized =
      typeof boot.normalizeVisitPerfTotals === "function"
        ? boot.normalizeVisitPerfTotals(item)
        : item;
    const parts = [`渲染 ${formatMs(normalized.renderMs)}`];
    if (Number(normalized.apiTotal) > 0 || Number(normalized.evalMs) > 0) {
      parts.push(`求值 ${formatMs(normalized.evalMs)}`);
    }
    parts.push(`总计 ${formatMs(normalized.totalMs)}`);
    return parts.join(" · ");
  }

  function formatRecordForAgent(item) {
    const api = store();
    if (api && typeof api.formatRecordForAgent === "function") {
      return api.formatRecordForAgent(item);
    }
    return JSON.stringify(item, null, 2);
  }

  function formatAllForAgent(items) {
    const api = store();
    if (api && typeof api.formatAllForAgent === "function") {
      return api.formatAllForAgent(items);
    }
    return JSON.stringify(items, null, 2);
  }

  async function copyText(text) {
    const payload = String(text || "");
    if (!payload) return false;
    try {
      if (navigator.clipboard && typeof navigator.clipboard.writeText === "function") {
        await navigator.clipboard.writeText(payload);
        return true;
      }
    } catch (_) {}
    try {
      const area = document.createElement("textarea");
      area.value = payload;
      area.setAttribute("readonly", "true");
      area.style.position = "fixed";
      area.style.left = "-9999px";
      document.body.appendChild(area);
      area.select();
      const ok = document.execCommand("copy");
      area.remove();
      return ok;
    } catch (_) {
      return false;
    }
  }

  function flashCopyHint(node, ok) {
    if (!(node instanceof HTMLElement)) return;
    const prev = node.textContent;
    node.textContent = ok ? "已复制" : "复制失败";
    node.dataset.tone = ok ? "good" : "danger";
    setTimeout(() => {
      node.textContent = prev;
      node.dataset.tone = "neutral";
    }, 1200);
  }

  function currentAppHeading() {
    const api = store();
    if (!api) return "访问历史";
    const ctx =
      api.collectVisitContext && typeof api.collectVisitContext === "function"
        ? api.collectVisitContext()
        : null;
    const label = String(ctx?.appTitle || ctx?.appId || "").trim();
    return label ? `访问历史 · ${label}` : "访问历史";
  }

  function ensurePopover() {
    let popover = document.getElementById(POPOVER_ID);
    if (popover) return popover;
    popover = document.createElement("div");
    popover.id = POPOVER_ID;
    popover.className = "visit-history-popover";
    popover.setAttribute("hidden", "hidden");
    popover.innerHTML =
      '<div class="visit-history-popover-backdrop" data-visit-history-close="mask"></div>' +
      '<section class="visit-history-popover-panel" role="dialog" aria-label="访问历史">' +
      '<header class="visit-history-popover-head">' +
      '<strong data-visit-history-title="true">访问历史</strong>' +
      '<div class="visit-history-popover-actions">' +
      '<button type="button" class="status-chip visit-history-copy-all" data-visit-history-copy-all="true" data-tone="neutral">复制全部</button>' +
      '<button type="button" class="visit-history-popover-close" data-visit-history-close="button" aria-label="关闭">×</button>' +
      "</div>" +
      "</header>" +
      '<div class="visit-history-popover-body" data-visit-history-list="true"></div>' +
      "</section>";
    popover.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (target.dataset.visitHistoryClose) {
        hidePopover();
        return;
      }
      if (target.dataset.visitHistoryCopyAll === "true") {
        void (async () => {
          const api = store();
          const items = api && typeof api.list === "function" ? api.list() : [];
          const ok = await copyText(formatAllForAgent(items));
          flashCopyHint(target, ok);
        })();
        return;
      }
      const rowCopy = target.closest("[data-visit-history-copy-id]");
      if (rowCopy instanceof HTMLElement && rowCopy.dataset.visitHistoryCopyId) {
        const id = rowCopy.dataset.visitHistoryCopyId;
        const api = store();
        const items = api && typeof api.list === "function" ? api.list() : [];
        const item = items.find((entry) => String(entry.id) === String(id));
        if (!item) return;
        void (async () => {
          const ok = await copyText(formatRecordForAgent(item));
          flashCopyHint(rowCopy, ok);
        })();
      }
    });
    document.body.appendChild(popover);
    return popover;
  }

  function renderList() {
    const popover = ensurePopover();
    const listHost = popover.querySelector("[data-visit-history-list]");
    const titleNode = popover.querySelector("[data-visit-history-title]");
    if (titleNode) titleNode.textContent = currentAppHeading();
    if (!listHost) return;
    const api = store();
    const items = api && typeof api.list === "function" ? api.list() : [];
    if (!items.length) {
      listHost.innerHTML = '<div class="visit-history-empty">暂无访问记录</div>';
      return;
    }
    listHost.innerHTML = items
      .map((item) => {
        const hint = item.uiShown ? "" : '<span class="visit-history-muted">未提示</span>';
        const contextBits = [
          item.workspace ? `工作区 ${truncate(item.workspace, 16)}` : "",
          item.scene ? `场景 ${truncate(item.scene, 24)}` : "",
          item.file ? `文件 ${truncate(item.file, 24)}` : "",
        ].filter(Boolean);
        const contextLine = contextBits.length
          ? `<div class="visit-history-context">${escapeHtml(contextBits.join(" · "))}</div>`
          : "";
        return (
          '<article class="visit-history-row">' +
          '<div class="visit-history-row-top">' +
          `<time>${escapeHtml(formatTime(item.at))}</time>` +
          `<span class="visit-history-kind">${escapeHtml(kindLabel(item.kind))}</span>` +
          hint +
          `<button type="button" class="status-chip visit-history-copy-one" data-visit-history-copy-id="${escapeHtml(item.id)}" data-tone="neutral">复制</button>` +
          "</div>" +
          `<div class="visit-history-label" title="${escapeHtml(item.label || item.path || "")}">${escapeHtml(truncate(item.label || item.path || "访问", 48))}</div>` +
          contextLine +
          `<div class="visit-history-perf">${escapeHtml(buildPerfLine(item))}</div>` +
          "</article>"
        );
      })
      .join("");
  }

  function showPopover() {
    const trigger = document.getElementById(TRIGGER_ID);
    if (!trigger) return;
    const popover = ensurePopover();
    renderList();
    popover.removeAttribute("hidden");
    popover.classList.add("is-open");
    const rect = trigger.getBoundingClientRect();
    const panel = popover.querySelector(".visit-history-popover-panel");
    if (panel instanceof HTMLElement) {
      const margin = 10;
      const panelWidth = Math.min(400, Math.max(300, window.innerWidth - margin * 2));
      panel.style.width = `${panelWidth}px`;
      let left = Math.max(margin, rect.left);
      if (left + panelWidth > window.innerWidth - margin) {
        left = Math.max(margin, window.innerWidth - margin - panelWidth);
      }
      panel.style.left = `${Math.round(left)}px`;
      panel.style.bottom = `${Math.round(window.innerHeight - rect.top + 8)}px`;
    }
  }

  function hidePopover() {
    const popover = document.getElementById(POPOVER_ID);
    if (!popover) return;
    popover.setAttribute("hidden", "hidden");
    popover.classList.remove("is-open");
  }

  function togglePopover() {
    const popover = document.getElementById(POPOVER_ID);
    if (popover && popover.classList.contains("is-open")) {
      hidePopover();
      return;
    }
    showPopover();
  }

  function updateTriggerHint() {
    const trigger = document.getElementById(TRIGGER_ID);
    const api = store();
    if (!trigger || !api || typeof api.list !== "function") return;
    const latest = api.list()[0];
    if (!latest) {
      trigger.title = "最近访问与加载耗时";
      return;
    }
    trigger.title = `${kindLabel(latest.kind)} · ${buildPerfLine(latest)}`;
  }

  function bindTrigger() {
    const trigger = document.getElementById(TRIGGER_ID);
    if (!trigger || trigger.dataset.meiVisitHistoryBound === "1") return;
    trigger.dataset.meiVisitHistoryBound = "1";
    trigger.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      togglePopover();
    });
    updateTriggerHint();
  }

  function isAccessMode() {
    return document.body.classList.contains("access-mode") || document.body.classList.contains("app-view");
  }

  function init() {
    if (!isAccessMode()) return;
    bindTrigger();
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") hidePopover();
    });
    document.addEventListener("mei:visit-history-updated", () => {
      updateTriggerHint();
      const popover = document.getElementById(POPOVER_ID);
      if (popover && popover.classList.contains("is-open")) renderList();
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init, { once: true });
  } else {
    init();
  }

  boot.refreshVisitHistoryPanel = function refreshVisitHistoryPanel() {
    bindTrigger();
    updateTriggerHint();
    renderList();
  };
})();
