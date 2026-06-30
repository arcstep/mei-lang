(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  const TOOLBAR_ID = "copilot-toolbar";
  const CAPTION_ID = "mei-copilot-caption";
  const DRAWER_ID = "copilot-script-drawer";
  const COPILOT_TITLE = "Copilot";

  const uiState = {
    toolbarOpen: false,
    captionVisible: true,
    selectMode: false,
    drawerOpen: false,
    mounted: false,
  };

  function engine() {
    return boot.presentationStepEngine || null;
  }

  function isCopilotRoute() {
    return /^\/apps\/copilot\//.test(String(window.location.pathname || ""));
  }

  function hasCopilotShell() {
    return Boolean(
      document.getElementById("copilot-shell") ||
        document.getElementById("speaker-shell") ||
        document.getElementById("mei-presentation-manifest") ||
        document.getElementById("mei-copilot-tour"),
    );
  }

  function floatingRoot() {
    return document.getElementById("access-chat-floating-root");
  }

  /** FAB 与工具条整体抬到 body 顶层，避免 slides_only 全屏层遮挡。 */
  function ensureCopilotFabElevation() {
    if (!copilotFabContextActive()) return;
    const root = floatingRoot();
    if (!root) return;
    if (root.parentElement !== document.body) {
      document.body.appendChild(root);
    }
    root.classList.add("copilot-fab-elevated");
    document.body.classList.add("mei-copilot-fab-mounted");
  }

  function refreshFabChrome() {
    const fab = document.getElementById("access-chat-fab");
    if (!fab) return;
    const eng = engine();
    const active = eng && eng.isActive();
    const paused = eng && typeof eng.isPaused === "function" && eng.isPaused();
    let label = "展开 Copilot 工具条";
    if (uiState.toolbarOpen) {
      if (active) label = "收起工具条（演说进行中）";
      else if (paused) label = "收起工具条（演说已暂停）";
      else label = "收起 Copilot 工具条";
    }
    fab.title = label;
    fab.setAttribute("aria-label", label);
  }

  function ensureCaption() {
    let node = document.getElementById(CAPTION_ID);
    if (node) return node;
    node = document.createElement("div");
    node.id = CAPTION_ID;
    node.className = "mei-copilot-caption mei-presenter-caption";
    node.setAttribute("hidden", "hidden");
    document.body.appendChild(node);
    return node;
  }

  function ensureDrawer() {
    let drawer = document.getElementById(DRAWER_ID);
    if (drawer) return drawer;
    drawer = document.createElement("aside");
    drawer.id = DRAWER_ID;
    drawer.className = "copilot-script-drawer";
    drawer.setAttribute("hidden", "hidden");
    drawer.innerHTML =
      '<header class="copilot-script-drawer-head">' +
      '<strong class="copilot-script-drawer-title">演说稿</strong>' +
      '<button type="button" class="copilot-script-drawer-close" data-copilot-drawer-close="true" aria-label="关闭演说稿">×</button>' +
      "</header>" +
      '<div class="copilot-script-drawer-body" data-copilot-script-body="true"></div>';
    drawer.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (target.dataset.copilotDrawerClose === "true") {
        uiState.drawerOpen = false;
        renderDrawer();
      }
    });
    document.body.appendChild(drawer);
    return drawer;
  }

  function ensureToolbar() {
    let toolbar = document.getElementById(TOOLBAR_ID);
    if (toolbar) return toolbar;
    const root = floatingRoot();
    toolbar = document.createElement("nav");
    toolbar.id = TOOLBAR_ID;
    toolbar.className = "copilot-toolbar";
    toolbar.setAttribute("aria-label", "Copilot 工具条");
    toolbar.innerHTML =
      '<div class="copilot-toolbar-inner">' +
      '<button type="button" class="copilot-toolbar-btn" data-copilot-session="true">开始</button>' +
      '<button type="button" class="copilot-toolbar-btn" data-copilot-prev="true">上一步</button>' +
      '<button type="button" class="copilot-toolbar-btn" data-copilot-next="true">下一步</button>' +
      '<button type="button" class="copilot-toolbar-btn" data-copilot-caption-toggle="true">气泡</button>' +
      '<button type="button" class="copilot-toolbar-btn" data-copilot-script="true">演说稿</button>' +
      '<button type="button" class="copilot-toolbar-btn" data-copilot-select-toggle="true">组件选择</button>' +
      '<button type="button" class="copilot-toolbar-btn" data-copilot-tts="true" disabled title="语音播报即将支持">播放</button>' +
      '<button type="button" class="copilot-toolbar-btn copilot-toolbar-btn--exit" data-copilot-exit="true">退出演说</button>' +
      '<button type="button" class="copilot-toolbar-btn copilot-toolbar-btn--ai" data-copilot-ai="true">AI 对话</button>' +
      "</div>";
    toolbar.addEventListener("click", onToolbarClick);
    if (root) {
      root.appendChild(toolbar);
    } else {
      document.body.appendChild(toolbar);
    }
    return toolbar;
  }

  function toggleAccessAiPanel(open) {
    const toggle = boot.toggleAccessFloatingPanel;
    if (typeof toggle === "function") {
      toggle(open);
      return;
    }
    const panel = document.getElementById("access-chat-overlay-panel");
    const fabRoot = floatingRoot();
    if (!panel || !fabRoot) return;
    const next = typeof open === "boolean" ? open : panel.hidden;
    panel.hidden = !next;
    fabRoot.dataset.open = next ? "true" : "false";
  }

  function exitPresentation() {
    const eng = engine();
    if (eng && typeof eng.stop === "function") {
      eng.stop();
    }
    uiState.drawerOpen = false;
    uiState.selectMode = false;
    document.body.classList.remove("mei-presenter-select-mode");
    renderAll();
    const match = String(window.location.pathname || "").match(
      /^\/apps\/(?:copilot|speaker)\/([^/]+)/,
    );
    if (match && match[1]) {
      window.location.href = `/apps/app/${encodeURIComponent(match[1])}/scene/home`;
    }
  }

  function sessionButtonLabel(eng) {
    if (!eng) return "开始";
    if (typeof eng.isActive === "function" && eng.isActive()) return "暂停";
    if (typeof eng.isPaused === "function" && eng.isPaused()) return "继续";
    return "开始";
  }

  function onToolbarClick(event) {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const eng = engine();
    if (target.dataset.copilotSession === "true") {
      if (!eng) return;
      if (typeof eng.isActive === "function" && eng.isActive()) {
        eng.pause();
      } else if (typeof eng.isPaused === "function" && eng.isPaused()) {
        eng.resume();
      } else {
        eng.start({ apply: true });
      }
      renderAll();
      return;
    }
    if (target.dataset.copilotExit === "true") {
      exitPresentation();
      return;
    }
    if (target.dataset.copilotPrev === "true") {
      if (eng) eng.prev();
      renderAll();
      return;
    }
    if (target.dataset.copilotNext === "true") {
      if (eng) eng.next();
      renderAll();
      return;
    }
    if (target.dataset.copilotCaptionToggle === "true") {
      uiState.captionVisible = !uiState.captionVisible;
      target.classList.toggle("is-active", uiState.captionVisible);
      renderCaption();
      return;
    }
    if (target.dataset.copilotScript === "true") {
      uiState.drawerOpen = !uiState.drawerOpen;
      renderDrawer();
      return;
    }
    if (target.dataset.copilotSelectToggle === "true") {
      uiState.selectMode = !uiState.selectMode;
      document.body.classList.toggle("mei-presenter-select-mode", uiState.selectMode);
      target.classList.toggle("is-active", uiState.selectMode);
      return;
    }
    if (target.dataset.copilotAi === "true") {
      toggleAccessAiPanel(true);
    }
  }

  function renderCaption() {
    const caption = ensureCaption();
    const eng = engine();
    const step = eng ? eng.currentStep() : null;
    const text = step ? String(step.caption || step.title || "") : "";
    if (!text || !uiState.captionVisible || !(eng && eng.isActive())) {
      caption.setAttribute("hidden", "hidden");
      caption.textContent = "";
      return;
    }
    caption.removeAttribute("hidden");
    caption.textContent = text;
  }

  function renderDrawer() {
    const drawer = ensureDrawer();
    const eng = engine();
    const step = eng ? eng.currentStep() : null;
    const body = drawer.querySelector("[data-copilot-script-body]");
    const notes = step ? String(step.speaker_notes || step.notes || "") : "";
    if (!uiState.drawerOpen || !notes) {
      drawer.setAttribute("hidden", "hidden");
      if (body) body.textContent = "";
      return;
    }
    drawer.removeAttribute("hidden");
    if (body) body.textContent = notes;
  }

  function renderToolbar() {
    const toolbar = ensureToolbar();
    const eng = engine();
    const step = eng ? eng.currentStep() : null;
    const manifest = eng ? eng.state.manifest : null;
    const sessionBtn = toolbar.querySelector("[data-copilot-session]");
    if (sessionBtn) {
      sessionBtn.textContent = sessionButtonLabel(eng);
    }
    toolbar.dataset.progress =
      eng && eng.steps.length ? `${eng.stepIndex + 1} / ${eng.steps.length}` : "";
    toolbar.title = step?.title || manifest?.title || COPILOT_TITLE;
    if (uiState.toolbarOpen) {
      toolbar.removeAttribute("hidden");
      floatingRoot()?.classList.add("copilot-toolbar-active");
    } else {
      toolbar.setAttribute("hidden", "hidden");
      floatingRoot()?.classList.remove("copilot-toolbar-active");
    }
    refreshFabChrome();
  }

  function renderAll() {
    renderToolbar();
    renderCaption();
    if (uiState.drawerOpen) renderDrawer();
  }

  function copilotFabContextActive() {
    const eng = engine();
    return Boolean(
      isCopilotRoute() ||
        hasCopilotShell() ||
        (eng && typeof eng.hasManifest === "function" && eng.hasManifest()),
    );
  }

  function bindFabBehavior() {
    if (boot.copilotFabBound) return;
    const fab = document.getElementById("access-chat-fab");
    const eng = engine();
    if (!fab || !eng || !copilotFabContextActive()) return;
    boot.copilotFabBound = true;
    fab.addEventListener(
      "click",
      (event) => {
        if (!eng.hasManifest()) return;
        if (boot.agentPanelState && boot.agentPanelState.accessFloatingDragMoved) return;
        event.preventDefault();
        event.stopImmediatePropagation();
        uiState.toolbarOpen = !uiState.toolbarOpen;
        renderToolbar();
      },
      true,
    );
    refreshFabChrome();
  }

  function bindSelectMode() {
    if (boot.copilotSelectBound) return;
    boot.copilotSelectBound = true;
    document.addEventListener(
      "click",
      (event) => {
        const eng = engine();
        if (!uiState.selectMode || !(eng && eng.isActive())) return;
        const target = event.target;
        if (!(target instanceof Element)) return;
        const host = target.closest("[data-mei-viewpoint]");
        if (!(host instanceof HTMLElement)) return;
        const viewpointId = String(host.dataset.meiViewpoint || "").trim();
        if (!viewpointId) return;
        event.preventDefault();
        event.stopPropagation();
        eng.highlight(viewpointId);
      },
      true,
    );
  }

  function shouldMount() {
    const eng = engine();
    return Boolean((eng && eng.hasManifest()) || isCopilotRoute() || hasCopilotShell());
  }

  function mount(options) {
    const opts = options && typeof options === "object" ? options : {};
    const eng = engine();
    if (!eng || !shouldMount()) return false;
    const force = opts.force === true;
    if (!eng.ensureLoaded() && !force && !isCopilotRoute() && !hasCopilotShell()) return false;
    ensureToolbar();
    ensureCaption();
    ensureCopilotFabElevation();
    bindSelectMode();
    bindFabBehavior();
    uiState.mounted = true;
    uiState.toolbarOpen = opts.toolbarOpen === true;
    if (opts.autoStart === true && (isCopilotRoute() || hasCopilotShell())) {
      eng.start({ apply: opts.apply !== false });
    }
    renderAll();
    return true;
  }

  const toolbar = {
    mount,
    renderAll,
    onStepApplied() {
      ensureCopilotFabElevation();
      renderAll();
    },
    toggleToolbar(next) {
      if (typeof next === "boolean") uiState.toolbarOpen = next;
      else uiState.toolbarOpen = !uiState.toolbarOpen;
      renderToolbar();
    },
    exitPresentation,
    uiState,
  };

  boot.copilotToolbar = toolbar;
  boot.toggleAccessFloatingPanel = boot.toggleAccessFloatingPanel || toggleAccessAiPanel;
})();
