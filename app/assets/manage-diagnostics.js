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

  syncFilterLinks();
  document.addEventListener("mei:manage-context-change", syncFilterLinks);
  document.addEventListener("mei:manage-tab-change", syncFilterLinks);
})();
