(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

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

  function installManageTabs() {
    if (typeof boot.disposeManageTabs === "function") {
      try {
        boot.disposeManageTabs();
      } catch (_) {}
      boot.disposeManageTabs = null;
    }

    if (!listTabs().length) return;

    const viewTabNodes = Array.from(document.querySelectorAll("[data-view-tab]"));
    let currentTab = "preview";

    function getPanels() {
      return {
        preview: document.querySelector('[data-manage-tab-panel="preview"]'),
        source: document.querySelector('[data-manage-tab-panel="source"]'),
        diagnostics: document.querySelector('[data-manage-tab-panel="diagnostics"]'),
      };
    }

    function normalizeTab(raw) {
      const value = String(raw || "").trim().toLowerCase();
      if (value === "source" || value === "diagnostics") return value;
      if (value === "diff") return "source";
      return "preview";
    }

    function resolveRenderableTab(tab) {
      const active = normalizeTab(tab);
      const panels = getPanels();
      if (active === "source" && !panels.source) {
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
      const panels = getPanels();
      const active = resolveRenderableTab(tab);
      if (panels.preview) panels.preview.hidden = active !== "preview";
      if (panels.source) panels.source.hidden = active !== "source";
      if (panels.diagnostics) panels.diagnostics.hidden = active !== "diagnostics";
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
      currentTab = active;
      clearManageTabLoadingOverlay();
      tabVisual(active);
      panelVisibility(active);
      if (opts.updateUrl !== false) {
        updateUrl(active);
      }
      syncDatasets(active);
      if (opts.emit !== false) {
        emitTabChange(active);
      }
      if (active !== "preview") {
        requestRuntimeAbort(`manage_tab:${active}`);
      }
      if (active === "preview") {
        window.dispatchEvent(
          new CustomEvent("meilang:preview-updated", { detail: { scope: "page" } }),
        );
        requestAnimationFrame(() => {
          window.dispatchEvent(
            new CustomEvent("meilang:preview-updated", { detail: { scope: "page" } }),
          );
          if (typeof boot.scheduleFrameViewportRelayout === "function") {
            try {
              boot.scheduleFrameViewportRelayout();
            } catch (_) {}
          }
        });
      }
      return active;
    }

    function maybeRewriteManageNavigation(target) {
      if (!target || !(target instanceof HTMLAnchorElement)) return;
      if (!target.href) return;
      if (target.dataset.preserveManageTab !== "1") return;
      try {
        const url = new URL(target.href, window.location.href);
        if (url.origin !== window.location.origin) return;
        if (
          !url.pathname.startsWith("/apps/build/") &&
          !url.pathname.startsWith("/apps/manage/")
        ) {
          return;
        }
        url.searchParams.set("tab", resolveRenderableTab(currentTab));
        target.href = url.toString();
      } catch (_) {}
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

    const onNavClick = (event) => {
      if (event.defaultPrevented) return;
      if (event.button !== 0) return;
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      const target =
        event.target instanceof Element ? event.target.closest("a[href]") : null;
      if (!target) return;
      if (target.matches("a.manage-view-tab[data-manage-tab]")) return;
      maybeRewriteManageNavigation(target);
    };

    document.addEventListener("click", onClick, true);
    document.addEventListener("click", onNavClick, true);

    boot.switchManageTab = function (tab) {
      return switchManageTab(tab, { updateUrl: true, emit: true });
    };

    boot.disposeManageTabs = function () {
      document.removeEventListener("click", onClick, true);
      document.removeEventListener("click", onNavClick, true);
      if (boot.switchManageTab) {
        boot.switchManageTab = null;
      }
      boot.disposeManageTabs = null;
    };

    switchManageTab(tabFromUrl(), { updateUrl: false, emit: false });
  }

  boot.installManageTabs = installManageTabs;
  installManageTabs();
})();
