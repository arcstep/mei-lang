  const LAYER2_WORKSPACE_ROOT_ID = "mei-layer2-workspace";
  const LAYER2_SIZE_OPTIONS = ["comfortable", "large", "fullscreen"];
  const LAYER2_SIZE_LABELS = {
    comfortable: "适中",
    large: "较大",
    fullscreen: "全屏",
  };
  const LAYER2_SHELL_SIZE_CLASSES = [
    "mei-layer2-browser-shell--size-comfortable",
    "mei-layer2-browser-shell--size-large",
    "mei-layer2-browser-shell--size-fullscreen",
  ];

  function useUnifiedLayer2() {
    const boot = window.__meiLangBoot || {};
    if (boot.unifiedLayer2 === false) return false;
    if (window.__mei && window.__mei.unified_layer2 === false) return false;
    return true;
  }

  /** 单 tab 内容页：无独立 backdrop/尺寸；标题在浏览器式 chrome 标签栏。 */
  function layer2OverlayPanelHtml() {
    return (
      '<section class="access-drilldown-overlay-panel mei-layer2-page-panel" role="document" aria-label="二层看板页">' +
      '<header class="access-drilldown-overlay-head mei-layer2-page-head" hidden>' +
      '<div class="access-drilldown-overlay-head-meta">' +
      '<div class="access-drilldown-overlay-title" data-drilldown-title="true"></div>' +
      '<div class="access-drilldown-overlay-note" data-drilldown-note="true" hidden></div>' +
      "</div>" +
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

  function layer2BrowserShellHtml() {
    const sizeControls = LAYER2_SIZE_OPTIONS.map(
      (size) =>
        `<button type="button" class="mei-layer2-size-btn" data-layer2-size="${size}" aria-label="尺寸：${LAYER2_SIZE_LABELS[size]}">${LAYER2_SIZE_LABELS[size]}</button>`,
    ).join("");
    return (
      '<div class="access-drilldown-overlay-backdrop" data-layer2-close="mask"></div>' +
      '<div class="mei-layer2-browser-shell" data-layer2-browser-shell="true" role="dialog" aria-modal="true" aria-label="二层多标签壳">' +
      '<header class="mei-layer2-chrome">' +
      '<nav class="mei-layer2-tab-bar" data-layer2-tab-bar="true" role="tablist" aria-label="已打开的看板"></nav>' +
      '<div class="mei-layer2-chrome-actions">' +
      `<div class="mei-layer2-size-switch" role="group" aria-label="窗口尺寸">${sizeControls}</div>` +
      '<button type="button" class="mei-layer2-window-close" data-layer2-close="window" aria-label="关闭全部标签">×</button>' +
      "</div>" +
      "</header>" +
      '<div class="mei-layer2-tab-panels" data-layer2-tab-panels="true"></div>' +
      "</div>"
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

  function closeActiveLayer2TabOrStack() {
    // 窗口级关闭（chrome 右侧 × / 遮罩）：整窗关闭，等同关掉浏览器。
    return closeLayer2Stack();
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
    root.innerHTML = layer2BrowserShellHtml();
    root.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      const sizeBtn = target.closest("[data-layer2-size]");
      if (sizeBtn instanceof HTMLElement && sizeBtn.dataset.layer2Size) {
        event.preventDefault();
        event.stopPropagation();
        applyLayer2TabSize(sizeBtn.dataset.layer2Size);
        return;
      }
      const tabClose = target.closest("[data-layer2-tab-close]");
      if (tabClose instanceof HTMLElement && tabClose.dataset.layer2TabClose) {
        event.preventDefault();
        event.stopPropagation();
        closeLayer2Tab(tabClose.dataset.layer2TabClose);
        return;
      }
      if (target.dataset.layer2Close || target.closest("[data-layer2-close]")) {
        event.preventDefault();
        closeActiveLayer2TabOrStack();
        return;
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
      boot.layer2Session = {
        tabs: [],
        activeTabId: null,
        overlaySize: "large",
        workspaceConfig: {},
      };
    }
    if (!boot.layer2Session.overlaySize) {
      boot.layer2Session.overlaySize = "large";
    }
    return boot.layer2Session;
  }

  function resolveBrowserShell(root) {
    const host = root instanceof HTMLElement ? root : document.getElementById(LAYER2_WORKSPACE_ROOT_ID);
    if (!(host instanceof HTMLElement)) return null;
    const shell = host.querySelector('[data-layer2-browser-shell="true"]');
    return shell instanceof HTMLElement ? shell : null;
  }

  function syncLayer2SizeButtons(root, overlaySize) {
    const host = root instanceof HTMLElement ? root : document.getElementById(LAYER2_WORKSPACE_ROOT_ID);
    if (!(host instanceof HTMLElement)) return;
    host.querySelectorAll("[data-layer2-size]").forEach((btn) => {
      if (!(btn instanceof HTMLElement)) return;
      const active = btn.dataset.layer2Size === overlaySize;
      btn.classList.toggle("is-active", active);
      btn.setAttribute("aria-pressed", active ? "true" : "false");
    });
  }

  function applyLayer2ShellSize(size) {
    const root = document.getElementById(LAYER2_WORKSPACE_ROOT_ID);
    const shell = resolveBrowserShell(root);
    if (!(shell instanceof HTMLElement)) return false;
    const normalized = nonEmptyString(size, "large");
    if (!LAYER2_SIZE_OPTIONS.includes(normalized)) return false;
    const session = layer2Session();
    session.overlaySize = normalized;
    session.tabs.forEach((tab) => {
      tab.overlaySize = normalized;
    });
    root.classList.remove(
      "access-drilldown-overlay--size-comfortable",
      "access-drilldown-overlay--size-large",
      "access-drilldown-overlay--size-fullscreen",
    );
    root.classList.add(`access-drilldown-overlay--size-${normalized}`);
    shell.classList.remove(...LAYER2_SHELL_SIZE_CLASSES);
    shell.classList.add(`mei-layer2-browser-shell--size-${normalized}`);
    shell.dataset.drilldownOverlaySize = normalized;
    syncLayer2SizeButtons(root, normalized);
    return true;
  }

  function applyLayer2TabSize(size) {
    return applyLayer2ShellSize(size);
  }

  function truncateTabLabel(label, maxLen = 18) {
    const text = String(label || "").trim();
    if (!text) return "未命名";
    if (text.length <= maxLen) return text;
    return `${text.slice(0, Math.max(1, maxLen - 1))}…`;
  }

  function syncLayer2TabBar(root) {
    const session = layer2Session();
    const bar = root.querySelector('[data-layer2-tab-bar="true"]');
    if (!(bar instanceof HTMLElement)) return;
    bar.replaceChildren();
    // 浏览器风格：即使仅 1 个标签也显示标题条
    bar.removeAttribute("hidden");
    session.tabs.forEach((tab) => {
      const wrap = document.createElement("div");
      wrap.className = "mei-layer2-tab-item";
      wrap.setAttribute("role", "presentation");
      if (tab.id === session.activeTabId) {
        wrap.classList.add("is-active");
      }
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "mei-layer2-tab";
      btn.dataset.layer2TabId = tab.id;
      btn.setAttribute("role", "tab");
      const fullLabel = nonEmptyString(tab.label, tab.sceneId, tab.id);
      btn.title = fullLabel;
      btn.textContent = truncateTabLabel(fullLabel);
      if (tab.id === session.activeTabId) {
        btn.classList.add("is-active");
        btn.setAttribute("aria-selected", "true");
      } else {
        btn.setAttribute("aria-selected", "false");
      }
      const closeBtn = document.createElement("button");
      closeBtn.type = "button";
      closeBtn.className = "mei-layer2-tab-close";
      closeBtn.dataset.layer2TabClose = tab.id;
      closeBtn.setAttribute("aria-label", `关闭 ${fullLabel}`);
      closeBtn.textContent = "×";
      wrap.appendChild(btn);
      wrap.appendChild(closeBtn);
      bar.appendChild(wrap);
    });
  }

  function resolveAppIdFromShell() {
    const shell = document.querySelector("[data-runtime-node][data-app-path], .shell[data-app-path]");
    return shell ? String(shell.getAttribute("data-app-path") || "").trim() : "";
  }

  function dispatchLayer2ScopeActivation(sceneId, source) {
    const scope = nonEmptyString(sceneId);
    if (!scope) return;
    const appId = resolveAppIdFromShell();
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        scope,
        sceneId: scope,
        appId,
        source: source || "layer2",
      });
      return;
    }
    document.dispatchEvent(
      new CustomEvent("meilang:scope-activation", {
        detail: { scope, sceneId: scope, appId, source: source || "layer2" },
      }),
    );
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
    syncLayer2SizeButtons(root, session.overlaySize || "large");
    const tab = session.tabs.find((entry) => entry.id === tabId);
    if (tab?.sceneId) {
      dispatchLayer2ScopeActivation(tab.sceneId, "layer2-tab");
    }
  }

  function resolveLayer2TabLabel(config, sceneId) {
    return nonEmptyString(
      config?.title,
      config?.mount?.title,
      config?.popup?.title,
      config?.detail?.label,
      config?.label,
      config?.summary,
      sceneId,
    );
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
    const tabPolicy = nonEmptyString(overlayWorkspace?.tab_policy, overlayWorkspace?.tabPolicy, "append");
    const reuseExisting = tabPolicy === "focus" || tabPolicy === "replace";
    let tab = reuseExisting ? session.tabs.find((entry) => entry.sceneId === sceneId) : null;
    if (tab) {
      session.activeTabId = tab.id;
      const nextLabel = resolveLayer2TabLabel(config, sceneId);
      if (nextLabel) tab.label = nextLabel;
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
        label: resolveLayer2TabLabel(config, sceneId),
        panel,
        overlaySize: session.overlaySize || "large",
      };
      session.tabs.push(tab);
      session.activeTabId = tabId;
    }
    const preserveShellSize = hasOpenLayer2Workspace() && session.tabs.length > 0;
    const overlaySize = preserveShellSize
      ? nonEmptyString(session.overlaySize, "large")
      : nonEmptyString(
          config?.overlaySize,
          overlayWorkspace?.size,
          config?.popup?.overlay_size,
          config?.popup?.overlaySize,
          "large",
        );
    applyLayer2ShellSize(overlaySize);
    applyDrilldownOverlayMeta(tab.panel, config);
    const titleFromMeta = nonEmptyString(
      tab.panel.querySelector('[data-drilldown-title="true"]')?.textContent,
      resolveLayer2TabLabel(config, sceneId),
    );
    if (titleFromMeta) tab.label = titleFromMeta;
    activateLayer2Tab(tab.id);
    root.removeAttribute("hidden");
    root.classList.add("is-open");
    document.body.classList.add("access-layer2-open");
    if (typeof boot.dispatchScopeActivation === "function") {
      boot.dispatchScopeActivation({
        scope: sceneId,
        sceneId,
        appId: resolveAppIdFromShell(),
        source: "layer2",
        overlaySize,
      });
    } else {
      document.dispatchEvent(
        new CustomEvent("meilang:scope-activation", {
          detail: {
            scope: sceneId,
            sceneId,
            appId: resolveAppIdFromShell(),
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
    if (!root) return false;
    const session = layer2Session();
    const tabs = session.tabs.slice();
    session.tabs = [];
    session.activeTabId = null;
    tabs.forEach((tab) => {
      if (tab?.panel instanceof HTMLElement) {
        abortDrilldownLoadIfNeeded(tab.panel);
        cleanupStructuredDrilldownWatcher(tab.panel);
        tab.panel.remove();
      }
    });
    root.setAttribute("hidden", "hidden");
    root.classList.remove("is-open");
    document.body.classList.remove("access-layer2-open");
    return true;
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
  boot.applyLayer2TabSize = applyLayer2TabSize;
  boot.applyLayer2ShellSize = applyLayer2ShellSize;
  boot.hasOpenLayer2Workspace = hasOpenLayer2Workspace;
