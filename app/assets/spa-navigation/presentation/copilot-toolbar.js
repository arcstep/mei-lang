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

  function fabLayout() {
    return boot.copilotFabLayout || null;
  }

  function scheduleFabLayout(options) {
    const layout = fabLayout();
    if (layout && typeof layout.scheduleCopilotFabToolbarLayout === "function") {
      layout.scheduleCopilotFabToolbarLayout(options);
      return;
    }
    if (layout && typeof layout.syncCopilotFabToolbarLayout === "function") {
      layout.syncCopilotFabToolbarLayout(options);
    }
  }

  function ensureFabDock() {
    const layout = fabLayout();
    if (layout && typeof layout.ensureFabDock === "function") {
      return layout.ensureFabDock();
    }
    return null;
  }

  function mountCopilotNode(node) {
    if (!(node instanceof HTMLElement)) {
      return node;
    }
    if (typeof boot.mountCopilotInViewport === "function" && boot.mountCopilotInViewport(node)) {
      return node;
    }
    if (node.parentElement !== document.body) {
      document.body.appendChild(node);
    }
    return node;
  }

  /** C 层 / FAB 挂入 viewport stage，与 T0/T1/T2 共用 letterbox 画布。 */
  function ensureCopilotInViewport() {
    if (typeof boot.relocateStageOverlaysInViewport === "function") {
      boot.relocateStageOverlaysInViewport();
      return;
    }
    if (typeof boot.relocateCopilotInViewport === "function") {
      boot.relocateCopilotInViewport();
      return;
    }
    const root = floatingRoot();
    if (root) {
      mountCopilotNode(root);
    }
  }

  function refreshFabChrome() {
    const fab = document.getElementById("access-chat-fab");
    if (!fab) return;
    const root = floatingRoot();
    root?.classList.toggle(
      "copilot-fab-elevated",
      Boolean(root?.classList.contains("mei-copilot-in-viewport")),
    );
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
    mountCopilotNode(node);
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
    mountCopilotNode(drawer);
    return drawer;
  }

  function ensureToolbar() {
    let toolbar = document.getElementById(TOOLBAR_ID);
    const dock = ensureFabDock();
    if (toolbar) {
      if (dock && toolbar.parentElement !== dock) {
        dock.appendChild(toolbar);
      }
      return toolbar;
    }
    toolbar = document.createElement("nav");
    toolbar.id = TOOLBAR_ID;
    toolbar.className = "copilot-toolbar";
    toolbar.setAttribute("aria-label", "Copilot 工具条");
    toolbar.setAttribute("hidden", "hidden");
    toolbar.innerHTML = toolbarInnerHtml();
    toolbar.addEventListener("click", onToolbarClick);
    const root = floatingRoot();
    if (dock) {
      dock.appendChild(toolbar);
    } else if (root) {
      mountCopilotNode(root);
      root.appendChild(toolbar);
    } else {
      document.body.appendChild(toolbar);
    }
    ensureCopilotInViewport();
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
    if (!eng) return "演";
    if (typeof eng.isActive === "function" && eng.isActive()) return "停";
    if (typeof eng.isPaused === "function" && eng.isPaused()) return "续";
    return "演";
  }

  function sessionButtonTitle(eng) {
    if (!eng) return "开始演说";
    if (typeof eng.isActive === "function" && eng.isActive()) return "暂停演说";
    if (typeof eng.isPaused === "function" && eng.isPaused()) return "继续演说";
    return "开始演说";
  }

  function toolbarGlyphButton(attrs) {
    const a = attrs && typeof attrs === "object" ? attrs : {};
    const classes = ["copilot-toolbar-btn", "copilot-toolbar-btn--glyph"];
    if (a.className) classes.push(a.className);
    const parts = [
      `<button type="button" class="${classes.join(" ")}"`,
      a.dataset ? ` data-${a.dataset.key}="true"` : "",
      a.disabled ? " disabled" : "",
      ` aria-label="${a.label || ""}"`,
      ` title="${a.title || a.label || ""}"`,
      ">",
      a.glyph || "",
      "</button>",
    ];
    return parts.join("");
  }

  function toolbarInnerHtml() {
    return (
      '<div class="copilot-toolbar-inner">' +
      toolbarGlyphButton({
        dataset: { key: "copilot-session" },
        glyph: "演",
        label: "开始演说",
        title: "开始演说",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-prev" },
        glyph: "上",
        label: "上一步",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-next" },
        glyph: "下",
        label: "下一步",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-caption-toggle" },
        glyph: "泡",
        label: "字幕气泡",
        title: "显示/隐藏字幕气泡",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-script-pick" },
        glyph: "选",
        label: "选择演说稿",
        title: "从演说稿目录载入或设为默认",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-script-panel" },
        glyph: "编",
        label: "讲稿编辑",
        title: "编辑并保存演说稿目录中的 MDX",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-script" },
        glyph: "稿",
        label: "演说稿",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-select-toggle" },
        glyph: "点",
        label: "组件选择",
        title: "点选场景组件",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-tts" },
        glyph: "音",
        label: "语音播报",
        title: "朗读当前步 caption / speaker notes",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-exit" },
        glyph: "完",
        label: "退出演说",
        className: "copilot-toolbar-btn--exit",
      }) +
      toolbarGlyphButton({
        dataset: { key: "copilot-ai" },
        glyph: "问",
        label: "AI 对话",
        className: "copilot-toolbar-btn--ai",
      }) +
      "</div>"
    );
  }

  function withEngineLoaded(run) {
    const eng = engine();
    if (!eng) return Promise.resolve(false);
    const invoke = (activeEng) => Promise.resolve(run(activeEng)).then(() => true);
    if (typeof eng.ensureLoaded === "function" && eng.ensureLoaded()) {
      return invoke(eng);
    }
    if (typeof eng.ensureLoadedAsync !== "function") return Promise.resolve(false);
    return eng.ensureLoadedAsync().then((ok) => {
      if (ok) return invoke(eng);
      return false;
    });
  }

  function ttsApi() {
    return boot.presentationTts || null;
  }

  function scriptPanel() {
    return boot.presentationScriptPanel || null;
  }

  function scriptLibrary() {
    return boot.presentationScriptLibrary || null;
  }

  function toolbarClickTarget(event) {
    const raw = event.target;
    if (!(raw instanceof Element)) return null;
    const button = raw.closest("button");
    return button instanceof HTMLButtonElement ? button : null;
  }

  function reportToolbarIssue(message) {
    const panel = scriptPanel();
    if (panel && typeof panel.setCompileResult === "function") {
      panel.setCompileResult(null, new Error(String(message || "操作失败")));
      if (typeof panel.togglePanel === "function") {
        panel.togglePanel(true);
      }
      return;
    }
    if (typeof console !== "undefined" && typeof console.warn === "function") {
      console.warn("[mei] copilot toolbar:", message);
    }
  }

  async function handleSessionClick() {
    const activeEng = engine();
    if (!activeEng) {
      reportToolbarIssue("演说步进引擎尚未就绪，请刷新页面后重试");
      return;
    }
    if (typeof activeEng.isActive === "function" && activeEng.isActive()) {
      activeEng.pause();
      renderAll();
      return;
    }
    if (typeof activeEng.isPaused === "function" && activeEng.isPaused()) {
      activeEng.resume();
      renderAll();
      return;
    }
    const lib = scriptLibrary();
    let started = false;
    if (lib && typeof lib.tryAutoStartPresentation === "function") {
      try {
        started = await lib.tryAutoStartPresentation({ apply: true });
      } catch (error) {
        reportToolbarIssue(error?.message || "载入默认演说稿失败");
        renderAll();
        return;
      }
    }
    if (!started) {
      const loaded = typeof activeEng.ensureLoadedAsync === "function"
        ? await activeEng.ensureLoadedAsync()
        : typeof activeEng.ensureLoaded === "function" && activeEng.ensureLoaded();
      if (loaded && typeof activeEng.start === "function") {
        started = Boolean(activeEng.start({ apply: true }));
      }
    }
    if (!started) {
      reportToolbarIssue("未找到可运行的演说稿，请点「选」从演说稿目录选择，或点「编」保存一份讲稿");
    }
    renderAll();
  }

  function onToolbarClick(event) {
    const target = toolbarClickTarget(event);
    if (!target) return;
    const eng = engine();
    if (target.dataset.copilotSession === "true") {
      void handleSessionClick();
      return;
    }
    if (target.dataset.copilotExit === "true") {
      exitPresentation();
      return;
    }
    if (target.dataset.copilotPrev === "true") {
      void withEngineLoaded((activeEng) => {
        activeEng.prev();
        renderAll();
      });
      return;
    }
    if (target.dataset.copilotNext === "true") {
      void withEngineLoaded((activeEng) => {
        activeEng.next();
        renderAll();
      });
      return;
    }
    if (target.dataset.copilotCaptionToggle === "true") {
      uiState.captionVisible = !uiState.captionVisible;
      target.classList.toggle("is-active", uiState.captionVisible);
      renderCaption();
      return;
    }
    if (target.dataset.copilotScriptPick === "true") {
      const panel = scriptPanel();
      if (!panel || typeof panel.openPicker !== "function") {
        reportToolbarIssue("演说稿目录模块尚未加载");
        return;
      }
      uiState.toolbarOpen = true;
      renderToolbar();
      void panel.openPicker().catch((error) => {
        reportToolbarIssue(error?.message || "读取演说稿目录失败");
      });
      return;
    }
    if (target.dataset.copilotScriptPanel === "true") {
      const panel = scriptPanel();
      if (!panel || typeof panel.togglePanel !== "function") {
        reportToolbarIssue("讲稿编辑面板尚未加载");
        return;
      }
      panel.togglePanel(true);
      uiState.toolbarOpen = true;
      renderToolbar();
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
    if (target.dataset.copilotTts === "true") {
      const tts = ttsApi();
      const step = eng ? eng.currentStep() : null;
      if (!tts || typeof tts.isSupported !== "function" || !tts.isSupported()) {
        target.classList.remove("is-active");
        return;
      }
      if (tts.state?.speaking) {
        tts.stopSpeech();
        target.classList.remove("is-active");
        return;
      }
      const enabled = typeof tts.toggleEnabled === "function" ? tts.toggleEnabled() : false;
      target.classList.toggle("is-active", enabled);
      if (enabled && step && typeof tts.speakStep === "function") {
        tts.speakStep(step, { force: true });
      }
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
    const html = step ? String(step.captionHtml || "") : "";
    const text = step ? String(step.caption || step.title || "") : "";
    if ((!html && !text) || !uiState.captionVisible || !(eng && eng.isActive())) {
      caption.setAttribute("hidden", "hidden");
      caption.innerHTML = "";
      return;
    }
    caption.removeAttribute("hidden");
    caption.innerHTML = html || `<p>${text}</p>`;
  }

  function renderDrawer() {
    const drawer = ensureDrawer();
    const eng = engine();
    const step = eng ? eng.currentStep() : null;
    const body = drawer.querySelector("[data-copilot-script-body]");
    const notesHtml = step ? String(step.speakerNotesHtml || "") : "";
    const notes = step ? String(step.speaker_notes || step.notes || "") : "";
    if (!uiState.drawerOpen || (!notesHtml && !notes)) {
      drawer.setAttribute("hidden", "hidden");
      if (body) body.innerHTML = "";
      return;
    }
    drawer.removeAttribute("hidden");
    if (body) body.innerHTML = notesHtml || `<p>${notes}</p>`;
  }

  function renderToolbar() {
    const toolbar = ensureToolbar();
    const eng = engine();
    const step = eng ? eng.currentStep() : null;
    const manifest = eng ? eng.state.manifest : null;
    const layout = fabLayout();
    if (layout && typeof layout.syncCopilotFabToolbarLayout === "function") {
      layout.syncCopilotFabToolbarLayout();
    }
    const sessionBtn = toolbar.querySelector("[data-copilot-session]");
    if (sessionBtn) {
      sessionBtn.textContent = sessionButtonLabel(eng);
      const sessionTitle = sessionButtonTitle(eng);
      sessionBtn.setAttribute("title", sessionTitle);
      sessionBtn.setAttribute("aria-label", sessionTitle);
    }
    const ttsBtn = toolbar.querySelector("[data-copilot-tts]");
    if (ttsBtn) {
      const tts = ttsApi();
      const supported = Boolean(tts && typeof tts.isSupported === "function" && tts.isSupported());
      ttsBtn.disabled = !supported;
      ttsBtn.classList.toggle("is-active", Boolean(tts?.state?.enabled || tts?.state?.speaking));
    }
    toolbar.dataset.progress =
      eng && eng.steps.length ? `${eng.stepIndex + 1} / ${eng.steps.length}` : "";
    toolbar.title = step?.title || manifest?.title || COPILOT_TITLE;
    const nextOpen = uiState.toolbarOpen;
    if (nextOpen) {
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
    const ctx = boot.copilotFabContext;
    if (ctx && typeof ctx.copilotFabContextActive === "function") {
      return ctx.copilotFabContextActive();
    }
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
    if (!fab || !copilotFabContextActive()) return;
    boot.copilotFabBound = true;
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
    const ctx = boot.copilotFabContext;
    if (ctx && typeof ctx.shouldMountCopilotToolbar === "function") {
      return ctx.shouldMountCopilotToolbar();
    }
    const eng = engine();
    return Boolean((eng && eng.hasManifest()) || isCopilotRoute() || hasCopilotShell());
  }

  function mount(options) {
    const opts = options && typeof options === "object" ? options : {};
    const eng = engine();
    if (!shouldMount()) return false;
    ensureToolbar();
    ensureCaption();
    ensureCopilotInViewport();
    bindSelectMode();
    bindFabBehavior();
    const panel = scriptPanel();
    if (panel && typeof panel.renderPanel === "function") {
      panel.renderPanel();
    }
    const alreadyMounted = uiState.mounted;
    uiState.mounted = true;
    if (!alreadyMounted) {
      uiState.toolbarOpen = opts.toolbarOpen === true;
    } else if (typeof opts.toolbarOpen === "boolean") {
      uiState.toolbarOpen = opts.toolbarOpen;
    }
    if (opts.autoStart === true && eng && (isCopilotRoute() || hasCopilotShell())) {
      if (typeof eng.ensureLoaded === "function") {
        eng.ensureLoaded();
      }
      eng.start({ apply: opts.apply !== false });
    }
    renderAll();
    scheduleFabLayout();
    return true;
  }

  const toolbar = {
    mount,
    renderAll,
    syncLayout: scheduleFabLayout,
    onStepApplied(step) {
      ensureCopilotInViewport();
      const tts = ttsApi();
      if (tts && tts.state?.enabled && tts.state?.autoSpeak && typeof tts.speakStep === "function") {
        tts.speakStep(step, { force: true });
      }
      renderAll();
      scheduleFabLayout();
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

  window.addEventListener("meilang:viewport-stage-ready", () => {
    if (shouldMount()) {
      mount({ autoStart: false, apply: false, toolbarOpen: false });
    } else {
      ensureCopilotInViewport();
      scheduleFabLayout();
    }
  });
  window.addEventListener("meilang:viewport-stage-layout", () => {
    ensureCopilotInViewport();
    scheduleFabLayout();
  });
})();
