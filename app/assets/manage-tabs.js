(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  const BUILD_VIEW_TABS = [
    "overview",
    "preview",
    "exec",
    "semantic",
    "eval",
    "artifact",
    "provenance",
    "agent",
  ];

  function listTabs() {
    return Array.from(
      document.querySelectorAll("a.manage-view-tab[data-manage-tab]"),
    ).filter((node) => !node.hidden);
  }

  function clearManageTabLoadingOverlay() {
    const main = document.querySelector("#workspace-root main.main");
    if (!main) return;
    main.removeAttribute("aria-busy");
    main.querySelectorAll('[data-mei-manage-nav-loading="true"]').forEach((node) => {
      node.remove();
    });
    const globalOverlay = document.getElementById("mei-spa-loading");
    if (globalOverlay) {
      globalOverlay.classList.remove("is-visible");
    }
  }

  function requestRuntimeAbort(reason) {
    try {
      window.dispatchEvent(
        new CustomEvent("mei:abort-runtime-queries", {
          detail: { reason: String(reason || "").trim() },
        }),
      );
    } catch (_) {}
  }

  function normalizeTab(raw) {
    const value = String(raw || "").trim().toLowerCase();
    if (BUILD_VIEW_TABS.includes(value)) return value;
    if (value === "source" || value === "diagnostics" || value === "diff") {
      return "overview";
    }
    return "";
  }

  function resolveRenderableTab(tab) {
    const active = normalizeTab(tab);
    if (active && BUILD_VIEW_TABS.includes(active)) return active;
    const shell = document.querySelector("[data-build-tab]");
    const fromShell = normalizeTab(shell && shell.getAttribute("data-build-tab"));
    if (fromShell) return fromShell;
    if (document.querySelector('[data-manage-tab-panel="preview"]')) return "preview";
    return "overview";
  }

  function getPanels() {
    const panels = {};
    document.querySelectorAll("[data-manage-tab-panel]").forEach((node) => {
      const id = normalizeTab(node.getAttribute("data-manage-tab-panel"));
      if (id) panels[id] = node;
    });
    return panels;
  }

  function tabFromUrl() {
    try {
      const url = new URL(window.location.href);
      return resolveRenderableTab(url.searchParams.get("tab"));
    } catch (_) {
      return resolveRenderableTab("");
    }
  }

  function panelVisibility(tab) {
    const active = resolveRenderableTab(tab);
    const panels = getPanels();
    BUILD_VIEW_TABS.forEach((slug) => {
      if (panels[slug]) {
        panels[slug].hidden = slug !== active;
      }
    });
  }

  function tabVisual(tab) {
    const active = resolveRenderableTab(tab);
    listTabs().forEach((node) => {
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
    const shell = document.querySelector("[data-build-tab]");
    if (shell) {
      shell.setAttribute("data-build-tab", active);
    }
  }

  function tabLink(tab) {
    const active = resolveRenderableTab(tab);
    const visible = listTabs();
    return (
      visible.find((node) => normalizeTab(node.getAttribute("data-manage-tab")) === active) ||
      visible[0]
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

  function emitTabChange(nextTab) {
    document.dispatchEvent(
      new CustomEvent("mei:manage-tab-change", {
        detail: { tab: resolveRenderableTab(nextTab) },
      }),
    );
  }

  function refreshBuildPanelForTab(tab) {
    const active = resolveRenderableTab(tab);
    if (typeof globalThis.__meiBuildCopyContextRefresh === "function") {
      globalThis.__meiBuildCopyContextRefresh(active);
    }
  }

  function installManageTabs() {
    if (typeof boot.disposeManageTabs === "function") {
      try {
        boot.disposeManageTabs();
      } catch (_) {}
      boot.disposeManageTabs = null;
    }

    if (!listTabs().length) return;

    let currentTab = tabFromUrl();

    function switchManageTab(nextTab, options) {
      const opts = options || {};
      const active = resolveRenderableTab(nextTab);
      currentTab = active;
      clearManageTabLoadingOverlay();
      tabVisual(active);
      panelVisibility(active);
      if (opts.updateUrl !== false) {
        updateUrl(active);
      }
      if (opts.emit !== false) {
        emitTabChange(active);
      }
      if (active !== "preview") {
        requestRuntimeAbort(`manage_tab:${active}`);
      }
      if (active === "preview") {
        requestAnimationFrame(() => {
          if (typeof boot.scheduleFrameViewportRelayout === "function") {
            try {
              boot.scheduleFrameViewportRelayout();
            } catch (_) {}
          }
          requestAnimationFrame(() => {
            window.dispatchEvent(
              new CustomEvent("meilang:preview-updated", { detail: { scope: "page" } }),
            );
          });
        });
      }
      refreshBuildPanelForTab(active);
      return active;
    }

    const onClick = (event) => {
      if (event.defaultPrevented) return;
      if (event.button !== 0) return;
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      const target =
        event.target instanceof Element
          ? event.target.closest("a.manage-view-tab[data-manage-tab]")
          : null;
      if (!target) return;
      if (target.hidden) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      switchManageTab(target.getAttribute("data-manage-tab"), {
        updateUrl: true,
        emit: true,
      });
    };

    document.addEventListener("click", onClick, true);

    boot.switchManageTab = function (tab) {
      return switchManageTab(tab, { updateUrl: true, emit: true });
    };

    boot.disposeManageTabs = function () {
      document.removeEventListener("click", onClick, true);
      if (boot.switchManageTab) {
        boot.switchManageTab = null;
      }
      boot.disposeManageTabs = null;
    };

    switchManageTab(currentTab, { updateUrl: false, emit: false });
    refreshBuildPanelForTab(currentTab);
  }

  boot.installManageTabs = installManageTabs;
  installManageTabs();
})();
