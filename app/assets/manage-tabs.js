(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (typeof boot.disposeManageTabs === "function") {
    try {
      boot.disposeManageTabs();
    } catch (_) {}
    boot.disposeManageTabs = null;
  }

  const tabs = Array.from(
    document.querySelectorAll("a.manage-view-tab[data-manage-tab]"),
  );
  if (!tabs.length) return;

  const panels = {
    preview: document.querySelector('[data-manage-tab-panel="preview"]'),
    source: document.querySelector('[data-manage-tab-panel="source"]'),
    diagnostics: document.querySelector('[data-manage-tab-panel="diagnostics"]'),
  };
  const viewTabNodes = Array.from(document.querySelectorAll("[data-view-tab]"));

  function normalizeTab(raw) {
    const value = String(raw || "").trim().toLowerCase();
    if (value === "source" || value === "diff" || value === "diagnostics") return value;
    return "preview";
  }

  function resolveRenderableTab(tab) {
    const active = normalizeTab(tab);
    if ((active === "source" || active === "diff") && !panels.source) {
      return "preview";
    }
    if (active === "diagnostics" && !panels.diagnostics) {
      return "preview";
    }
    return active;
  }

  function tabFromUrl() {
    try {
      const url = new URL(window.location.href);
      return normalizeTab(url.searchParams.get("tab"));
    } catch (_) {
      return "preview";
    }
  }

  function panelVisibility(tab) {
    const active = resolveRenderableTab(tab);
    if (panels.preview) panels.preview.hidden = active !== "preview";
    if (panels.source) panels.source.hidden = !(active === "source" || active === "diff");
    if (panels.diagnostics) panels.diagnostics.hidden = active !== "diagnostics";
  }

  function tabVisual(tab) {
    const active = resolveRenderableTab(tab);
    tabs.forEach((node) => {
      const nodeTab = normalizeTab(node.getAttribute("data-manage-tab"));
      const isActive = nodeTab === active;
      node.classList.toggle("is-active", isActive);
      node.setAttribute("aria-selected", isActive ? "true" : "false");
      if (isActive) {
        node.setAttribute("aria-current", "page");
      } else {
        node.removeAttribute("aria-current");
      }
    });
  }

  function tabLink(tab) {
    const active = resolveRenderableTab(tab);
    return (
      tabs.find((node) => normalizeTab(node.getAttribute("data-manage-tab")) === active) ||
      tabs[0]
    );
  }

  function updateUrl(nextTab) {
    const link = tabLink(nextTab);
    if (!link || !link.href) return;
    const nextHref = new URL(link.href, window.location.href).toString();
    const currentHref = window.location.href;
    if (nextHref === currentHref) return;
    window.history.replaceState(window.history.state, "", nextHref);
  }

  function syncDatasets(nextTab) {
    const active = resolveRenderableTab(nextTab);
    viewTabNodes.forEach((node) => {
      if (node && node.dataset) {
        node.dataset.viewTab = active;
      }
    });
  }

  function emitTabChange(nextTab) {
    document.dispatchEvent(
      new CustomEvent("mei:manage-tab-change", {
        detail: { tab: resolveRenderableTab(nextTab) },
      }),
    );
  }

  function switchManageTab(nextTab, options) {
    const opts = options || {};
    const active = resolveRenderableTab(nextTab);
    tabVisual(active);
    panelVisibility(active);
    if (opts.updateUrl !== false) {
      updateUrl(active);
    }
    syncDatasets(active);
    if (opts.emit !== false) {
      emitTabChange(active);
    }
    return active;
  }

  function onClick(event) {
    if (event.defaultPrevented) return;
    if (event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const target =
      event.target instanceof Element
        ? event.target.closest("a.manage-view-tab[data-manage-tab]")
        : null;
    if (!target) return;
    event.preventDefault();
    switchManageTab(target.getAttribute("data-manage-tab"), {
      updateUrl: true,
      emit: true,
    });
  }

  document.addEventListener("click", onClick, true);

  boot.switchManageTab = function (tab) {
    return switchManageTab(tab, { updateUrl: true, emit: true });
  };

  switchManageTab(tabFromUrl(), { updateUrl: false, emit: false });

  boot.disposeManageTabs = function () {
    document.removeEventListener("click", onClick, true);
    if (boot.switchManageTab) {
      boot.switchManageTab = null;
    }
  };
})();
