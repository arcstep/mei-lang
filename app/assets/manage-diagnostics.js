(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.manageDiagnosticsMounted) return;
  boot.manageDiagnosticsMounted = true;
  const LAYOUT_AUDIT_EVENT = "mei:layout-audit";

  function root() {
    return document.getElementById("mei-manage-diagnostics-root");
  }

  function filterModeFromUrl() {
    try {
      const url = new URL(window.location.href);
      return url.searchParams.get("diag_filter") === "all" ? "all" : "current";
    } catch (_) {
      return "current";
    }
  }

  function syncFilterLinks() {
    const node = root();
    if (!node) return;
    const mode = filterModeFromUrl();
    node.setAttribute("data-mei-diag-filter", mode);
    document.querySelectorAll("[data-mei-diag-filter-link]").forEach((link) => {
      const value = String(link.getAttribute("data-mei-diag-filter-link") || "");
      const active = value === mode;
      link.classList.toggle("is-active", active);
      if (active) {
        link.setAttribute("aria-current", "true");
      } else {
        link.removeAttribute("aria-current");
      }
    });
  }

  function layoutAuditRoot() {
    return document.getElementById("mei-runtime-layout-audit");
  }

  function currentLayoutAuditSource() {
    try {
      const url = new URL(window.location.href);
      return String(url.searchParams.get("file") || "main.mei").trim() || "main.mei";
    } catch (_) {
      return "main.mei";
    }
  }

  function layoutAuditStorageKey(sourcePath) {
    const pathname = String(window.location.pathname || "").trim();
    const source = String(sourcePath || currentLayoutAuditSource()).trim() || "main.mei";
    return `mei:layout-audit:${pathname}:${source}`;
  }

  function isCurrentLayoutAudit(detail) {
    if (!detail || typeof detail !== "object") return false;
    return String(detail.sourcePath || "").trim() === currentLayoutAuditSource();
  }

  function readCachedLayoutAudit() {
    try {
      const raw = sessionStorage.getItem(layoutAuditStorageKey());
      if (!raw) return null;
      const detail = JSON.parse(raw);
      return detail && typeof detail === "object" ? detail : null;
    } catch (_) {
      return null;
    }
  }

  function currentLayoutAuditDetail() {
    const live = isCurrentLayoutAudit(window.__meiLastLayoutEval) ? window.__meiLastLayoutEval : null;
    return live || readCachedLayoutAudit();
  }

  function escapeHtml(input) {
    return String(input ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&#39;");
  }

  function severityClass(severity) {
    const value = String(severity || "").trim().toLowerCase();
    if (value === "error") return "text-red-300 border-red-400/35 bg-red-950/25";
    if (value === "warning")
      return "text-amber-200 border-amber-300/35 bg-amber-950/20";
    return "text-cyan-200 border-cyan-300/30 bg-cyan-950/20";
  }

  function layoutEvalStatus(detail) {
    if (detail?.blocking) return "阻塞";
    if (Array.isArray(detail?.diagnostics) && detail.diagnostics.length) return "警告";
    return "通过";
  }

  function renderLayoutAudit(detail) {
    const node = layoutAuditRoot();
    if (!node) return;
    const diagnostics = Array.isArray(detail?.diagnostics) ? detail.diagnostics : [];
    const emptyText =
      String(node.getAttribute("data-empty-text") || "").trim() ||
      "尚未发现布局几何问题。";
    const metrics = detail?.metrics && typeof detail.metrics === "object" ? detail.metrics : {};
    const worstPanels = Array.isArray(detail?.worstPanels) ? detail.worstPanels : [];
    if (!diagnostics.length) {
      node.textContent = emptyText;
      return;
    }
    const summary = `
      <div class="rounded-lg border border-slate-700/70 bg-slate-950/40 px-2.5 py-2">
        <div class="flex flex-wrap items-center gap-2 text-[11px]">
          <strong class="text-slate-50">状态：${escapeHtml(layoutEvalStatus(detail))}</strong>
          <span class="text-slate-300">score=${escapeHtml(detail?.score ?? 0)}</span>
          <span class="text-slate-400">error ${escapeHtml(metrics.errors ?? 0)} / warning ${escapeHtml(metrics.warnings ?? 0)} / info ${escapeHtml(metrics.infos ?? 0)}</span>
        </div>
        ${
          worstPanels.length
            ? `<div class="mt-1 text-[10px] text-slate-400">worstPanels: ${worstPanels
                .map((panel) => `${escapeHtml(panel.panelId || "unknown")}(${escapeHtml(panel.score || 0)})`)
                .join("、")}</div>`
            : ""
        }
      </div>
    `;
    const items = diagnostics
      .map((diag) => {
        const code = escapeHtml(diag?.code || "layout_eval_runtime");
        const message = escapeHtml(diag?.message || "检测到布局问题");
        const source = escapeHtml(diag?.source_path || detail?.sourcePath || "当前预览");
        const severity = severityClass(diag?.severity || "warning");
        const panelId = escapeHtml(diag?.panelId || "");
        return `
          <div class="mt-2 grid gap-1 rounded-lg border px-2.5 py-2 ${severity}">
            <strong class="text-[11px] font-semibold">${code}</strong>
            <span class="text-[11px] leading-5 text-slate-100">${message}</span>
            ${panelId ? `<span class="text-[10px] font-mono text-slate-300">panel：${panelId}</span>` : ""}
            <span class="text-[10px] font-mono text-slate-400">来源：${source}</span>
          </div>
        `;
      })
      .join("");
    node.innerHTML = summary + items;
  }

  syncFilterLinks();
  renderLayoutAudit(currentLayoutAuditDetail());
  document.addEventListener("mei:manage-context-change", () => {
    syncFilterLinks();
    renderLayoutAudit(currentLayoutAuditDetail());
  });
  document.addEventListener("mei:manage-tab-change", () => {
    syncFilterLinks();
    renderLayoutAudit(currentLayoutAuditDetail());
  });
  document.addEventListener(LAYOUT_AUDIT_EVENT, (event) => {
    if (!isCurrentLayoutAudit(event?.detail || null)) return;
    renderLayoutAudit(event?.detail || null);
  });
  window.addEventListener("message", (event) => {
    if (event.origin !== window.location.origin) return;
    if (event.data?.type !== LAYOUT_AUDIT_EVENT) return;
    if (!isCurrentLayoutAudit(event.data?.detail || null)) return;
    renderLayoutAudit(event.data?.detail || null);
  });
})();
