(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.manageDiagnosticsMounted) return;
  boot.manageDiagnosticsMounted = true;

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

  function renderLayoutAudit(detail) {
    const node = layoutAuditRoot();
    if (!node) return;
    const diagnostics = Array.isArray(detail?.diagnostics) ? detail.diagnostics : [];
    const emptyText =
      String(node.getAttribute("data-empty-text") || "").trim() ||
      "尚未发现布局几何问题。";
    if (!diagnostics.length) {
      node.textContent = emptyText;
      return;
    }
    const items = diagnostics
      .map((diag) => {
        const code = escapeHtml(diag?.code || "layout_audit_runtime");
        const message = escapeHtml(diag?.message || "检测到布局问题");
        const source = escapeHtml(diag?.source_path || detail?.sourcePath || "当前预览");
        const severity = severityClass(diag?.severity || "warning");
        return `
          <div class="mt-2 grid gap-1 rounded-lg border px-2.5 py-2 ${severity}">
            <strong class="text-[11px] font-semibold">${code}</strong>
            <span class="text-[11px] leading-5 text-slate-100">${message}</span>
            <span class="text-[10px] font-mono text-slate-400">来源：${source}</span>
          </div>
        `;
      })
      .join("");
    node.innerHTML = items;
  }

  syncFilterLinks();
  document.addEventListener("mei:manage-context-change", () => {
    syncFilterLinks();
    renderLayoutAudit({ diagnostics: [] });
  });
  document.addEventListener("mei:manage-tab-change", () => {
    syncFilterLinks();
  });
  document.addEventListener("mei:layout-audit", (event) => {
    renderLayoutAudit(event?.detail || null);
  });
})();
