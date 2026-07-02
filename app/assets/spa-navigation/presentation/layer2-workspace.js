  const LAYER2_WORKSPACE_ROOT_ID = "mei-layer2-workspace";

  function useUnifiedLayer2() {
    const boot = window.__meiLangBoot || {};
    if (boot.unifiedLayer2 === false) return false;
    if (window.__mei && window.__mei.unified_layer2 === false) return false;
    return true;
  }

  function layer2OverlayPanelHtml() {
    return (
      '<div class="access-drilldown-overlay-backdrop" data-layer2-close="mask"></div>' +
      '<section class="access-drilldown-overlay-panel" role="dialog" aria-modal="true" aria-label="二层看板">' +
      '<header class="access-drilldown-overlay-head">' +
      '<div class="access-drilldown-overlay-head-meta">' +
      '<div class="access-drilldown-overlay-title" data-drilldown-title="true"></div>' +
      '<div class="access-drilldown-overlay-note" data-drilldown-note="true" hidden></div>' +
      "</div>" +
      '<button type="button" class="access-drilldown-overlay-close" data-layer2-close="button" aria-label="关闭">×</button>' +
      "</header>" +
      '<div class="access-drilldown-panel-hero" data-drilldown-hero="true" hidden>' +
      '<div class="access-drilldown-panel-hero-title" data-drilldown-hero-title="true"></div>' +
      '<div class="access-drilldown-panel-hero-note" data-drilldown-hero-note="true" hidden></div>' +
      "</div>" +
      '<div class="access-drilldown-overlay-tabs" data-drilldown-tabs="true" hidden></div>' +
      '<div class="access-drilldown-overlay-body" data-drilldown-body-mode="generic">' +
      drilldownLoadingStatusHtml("正在加载明细表...") +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="error" hidden>明细表加载失败，请稍后重试。</div>' +
      '<div class="access-drilldown-table-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-drilldown-table-host" data-drilldown-table-host="true"></div>' +
      "</div>" +
      "</div>" +
      '<div class="access-drilldown-overlay-body access-drilldown-overlay-body--structured" data-drilldown-body-mode="structured" hidden>' +
      drilldownLoadingStatusHtml("正在加载看板...") +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="error" hidden>看板加载失败，请稍后重试。</div>' +
      '<div class="access-drilldown-structured-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-drilldown-structured-layout" data-drilldown-structured-layout="true"></div>' +
      "</div>" +
      "</div>" +
      "</section>"
    );
  }

  function resolveViewportStageHost() {
    if (typeof boot.resolveViewportStageHost === "function") {
      return boot.resolveViewportStageHost();
    }
    const viewport = document.querySelector('[data-mei-frame-viewport="true"]');
    if (viewport instanceof HTMLElement) {
      const stage = viewport.querySelector(".preview-stage-shell");
      if (stage instanceof HTMLElement) {
        return stage;
      }
    }
    return document.body;
  }

  function resolveLayer2MountRoot() {
    if (typeof boot.resolveViewportStageSurface === "function") {
      const surface = boot.resolveViewportStageSurface();
      if (surface instanceof HTMLElement && surface !== document.body) {
        return surface;
      }
    }
    return resolveViewportStageHost();
  }

  function ensureLayer2WorkspaceRoot() {
    let root = document.getElementById(LAYER2_WORKSPACE_ROOT_ID);
    const stageHost = resolveLayer2MountRoot();
    if (root) {
      if (root.parentElement !== stageHost) {
        stageHost.appendChild(root);
      }
      root.classList.toggle("mei-layer2-in-viewport", stageHost !== document.body);
      return root;
    }
    root = document.createElement("div");
    root.id = LAYER2_WORKSPACE_ROOT_ID;
    root.className = "mei-layer2-workspace access-drilldown-overlay";
    if (stageHost !== document.body) {
      root.classList.add("mei-layer2-in-viewport");
    }
    root.setAttribute("hidden", "hidden");
    root.innerHTML =
      '<div class="mei-layer2-workspace-shell">' +
      '<nav class="mei-layer2-tab-bar" data-layer2-tab-bar="true" hidden></nav>' +
      '<div class="mei-layer2-tab-panels" data-layer2-tab-panels="true"></div>' +
      "</div>";
    root.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (target.dataset.layer2Close) {
        closeLayer2Stack();
      }
      const tabBtn = target.closest("[data-layer2-tab-id]");
      if (tabBtn instanceof HTMLElement && tabBtn.dataset.layer2TabId) {
        activateLayer2Tab(tabBtn.dataset.layer2TabId);
      }
    });
    stageHost.appendChild(root);
    return root;
  }

  function layer2Session() {
    const boot = window.__meiLangBoot || {};
    if (!boot.layer2Session) {
      boot.layer2Session = { tabs: [], activeTabId: null, workspaceConfig: {} };
    }
    return boot.layer2Session;
  }

  function syncLayer2TabBar(root) {
    const session = layer2Session();
    const bar = root.querySelector('[data-layer2-tab-bar="true"]');
    if (!(bar instanceof HTMLElement)) return;
    bar.replaceChildren();
    const showBar = session.tabs.length > 1;
    bar.toggleAttribute("hidden", !showBar);
    session.tabs.forEach((tab) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "mei-layer2-tab";
      btn.dataset.layer2TabId = tab.id;
      btn.textContent = tab.label || tab.id;
      if (tab.id === session.activeTabId) {
        btn.classList.add("is-active");
        btn.setAttribute("aria-selected", "true");
      }
      bar.appendChild(btn);
    });
  }

  function activateLayer2Tab(tabId) {
    const root = document.getElementById(LAYER2_WORKSPACE_ROOT_ID);
    if (!root) return;
    const session = layer2Session();
    session.activeTabId = tabId;
    root.querySelectorAll("[data-layer2-tab-panel]").forEach((panel) => {
      if (!(panel instanceof HTMLElement)) return;
      const active = panel.dataset.layer2TabPanel === tabId;
      panel.toggleAttribute("hidden", !active);
      panel.classList.toggle("is-active", active);
    });
    syncLayer2TabBar(root);
  }

  function openLayer2Tab(config) {
    const root = ensureLayer2WorkspaceRoot();
    const session = layer2Session();
    const sceneId = nonEmptyString(config?.boardSceneId, config?.sceneId, "board");
    const overlayWorkspace =
      (config?.overlayWorkspace && typeof config.overlayWorkspace === "object" && config.overlayWorkspace) ||
      (typeof boot.resolveOverlayWorkspace === "function"
        ? boot.resolveOverlayWorkspace(config?.popup, config)
        : null) ||
      {};
    const tabPolicy = nonEmptyString(overlayWorkspace?.tab_policy, "append");
    let tab = session.tabs.find((entry) => entry.sceneId === sceneId);
    if (tab && tabPolicy === "focus") {
      session.activeTabId = tab.id;
    } else {
      const tabId = `${sceneId}-${Date.now()}`;
      const panel = document.createElement("div");
      panel.className = "mei-layer2-tab-panel access-drilldown-overlay";
      panel.dataset.layer2TabPanel = tabId;
      panel.innerHTML = layer2OverlayPanelHtml();
      root.querySelector('[data-layer2-tab-panels="true"]')?.appendChild(panel);
      tab = {
        id: tabId,
        sceneId,
        label: nonEmptyString(
          config?.title,
          config?.mount?.title,
          config?.popup?.title,
          sceneId,
        ),
        panel,
      };
      session.tabs.push(tab);
      session.activeTabId = tabId;
    }
    const overlaySize = nonEmptyString(
      config?.overlaySize,
      overlayWorkspace?.size,
      config?.popup?.overlay_size,
      config?.popup?.overlaySize,
      "large",
    );
    tab.panel.classList.remove(
      "access-drilldown-overlay--size-comfortable",
      "access-drilldown-overlay--size-large",
      "access-drilldown-overlay--size-fullscreen",
    );
    tab.panel.classList.add(`access-drilldown-overlay--size-${overlaySize}`);
    applyDrilldownOverlayMeta(tab.panel, config);
    activateLayer2Tab(tab.id);
    root.removeAttribute("hidden");
    root.classList.add("is-open");
    document.body.classList.add("access-layer2-open");
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        scope: sceneId,
        sceneId,
        source: "layer2",
        overlaySize,
      });
    } else {
      document.dispatchEvent(
        new CustomEvent("meilang:scope-activation", {
          detail: {
            scope: sceneId,
            source: "layer2",
            overlaySize,
          },
        }),
      );
    }
    return tab.panel;
  }

  function closeLayer2Tab(tabId) {
    const root = document.getElementById(LAYER2_WORKSPACE_ROOT_ID);
    if (!root) return false;
    const session = layer2Session();
    if (!session.tabs.length) return false;
    const targetId = tabId || session.activeTabId;
    const index = session.tabs.findIndex((tab) => tab.id === targetId);
    if (index < 0) return false;
    const [removed] = session.tabs.splice(index, 1);
    if (removed?.panel instanceof HTMLElement) {
      abortDrilldownLoadIfNeeded(removed.panel);
      cleanupStructuredDrilldownWatcher(removed.panel);
      removed.panel.remove();
    }
    if (!session.tabs.length) {
      closeLayer2Stack();
      return true;
    }
    const next = session.tabs[Math.min(index, session.tabs.length - 1)];
    session.activeTabId = next.id;
    activateLayer2Tab(next.id);
    return true;
  }

  function closeLayer2Stack() {
    const root = document.getElementById(LAYER2_WORKSPACE_ROOT_ID);
    if (!root) return;
    const session = layer2Session();
    session.tabs.slice().forEach((tab) => closeLayer2Tab(tab.id));
    session.tabs = [];
    session.activeTabId = null;
    root.setAttribute("hidden", "hidden");
    root.classList.remove("is-open");
    document.body.classList.remove("access-layer2-open");
  }

  function hasOpenLayer2Workspace() {
    const root = document.getElementById(LAYER2_WORKSPACE_ROOT_ID);
    return Boolean(root && !root.hidden && root.classList.contains("is-open"));
  }

  boot.useUnifiedLayer2 = useUnifiedLayer2;
  boot.ensureLayer2WorkspaceRoot = ensureLayer2WorkspaceRoot;
  boot.openLayer2Tab = openLayer2Tab;
  boot.closeLayer2Tab = closeLayer2Tab;
  boot.closeLayer2Stack = closeLayer2Stack;
  boot.hasOpenLayer2Workspace = hasOpenLayer2Workspace;
