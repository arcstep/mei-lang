(function () {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (typeof boot.disposeAgentPanel === "function") {
    try {
      boot.disposeAgentPanel();
    } catch (_) {}
    boot.disposeAgentPanel = null;
  }

  const root = document.getElementById("meilang-author-panel");
  if (!root) return;

  const els = {
    serverDot: document.getElementById("author-server-dot"),
    serverStatus: document.getElementById("author-server-status"),
    config: document.getElementById("author-config-line"),
    modelLabel: document.getElementById("author-model-label"),
    reconnect: document.getElementById("author-reconnect-btn"),
    newSession: document.getElementById("author-session-btn"),
    skillLine: document.getElementById("author-skill-line"),
    sessionSelect: document.getElementById("author-session-select"),
    chatLog: document.getElementById("author-chat-log"),
    progressStrip: document.getElementById("author-progress-strip"),
    progressLabel: document.getElementById("author-progress-label"),
    progressDetail: document.getElementById("author-progress-detail"),
    progressItems: document.getElementById("author-progress-items"),
    contextRefresh: document.getElementById("author-context-refresh-btn"),
    contextScope: document.getElementById("author-context-preview-scope"),
    contextSkill: document.getElementById("author-context-preview-skill"),
    contextTools: document.getElementById("author-context-preview-tools"),
    contextInventory: document.getElementById("author-context-preview-inventory"),
    contextPrompt: document.getElementById("author-context-preview-prompt"),
    contextDeltaDebug: document.getElementById("author-context-preview-delta-debug"),
    input: document.getElementById("author-intent-input"),
    run: document.getElementById("author-run-btn"),
    modeAsk:
      document.getElementById("author-mode-ask-btn") ||
      document.getElementById("author-mode-plan-btn"),
    modeBuild: document.getElementById("author-mode-build-btn"),
    completionModelSelect: document.getElementById("author-completion-model-select"),
    completionModelWrap: document.getElementById("author-completion-model-wrap"),
    undo: document.getElementById("author-undo-btn"),
    redo: document.getElementById("author-redo-btn"),
    sourceViewDiffBtn: document.getElementById("source-view-diff-btn"),
    sourceViewHost: document.getElementById("source-view-host"),
    sourceViewSourcePanel: document.getElementById("source-view-source-panel"),
    sourceViewSourceRaw: document.getElementById("source-view-source-raw"),
    sourceViewDiffPanel: document.getElementById("source-view-diff-panel"),
    accessFloatingRoot: document.getElementById("access-chat-floating-root"),
    accessFab: document.getElementById("access-chat-fab"),
    accessClose: document.getElementById("access-chat-close"),
    accessPanel: document.getElementById("access-chat-overlay-panel"),
    statusModelService: document.getElementById("mei-status-model-service"),
  };

  const state = {
    config: null,
    runtime: null,
    skillStatus: null,
    health: null,
    sessions: [],
    sessionId: "",
    sessionTargetKey: "",
    messages: [],
    loading: false,
    sending: false,
    aborting: false,
    eventSource: null,
    eventSourceSessionId: "",
    streamConnected: false,
    modelLabel: "模型",
    sessionsCacheAtMs: 0,
    sessionsFetchInFlight: null,
    sendAbortController: null,
    pendingPromptDraft: "",
    generationSettleTimer: null,
    _meiAutoSessionOnce: false,
    _meiClientAutoOpencodeOnce: false,
    lastMessagesFingerprint: "",
    inlineNote: "",
    agentMode: "build",
    sessionHasRevertedChanges: {},
    revertedMessageIds: {},
    messageMeta: {},
    messageDiffCache: {},
    pendingReloadMessageId: "",
    pendingPermissionsFingerprint: "",
    pendingPermissionsFetchedAt: 0,
    pendingPermissionNotices: [],
    pendingPermissionsBootstrappedSessionId: "",
    activeGenerationMessageId: "",
    latestRoundAssistantId: "",
    latestDiffMessageId: "",
    sourceViewMode: "source",
    sourceDiffMessageId: "",
    sourceDiffMergeView: null,
    sourceCodeMirror: null,
    sourceEditorContainer: null,
    sourceDiffResizeObserver: null,
    sourceViewResizeObserver: null,
    contextPreview: null,
    contextPreviewBackoffUntilMs: 0,
    contextPreviewFetchedAtMs: 0,
    contextPreviewScopeKey: "",
    modelProbe: null,
    modelProbeFetchedAtMs: 0,
    modelProbeFailureStreak: 0,
    modelProbeLastSuccessAtMs: 0,
    accessFloatingOpen: false,
    accessFloatingDragMoved: false,
    deltaDebugLog: [],
    progress: {
      visible: false,
      label: "",
      detail: "",
      items: [],
    },
  };

  const SESSION_CACHE_KEY = "mei.author.sessions.v1";
  const SESSION_CACHE_TTL_MS = 30000;
  const MODEL_PROBE_RED_AFTER_STREAK = 3;
  const MODEL_PROBE_RED_AFTER_MS = 20000;
  const MODEL_PROBE_COLD_START_RED_AFTER_STREAK = 5;
  const ACCESS_FLOATING_MARGIN_PX = 10;
  const ACCESS_FLOATING_DRAG_THRESHOLD_PX = 4;
  let accessFloatingDragState = null;
  const CHAT_BOTTOM_STICKY_THRESHOLD_PX = 28;
  const COMPOSER_MIN_ROWS = 2;
  const COMPOSER_MAX_ROWS = 12;
  const CHAT_CLASS = {
    messageBase:
      "author-chat-message group grid gap-1 bg-transparent px-0 py-0.5 pl-2 border-l-2 border-l-transparent",
    messageUser: "author-chat-user border-l-blue-400/65",
    messageAssistant: "author-chat-assistant border-l-emerald-400/55",
    messageAssistantReverted: "author-chat-assistant-reverted border-l-slate-400/65",
    messageSystem: "author-chat-system border-l-amber-300/55",
    roleBase: "author-chat-role text-[10px] font-bold tracking-[0.02em] opacity-90",
    roleUser: "text-blue-300",
    roleAssistant: "text-emerald-300",
    roleAssistantReverted: "text-slate-400",
    roleSystem: "text-amber-300",
    head: "author-chat-head flex items-center justify-between gap-2",
    meta:
      "author-chat-meta inline-flex items-center gap-1.5 opacity-0 pointer-events-none transition-opacity group-hover:opacity-100 group-hover:pointer-events-auto",
    time: "author-chat-time whitespace-nowrap text-[10px] text-slate-400",
    copyButton:
      "author-chat-copy-btn agent-copy-btn rounded-full border border-blue-400/30 bg-slate-950/40 px-2 py-0.5 text-[10px] font-bold text-blue-300 transition-colors hover:border-blue-300/70 hover:bg-blue-600/20",
    inlineActions: "author-chat-inline-actions flex flex-wrap gap-2",
    actionButton:
      "author-chat-action-btn agent-action-btn rounded-full border border-blue-300/45 bg-blue-900/30 px-2.5 py-1.5 text-[11px] font-bold text-slate-200 transition-colors hover:border-blue-200/80 hover:bg-blue-600/40",
    round: "author-chat-round grid gap-2",
    empty:
      "author-chat-empty rounded-xl border border-dashed border-slate-600/55 px-4 py-4 text-center text-xs leading-6 text-slate-400",
    block: "author-chat-block grid gap-1 border-none bg-transparent p-0",
    blockDetails: "author-chat-block-details grid gap-1.5",
    blockSummary: "author-chat-block-label list-none cursor-pointer text-[11px] font-bold tracking-[0.01em]",
    blockLabel: "author-chat-block-label text-[11px] font-bold tracking-[0.01em]",
    body: "author-chat-body m-0 whitespace-pre-wrap break-words font-mono text-xs leading-6 text-slate-200",
    bodyMarkdown: "author-chat-body author-chat-md text-xs leading-relaxed text-slate-200",
    progressChip:
      "author-progress-chip inline-flex items-center gap-1.5 rounded-full border border-slate-600/60 bg-slate-950/45 px-2 py-0.5 text-[10px] font-bold text-slate-300",
    progressChipRunning: "border-teal-400/50 bg-teal-700/20 text-teal-100",
    progressChipDone: "border-blue-400/50 bg-blue-800/25 text-blue-100",
    progressChipError: "border-red-400/55 bg-red-900/30 text-red-100",
    progressChipPending: "border-amber-400/45 bg-amber-900/25 text-amber-100",
  };

  function chatMessageRoleClass(roleRaw, reverted) {
    if (roleRaw === "user") return CHAT_CLASS.messageUser;
    if (roleRaw === "assistant") {
      return reverted ? CHAT_CLASS.messageAssistantReverted : CHAT_CLASS.messageAssistant;
    }
    return CHAT_CLASS.messageSystem;
  }

  function chatRoleTextClass(roleRaw, reverted) {
    if (roleRaw === "user") return CHAT_CLASS.roleUser;
    if (roleRaw === "assistant") {
      return reverted ? CHAT_CLASS.roleAssistantReverted : CHAT_CLASS.roleAssistant;
    }
    return CHAT_CLASS.roleSystem;
  }

  function chatBlockLabelToneClass(type) {
    const kind = String(type || "text").toLowerCase();
    if (kind === "reasoning") return "text-amber-200";
    if (kind === "tool") return "text-teal-200";
    if (kind === "patch") return "text-orange-200";
    if (kind === "debug") return "text-violet-200";
    if (kind === "diff") return "text-amber-300";
    if (kind === "code") return "text-blue-200";
    return "text-blue-300";
  }

  function progressChipClass(status) {
    const kind = String(status || "pending").toLowerCase();
    if (kind === "running") return CHAT_CLASS.progressChip + " " + CHAT_CLASS.progressChipRunning;
    if (kind === "done") return CHAT_CLASS.progressChip + " " + CHAT_CLASS.progressChipDone;
    if (kind === "error") return CHAT_CLASS.progressChip + " " + CHAT_CLASS.progressChipError;
    return CHAT_CLASS.progressChip + " " + CHAT_CLASS.progressChipPending;
  }

  function escapeHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  async function fetchJson(url, init) {
    const response = await fetch(url, init);
    if (!response.ok) {
      let detail = "";
      try {
        detail = (await response.text()).trim();
      } catch (_) {}
      throw new Error(detail || (url + " -> " + response.status));
    }
    return response.json();
  }

  function currentTarget() {
    const params = new URLSearchParams(window.location.search);
    const fromUrl = params.get("target");
    if (fromUrl && String(fromUrl).trim()) return String(fromUrl).trim();
    // 与编译态「当前 entry」的 .mei 路径对齐；裸 `main.mei` 常与 scene 绑定校验冲突
    const entryTarget = String(root.dataset.entryTarget || "").trim();
    if (entryTarget) return entryTarget;
    return String(root.dataset.target || "").trim();
  }

  function currentManageTab() {
    const params = new URLSearchParams(window.location.search);
    const raw = String(params.get("tab") || root.dataset.viewTab || "preview")
      .trim()
      .toLowerCase();
    if (raw === "source" || raw === "diff" || raw === "diagnostics") return raw;
    return "preview";
  }

  function setManageTab(tab) {
    const next = String(tab || "").trim().toLowerCase();
    if (!next) return currentManageTab();
    if (typeof boot.switchManageTab === "function") {
      return boot.switchManageTab(next);
    }
    const url = new URL(window.location.href);
    url.searchParams.set("tab", next);
    window.location.assign(url.toString());
    return next;
  }

  function normalizeTargetKey(target) {
    return String(target || "")
      .trim()
      .replace(/\\/g, "/")
      .replace(/^\.\/+/, "");
  }

  function currentTargetKey() {
    return normalizeTargetKey(currentTarget());
  }

  function currentAppKey() {
    const fromDataset = String(root.dataset.app || "").trim();
    try {
      const path = window.location.pathname || "";
      const prefixes = ["/apps/manage/", "/apps/access/"];
      for (const prefix of prefixes) {
        if (!path.startsWith(prefix)) continue;
        let rest = path.slice(prefix.length);
        const slashQ = rest.indexOf("/?");
        if (slashQ >= 0) rest = rest.slice(0, slashQ);
        rest = rest.replace(/\/+$/, "");
        if (rest) return rest;
        break;
      }
    } catch (_) {}
    return fromDataset;
  }

  function currentSceneId() {
    return String(root.dataset.scene || "").trim();
  }

  function sessionStorageKey() {
    return "mei-lang.agent.session." + currentAppKey() + "." + currentTargetKey();
  }

  function modeStorageKey() {
    return "mei-lang.agent.mode." + currentAppKey() + "." + currentTargetKey();
  }

  function accessFloatingStorageKey() {
    return "mei-lang.agent.access-floating." + currentAppKey();
  }

  function accessFloatingPositionStorageKey() {
    return "mei-lang.agent.access-floating-position." + currentAppKey();
  }

  function revertedStorageKey() {
    return "mei-lang.agent.reverted." + currentAppKey() + "." + currentTargetKey();
  }

  function deltaDebugStorageKey(sessionId) {
    const sid = String(sessionId || "").trim();
    if (!sid) return "";
    return "mei-lang.agent.delta-debug." + currentAppKey() + "." + sid;
  }

  function normalizeDeltaDebugRows(rows) {
    const src = Array.isArray(rows) ? rows : [];
    return src
      .map(function (item) {
        if (!item || typeof item !== "object") return null;
        const serverTs = Number(item.serverTs || 0);
        const clientRxTs =
          Number(item.clientRxTs || 0) || Number(item.clientTs || 0);
        const gapRxMs =
          item.gapRxMs != null && Number.isFinite(Number(item.gapRxMs))
            ? Number(item.gapRxMs)
            : item.gapMs != null && Number.isFinite(Number(item.gapMs))
              ? Number(item.gapMs)
              : null;
        const paintTs =
          item.paintTs != null && Number.isFinite(Number(item.paintTs))
            ? Number(item.paintTs)
            : null;
        const gapPaintMs =
          item.gapPaintMs != null && Number.isFinite(Number(item.gapPaintMs))
            ? Number(item.gapPaintMs)
            : null;
        return {
          serverTs: Number.isFinite(serverTs) ? serverTs : 0,
          clientRxTs: Number.isFinite(clientRxTs) ? clientRxTs : 0,
          paintTs: paintTs,
          partId: String(item.partId || ""),
          messageId: String(item.messageId || ""),
          chars: Number(item.chars || 0),
          preview: String(item.preview || ""),
          gapRxMs: gapRxMs,
          gapPaintMs: gapPaintMs,
        };
      })
      .filter(Boolean)
      .slice(0, 120);
  }

  function writeDeltaDebugLogToStorage(sessionId, rows) {
    if (!window.sessionStorage) return;
    const key = deltaDebugStorageKey(sessionId);
    if (!key) return;
    try {
      window.sessionStorage.setItem(
        key,
        JSON.stringify({
          updatedAtMs: Date.now(),
          rows: normalizeDeltaDebugRows(rows),
        }),
      );
    } catch (_) {}
  }

  function readDeltaDebugLogFromStorage(sessionId) {
    if (!window.sessionStorage) return [];
    const key = deltaDebugStorageKey(sessionId);
    if (!key) return [];
    try {
      const raw = window.sessionStorage.getItem(key);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      return normalizeDeltaDebugRows(parsed && parsed.rows);
    } catch (_) {
      return [];
    }
  }

  function restoreDeltaDebugLog(sessionId) {
    state.deltaDebugLog = readDeltaDebugLogFromStorage(sessionId);
    renderDeltaDebugLog();
  }

  function normalizeAgentMode(value) {
    const normalizedRoute = normalizeRouteMode(root.dataset.mode);
    const allowed = String(root.dataset.allowedModes || "")
      .split(",")
      .map(function (item) {
        const raw = String(item || "").trim().toLowerCase();
        if (raw === "plan") return "ask";
        if (raw === "ask" || raw === "build") return raw;
        return "";
      })
      .filter(Boolean);
    if (!allowed.length) {
      if (normalizedRoute === "access") {
        allowed.push("ask");
      } else {
        allowed.push("build");
      }
    }
    const defaultFromDataset = String(root.dataset.defaultAgentMode || "").trim().toLowerCase();
    const fallback =
      allowed.indexOf(defaultFromDataset) >= 0
        ? defaultFromDataset
        : allowed[0];
    const raw = String(value || "").trim().toLowerCase();
    const mapped = raw === "plan" ? "ask" : raw === "ask" ? "ask" : "build";
    return allowed.indexOf(mapped) >= 0 ? mapped : fallback;
  }

  function normalizeRouteMode(value) {
    const mode = String(value || "").toLowerCase();
    return mode === "access" ? "access" : "manage";
  }

  function isAccessFloatingMode() {
    return (
      normalizeRouteMode(root.dataset.mode) === "access" &&
      !!els.accessFloatingRoot &&
      !!els.accessFab &&
      !!els.accessPanel
    );
  }

  function clampAccessFloatingPosition(left, top) {
    if (!isAccessFloatingMode()) return null;
    const width = Math.max(48, Number(els.accessFloatingRoot.offsetWidth || 68));
    const height = Math.max(48, Number(els.accessFloatingRoot.offsetHeight || 68));
    const minLeft = ACCESS_FLOATING_MARGIN_PX;
    const minTop = ACCESS_FLOATING_MARGIN_PX;
    const maxLeft = Math.max(
      minLeft,
      Number(window.innerWidth || 0) - width - ACCESS_FLOATING_MARGIN_PX,
    );
    const maxTop = Math.max(
      minTop,
      Number(window.innerHeight || 0) - height - ACCESS_FLOATING_MARGIN_PX,
    );
    const nextLeft = Math.min(maxLeft, Math.max(minLeft, Math.round(Number(left) || 0)));
    const nextTop = Math.min(maxTop, Math.max(minTop, Math.round(Number(top) || 0)));
    return { left: nextLeft, top: nextTop };
  }

  function applyAccessFloatingPosition(left, top) {
    if (!isAccessFloatingMode()) return null;
    const pos = clampAccessFloatingPosition(left, top);
    if (!pos) return null;
    els.accessFloatingRoot.style.left = String(pos.left) + "px";
    els.accessFloatingRoot.style.top = String(pos.top) + "px";
    els.accessFloatingRoot.style.right = "auto";
    els.accessFloatingRoot.style.bottom = "auto";
    els.accessFloatingRoot.dataset.positioned = "true";
    return pos;
  }

  function clearAccessFloatingPosition() {
    if (!isAccessFloatingMode()) return;
    els.accessFloatingRoot.style.left = "";
    els.accessFloatingRoot.style.top = "";
    els.accessFloatingRoot.style.right = "";
    els.accessFloatingRoot.style.bottom = "";
    delete els.accessFloatingRoot.dataset.positioned;
  }

  function rememberAccessFloatingPosition(left, top) {
    if (!isAccessFloatingMode()) return;
    const pos = clampAccessFloatingPosition(left, top);
    if (!pos) return;
    try {
      localStorage.setItem(accessFloatingPositionStorageKey(), JSON.stringify(pos));
    } catch (_) {}
  }

  function restoreAccessFloatingPosition() {
    if (!isAccessFloatingMode()) return;
    try {
      const raw = localStorage.getItem(accessFloatingPositionStorageKey());
      if (!raw) {
        clearAccessFloatingPosition();
        return;
      }
      const parsed = JSON.parse(raw);
      const left = Number(parsed && parsed.left);
      const top = Number(parsed && parsed.top);
      if (!Number.isFinite(left) || !Number.isFinite(top)) {
        clearAccessFloatingPosition();
        return;
      }
      const pos = applyAccessFloatingPosition(left, top);
      if (pos) rememberAccessFloatingPosition(pos.left, pos.top);
    } catch (_) {
      clearAccessFloatingPosition();
    }
  }

  function renderAccessFloatingPanel() {
    if (!isAccessFloatingMode()) return;
    const open = !!state.accessFloatingOpen;
    els.accessFloatingRoot.dataset.open = open ? "true" : "false";
    els.accessPanel.hidden = !open;
    els.accessFab.title = open ? "关闭助手对话框" : "打开助手对话框";
    els.accessFab.setAttribute("aria-label", open ? "关闭助手对话框" : "打开助手对话框");
  }

  function rememberAccessFloatingPanel() {
    if (!isAccessFloatingMode()) return;
    try {
      localStorage.setItem(accessFloatingStorageKey(), state.accessFloatingOpen ? "1" : "0");
    } catch (_) {}
  }

  function restoreAccessFloatingPanel() {
    if (!isAccessFloatingMode()) return;
    restoreAccessFloatingPosition();
    try {
      const saved = localStorage.getItem(accessFloatingStorageKey());
      state.accessFloatingOpen = saved === "1";
    } catch (_) {
      state.accessFloatingOpen = false;
    }
    renderAccessFloatingPanel();
  }

  function toggleAccessFloatingPanel(next) {
    if (!isAccessFloatingMode()) return;
    if (typeof next === "boolean") {
      state.accessFloatingOpen = next;
    } else {
      state.accessFloatingOpen = !state.accessFloatingOpen;
    }
    rememberAccessFloatingPanel();
    renderAccessFloatingPanel();
    if (state.accessFloatingOpen && els.input) {
      window.setTimeout(function () {
        try {
          els.input.focus();
        } catch (_) {}
      }, 0);
    }
  }

  function beginAccessFloatingDrag(event) {
    if (!isAccessFloatingMode()) return;
    if (event && event.button != null && event.button !== 0) return;
    const rect = els.accessFloatingRoot.getBoundingClientRect();
    accessFloatingDragState = {
      pointerId: event ? event.pointerId : null,
      startX: Number(event && event.clientX),
      startY: Number(event && event.clientY),
      baseLeft: Number(rect.left || 0),
      baseTop: Number(rect.top || 0),
      moved: false,
      lastLeft: Number(rect.left || 0),
      lastTop: Number(rect.top || 0),
    };
    state.accessFloatingDragMoved = false;
    els.accessFloatingRoot.dataset.dragging = "true";
    try {
      if (els.accessFab && event && event.pointerId != null) {
        els.accessFab.setPointerCapture(event.pointerId);
      }
    } catch (_) {}
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
  }

  function continueAccessFloatingDrag(event) {
    if (!accessFloatingDragState || !isAccessFloatingMode()) return;
    if (
      accessFloatingDragState.pointerId != null &&
      event &&
      event.pointerId != null &&
      event.pointerId !== accessFloatingDragState.pointerId
    ) {
      return;
    }
    const nextX = Number(event && event.clientX);
    const nextY = Number(event && event.clientY);
    if (!Number.isFinite(nextX) || !Number.isFinite(nextY)) return;
    const dx = nextX - accessFloatingDragState.startX;
    const dy = nextY - accessFloatingDragState.startY;
    if (
      !accessFloatingDragState.moved &&
      Math.hypot(dx, dy) < ACCESS_FLOATING_DRAG_THRESHOLD_PX
    ) {
      return;
    }
    accessFloatingDragState.moved = true;
    state.accessFloatingDragMoved = true;
    const pos = applyAccessFloatingPosition(
      accessFloatingDragState.baseLeft + dx,
      accessFloatingDragState.baseTop + dy,
    );
    if (!pos) return;
    accessFloatingDragState.lastLeft = pos.left;
    accessFloatingDragState.lastTop = pos.top;
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
  }

  function endAccessFloatingDrag(event) {
    if (!accessFloatingDragState) return;
    if (
      accessFloatingDragState.pointerId != null &&
      event &&
      event.pointerId != null &&
      event.pointerId !== accessFloatingDragState.pointerId
    ) {
      return;
    }
    const moved = !!accessFloatingDragState.moved;
    const left = accessFloatingDragState.lastLeft;
    const top = accessFloatingDragState.lastTop;
    accessFloatingDragState = null;
    if (els.accessFloatingRoot) {
      delete els.accessFloatingRoot.dataset.dragging;
    }
    try {
      if (els.accessFab && event && event.pointerId != null) {
        els.accessFab.releasePointerCapture(event.pointerId);
      }
    } catch (_) {}
    if (moved) {
      rememberAccessFloatingPosition(left, top);
      window.setTimeout(function () {
        state.accessFloatingDragMoved = false;
      }, 0);
    }
  }

  function composerDraftText() {
    return els.input && typeof els.input.value === "string" ? String(els.input.value) : "";
  }

  function refreshLinkedViewRefs() {
    els.sourceViewHost = document.getElementById("source-view-host");
    els.sourceViewSourcePanel = document.getElementById("source-view-source-panel");
    els.sourceViewSourceRaw = document.getElementById("source-view-source-raw");
    els.sourceViewDiffPanel = document.getElementById("source-view-diff-panel");
    els.accessFloatingRoot = document.getElementById("access-chat-floating-root");
    els.accessFab = document.getElementById("access-chat-fab");
    els.accessClose = document.getElementById("access-chat-close");
    els.accessPanel = document.getElementById("access-chat-overlay-panel");
    els.statusModelService = document.getElementById("mei-status-model-service");
  }

  function parsePx(value) {
    const n = Number.parseFloat(String(value || "0"));
    return Number.isFinite(n) ? n : 0;
  }

  function resolveComposerLineHeightPx(inputEl, style) {
    const explicit = parsePx(style && style.lineHeight ? style.lineHeight : "");
    if (explicit > 0) return explicit;
    const fontSize = parsePx(style && style.fontSize ? style.fontSize : "");
    return fontSize > 0 ? fontSize * 1.4 : 18;
  }

  function autoResizeComposerInput() {
    if (!els.input) return;
    const inputEl = els.input;
    const style = window.getComputedStyle(inputEl);
    const lineHeight = resolveComposerLineHeightPx(inputEl, style);
    const verticalPadding =
      parsePx(style.paddingTop) +
      parsePx(style.paddingBottom) +
      parsePx(style.borderTopWidth) +
      parsePx(style.borderBottomWidth);
    const minHeight = Math.round(lineHeight * COMPOSER_MIN_ROWS + verticalPadding);
    const maxHeight = Math.round(lineHeight * COMPOSER_MAX_ROWS + verticalPadding);
    inputEl.style.height = "auto";
    const scrollHeight = Math.max(inputEl.scrollHeight, minHeight);
    const nextHeight = Math.min(scrollHeight, maxHeight);
    inputEl.style.height = String(nextHeight) + "px";
    inputEl.style.overflowY = scrollHeight > maxHeight ? "auto" : "hidden";
  }

  function canSubmitPrompt() {
    return composerDraftText().trim().length > 0;
  }

  function normalizeFilePath(value) {
    return String(value || "")
      .trim()
      .replace(/\\/g, "/")
      .replace(/^\.\/+/, "");
  }

  function sourceTargetKey() {
    refreshLinkedViewRefs();
    const targetNode = els.sourceViewSourceRaw || els.sourceViewSourcePanel;
    if (targetNode && targetNode.dataset && targetNode.dataset.sourceTarget) {
      return normalizeFilePath(targetNode.dataset.sourceTarget);
    }
    return currentTargetKey();
  }

  function sourceLanguage() {
    refreshLinkedViewRefs();
    const targetNode = els.sourceViewSourceRaw || els.sourceViewSourcePanel;
    if (targetNode && targetNode.dataset && targetNode.dataset.sourceLang) {
      return String(targetNode.dataset.sourceLang || "").trim().toLowerCase() || "plain";
    }
    return "plain";
  }

  function sourceRawText() {
    refreshLinkedViewRefs();
    return els.sourceViewSourceRaw ? String(els.sourceViewSourceRaw.textContent || "") : "";
  }

  function conversationRounds(messages) {
    const rounds = [];
    let current = null;
    let orphan = 0;
    (Array.isArray(messages) ? messages : []).forEach(function (message) {
      if (!message || typeof message !== "object") return;
      const role = String(message.role || "");
      if (role === "user") {
        current = {
          id: "round-user-" + String(message.id || String(rounds.length)),
          user: message,
          assistants: [],
          system: [],
        };
        rounds.push(current);
        return;
      }
      if (role === "assistant") {
        if (!current) {
          orphan += 1;
          current = {
            id: "round-orphan-" + String(orphan),
            user: null,
            assistants: [],
            system: [],
          };
          rounds.push(current);
        }
        current.assistants.push(message);
        return;
      }
      if (!current) {
        orphan += 1;
        current = {
          id: "round-system-" + String(orphan),
          user: null,
          assistants: [],
          system: [],
        };
        rounds.push(current);
      }
      current.system.push(message);
    });
    return rounds;
  }

  function latestRoundAssistantMessageId() {
    const rounds = conversationRounds(state.messages);
    for (let index = rounds.length - 1; index >= 0; index -= 1) {
      const round = rounds[index];
      const assistants = round && Array.isArray(round.assistants) ? round.assistants : [];
      const assistant = assistants.length ? assistants[assistants.length - 1] : null;
      const messageId = String(assistant && assistant.id ? assistant.id : "").trim();
      if (messageId) return messageId;
    }
    return "";
  }

  function latestDiffEligibleMessageId() {
    const latestAssistantId = latestRoundAssistantMessageId();
    if (!latestAssistantId) return "";
    const meta = getMessageMeta(state.sessionId, latestAssistantId);
    if (!meta || meta.hasDiff !== true) return "";
    return latestAssistantId;
  }

  function messageKey(sessionId, messageId) {
    return String(sessionId || "") + "::" + String(messageId || "");
  }

  /** diff 结果随当前管理页目标路径变化，缓存键需包含 path。 */
  function diffCacheKey(sessionId, messageId) {
    const base = messageKey(sessionId, messageId);
    const p = sourceTargetKey();
    return p ? base + "::diffPath::" + p : base;
  }

  function setMessageMeta(messageId, patch) {
    const key = messageKey(state.sessionId, messageId);
    if (!key || key === "::") return;
    const prev = state.messageMeta[key] || {};
    state.messageMeta[key] = Object.assign({}, prev, patch || {});
  }

  function getMessageMeta(sessionId, messageId) {
    return state.messageMeta[messageKey(sessionId, messageId)] || null;
  }

  function setSessionRevertedFlag(sessionId, hasReverted) {
    const sid = String(sessionId || "").trim();
    if (!sid) return;
    state.sessionHasRevertedChanges[sid] = !!hasReverted;
  }

  function hasSessionRevertedChanges(sessionId) {
    return !!state.sessionHasRevertedChanges[String(sessionId || "").trim()];
  }

  function persistRevertedState() {
    try {
      localStorage.setItem(revertedStorageKey(), JSON.stringify(state.revertedMessageIds));
    } catch (_) {}
  }

  function restoreRevertedState() {
    state.revertedMessageIds = {};
    state.sessionHasRevertedChanges = {};
    try {
      const raw = localStorage.getItem(revertedStorageKey());
      const parsed = raw ? JSON.parse(raw) : {};
      if (!parsed || typeof parsed !== "object") return;
      state.revertedMessageIds = parsed;
      Object.keys(parsed).forEach(function (sid) {
        setSessionRevertedFlag(sid, Array.isArray(parsed[sid]) && parsed[sid].length > 0);
      });
    } catch (_) {}
  }

  function revertedIdsForSession(sessionId) {
    const sid = String(sessionId || "").trim();
    const list = sid ? state.revertedMessageIds[sid] : null;
    return Array.isArray(list) ? list.slice() : [];
  }

  function setRevertedIdsForSession(sessionId, nextIds) {
    const sid = String(sessionId || "").trim();
    if (!sid) return;
    const deduped = Array.from(
      new Set(
        (Array.isArray(nextIds) ? nextIds : [])
          .map(function (item) { return String(item || "").trim(); })
          .filter(Boolean),
      ),
    );
    state.revertedMessageIds[sid] = deduped;
    setSessionRevertedFlag(sid, deduped.length > 0);
    persistRevertedState();
  }

  function isMessageReverted(sessionId, messageId) {
    return revertedIdsForSession(sessionId).includes(String(messageId || "").trim());
  }

  function latestUndoMessageId() {
    if (!state.sessionId) return "";
    const rounds = conversationRounds(state.messages);
    for (let index = rounds.length - 1; index >= 0; index -= 1) {
      const message = rounds[index] && rounds[index].assistant;
      const messageId = String(message && message.id ? message.id : "").trim();
      if (!messageId) continue;
      const meta = getMessageMeta(state.sessionId, messageId);
      if (!meta || meta.hasDiff !== true) continue;
      if (isMessageReverted(state.sessionId, messageId)) continue;
      return messageId;
    }
    return "";
  }

  function canUndo() {
    return !!latestUndoMessageId();
  }

  function canRedo() {
    return hasSessionRevertedChanges(state.sessionId);
  }

  function progressStatusClass(status) {
    const value = String(status || "").trim().toLowerCase();
    if (value === "completed" || value === "done" || value === "finished") return "done";
    if (value === "error" || value === "failed") return "error";
    if (value === "running") return "running";
    return "pending";
  }

  function progressLabelForTool(tool) {
    const title = String(tool && tool.title ? tool.title : "").trim();
    const name = String(tool && tool.tool ? tool.tool : "").trim();
    return title || (name ? "工具：" + name : "工具步骤");
  }

  function activeAssistantRawMessage(rawMessages) {
    const rows = Array.isArray(rawMessages) ? rawMessages : [];
    const activeId = String(state.activeGenerationMessageId || "").trim();
    if (activeId) {
      const match = rows.find(function (row) {
        return (
          row &&
          String(row.role || "") === "assistant" &&
          String(row.message_id || "").trim() === activeId
        );
      });
      if (match) return match;
    }
    for (let index = rows.length - 1; index >= 0; index -= 1) {
      const row = rows[index];
      if (row && String(row.role || "") === "assistant") {
        return row;
      }
    }
    return null;
  }

  function deriveProgressFromMessages(rawMessages) {
    const active = activeAssistantRawMessage(rawMessages);
    if (!state.sending || !active) {
      return {
        visible: false,
        label: "",
        detail: "",
        items: [],
      };
    }
    const messageId = String(active.message_id || "").trim();
    const meta = getMessageMeta(state.sessionId, messageId) || {};
    const agent = normalizeAgentMode(meta.agent || state.agentMode);
    const parts = Array.isArray(active.parts) ? active.parts : [];
    const stepStarts = parts.filter(function (part) {
      return String(part && part.part_type || "") === "step-start";
    }).length;
    const stepFinishes = parts.filter(function (part) {
      return String(part && part.part_type || "") === "step-finish";
    }).length;
    const tools = parts
      .filter(function (part) {
        return String(part && part.part_type || "") === "tool" && part.tool;
      })
      .map(function (part) {
        return part.tool;
      });
    const runningTools = tools.filter(function (tool) {
      return String(tool && tool.status || "").trim().toLowerCase() === "running";
    });
    const pendingTools = tools.filter(function (tool) {
      return String(tool && tool.status || "").trim().toLowerCase() === "pending";
    });
    const doneTools = tools.filter(function (tool) {
      return String(tool && tool.status || "").trim().toLowerCase() === "completed";
    });
    const errorTools = tools.filter(function (tool) {
      return String(tool && tool.status || "").trim().toLowerCase() === "error";
    });

    let label = agent === "ask" ? "问答处理中" : "脚本生成中";
    if (runningTools.length > 0) {
      label = (agent === "ask" ? "问答处理中" : "脚本生成中") + " · 工具运行中";
    } else if (stepStarts > stepFinishes) {
      label = (agent === "ask" ? "问答处理中" : "脚本生成中") + " · 步骤处理中";
    } else if (parts.length > 0) {
      label = agent === "ask" ? "正在生成回答" : "正在生成结果";
    }

    const totalSteps = Math.max(stepStarts, stepFinishes);
    const detailParts = [];
    if (totalSteps > 0) {
      detailParts.push("步骤 " + String(stepFinishes) + "/" + String(totalSteps));
    }
    if (runningTools.length > 0) {
      detailParts.push("运行中工具 " + String(runningTools.length));
    } else if (pendingTools.length > 0) {
      detailParts.push("待处理工具 " + String(pendingTools.length));
    } else if (doneTools.length > 0) {
      detailParts.push("已完成工具 " + String(doneTools.length));
    }

    const items = [];
    tools.slice(-4).forEach(function (tool) {
      items.push({
        label: progressLabelForTool(tool),
        status: progressStatusClass(tool && tool.status),
      });
    });
    if (!items.length && totalSteps > 0) {
      for (let index = 0; index < totalSteps; index += 1) {
        items.push({
          label: "步骤 " + String(index + 1),
          status: index < stepFinishes ? "done" : (index < stepStarts ? "running" : "pending"),
        });
      }
    }
    if (!items.length) {
      items.push({
        label: agent === "ask" ? "等待回答输出" : "等待执行输出",
        status: "running",
      });
    }

    return {
      visible: true,
      label: label,
      detail: detailParts.join(" · "),
      items: items,
    };
  }

  function destroySourceEditor() {
    if (state.sourceViewResizeObserver) {
      try {
        state.sourceViewResizeObserver.disconnect();
      } catch (_) {}
      state.sourceViewResizeObserver = null;
    }
    state.sourceCodeMirror = null;
    state.sourceEditorContainer = null;
    if (els.sourceViewSourcePanel) {
      els.sourceViewSourcePanel.innerHTML = "";
    }
  }

  function destroySourceDiffView() {
    refreshLinkedViewRefs();
    if (state.sourceDiffResizeObserver) {
      try {
        state.sourceDiffResizeObserver.disconnect();
      } catch (_) {}
      state.sourceDiffResizeObserver = null;
    }
    state.sourceDiffMergeView = null;
    if (els.sourceViewDiffPanel) {
      els.sourceViewDiffPanel.innerHTML = "";
    }
  }

  function refreshSourceEditors() {
    const views = [
      state.sourceCodeMirror,
      state.sourceDiffMergeView && typeof state.sourceDiffMergeView.editor === "function"
        ? state.sourceDiffMergeView.editor()
        : null,
      state.sourceDiffMergeView && typeof state.sourceDiffMergeView.leftOriginal === "function"
        ? state.sourceDiffMergeView.leftOriginal()
        : null,
      state.sourceDiffMergeView && typeof state.sourceDiffMergeView.rightOriginal === "function"
        ? state.sourceDiffMergeView.rightOriginal()
        : null,
    ].filter(Boolean);
    views.forEach(function (view) {
      if (view && typeof view.refresh === "function") {
        view.refresh();
      }
    });
  }

  function refreshSourceDiffView() {
    refreshSourceEditors();
  }

  function scheduleSourceDiffRefresh() {
    if (!state.sourceDiffMergeView || typeof window.requestAnimationFrame !== "function") {
      refreshSourceDiffView();
      return;
    }
    window.requestAnimationFrame(function () {
      refreshSourceDiffView();
      window.requestAnimationFrame(function () {
        refreshSourceDiffView();
      });
    });
  }

  function bindSourceDiffResizeRefresh() {
    refreshLinkedViewRefs();
    if (!els.sourceViewDiffPanel || typeof ResizeObserver !== "function") {
      return;
    }
    if (state.sourceDiffResizeObserver) {
      try {
        state.sourceDiffResizeObserver.disconnect();
      } catch (_) {}
    }
    state.sourceDiffResizeObserver = new ResizeObserver(function () {
      scheduleSourceDiffRefresh();
    });
    state.sourceDiffResizeObserver.observe(els.sourceViewDiffPanel);
  }

  function bindSourceViewResizeRefresh() {
    refreshLinkedViewRefs();
    if (!els.sourceViewHost || typeof ResizeObserver !== "function") {
      return;
    }
    if (state.sourceViewResizeObserver) {
      try {
        state.sourceViewResizeObserver.disconnect();
      } catch (_) {}
    }
    state.sourceViewResizeObserver = new ResizeObserver(function () {
      scheduleSourceDiffRefresh();
    });
    state.sourceViewResizeObserver.observe(els.sourceViewHost);
    if (els.sourceViewSourcePanel) {
      state.sourceViewResizeObserver.observe(els.sourceViewSourcePanel);
    }
  }

  function ensureSourceEditor() {
    refreshLinkedViewRefs();
    if (!els.sourceViewSourcePanel || !window.CodeMirror) {
      return;
    }
    if (
      state.sourceCodeMirror &&
      state.sourceEditorContainer === els.sourceViewSourcePanel
    ) {
      refreshSourceEditors();
      return;
    }
    initSourceEditor();
  }

  function codeMirrorModeOption() {
    const lang = sourceLanguage();
    const target = sourceTargetKey();
    const ext = (target.split(".").pop() || "").toLowerCase();
    if (lang === "mei" || ext === "mei" || ext === "star") return "mei";
    if (lang === "json" || ext === "json" || ext === "jsonc") {
      return { name: "javascript", json: true };
    }
    if (lang === "typescript" || ext === "ts" || ext === "tsx") {
      return { name: "javascript", typescript: true };
    }
    if (lang === "javascript" || ext === "js" || ext === "jsx" || ext === "mjs" || ext === "cjs") {
      return "javascript";
    }
    if (lang === "css" || ext === "css" || ext === "scss" || ext === "less") return "css";
    if (lang === "python" || ext === "py" || ext === "pyi") return "python";
    if (lang === "xml" || ext === "xml" || ext === "svg") {
      return { name: "xml", htmlMode: false };
    }
    if (lang === "html" || ext === "html" || ext === "htm") {
      return { name: "xml", htmlMode: true };
    }
    return null;
  }

  function initSourceEditor() {
    refreshLinkedViewRefs();
    if (!els.sourceViewSourcePanel || !window.CodeMirror) {
      return;
    }
    destroySourceEditor();
    state.sourceCodeMirror = window.CodeMirror(els.sourceViewSourcePanel, {
      value: sourceRawText(),
      lineNumbers: true,
      readOnly: true,
      mode: codeMirrorModeOption(),
      theme: "default",
      lineWrapping: false,
      scrollbarStyle: "native",
    });
    state.sourceEditorContainer = els.sourceViewSourcePanel;
    bindSourceViewResizeRefresh();
    scheduleSourceDiffRefresh();
  }

  function renderSourceViewMode(mode) {
    refreshLinkedViewRefs();
    const nextMode = mode === "diff" ? "diff" : "source";
    state.sourceViewMode = nextMode;
    if (els.sourceViewSourcePanel) {
      els.sourceViewSourcePanel.hidden = nextMode !== "source";
    }
    if (els.sourceViewDiffPanel) {
      els.sourceViewDiffPanel.hidden = nextMode !== "diff";
    }
    if (els.sourceViewDiffBtn) {
      const active = nextMode === "diff";
      els.sourceViewDiffBtn.classList.toggle("is-active", active);
      els.sourceViewDiffBtn.setAttribute("aria-pressed", active ? "true" : "false");
    }
    if (nextMode === "source") {
      ensureSourceEditor();
    }
    scheduleSourceDiffRefresh();
  }

  function pickDiffFileForTarget(diff) {
    const files = Array.isArray(diff && diff.files) ? diff.files : [];
    if (!files.length) return null;
    const target = sourceTargetKey();
    const exact = files.find(function (file) {
      return normalizeFilePath(file && file.file) === target;
    });
    if (exact) return exact;
    const targetName = target.split("/").pop() || target;
    const fuzzy = files.find(function (file) {
      const filePath = normalizeFilePath(file && file.file);
      return filePath === targetName || filePath.endsWith("/" + targetName);
    });
    return fuzzy || files[0];
  }

  function renderSourceDiff(fileDiff, messageId) {
    refreshLinkedViewRefs();
    if (!els.sourceViewDiffPanel) return false;
    if (!window.CodeMirror || typeof window.CodeMirror.MergeView !== "function") {
      setInlineNote("差异视图不可用：CodeMirror 未加载。");
      return false;
    }
    if (typeof window.diff_match_patch !== "function") {
      setInlineNote("差异视图不可用：diff 引擎未加载。");
      return false;
    }
    const beforeText = String(fileDiff && fileDiff.before ? fileDiff.before : "");
    const afterText = String(fileDiff && fileDiff.after ? fileDiff.after : "");
    destroySourceDiffView();
    renderSourceViewMode("diff");
    state.sourceDiffMergeView = window.CodeMirror.MergeView(els.sourceViewDiffPanel, {
      value: afterText,
      orig: beforeText,
      lineNumbers: true,
      readOnly: true,
      mode: "mei",
      theme: "default",
      highlightDifferences: true,
      connect: "align",
      collapseIdentical: false,
      revertButtons: false,
    });
    state.sourceDiffMessageId = String(messageId || "");
    bindSourceDiffResizeRefresh();
    scheduleSourceDiffRefresh();
    return true;
  }

  function leaveDiffView() {
    refreshLinkedViewRefs();
    state.sourceDiffMessageId = "";
    destroySourceDiffView();
    const keepDiffMode = currentManageTab() === "diff";
    renderSourceViewMode(keepDiffMode ? "diff" : "source");
    if (keepDiffMode && els.sourceViewDiffPanel) {
      els.sourceViewDiffPanel.innerHTML =
        '<div class="grid place-content-center gap-2 rounded-xl border border-dashed border-slate-600/55 bg-slate-950/35 p-6 text-center text-xs text-slate-400">暂无可显示差异</div>';
    }
  }

  function applyManageTabMode(tab) {
    renderDeltaDebugLog();
    refreshLinkedViewRefs();
    const next = String(tab || "").trim().toLowerCase();
    if (next === "source") {
      ensureSourceEditor();
      leaveDiffView();
      return;
    }
    if (next !== "diff") return;
    renderSourceViewMode("diff");
    if (!state.latestDiffMessageId) {
      if (els.sourceViewDiffPanel) {
        els.sourceViewDiffPanel.innerHTML =
          '<div class="grid place-content-center gap-2 rounded-xl border border-dashed border-slate-600/55 bg-slate-950/35 p-6 text-center text-xs text-slate-400">暂无可查看差异</div>';
      }
      return;
    }
    inspectDiffForMessage(state.latestDiffMessageId).catch(function (error) {
      setInlineNote("读取差异失败：" + String(error.message || error));
    });
  }

  async function inspectDiffForMessage(messageId) {
    const sid = String(state.sessionId || "").trim();
    const mid = String(messageId || "").trim();
    if (!sid || !mid) return false;
    if (mid !== String(state.latestDiffMessageId || "")) {
      setInlineNote("仅支持查看最后一轮 Build 的差异。");
      return false;
    }
    const cacheKey = diffCacheKey(sid, mid);
    const diff = state.messageDiffCache[cacheKey] || (await fetchSessionDiff(mid));
    state.messageDiffCache[cacheKey] = diff;
    const hasFiles = sessionDiffHasMaterialChanges(diff);
    setMessageMeta(mid, { hasDiff: hasFiles });
    if (!hasFiles) {
      setInlineNote("暂无可显示的文件差异。");
      leaveDiffView();
      setDiffTabBadge(0, 0);
      return false;
    }
    const fileDiff = pickDiffFileForTarget(diff);
    if (!fileDiff) {
      setInlineNote("当前目标文件没有可显示差异。");
      leaveDiffView();
      setDiffTabBadge(0, 0);
      return false;
    }
    const st = diffLineStatsFromSummary(diff);
    setDiffTabBadge(st.additions, st.deletions);
    return renderSourceDiff(fileDiff, mid);
  }

  function ensureManageDiffTabBadge() {
    const tab = document.getElementById("manage-tab-diff");
    if (!tab) return null;
    let badge = document.getElementById("manage-tab-diff-badge");
    if (!badge) {
      badge = document.createElement("span");
      badge.id = "manage-tab-diff-badge";
      badge.className = "manage-view-tab-badge";
      badge.hidden = true;
      tab.appendChild(badge);
    }
    return badge;
  }

  function setDiffTabBadge(additions, deletions) {
    const a = Math.max(0, Number(additions) || 0);
    const d = Math.max(0, Number(deletions) || 0);
    const total = a + d;
    const badge = ensureManageDiffTabBadge();
    if (!badge) return;
    if (!total) {
      badge.textContent = "";
      badge.hidden = true;
      badge.removeAttribute("title");
      return;
    }
    badge.textContent = String(total);
    badge.hidden = false;
    badge.title = "相对上一轮 Build：新增 +" + String(a) + " 行，删除 -" + String(d) + " 行";
  }

  /** 与 GET /diff 返回结构一致：优先用 additions/deletions，否则从 patch 文本粗算 +/- 行。 */
  function diffLineStatsFromSummary(diff) {
    if (!diff || typeof diff !== "object") return { additions: 0, deletions: 0 };
    let a = Number(diff.additions);
    let d = Number(diff.deletions);
    if (Number.isFinite(a) && Number.isFinite(d) && (a > 0 || d > 0)) {
      return { additions: Math.max(0, a), deletions: Math.max(0, d) };
    }
    let hitA = 0;
    let hitD = 0;
    const files = Array.isArray(diff.files) ? diff.files : [];
    files.forEach(function (f) {
      if (!f || typeof f !== "object") return;
      const fa = Number(f.additions);
      const fd = Number(f.deletions);
      if (Number.isFinite(fa) && Number.isFinite(fd) && (fa > 0 || fd > 0)) {
        hitA += Math.max(0, fa);
        hitD += Math.max(0, fd);
        return;
      }
      const after = String(f.after || "");
      after.split("\n").forEach(function (line) {
        const t = String(line || "");
        if (t.startsWith("+") && !t.startsWith("+++")) hitA += 1;
        else if (t.startsWith("-") && !t.startsWith("---")) hitD += 1;
      });
    });
    return { additions: hitA, deletions: hitD };
  }

  async function refreshDiffTabBadge() {
    if (!state.sessionId || !state.health || !state.health.healthy || historyUnavailableReason()) {
      setDiffTabBadge(0, 0);
      return;
    }
    const mid = String(state.latestDiffMessageId || "").trim();
    if (!mid) {
      setDiffTabBadge(0, 0);
      return;
    }
    try {
      const cacheKey = diffCacheKey(state.sessionId, mid);
      const diff =
        state.messageDiffCache[cacheKey] || (await fetchSessionDiff(mid));
      if (diff && typeof diff === "object") {
        state.messageDiffCache[cacheKey] = diff;
      }
      if (!sessionDiffHasMaterialChanges(diff)) {
        setDiffTabBadge(0, 0);
        return;
      }
      const stats = diffLineStatsFromSummary(diff);
      setDiffTabBadge(stats.additions, stats.deletions);
    } catch (_) {
      setDiffTabBadge(0, 0);
    }
  }

  function syncSourceDiffEntry() {
    state.latestRoundAssistantId = latestRoundAssistantMessageId();
    state.latestDiffMessageId = latestDiffEligibleMessageId();
    if (els.sourceViewDiffBtn) {
      const enabled = !!state.latestDiffMessageId && !historyUnavailableReason();
      els.sourceViewDiffBtn.disabled = !enabled;
      els.sourceViewDiffBtn.title = enabled
        ? "查看最后一轮 Build 差异（行数见管理页「修改」角标）"
        : (historyUnavailableReason() || "暂无可查看差异");
    }
    const diffTab = document.getElementById("manage-tab-diff");
    if (diffTab) {
      const enabled = !!state.latestDiffMessageId && !historyUnavailableReason();
      diffTab.hidden = !enabled;
    }
    if (
      state.sourceViewMode === "diff" &&
      state.sourceDiffMessageId &&
      state.sourceDiffMessageId !== state.latestDiffMessageId
    ) {
      leaveDiffView();
    } else if (!state.latestDiffMessageId && state.sourceViewMode === "diff") {
      leaveDiffView();
    }
    void refreshDiffTabBadge();
  }

  function scheduleHostReload(reason) {
    const text = String(reason || "").trim();
    if (text) setInlineNote(text);
    state.pendingReloadMessageId = "";
    window.setTimeout(function () {
      window.location.reload();
    }, 120);
  }

  function continueEditing() {
    if (!els.input) return;
    els.input.focus();
  }

  function isNotFoundError(error) {
    const text = String((error && error.message) || error || "");
    return text.includes("404") || text.includes("Not Found");
  }

  function historyUnavailableReason() {
    if (!state.health || state.health.history_available !== false) return "";
    return String(state.health.history_reason || "").trim();
  }

  function renderInlineNote() {
    if (!els.config) return;
    const text = String(state.inlineNote || "").trim() || historyUnavailableReason();
    els.config.hidden = !text;
    els.config.textContent = text;
  }

  function renderProgressStrip() {
    if (!els.progressStrip || !els.progressLabel || !els.progressDetail || !els.progressItems) {
      return;
    }
    const progress = state.progress || {};
    const visible = !!progress.visible;
    els.progressStrip.hidden = !visible;
    if (!visible) {
      els.progressLabel.textContent = "";
      els.progressDetail.textContent = "";
      els.progressItems.innerHTML = "";
      return;
    }
    els.progressLabel.textContent = String(progress.label || "").trim();
    els.progressDetail.textContent = String(progress.detail || "").trim();
    els.progressItems.innerHTML = (Array.isArray(progress.items) ? progress.items : [])
      .map(function (item) {
        const label = escapeHtml(String(item && item.label ? item.label : "").trim());
        const status = String(item && item.status ? item.status : "pending").trim();
        return '<span class="' + progressChipClass(status) + '">' + label + "</span>";
      })
      .join("");
  }

  function renderHistoryButtons() {
    const unavailableReason = historyUnavailableReason();
    const undoEnabled =
      !unavailableReason && !state.loading && !state.sending && !state.aborting && canUndo();
    const redoEnabled =
      !unavailableReason && !state.loading && !state.sending && !state.aborting && canRedo();
    if (els.undo) {
      els.undo.disabled = !undoEnabled;
      els.undo.classList.toggle("is-active", undoEnabled);
      els.undo.title = unavailableReason || "撤回本轮代码修改";
    }
    if (els.redo) {
      els.redo.disabled = !redoEnabled;
      els.redo.classList.toggle("is-active", redoEnabled);
      els.redo.title = unavailableReason || "恢复最近撤回的代码修改";
    }
  }

  function buildBoundSessionTitle(targetKey) {
    const params = new URLSearchParams();
    params.set("app", String(root.dataset.app || ""));
    params.set("target", String(targetKey || ""));
    if (root.dataset.scene) {
      params.set("scene", String(root.dataset.scene || ""));
    }
    if (root.dataset.entry) {
      params.set("entry", String(root.dataset.entry || ""));
    }
    return "MEI|" + params.toString();
  }

  function parseBoundSessionTitle(title) {
    const value = String(title || "");
    if (!value.startsWith("MEI|")) return null;
    try {
      const params = new URLSearchParams(value.slice(4));
      const app = String(params.get("app") || "").trim();
      const target = normalizeTargetKey(params.get("target") || "");
      const scene = String(params.get("scene") || "").trim();
      const entry = String(params.get("entry") || "").trim();
      if (!app || !target) return null;
      return { app: app, target: target, scene: scene, entry: entry };
    } catch (_) {
      return null;
    }
  }

  function renderRunButton(disabled) {
    if (!els.run) return;
    const isSending = state.sending;
    const isStopping = state.aborting;
    const canSubmit = canSubmitPrompt();
    const isPassive = !isSending && !canSubmit;
    els.run.disabled = isSending ? isStopping : (disabled || !canSubmit);
    els.run.textContent = isSending ? "■" : "➤";
    els.run.title = isSending
      ? (isStopping ? "停止中" : "停止发送")
      : canSubmit
        ? "发送"
        : "输入内容后可发送";
    els.run.setAttribute(
      "aria-label",
      isSending ? (isStopping ? "停止中" : "停止发送") : canSubmit ? "发送" : "等待输入",
    );
    els.run.classList.toggle("author-btn-danger", isSending);
    els.run.classList.toggle("author-btn-primary", !isSending && canSubmit);
    els.run.classList.toggle("author-btn-passive", isPassive);
  }

  function setInlineNote(message) {
    state.inlineNote = String(message || "").trim();
    renderInlineNote();
  }

  function setButtonState(disabled) {
    const controlsDisabled = disabled || state.sending || state.aborting;
    if (els.reconnect) els.reconnect.disabled = controlsDisabled;
    if (els.newSession) els.newSession.disabled = controlsDisabled;
    if (els.sessionSelect) els.sessionSelect.disabled = controlsDisabled;
    if (els.modeAsk) els.modeAsk.disabled = controlsDisabled;
    if (els.modeBuild) els.modeBuild.disabled = controlsDisabled;
    if (els.completionModelSelect) {
      els.completionModelSelect.disabled =
        controlsDisabled || els.completionModelSelect.hidden || !els.completionModelSelect.options.length;
    }
    if (els.contextRefresh) els.contextRefresh.disabled = controlsDisabled;
    renderRunButton(disabled);
    renderHistoryButtons();
  }

  function clearGenerationSettleTimer() {
    if (state.generationSettleTimer) {
      window.clearTimeout(state.generationSettleTimer);
    }
    state.generationSettleTimer = null;
  }

  function mergeDraftBackIntoInput() {
    const draft = String(state.pendingPromptDraft || "");
    if (!draft || !els.input) return;
    const current = String(els.input.value || "");
    els.input.value = current.trim() ? draft + "\n\n" + current : draft;
    autoResizeComposerInput();
    const cursor = draft.length;
    try {
      els.input.focus();
      els.input.setSelectionRange(cursor, cursor);
    } catch (_) {}
    state.pendingPromptDraft = "";
  }

  function finishSending(options) {
    const opts = options || {};
    clearGenerationSettleTimer();
    state.sending = false;
    state.aborting = false;
    state.sendAbortController = null;
    state.activeGenerationMessageId = "";
    if (opts.restoreDraft) {
      mergeDraftBackIntoInput();
    } else {
      state.pendingPromptDraft = "";
    }
    state.progress = {
      visible: false,
      label: "",
      detail: "",
      items: [],
    };
    setButtonState(false);
    renderProgressStrip();
  }

  function markGenerationActivity() {
    if (!state.sending) return;
    clearGenerationSettleTimer();
  }

  function clearDeltaDebugLog(options) {
    const opts = options || {};
    state.deltaDebugLog = [];
    if (opts.dropPersisted === true) {
      writeDeltaDebugLogToStorage(String(state.sessionId || ""), []);
    }
    renderDeltaDebugLog();
  }

  function activeGenerationFinished(rawMessages) {
    if (!state.sending) return false;
    const activeId = String(state.activeGenerationMessageId || "").trim();
    if (!activeId) return false;
    const message = (Array.isArray(rawMessages) ? rawMessages : []).find(function (item) {
      return String(item && item.message_id ? item.message_id : "") === activeId;
    });
    if (!message || String(message.role || "") !== "assistant") return false;
    return String(message.finish || "").trim().length > 0;
  }

  function renderStatus() {
    const runtime = state.runtime;
    const health = state.health;
    let label = "未配置";
    let dotClass = "author-server-dot author-server-dot-off";
    if (state.loading) {
      label = "刷新中";
    } else if (health && health.healthy && state.streamConnected) {
      label = "会话中";
      dotClass = "author-server-dot author-server-dot-on";
    } else if (health && health.healthy) {
      label = "已连接";
      dotClass = "author-server-dot author-server-dot-on";
    } else if (
      runtime &&
      String(runtime.connection_source || "").toLowerCase() === "native" &&
      runtime.running
    ) {
      label = health && health.healthy ? "已连接" : "内置助手未就绪";
      dotClass =
        health && health.healthy ? "author-server-dot author-server-dot-on" : "author-server-dot author-server-dot-off";
    } else if (runtime && runtime.connection_source === "managed" && runtime.running) {
      label = "启动中";
    } else if (runtime && runtime.connection_source === "external" && runtime.running) {
      label = "未连接";
    }
    if (els.serverStatus) {
      els.serverStatus.textContent = label;
    }
    if (els.serverDot) {
      els.serverDot.className = dotClass;
    }
    if (els.reconnect) {
      const shouldShowReconnect =
        !state.loading &&
        !!(runtime && runtime.running) &&
        !(health && health.healthy);
      els.reconnect.hidden = !shouldShowReconnect;
    }
  }

  function completionModelStorageKey() {
    try {
      var app = String(root.dataset.app || "default");
      return "mei.author.completionModel.v1." + app;
    } catch (_) {
      return "mei.author.completionModel.v1";
    }
  }

  function encodeCompletionOptionValue(providerId, modelId) {
    return String(providerId || "") + "\x1f" + String(modelId || "");
  }

  function decodeCompletionOptionValue(value) {
    var v = String(value || "");
    var i = v.indexOf("\x1f");
    if (i < 0) return null;
    var p = v.slice(0, i).trim();
    var m = v.slice(i + 1).trim();
    if (!p || !m) return null;
    return { provider_id: p, model_id: m };
  }

  function completionChoiceDisplayName(row) {
    if (!row) return "";
    var mid = String(row.model_id || "").trim();
    var lab = String(row.label || "").trim();
    if (lab && (lab.indexOf("·") >= 0 || lab.indexOf("\u00b7") >= 0)) {
      var sep = lab.indexOf("·") >= 0 ? "·" : "\u00b7";
      var parts = lab.split(sep);
      var last = String(parts[parts.length - 1] || "").trim();
      if (last) return last;
    }
    if (lab) return lab;
    return mid;
  }

  function setCompletionModelWrapVisible(show) {
    var wrap = els.completionModelWrap;
    if (!wrap) return;
    if (show) wrap.classList.remove("hidden");
    else wrap.classList.add("hidden");
  }

  var _markdownOptionsApplied = false;
  function configureMarkdownOnce() {
    if (_markdownOptionsApplied) return;
    _markdownOptionsApplied = true;
    try {
      var mk = typeof marked !== "undefined" && marked && typeof marked.use === "function" ? marked : null;
      if (mk) {
        mk.use({
          async: false,
          breaks: true,
          gfm: true,
          renderer: {
            html: function (token) {
              var raw =
                token && token.raw != null
                  ? String(token.raw)
                  : token && token.text != null
                    ? String(token.text)
                    : "";
              return '<span class="author-chat-md-literal">' + escapeHtml(raw) + "</span>";
            },
          },
        });
      } else if (typeof marked !== "undefined" && marked && typeof marked.setOptions === "function") {
        marked.setOptions({ async: false, breaks: true, gfm: true });
      }
    } catch (_) {}
  }

  function renderMarkdownToSafeHtml(src) {
    var raw = String(src || "");
    if (!raw.trim()) return "";
    configureMarkdownOnce();
    try {
      var mk = typeof marked !== "undefined" && marked && typeof marked.parse === "function" ? marked : null;
      var pur =
        typeof DOMPurify !== "undefined" && DOMPurify && typeof DOMPurify.sanitize === "function"
          ? DOMPurify
          : null;
      if (mk && pur) {
        var html = mk.parse(raw);
        return pur.sanitize(html, {
          ALLOWED_URI_REGEXP:
            /^(?:(?:https?|mailto):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i,
        });
      }
    } catch (_) {}
    return "<pre class=\"" + CHAT_CLASS.body + "\">" + escapeHtml(raw) + "</pre>";
  }

  function sizeCompletionModelSelectWidth() {
    var sel = els.completionModelSelect;
    var wrap = els.completionModelWrap;
    if (!sel || sel.hidden || !wrap || wrap.classList.contains("hidden") || !sel.options.length) {
      if (sel) sel.style.width = "";
      return;
    }
    var opt = sel.options[sel.selectedIndex];
    if (!opt) return;
    var text = String(opt.textContent || "");
    if (!state._completionModelMeasure) {
      var span = document.createElement("span");
      span.id = "author-completion-model-measure";
      span.setAttribute("aria-hidden", "true");
      span.style.cssText = "position:absolute;left:-9999px;top:0;white-space:nowrap;visibility:hidden;pointer-events:none;";
      document.body.appendChild(span);
      state._completionModelMeasure = span;
    }
    var measure = state._completionModelMeasure;
    var cs = window.getComputedStyle(sel);
    measure.style.font = cs.font;
    measure.style.fontSize = cs.fontSize;
    measure.style.fontFamily = cs.fontFamily;
    measure.style.fontWeight = cs.fontWeight;
    measure.style.letterSpacing = cs.letterSpacing;
    measure.textContent = text || "模型";
    var tw = measure.getBoundingClientRect().width;
    var pad = 22;
    var maxPx = 280;
    sel.style.width = Math.min(Math.max(40, tw + pad), maxPx) + "px";
  }

  function normalizedCompletionChoices(config) {
    if (!config || typeof config !== "object") return [];
    var raw = config.completion_model_choices;
    if (Array.isArray(raw) && raw.length) {
      return raw.map(function (row) {
        return {
          provider_id: String((row && row.provider_id) || "").trim(),
          model_id: String((row && row.model_id) || "").trim(),
          label: String((row && row.label) || "").trim(),
        };
      }).filter(function (row) {
        return row.provider_id && row.model_id;
      });
    }
    var pid = String(config.provider_id || "qwen").trim();
    var mid = String(config.completion_model || "").trim();
    if (!mid) return [];
    return [
      {
        provider_id: pid,
        model_id: mid,
        label: mid,
      },
    ];
  }

  function rememberSelectedCompletionModel(value) {
    try {
      localStorage.setItem(completionModelStorageKey(), String(value || ""));
    } catch (_) {}
  }

  function syncCompletionModelSelectFromConfig() {
    var sel = els.completionModelSelect;
    if (!sel) return;
    var config = state.config;
    var choices = normalizedCompletionChoices(config);
    var prevValue = String(sel.value || "");
    sel.innerHTML = "";
    for (var i = 0; i < choices.length; i++) {
      var row = choices[i];
      var op = document.createElement("option");
      op.value = encodeCompletionOptionValue(row.provider_id, row.model_id);
      op.textContent = completionChoiceDisplayName(row);
      sel.appendChild(op);
    }
    var saved = "";
    try {
      saved = String(localStorage.getItem(completionModelStorageKey()) || "").trim();
    } catch (_) {}
    var pick = saved || prevValue || "";
    var found = false;
    if (pick) {
      for (var j = 0; j < sel.options.length; j++) {
        if (sel.options[j].value === pick) {
          sel.selectedIndex = j;
          found = true;
          break;
        }
      }
    }
    if (!found && sel.options.length) {
      sel.selectedIndex = 0;
      rememberSelectedCompletionModel(sel.value);
    }
    var show = choices.length > 0;
    sel.hidden = !show;
    sel.disabled = !show;
    setCompletionModelWrapVisible(show);
    if (!show) {
      sel.innerHTML = "";
      sel.style.width = "";
    } else {
      sizeCompletionModelSelectWidth();
    }
  }

  function getSelectedCompletionModelRef() {
    var sel = els.completionModelSelect;
    if (!sel || sel.hidden || sel.disabled || !sel.options.length) return null;
    return decodeCompletionOptionValue(sel.value);
  }

  function syncModelLabelFromCompletionSelect() {
    var sel = els.completionModelSelect;
    if (sel && !sel.hidden && sel.selectedOptions && sel.selectedOptions[0]) {
      var ref = decodeCompletionOptionValue(sel.value);
      var t =
        ref && ref.model_id
          ? String(ref.model_id).trim()
          : String(sel.selectedOptions[0].textContent || "").trim();
      if (t) {
        state.modelLabel = t;
        if (els.modelLabel) els.modelLabel.textContent = state.modelLabel;
        renderStatusBarOpenCode();
      }
    }
    sizeCompletionModelSelectWidth();
  }

  function renderConfig() {
    const config = state.config;
    if (!config) {
      state.modelLabel = "模型";
      if (els.modelLabel) els.modelLabel.textContent = state.modelLabel;
      if (els.completionModelSelect) {
        els.completionModelSelect.innerHTML = "";
        els.completionModelSelect.hidden = true;
        els.completionModelSelect.disabled = true;
        els.completionModelSelect.style.width = "";
      }
      setCompletionModelWrapVisible(false);
      renderStatusBarOpenCode();
      return;
    }
    syncCompletionModelSelectFromConfig();
    if (
      els.completionModelSelect &&
      !els.completionModelSelect.hidden &&
      els.completionModelSelect.options.length
    ) {
      syncModelLabelFromCompletionSelect();
    } else {
      state.modelLabel =
        String(config.completion_model || config.provider_name || config.provider_id || "模型").trim() ||
        "模型";
      if (state.modelLabel && (state.modelLabel.indexOf("·") >= 0 || state.modelLabel.indexOf("\u00b7") >= 0)) {
        var sep2 = state.modelLabel.indexOf("·") >= 0 ? "·" : "\u00b7";
        var parts2 = state.modelLabel.split(sep2);
        var last = String(parts2[parts2.length - 1] || "").trim();
        if (last) state.modelLabel = last;
      }
      if (els.modelLabel) els.modelLabel.textContent = state.modelLabel;
    }
    renderStatusBarOpenCode();
  }

  function renderAgentMode() {
    const mode = normalizeAgentMode(state.agentMode);
    state.agentMode = mode;
    if (els.modeAsk) {
      const active = mode === "ask";
      els.modeAsk.classList.toggle("is-active", active);
      els.modeAsk.setAttribute("aria-pressed", active ? "true" : "false");
    }
    if (els.modeBuild) {
      const active = mode === "build";
      els.modeBuild.classList.toggle("is-active", active);
      els.modeBuild.setAttribute("aria-pressed", active ? "true" : "false");
    }
  }

  function rememberAgentMode() {
    try {
      localStorage.setItem(modeStorageKey(), normalizeAgentMode(state.agentMode));
    } catch (_) {}
  }

  function restoreAgentMode() {
    try {
      const saved = localStorage.getItem(modeStorageKey());
      if (saved) {
        state.agentMode = normalizeAgentMode(saved);
      }
    } catch (_) {}
    renderAgentMode();
  }

  function switchAgentMode(nextMode) {
    state.agentMode = normalizeAgentMode(nextMode);
    rememberAgentMode();
    renderAgentMode();
    setInlineNote(
      state.agentMode === "ask"
        ? "已切换到 Ask（访问侧问答，只读）"
        : "已切换到 Build（可生成并改写当前脚本）",
    );
  }

  function renderRuntime() {
    renderStatus();
    renderInlineNote();
    renderStatusBarOpenCode();
  }

  function currentScopeParams() {
    const params = new URLSearchParams();
    const app = currentAppKey();
    const sceneId = currentSceneId();
    const routeMode = normalizeRouteMode(root.dataset.mode);
    const mode = normalizeAgentMode(state.agentMode);
    const entryId = String(root.dataset.entry || "").trim();
    const entryTarget = normalizeTargetKey(String(root.dataset.entryTarget || ""));
    const target = currentTargetKey();
    if (app) params.set("app_id", app);
    if (target) params.set("target_file", target);
    params.set("route_mode", routeMode);
    params.set("mode", mode);
    // 非入口文件预览（如 data/dataset/**）不应携带 scene/entry 约束，
    // 否则会触发 scope 校验失败并导致无意义重试。
    const scopedToEntry = !target || (entryTarget && target === entryTarget);
    if (scopedToEntry) {
      if (sceneId) params.set("scene_id", sceneId);
      if (entryId) params.set("entry_id", entryId);
    }
    return params;
  }

  function formatContextScopeText(payload) {
    const app = String((payload && payload.app_id) || currentAppKey() || "-");
    const scene = String((payload && payload.scene_id) || currentSceneId() || "-");
    const entry = String((payload && payload.entry_id) || root.dataset.entry || "-");
    const target = String((payload && payload.target_file) || currentTargetKey() || "-");
    return "scope: app=" + app + " | scene=" + scene + " | entry=" + entry + " | target=" + target;
  }

  function formatContextSkillText(payload) {
    const skill = payload && payload.skill_status ? payload.skill_status : null;
    if (!skill || typeof skill !== "object") {
      return "skill: (none)";
    }
    const mode = skill.installed ? (skill.stale ? "已安装(待同步)" : "已安装") : "仅源目录";
    const rev = String(skill.revision || "").trim();
    return "skill: " + mode + (rev ? " | rev=" + rev : "");
  }

  function formatContextToolsText(payload) {
    const tools = Array.isArray(payload && payload.query_tools) ? payload.query_tools : [];
    if (!tools.length) return "(none)";
    return tools.map(function (tool) {
      const id = String(tool && tool.id ? tool.id : "unknown");
      const purpose = String(tool && tool.purpose ? tool.purpose : "");
      const input = String(tool && tool.input ? tool.input : "");
      return "- " + id + (purpose ? " | " + purpose : "") + (input ? "\n  input: " + input : "");
    }).join("\n");
  }

  function formatContextPromptText(payload) {
    // system_prompt 已由服务端拼接 skill + [MeiLang Session Context] + 动态块；
    // session_context 与之同源，并列展示会造成整段重复，这里只展示实际注入的 system。
    const system = String((payload && payload.system_prompt) || "").trim();
    if (system) return system;
    const context = String((payload && payload.session_context) || "").trim();
    if (context) return "[Session Context]\n" + context;
    return "(empty)";
  }

  function trimDeltaPreview(text, maxChars) {
    const raw = String(text || "");
    if (!raw) return "";
    const normalized = raw.replace(/\s+/g, " ").trim();
    if (!normalized) return "";
    if (normalized.length <= maxChars) return normalized;
    return normalized.slice(0, Math.max(0, maxChars - 1)) + "…";
  }

  function formatDeltaDebugTs(stamp) {
    const ms = Number(stamp || 0);
    if (!Number.isFinite(ms) || ms <= 0) return "-";
    const d = new Date(ms);
    const pad = function (n, w) {
      const s = String(Number(n) || 0);
      return s.length >= w ? s : "0".repeat(w - s.length) + s;
    };
    return (
      pad(d.getHours(), 2) +
      ":" +
      pad(d.getMinutes(), 2) +
      ":" +
      pad(d.getSeconds(), 2) +
      "." +
      pad(d.getMilliseconds(), 3)
    );
  }

  function recordDeltaDebugEvent(event) {
    const serverTs = Number(event && event.server_ts_ms ? event.server_ts_ms : 0);
    const clientRxTs = Date.now();
    const deltaRaw = event && typeof event.delta === "string" ? event.delta : "";
    const preview = trimDeltaPreview(deltaRaw, 48);
    const gapRxMs =
      Number.isFinite(serverTs) && serverTs > 0 ? clientRxTs - serverTs : null;
    const row = {
      serverTs: Number.isFinite(serverTs) ? serverTs : 0,
      clientRxTs: clientRxTs,
      paintTs: null,
      partId: String(event && event.part_id ? event.part_id : ""),
      messageId: String(event && event.message_id ? event.message_id : ""),
      chars: deltaRaw.length,
      preview: preview,
      gapRxMs: gapRxMs,
      gapPaintMs: null,
    };
    state.deltaDebugLog.unshift(row);
    if (state.deltaDebugLog.length > 120) {
      state.deltaDebugLog.length = 120;
    }
    writeDeltaDebugLogToStorage(String(state.sessionId || ""), state.deltaDebugLog);
    renderDeltaDebugLog();
    requestAnimationFrame(function () {
      requestAnimationFrame(function () {
        const paintTs = Date.now();
        row.paintTs = paintTs;
        row.gapPaintMs =
          row.serverTs > 0 && Number.isFinite(row.serverTs) ? paintTs - row.serverTs : null;
        writeDeltaDebugLogToStorage(String(state.sessionId || ""), state.deltaDebugLog);
        renderDeltaDebugLog();
      });
    });
  }

  function renderDeltaDebugLog() {
    const log = Array.isArray(state.deltaDebugLog) ? state.deltaDebugLog : [];
    const manageEl = document.getElementById("mei-manage-debug-agent-sse-delta");
    const emptyManageHint =
      "尚无助手流式 delta 记录。请在右侧「作者」连接会话并发消息；出现 srv/cli_rx/gap_rx 与 cli_paint/gap_paint（后者为连续两次 requestAnimationFrame 后的墙钟，近似「排帧后」与首绘间隔）。SPA 换文件后若曾收过 delta，请再点一次「调试」页签或发新消息以刷新本区。";
    if (!log.length) {
      if (els.contextDeltaDebug) els.contextDeltaDebug.textContent = "(empty)";
      if (manageEl) manageEl.textContent = emptyManageHint;
      return;
    }
    const lines = log.slice(0, 60).map(function (item, index) {
      const rxTs =
        item && item.clientRxTs != null
          ? item.clientRxTs
          : item && item.clientTs != null
            ? item.clientTs
            : 0;
      const gapRxLabel =
        item && item.gapRxMs != null && Number.isFinite(item.gapRxMs)
          ? String(item.gapRxMs) + "ms"
          : item && item.gapMs != null && Number.isFinite(item.gapMs)
            ? String(item.gapMs) + "ms"
            : "-";
      const paintTs = item && item.paintTs != null ? item.paintTs : null;
      const cliPaintStr =
        paintTs != null && Number.isFinite(paintTs) && paintTs > 0
          ? formatDeltaDebugTs(paintTs)
          : "-";
      const gapPaintLabel =
        item && item.gapPaintMs != null && Number.isFinite(item.gapPaintMs)
          ? String(item.gapPaintMs) + "ms"
          : "-";
      return (
        "#" +
        String(index + 1).padStart(2, "0") +
        " srv=" +
        formatDeltaDebugTs(item.serverTs) +
        " cli_rx=" +
        formatDeltaDebugTs(rxTs) +
        " gap_rx=" +
        gapRxLabel +
        " cli_paint=" +
        cliPaintStr +
        " gap_paint=" +
        gapPaintLabel +
        " chars=" +
        String(item.chars || 0) +
        " part=" +
        String(item.partId || "-") +
        " msg=" +
        String(item.messageId || "-") +
        " delta=\"" +
        String(item.preview || "") +
        "\""
      );
    });
    const text = lines.join("\n");
    if (els.contextDeltaDebug) els.contextDeltaDebug.textContent = text;
    if (manageEl) manageEl.textContent = text;
  }

  function readContextInventory(payload) {
    const inventory = payload && payload.resource_inventory ? payload.resource_inventory : null;
    if (!inventory || typeof inventory !== "object") {
      return { target: "", total: 0, items: [] };
    }
    return {
      target: String(inventory.target_file || "").trim(),
      total: Number.isFinite(Number(inventory.total_items)) ? Number(inventory.total_items) : 0,
      items: Array.isArray(inventory.items) ? inventory.items : [],
    };
  }

  function groupInventoryItemsByType(items) {
    const grouped = {};
    (Array.isArray(items) ? items : []).forEach(function (item) {
      if (!item || typeof item !== "object") return;
      const type = String(item.resource_type || "unknown").trim() || "unknown";
      if (!grouped[type]) grouped[type] = [];
      grouped[type].push(item);
    });
    return grouped;
  }

  function renderContextInventory(payload) {
    if (!els.contextInventory) return;
    const inventory = readContextInventory(payload);
    const groups = groupInventoryItemsByType(inventory.items);
    const types = Object.keys(groups).sort();
    els.contextInventory.innerHTML = "";
    if (!types.length) {
      els.contextInventory.textContent = "(none)";
      return;
    }
    const head = document.createElement("div");
    head.className = "text-[10px] text-slate-400";
    head.textContent =
      "target=" + (inventory.target || "-") + " | total=" + String(inventory.total || 0);
    els.contextInventory.appendChild(head);

    types.forEach(function (type, index) {
      const items = groups[type] || [];
      const details = document.createElement("details");
      details.className = "rounded border border-slate-700/60 bg-slate-950/40 px-2 py-1";
      details.open = index < 2;

      const summary = document.createElement("summary");
      summary.className = "cursor-pointer text-[10px] font-bold text-slate-200";
      summary.textContent = type + " (" + String(items.length) + ")";
      details.appendChild(summary);

      const list = document.createElement("div");
      list.className = "mt-1 grid gap-1";
      items.forEach(function (item) {
        const row = document.createElement("div");
        row.className = "rounded border border-slate-700/50 bg-slate-900/45 px-1.5 py-1";
        const id = String(item.id || "").trim() || "(no-id)";
        const title = String(item.title || "").trim();
        const summaryText = String(item.summary || "").trim();
        const sourcePath = String(item.source_path || "").trim();
        const refs = Array.isArray(item.references) ? item.references : [];
        const related = item.related_to_target ? " [target]" : "";
        const firstLine = document.createElement("div");
        firstLine.className = "font-mono text-[10px] text-slate-100";
        firstLine.textContent = id + (title ? " · " + title : "") + related;
        row.appendChild(firstLine);
        if (summaryText) {
          const sub = document.createElement("div");
          sub.className = "text-[10px] text-slate-300";
          sub.textContent = summaryText;
          row.appendChild(sub);
        }
        if (sourcePath) {
          const sub = document.createElement("div");
          sub.className = "font-mono text-[10px] text-blue-300";
          sub.textContent = "source: " + sourcePath;
          row.appendChild(sub);
        }
        if (refs.length) {
          const sub = document.createElement("div");
          sub.className = "text-[10px] text-slate-400";
          sub.textContent = "refs: " + refs.slice(0, 8).join(", ");
          row.appendChild(sub);
        }
        list.appendChild(row);
      });
      details.appendChild(list);
      els.contextInventory.appendChild(details);
    });
  }

  function renderContextPreview() {
    if (els.contextScope) {
      els.contextScope.textContent = formatContextScopeText(state.contextPreview);
    }
    if (els.contextSkill) {
      els.contextSkill.textContent = formatContextSkillText(state.contextPreview);
    }
    if (els.contextTools) {
      els.contextTools.textContent = formatContextToolsText(state.contextPreview);
    }
    if (els.contextInventory) {
      renderContextInventory(state.contextPreview);
    }
    if (els.contextPrompt) {
      els.contextPrompt.textContent = formatContextPromptText(state.contextPreview);
    }
    renderDeltaDebugLog();
  }

  async function refreshContextPreview(force) {
    const forceRefresh = Boolean(force);
    if (!forceRefresh && state.contextPreviewBackoffUntilMs > Date.now()) {
      return;
    }
    const app = currentAppKey();
    if (!app) {
      state.contextPreview = null;
      renderContextPreview();
      return;
    }
    try {
      const params = currentScopeParams();
      const scopeKey = params.toString();
      const nowMs = Date.now();
      const sameScope =
        state.contextPreviewScopeKey &&
        state.contextPreviewScopeKey === scopeKey;
      if (
        !forceRefresh &&
        sameScope &&
        state.contextPreviewFetchedAtMs > 0 &&
        nowMs - state.contextPreviewFetchedAtMs < 60000
      ) {
        return;
      }
      const payload = await fetchJson("/api/agent/context/preview?" + params.toString());
      state.contextPreview = payload;
      state.contextPreviewScopeKey = scopeKey;
      state.contextPreviewFetchedAtMs = nowMs;
      const previewError = String(payload && payload.preview_error ? payload.preview_error : "").trim();
      if (previewError && !forceRefresh) {
        // 对确定性失败做退避，避免持续无意义轮询刷日志。
        state.contextPreviewBackoffUntilMs = Date.now() + 60000;
      } else {
        state.contextPreviewBackoffUntilMs = 0;
      }
      renderContextPreview();
    } catch (error) {
      state.contextPreview = null;
      state.contextPreviewScopeKey = "";
      state.contextPreviewFetchedAtMs = 0;
      if (!forceRefresh) {
        state.contextPreviewBackoffUntilMs = Date.now() + 60000;
      }
      renderContextPreview();
      setInlineNote("读取上下文预览失败：" + String(error.message || error));
    }
  }

  function formatMsTime(value) {
    const stamp = Number(value || 0);
    if (!Number.isFinite(stamp) || stamp <= 0) return "";
    return new Date(stamp).toLocaleString("zh-CN", {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function selectedModelProbeQueryString() {
    const params = new URLSearchParams();
    const mref = getSelectedCompletionModelRef();
    if (mref && mref.provider_id) {
      params.set("provider_id", String(mref.provider_id));
    }
    if (mref && mref.model_id) {
      params.set("model_id", String(mref.model_id));
    }
    return params.toString();
  }

  function noteModelProbeResult(probe, atMs) {
    const ts = Number(atMs || Date.now());
    if (probe && probe.reachable) {
      state.modelProbeFailureStreak = 0;
      state.modelProbeLastSuccessAtMs = ts;
      return;
    }
    state.modelProbeFailureStreak = Number(state.modelProbeFailureStreak || 0) + 1;
  }

  async function refreshModelProbe(force) {
    if (!els.statusModelService) return;
    const forceRefresh = Boolean(force);
    const nowMs = Date.now();
    if (!forceRefresh && state.modelProbeFetchedAtMs > 0 && nowMs - state.modelProbeFetchedAtMs < 30000) {
      return;
    }
    const query = selectedModelProbeQueryString();
    try {
      state.modelProbe = await fetchJson(
        "/api/agent/model/probe" + (query ? "?" + query : ""),
      );
      state.modelProbeFetchedAtMs = nowMs;
      noteModelProbeResult(state.modelProbe, nowMs);
    } catch (error) {
      state.modelProbe = {
        reachable: false,
        provider_id: "",
        model_id: "",
        base_url: "",
        error: String(error && error.message ? error.message : error || ""),
      };
      state.modelProbeFetchedAtMs = nowMs;
      noteModelProbeResult(state.modelProbe, nowMs);
    }
    renderStatusBarOpenCode();
  }

  function renderSkillStatus() {
    const skill = state.skillStatus;
    if (!skill || !skill.source_present) {
      if (els.skillLine) {
        els.skillLine.textContent = "Skill: 未发现 MeiLang skill 源目录";
      }
      renderStatusBarOpenCode();
      return;
    }
    const summary = [];
    summary.push(skill.installed ? "Skill: 已安装" : "Skill: 仅源目录");
    if (skill.stale) summary.push("待同步");
    if (Number.isFinite(Number(skill.file_count))) {
      summary.push("文件 " + String(skill.file_count));
    }
    const updated = formatMsTime(skill.install_updated_at_ms || skill.source_updated_at_ms);
    if (updated) {
      summary.push(updated);
    }
    if (skill.revision) {
      summary.push("rev " + String(skill.revision));
    }
    if (els.skillLine) {
      els.skillLine.textContent = summary.join(" · ");
    }
    renderStatusBarOpenCode();
  }

  function renderStatusBarSkill() {
    renderStatusBarOpenCode();
  }

  function renderStatusBarOpenCode() {
    if (!els.statusModelService) return;
    if (state.loading) {
      els.statusModelService.textContent = "模型服务 刷新中";
      els.statusModelService.title = "正在刷新模型服务状态";
      els.statusModelService.dataset.tone = "info";
      return;
    }
    const probe = state.modelProbe;
    if (!probe || typeof probe !== "object") {
      els.statusModelService.textContent = "模型服务 探测中";
      els.statusModelService.title = "正在探测当前模型服务连接状态";
      els.statusModelService.dataset.tone = "info";
      return;
    }
    const provider = String(probe && probe.provider_id ? probe.provider_id : "").trim() || "--";
    const model = String(probe && probe.model_id ? probe.model_id : "").trim() || "--";
    const latency = Number(probe && probe.latency_ms ? probe.latency_ms : 0);
    const latencyText = Number.isFinite(latency) && latency > 0 ? " · " + String(latency) + "ms" : "";
    if (probe && probe.reachable) {
      els.statusModelService.textContent = "模型服务 在线";
      els.statusModelService.title = "provider=" + provider + " · model=" + model + latencyText;
      els.statusModelService.dataset.tone = "good";
      return;
    }
    const nowMs = Date.now();
    const streak = Number(state.modelProbeFailureStreak || 0);
    const lastSuccessAt = Number(state.modelProbeLastSuccessAtMs || 0);
    const hasSuccess = Number.isFinite(lastSuccessAt) && lastSuccessAt > 0;
    const withinGrace = hasSuccess && nowMs - lastSuccessAt < MODEL_PROBE_RED_AFTER_MS;
    const transientFailure = hasSuccess
      ? streak < MODEL_PROBE_RED_AFTER_STREAK || withinGrace
      : streak < MODEL_PROBE_COLD_START_RED_AFTER_STREAK;
    const error = String(probe && probe.error ? probe.error : "").trim();
    const title = (error ? error + " · " : "") + "provider=" + provider + " · model=" + model + latencyText;
    if (transientFailure) {
      els.statusModelService.textContent = "模型服务 连接中";
      els.statusModelService.title = "正在尝试连接 · " + title;
      els.statusModelService.dataset.tone = "info";
      return;
    }
    els.statusModelService.textContent = "模型服务 异常";
    els.statusModelService.title = title;
    els.statusModelService.dataset.tone = "danger";
  }

  function readSessionCache() {
    if (!window.sessionStorage) return null;
    try {
      const raw = window.sessionStorage.getItem(SESSION_CACHE_KEY);
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      const updatedAtMs = Number(parsed && parsed.updatedAtMs);
      const list = Array.isArray(parsed && parsed.list) ? parsed.list : [];
      if (!Number.isFinite(updatedAtMs) || updatedAtMs <= 0) return null;
      return { updatedAtMs: updatedAtMs, list: list };
    } catch (_) {
      return null;
    }
  }

  function writeSessionCache(list) {
    if (!window.sessionStorage) return;
    try {
      window.sessionStorage.setItem(
        SESSION_CACHE_KEY,
        JSON.stringify({
          updatedAtMs: Date.now(),
          list: Array.isArray(list) ? list : [],
        }),
      );
    } catch (_) {}
  }

  function invalidateSessionCache() {
    if (!window.sessionStorage) return;
    try {
      window.sessionStorage.removeItem(SESSION_CACHE_KEY);
    } catch (_) {}
    state.sessionsCacheAtMs = 0;
  }

  function sessionIdInList(sessions, id) {
    const sid = String(id || "").trim();
    if (!sid) return false;
    return (Array.isArray(sessions) ? sessions : []).some(function (item) {
      return item && String(item.id || "") === sid;
    });
  }

  function isSessionCacheFresh(cache) {
    if (!cache) return false;
    const age = Date.now() - Number(cache.updatedAtMs || 0);
    return Number.isFinite(age) && age >= 0 && age <= SESSION_CACHE_TTL_MS;
  }

  async function fetchAllSessionsFromServer() {
    const payload = await fetchJson("/api/agent/session");
    return Array.isArray(payload) ? payload : [];
  }

  async function fetchAllSessions(options) {
    const opts = options || {};
    const preferCache = opts.preferCache === true;
    const skipCache = opts.skipCache === true;
    if (!skipCache && preferCache) {
      const cached = readSessionCache();
      if (cached && cached.list.length > 0) {
        state.sessions = cached.list.slice();
        state.sessionsCacheAtMs = Number(cached.updatedAtMs || 0);
        if (!isSessionCacheFresh(cached)) {
          fetchAllSessions({ skipCache: true }).catch(function () {});
        }
        return state.sessions;
      }
    }
    if (state.sessionsFetchInFlight) {
      try {
        return await state.sessionsFetchInFlight;
      } catch (_) {
        return [];
      }
    }
    const request = (async function () {
      const list = await fetchAllSessionsFromServer();
      state.sessions = list.slice();
      state.sessionsCacheAtMs = Date.now();
      writeSessionCache(list);
      return list;
    })();
    state.sessionsFetchInFlight = request;
    try {
      return await request;
    } finally {
      if (state.sessionsFetchInFlight === request) {
        state.sessionsFetchInFlight = null;
      }
    }
  }

  function formatSessionOptionLabel(session) {
    const id = String((session && session.id) || "");
    const id8 = id.length > 8 ? id.slice(-8) : id;
    const updated =
      Number(session && session.updated_at_ms) ||
      Number(session && session.created_at_ms);
    if (Number.isFinite(updated) && updated > 0) {
      const time = new Date(updated).toLocaleString("zh-CN", {
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
      });
      return id8 + " · " + time;
    }
    return id8 || "unknown";
  }

  function listBoundSessionsForTarget(sessions, targetKey) {
    const app = String(root.dataset.app || "");
    const scene = currentSceneId();
    const entry = String(root.dataset.entry || "");
    const target = normalizeTargetKey(targetKey);
    return (Array.isArray(sessions) ? sessions : [])
      .filter(function (session) {
        if (!session || typeof session !== "object") return false;
        const meta = parseBoundSessionTitle(session.title);
        if (!meta) return false;
        if (meta.app !== app) return false;
        if (meta.target !== target) return false;
        if (scene && meta.scene && meta.scene !== scene) return false;
        if (entry && meta.entry && meta.entry !== entry) return false;
        return true;
      })
      .sort(function (a, b) {
        const ta = Number(a && a.updated_at_ms) || 0;
        const tb = Number(b && b.updated_at_ms) || 0;
        return tb - ta;
      });
  }

  async function refreshSessionPicker(selectedId, targetKey) {
    if (!els.sessionSelect) return;
    const desiredTarget = normalizeTargetKey(targetKey || currentTargetKey());
    const sessions = listBoundSessionsForTarget(
      await fetchAllSessions({ preferCache: true }),
      desiredTarget,
    );
    const current = String(selectedId || state.sessionId || "");
    els.sessionSelect.innerHTML = "";
    const placeholder = document.createElement("sl-option");
    placeholder.value = "";
    placeholder.textContent = "历史（当前文件）";
    els.sessionSelect.appendChild(placeholder);
    sessions.forEach(function (session) {
      if (!session || typeof session !== "object") return;
      const id = String(session.id || "");
      if (!id) return;
      const option = document.createElement("sl-option");
      option.value = id;
      option.textContent = formatSessionOptionLabel(session);
      els.sessionSelect.appendChild(option);
    });
    els.sessionSelect.value =
      current && sessions.some(function (item) { return String(item && item.id || "") === current; })
        ? current
        : "";
  }

  function renderSessions() {
    refreshSessionPicker(state.sessionId, state.sessionTargetKey || currentTargetKey()).catch(function () {});
  }

  function makeTextBlock(label, content, type, collapsed) {
    const text = String(content || "").trim();
    if (!text) return null;
    return {
      type: String(type || "text"),
      label: String(label || ""),
      content: text,
      collapsed: collapsed === true,
    };
  }

  /** 折叠摘要一行：工具名 + 与 OpenCode 侧一致的「主参数」提示（input 存在 filePath 字段）。 */
  function toolBlockLabel(part) {
    const tool = part && part.tool ? part.tool : null;
    if (!tool) return "工具调用";
    const name = String(tool.tool || "").trim() || "unknown";
    const fp = String(tool.input_path || "").trim();
    const title = String(tool.title || "").trim();
    if (name === "read_file" && fp) return "read_file · path=" + fp;
    if (name === "skill_read" && fp) return "skill_read · path=" + fp;
    if (name === "resource_get" && fp) return "resource_get · id=" + fp;
    if (name === "resource_list") return "resource_list";
    if (name === "resource_runtime_peek") return "resource_runtime_peek";
    if (name === "skill_list") return "skill_list";
    if (fp) return name + " · filePath=" + fp;
    if (title && title !== name) return name + " · " + title;
    return name;
  }

  function formatToolPart(part) {
    const tool = part && part.tool ? part.tool : null;
    if (!tool) return null;
    const name = String(tool.tool || "unknown");
    const lines = [];
    lines.push("工具: " + name);
    const fp = String(tool.input_path || "").trim();
    if (name === "read_file" && fp) lines.push("参数 path: " + fp);
    else if (name === "skill_read" && fp) lines.push("参数 path: " + fp);
    else if (name === "resource_get" && fp) lines.push("参数 id: " + fp);
    else if (fp) lines.push("参数 filePath: " + fp);
    lines.push("状态: " + String(tool.status || "pending"));
    const cid = String(tool.call_id || "").trim();
    if (cid) lines.push("call_id: " + cid);
    const title = String(tool.title || "").trim();
    if (title) lines.push("标题: " + title);
    if (tool.output) lines.push("输出:\n" + String(tool.output));
    if (tool.error) lines.push("错误:\n" + String(tool.error));
    return lines.join("\n");
  }

  function looksLikeSkillPath(path) {
    return String(path || "").replaceAll("\\", "/").includes("/.mei/skills/meilang-author");
  }

  function blockedPermissionNoticeFromData(data) {
    const permissionId = String((data && data.permission_id) || "").trim();
    const permission = String((data && data.permission) || "unknown").trim() || "unknown";
    const patterns = Array.isArray(data && data.patterns)
      ? data.patterns
          .map(function (item) { return String(item || "").trim(); })
          .filter(Boolean)
      : [];
    const rawPath = String((data && data.path) || "").trim();
    const path = rawPath || (patterns.length > 0 ? patterns[0] : "");
    const requiresAdmin = !!(data && data.requires_admin);
    const message = String((data && data.message) || "").trim();
    return {
      id: permissionId || "path:" + (path || permission || "unknown"),
      permissionId: permissionId,
      permission: permission,
      path: path,
      patterns: patterns,
      requiresAdmin: requiresAdmin,
      message: message,
    };
  }

  function blockedPermissionNoticeFromRunningRead(messageId, part) {
    const tool = part && part.tool ? part.tool : null;
    const path = String((tool && tool.input_path) || "").trim();
    const id = String((part && part.part_id) || "") || String(messageId || "");
    if (!path) return null;
    if (looksLikeSkillPath(path)) {
      return {
        id: "running-read:" + id,
        permissionId: "",
        permission: "external_directory",
        path: path,
        patterns: [path],
        requiresAdmin: true,
        message:
          "系统尝试读取 MeiLang skill 目录但当前未获授权。请在权限提示中批准，或请管理员检查 external_directory 策略。",
      };
    }
    return {
      id: "running-read:" + id,
      permissionId: "",
      permission: "external_directory",
      path: path,
      patterns: [path],
      requiresAdmin: true,
      message:
        "检测到会话尝试访问未授权目录。请先检查你输入的目标路径；若这是系统预期目录，请联系管理员处理白名单。",
    };
  }

  function blockedPermissionBody(notice) {
    const lines = [];
    lines.push("类型: 权限阻塞");
    lines.push("permission: " + String(notice.permission || "unknown"));
    if (notice.permissionId) lines.push("permission_id: " + String(notice.permissionId));
    if (notice.path) lines.push("目录: " + String(notice.path));
    if (notice.patterns && notice.patterns.length > 0) {
      lines.push("匹配模式:");
      notice.patterns.forEach(function (pattern) {
        lines.push("- " + String(pattern));
      });
    }
    if (notice.message) lines.push("说明: " + String(notice.message));
    lines.push(
      notice.requiresAdmin
        ? "建议: 若目录正确，请联系管理员；若目录异常，请修正你的任务路径。"
        : "建议: 请检查当前任务与目录范围。",
    );
    return lines.join("\n");
  }

  function mergeBlockedPermissionNotices(primary, fallback) {
    const merged = [];
    const seen = new Set();
    function addList(list) {
      (Array.isArray(list) ? list : []).forEach(function (item) {
        if (!item || typeof item !== "object") return;
        const id = String(item.id || "").trim();
        if (!id || seen.has(id)) return;
        seen.add(id);
        merged.push(item);
      });
    }
    addList(primary);
    addList(fallback);
    return merged;
  }

  function blockedPermissionFingerprint(notices) {
    return mergeBlockedPermissionNotices(notices, [])
      .map(function (item) {
        return [
          String(item && item.id || ""),
          String(item && item.permissionId || ""),
          String(item && item.path || ""),
        ].join("|");
      })
      .filter(Boolean)
      .sort()
      .join("||");
  }

  function rememberBlockedPermissionNotice(notice) {
    state.pendingPermissionNotices = mergeBlockedPermissionNotices(
      [notice],
      state.pendingPermissionNotices,
    );
  }

  function resetPendingPermissionState() {
    state.pendingPermissionsFingerprint = "";
    state.pendingPermissionsFetchedAt = 0;
    state.pendingPermissionNotices = [];
    state.pendingPermissionsBootstrappedSessionId = "";
    state.activeGenerationMessageId = "";
    state.latestRoundAssistantId = "";
    state.latestDiffMessageId = "";
    state.sourceDiffMessageId = "";
    if (state.sourceViewMode === "diff") {
      leaveDiffView();
    } else {
      destroySourceDiffView();
    }
    state.progress = {
      visible: false,
      label: "",
      detail: "",
      items: [],
    };
    renderProgressStrip();
  }

  function applyBlockedPermissionNotices(notices) {
    const list = Array.isArray(notices) ? notices : [];
    if (!list.length) return;
    const summary = list
      .map(function (notice) {
        const path = String(notice && notice.path ? notice.path : "").trim();
        const message = String(notice && notice.message ? notice.message : "").trim();
        return path ? "已拒绝未授权目录：" + path : message;
      })
      .filter(Boolean)
      .join("；");
    if (summary) {
      setInlineNote(summary);
    }
  }

  function deriveBlockedNoticesFromRawMessages(rawMessages) {
    const notices = [];
    (Array.isArray(rawMessages) ? rawMessages : []).forEach(function (raw) {
      if (!raw || String(raw.role || "") !== "assistant") return;
      const messageId = String(raw.message_id || "");
      const parts = Array.isArray(raw.parts) ? raw.parts : [];
      parts.forEach(function (part) {
        if (!part || String(part.part_type || "") !== "tool") return;
        const tool = part.tool || null;
        if (!tool) return;
        if (String(tool.tool || "") !== "read") return;
        if (String(tool.status || "") !== "running") return;
        const notice = blockedPermissionNoticeFromRunningRead(messageId, part);
        if (notice) notices.push(notice);
      });
    });
    return notices;
  }

  function copyText(text) {
    const value = String(text || "");
    if (!value) return Promise.resolve();
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(value);
    }
    return new Promise(function (resolve, reject) {
      try {
        const temp = document.createElement("textarea");
        temp.value = value;
        temp.setAttribute("readonly", "readonly");
        temp.style.position = "fixed";
        temp.style.left = "-9999px";
        temp.style.top = "-9999px";
        document.body.appendChild(temp);
        temp.select();
        document.execCommand("copy");
        document.body.removeChild(temp);
        resolve();
      } catch (error) {
        reject(error);
      }
    });
  }

  function normalizeMessage(raw) {
    const partsRaw = Array.isArray(raw && raw.parts) ? raw.parts : [];
    const parts = partsRaw.slice();
    parts.sort(function (a, b) {
      const ao = Number(a && a.sort_order);
      const bo = Number(b && b.sort_order);
      if (Number.isFinite(ao) && Number.isFinite(bo) && ao !== bo) return ao - bo;
      return 0;
    });
    const role = String((raw && raw.role) || "assistant");
    const blocks = [];
    function pushTextBlock(text) {
      const t = String(text || "").trim();
      if (!t) return;
      const tb = makeTextBlock("", t, "text");
      if (tb) blocks.push(tb);
    }
    function pushReasoningBlock(text) {
      const t = String(text || "").trim();
      if (!t) return;
      const rb = makeTextBlock("思考（可折叠调试）", t, "reasoning", true);
      if (rb) blocks.push(rb);
    }
    parts.forEach(function (part) {
      const type = String((part && part.part_type) || "");
      if (type === "text") {
        pushTextBlock(part && part.text ? part.text : "");
        return;
      }
      if (type === "reasoning") {
        pushReasoningBlock(part && part.text ? part.text : "");
        return;
      }
      if (type === "tool") {
        const toolBody = formatToolPart(part);
        if (toolBody) {
          const label = toolBlockLabel(part);
          const block = makeTextBlock(label, toolBody, "tool", true);
          if (block) blocks.push(block);
        }
        return;
      }
      if (type === "patch") {
        const patchText = String(part && part.text ? part.text : "").trim();
        if (patchText) {
          const pb = makeTextBlock("代码补丁", patchText, "patch", true);
          if (pb) blocks.push(pb);
        }
        return;
      }
      if (part && part.raw) {
        const debugBody = JSON.stringify(part.raw, null, 2);
        const block = makeTextBlock("结构化片段", debugBody, "debug", true);
        if (block) blocks.push(block);
      }
    });
    const body =
      blocks.length > 0
        ? blocks
            .map(function (block) {
              return (block.label ? "[" + block.label + "]\n" : "") + block.content;
            })
            .join("\n\n")
        : "(空消息)";
    return {
      id: String((raw && raw.message_id) || ""),
      role: role,
      body: body,
      blocks: blocks,
      time: new Date().toLocaleTimeString("zh-CN", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      }),
      actions: [],
    };
  }

  function inferAgentModeFromRawMessage(raw) {
    if (!raw || String(raw.role || "") !== "assistant") return null;
    const parts = Array.isArray(raw.parts) ? raw.parts : [];
    const hasPatchPart = parts.some(function (part) {
      return String(part && part.part_type ? part.part_type : "") === "patch";
    });
    return hasPatchPart ? "build" : null;
  }

  function renderMessageActions(message, messageId) {
    const actions = Array.isArray(message && message.actions) ? message.actions : [];
    if (!actions.length) return "";
    return (
      '<div class="' + CHAT_CLASS.inlineActions + '">' +
      actions
        .map(function (action, index) {
          return (
            '<button type="button" class="' + CHAT_CLASS.actionButton + '" data-message-id="' +
            escapeHtml(messageId) +
            '" data-action-index="' +
            String(index) +
            '">' +
            escapeHtml(action && action.label ? action.label : "执行") +
            "</button>"
          );
        })
        .join("") +
      "</div>"
    );
  }

  async function fetchSessionDiff(messageId) {
    if (!state.sessionId) return null;
    const params = new URLSearchParams();
    const mid = String(messageId || "").trim();
    if (mid) params.set("message_id", mid);
    const pathKey = sourceTargetKey();
    if (pathKey) params.set("path", pathKey);
    const qs = params.toString();
    return fetchJson(
      "/api/agent/session/" +
        encodeURIComponent(state.sessionId) +
        "/diff" +
        (qs ? "?" + qs : ""),
    );
  }

  /** 与 `GET .../diff` 语义一致：占位快照或空 diff 不算「有改动」，避免误触发整页 reload。 */
  function sessionDiffHasMaterialChanges(diff) {
    if (!diff || typeof diff !== "object") return false;
    const topAdd = Number(diff.additions);
    const topDel = Number(diff.deletions);
    if ((Number.isFinite(topAdd) && topAdd > 0) || (Number.isFinite(topDel) && topDel > 0)) {
      return true;
    }
    const files = Array.isArray(diff.files) ? diff.files : [];
    return files.some(function (f) {
      if (!f || typeof f !== "object") return false;
      const a = Number(f.additions);
      const d = Number(f.deletions);
      if ((Number.isFinite(a) && a > 0) || (Number.isFinite(d) && d > 0)) return true;
      const after = String(f.after || "").trim();
      if (!after) return false;
      const low = after.toLowerCase();
      if (low.includes("no git worktree") || low.includes("native diff snapshot:")) return false;
      return after.split("\n").some(function (line) {
        const t = String(line || "");
        return (
          (t.startsWith("+") && !t.startsWith("+++")) ||
          (t.startsWith("-") && !t.startsWith("---"))
        );
      });
    });
  }

  async function applyRevertForMessage(messageId) {
    const sid = String(state.sessionId || "").trim();
    const mid = String(messageId || "").trim();
    if (!sid || !mid) return;
    await fetchJson("/api/agent/session/" + encodeURIComponent(sid) + "/revert", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ message_id: mid }),
    });
    setSessionRevertedFlag(sid, true);
    setMessageMeta(mid, { reverted: true });
    const revertedIds = revertedIdsForSession(sid);
    revertedIds.push(mid);
    setRevertedIdsForSession(sid, revertedIds);
    setInlineNote("已撤回上一轮代码修改。");
    await refreshMessages();
    scheduleHostReload("已撤回修改，正在刷新预览与源码…");
  }

  async function applyUnrevertForSession() {
    const sid = String(state.sessionId || "").trim();
    if (!sid) return;
    await fetchJson("/api/agent/session/" + encodeURIComponent(sid) + "/unrevert", {
      method: "POST",
      headers: { "content-type": "application/json" },
    });
    setSessionRevertedFlag(sid, false);
    setRevertedIdsForSession(sid, []);
    Object.keys(state.messageMeta).forEach(function (key) {
      if (key.startsWith(sid + "::")) {
        state.messageMeta[key] = Object.assign({}, state.messageMeta[key], {
          reverted: false,
        });
      }
    });
    setInlineNote("已恢复最近撤回的代码修改。");
    await refreshMessages();
    scheduleHostReload("已恢复撤回修改，正在刷新预览与源码…");
  }

  function actionsForAssistantMessage(_message) {
    return [];
  }

  async function hydrateBuildDiffMeta(messages) {
    if (!Array.isArray(messages) || !state.sessionId) return;
    if (historyUnavailableReason()) {
      renderHistoryButtons();
      setDiffTabBadge(0, 0);
      return;
    }
    let changed = false;
    let shouldReloadForPendingBuild = false;
    for (const message of messages) {
      if (!message || String(message.role || "") !== "assistant") continue;
      const messageId = String(message.id || "");
      if (!messageId) continue;
      const meta = getMessageMeta(state.sessionId, messageId) || {};
      if (typeof meta.hasDiff === "boolean") continue;
      try {
        const diff = await fetchSessionDiff(messageId);
        const hasDiff = sessionDiffHasMaterialChanges(diff);
        setMessageMeta(messageId, { hasDiff: hasDiff });
        if (hasDiff) {
          state.messageDiffCache[diffCacheKey(state.sessionId, messageId)] = diff;
          if (
            state.pendingReloadMessageId &&
            String(state.pendingReloadMessageId) === messageId
          ) {
            shouldReloadForPendingBuild = true;
          }
        }
        changed = true;
      } catch (_) {}
    }
    if (changed) {
      state.messages = state.messages.map(function (row) {
        return Object.assign({}, row, { actions: actionsForAssistantMessage(row) });
      });
      renderMessages();
    }
    renderHistoryButtons();
    syncSourceDiffEntry();
    void refreshDiffTabBadge();
    if (shouldReloadForPendingBuild) {
      scheduleHostReload("Build 已修改文件，正在刷新预览与源码…");
    }
  }

  function decorateMessageActions() {
    syncSourceDiffEntry();
    state.messages = state.messages.map(function (message) {
      return Object.assign({}, message, {
        actions:
          String(message.role || "") === "assistant"
            ? actionsForAssistantMessage(message)
            : Array.isArray(message.actions)
              ? message.actions
              : [],
      });
    });
    renderHistoryButtons();
  }

  function chatScrollSnapshot() {
    if (!els.chatLog) return null;
    const scrollTop = Number(els.chatLog.scrollTop || 0);
    const clientHeight = Number(els.chatLog.clientHeight || 0);
    const scrollHeight = Number(els.chatLog.scrollHeight || 0);
    const distanceToBottom = scrollHeight - (scrollTop + clientHeight);
    return {
      scrollTop: scrollTop,
      nearBottom: distanceToBottom <= CHAT_BOTTOM_STICKY_THRESHOLD_PX,
    };
  }

  function restoreChatScroll(snapshot, autoStickBottom) {
    if (!els.chatLog) return;
    if (autoStickBottom) {
      els.chatLog.scrollTop = els.chatLog.scrollHeight;
      return;
    }
    if (!snapshot) return;
    els.chatLog.scrollTop = snapshot.scrollTop;
  }

  function renderChatMessageCard(message, forcedRole, extraClass) {
    const roleRaw = String(forcedRole || message && message.role || "assistant").toLowerCase();
    const role = escapeHtml(roleRaw);
    const messageId = String(message && message.id ? message.id : "");
    const reverted = roleRaw === "assistant" && isMessageReverted(state.sessionId, messageId);
    const classList = [
      CHAT_CLASS.messageBase,
      chatMessageRoleClass(roleRaw, reverted),
    ];
    if (extraClass) classList.push(extraClass);
    const cls = classList.join(" ");
    const roleTextClass = chatRoleTextClass(roleRaw, reverted);
    const blocks = Array.isArray(message && message.blocks) ? message.blocks : [];
    const time = escapeHtml(String(message && message.time ? message.time : ""));
    function blockBodyHtml(block) {
      const content = String(block.content || "");
      const blockType = String(block.type || "text");
      const collapsed = !!block.collapsed;
      if (collapsed) {
        return escapeHtml(content);
      }
      if (blockType !== "text") {
        return escapeHtml(content);
      }
      if (roleRaw === "assistant") {
        return renderMarkdownToSafeHtml(content);
      }
      return escapeHtml(content);
    }
    function blockBodyTag(block) {
      const blockType = String(block.type || "text");
      const collapsed = !!block.collapsed;
      if (collapsed || blockType !== "text" || roleRaw !== "assistant") {
        return "pre";
      }
      return "div";
    }
    function blockBodyClass(block) {
      const blockType = String(block.type || "text");
      const collapsed = !!block.collapsed;
      if (collapsed || blockType !== "text" || roleRaw !== "assistant") {
        return CHAT_CLASS.body;
      }
      return CHAT_CLASS.bodyMarkdown;
    }
    const bodyHtml =
      blocks.length > 0
        ? blocks
            .map(function (block) {
              const label = String(block.label || "").trim();
              const blockType = String(block.type || "text");
              const labelToneClass = chatBlockLabelToneClass(blockType);
              const inner = blockBodyHtml(block);
              const tag = blockBodyTag(block);
              const bodyClass = blockBodyClass(block);
              if (block.collapsed) {
                return (
                  '<details class="' +
                  CHAT_CLASS.block +
                  " " +
                  CHAT_CLASS.blockDetails +
                  " author-chat-block-" +
                  escapeHtml(blockType) +
                  '"><summary class="' +
                  CHAT_CLASS.blockSummary +
                  " " +
                  labelToneClass +
                  '">' +
                  escapeHtml(label || "展开") +
                  '</summary><pre class="' +
                  bodyClass +
                  '">' +
                  inner +
                  "</pre></details>"
                );
              }
              return (
                '<section class="' +
                CHAT_CLASS.block +
                " author-chat-block-" +
                escapeHtml(blockType) +
                '">' +
                (label
                  ? '<div class="' + CHAT_CLASS.blockLabel + " " + labelToneClass + '">' + escapeHtml(label) + "</div>"
                  : "") +
                "<" +
                tag +
                ' class="' +
                bodyClass +
                '">' +
                inner +
                "</" +
                tag +
                "></section>"
              );
            })
            .join("")
        : (function () {
            const fallback = String(message && message.body ? message.body : "");
            if (roleRaw === "assistant") {
              return (
                '<div class="' + CHAT_CLASS.bodyMarkdown + '">' + renderMarkdownToSafeHtml(fallback) + "</div>"
              );
            }
            return '<pre class="' + CHAT_CLASS.body + '">' + escapeHtml(fallback) + "</pre>";
          })();
    const actions = roleRaw === "assistant" ? renderMessageActions(message, messageId) : "";
    return (
      '<div class="' +
      cls +
      '" data-message-id="' +
      escapeHtml(messageId) +
      '">' +
      '<div class="' + CHAT_CLASS.head + '"><div class="' + CHAT_CLASS.roleBase + " author-chat-role-" +
      role +
      " " +
      roleTextClass +
      '">' +
      (roleRaw === "user" ? "我" : roleRaw === "assistant" ? escapeHtml(state.modelLabel || "模型") : "系统") +
      '</div><div class="' + CHAT_CLASS.meta + '"><span class="' + CHAT_CLASS.time + '">' +
      time +
      '</span><button type="button" class="' +
      CHAT_CLASS.copyButton +
      '" title="复制对话内容（Markdown 原文）" data-message-id="' +
      escapeHtml(messageId) +
      '">⧉</button></div></div>' +
      bodyHtml +
      actions +
      "</div>"
    );
  }

  function debugCopyTextForMessage(message) {
    const messageId = String(message && message.id ? message.id : "");
    const blocks = Array.isArray(message && message.blocks) ? message.blocks : [];
    const body =
      blocks.length > 0
        ? blocks
            .map(function (block) {
              return (block.label ? "[" + String(block.label) + "]\n" : "") + String(block.content || "");
            })
            .join("\n\n")
        : String(message && message.body ? message.body : "");
    return [
      "session_id: " + String(state.sessionId || ""),
      "message_id: " + messageId,
      "role: " + String(message && message.role ? message.role : ""),
      "",
      body,
    ].join("\n");
  }

  function renderMessages() {
    if (!els.chatLog) return;
    const scrollSnapshot = chatScrollSnapshot();
    const shouldStickBottom = !scrollSnapshot || scrollSnapshot.nearBottom;
    if (!state.sessionId) {
      els.chatLog.innerHTML =
        '<div class="' + CHAT_CLASS.empty + '">未选择会话。可先点击“新建对话”，或等待宿主自动创建/恢复会话。</div>';
      restoreChatScroll(scrollSnapshot, shouldStickBottom);
      return;
    }
    if (!state.messages.length) {
      els.chatLog.innerHTML =
        '<div class="' + CHAT_CLASS.empty + '">发送任务后，这里会连续显示输入、参考信息和模型回复。</div>';
      restoreChatScroll(scrollSnapshot, shouldStickBottom);
      return;
    }
    const rounds = conversationRounds(state.messages);
    const html = rounds
      .map(function (round) {
        const user = round && round.user ? round.user : null;
        const assistants = round && Array.isArray(round.assistants) ? round.assistants : [];
        const systemOnly = !user && assistants.length === 0 && round && Array.isArray(round.system)
          ? round.system
          : [];
        if (systemOnly.length > 0) {
          return systemOnly
            .map(function (message) {
              return renderChatMessageCard(message, "system", "author-chat-row-system");
            })
            .join("");
        }
        return (
          '<section class="' + CHAT_CLASS.round + '">' +
          (user ? renderChatMessageCard(user, "user", "author-chat-row-user") : "") +
          assistants
            .map(function (assistant, assistantIndex) {
              return renderChatMessageCard(
                assistant,
                "assistant",
                "author-chat-row-assistant" +
                  (assistantIndex > 0 ? " author-chat-row-assistant-followup" : ""),
              );
            })
            .join("") +
          "</section>"
        );
      })
      .join("");
    els.chatLog.innerHTML = html;

    Array.from(els.chatLog.querySelectorAll(".agent-action-btn")).forEach(function (button) {
      button.addEventListener("click", function () {
        const messageId = String(button.getAttribute("data-message-id") || "");
        const actionIndex = Number(button.getAttribute("data-action-index") || "-1");
        const message = state.messages.find(function (item) {
          return String(item && item.id ? item.id : "") === messageId;
        });
        const actions = Array.isArray(message && message.actions) ? message.actions : [];
        const action = actionIndex >= 0 ? actions[actionIndex] : null;
        if (action && typeof action.onClick === "function") {
          action.onClick();
        }
      });
    });

    Array.from(els.chatLog.querySelectorAll(".agent-copy-btn")).forEach(function (button) {
      button.addEventListener("click", function () {
        const messageId = String(button.getAttribute("data-message-id") || "");
        const message = state.messages.find(function (item) {
          return String(item && item.id ? item.id : "") === messageId;
        });
        if (!message) return;
        const text = debugCopyTextForMessage(message);
        const prevLabel = button.textContent;
        copyText(text)
          .then(function () {
            button.textContent = "已复制";
            button.setAttribute("title", "已复制到剪贴板");
            window.setTimeout(function () {
              button.textContent = prevLabel;
              button.setAttribute("title", "复制对话内容（Markdown 原文）");
            }, 1600);
          })
          .catch(function () {
            button.textContent = "失败";
            button.setAttribute("title", "复制失败，请重试");
            window.setTimeout(function () {
              button.textContent = prevLabel;
              button.setAttribute("title", "复制对话内容（Markdown 原文）");
            }, 1600);
          });
      });
    });

    restoreChatScroll(scrollSnapshot, shouldStickBottom);
  }

  function closeEventStream() {
    clearGenerationSettleTimer();
    if (state.eventSource) {
      try {
        state.eventSource.close();
      } catch (_) {}
    }
    state.eventSource = null;
    state.eventSourceSessionId = "";
    state.streamConnected = false;
  }

  function connectEvents(forceReconnect) {
    const sessionId = String(state.sessionId || "").trim();
    if (!(state.health && state.health.healthy) || !sessionId) {
      closeEventStream();
      return;
    }
    if (state.eventSource && state.eventSourceSessionId === sessionId && !forceReconnect) {
      return;
    }
    closeEventStream();
    try {
      const source = new EventSource(
        "/api/agent/session/" + encodeURIComponent(sessionId) + "/events",
      );
      source.onopen = function () {
        state.streamConnected = true;
        renderStatus();
      };
      source.onerror = function () {
        state.streamConnected = false;
        renderStatus();
      };
      source.onmessage = function (event) {
        try {
          applyHostEvent(JSON.parse(String(event.data || "{}")));
        } catch (_) {}
      };
      state.eventSource = source;
      state.eventSourceSessionId = sessionId;
    } catch (_) {
      state.streamConnected = false;
      renderStatus();
    }
  }

  async function respondPermissionRequest(permissionId, responseKind) {
    const sid = String(state.sessionId || "").trim();
    const pid = String(permissionId || "").trim();
    const reply = String(responseKind || "").trim();
    if (!sid || !pid || !reply) return;
    await fetchJson(
      "/api/agent/session/" +
        encodeURIComponent(sid) +
        "/permissions/" +
        encodeURIComponent(pid),
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ response: reply }),
      },
    );
    setInlineNote("权限请求已处理：permission_id=" + pid + "，response=" + reply);
  }

  function applyHostEvent(event) {
    if (!event || typeof event !== "object") return;
    const kind = String(event.kind || "");
    if (!kind) return;
    if (kind === "session_status") {
      const st = String(event.status || "");
      if (st === "connected") {
        state.streamConnected = true;
      }
      if (state.sending && (st === "connected" || st === "heartbeat")) {
        markGenerationActivity();
      }
      if (st === "agent_unavailable" || st === "upstream_unavailable") {
        state.streamConnected = false;
        closeEventStream();
        if (state.sending) {
          finishSending({ restoreDraft: true });
        }
      }
      renderStatus();
      return;
    }
    if (
      kind === "message_info" ||
      kind === "message_part_upsert" ||
      kind === "message_part_delta" ||
      kind === "message_part_removed"
    ) {
      if (kind === "message_part_delta") {
        recordDeltaDebugEvent(event);
      }
      markGenerationActivity();
      refreshMessages().catch(function () {});
      return;
    }
    if (kind === "permission_requested") {
      markGenerationActivity();
      const notice = blockedPermissionNoticeFromData(event);
      rememberBlockedPermissionNotice(notice);
      setInlineNote(
        "内置助手请求目录访问权限：" + String(notice.path || notice.permission || "unknown") + "（请在管理页批准或拒绝）",
      );
      return;
    }
    if (kind === "permission_blocked") {
      markGenerationActivity();
      const notice = blockedPermissionNoticeFromData(event);
      rememberBlockedPermissionNotice(notice);
      setInlineNote(String(notice.message || "会话触发了未授权访问，已自动拒绝。"));
      return;
    }
    if (kind === "permission_resolved") {
      markGenerationActivity();
      setInlineNote(
        "权限请求已自动处理：permission_id=" +
          String(event.permission_id || "") +
          "，response=" +
          String(event.response || ""),
      );
    }
  }

  function rememberSession() {
    try {
      if (state.sessionId) {
        localStorage.setItem(sessionStorageKey(), state.sessionId);
      } else {
        localStorage.removeItem(sessionStorageKey());
      }
    } catch (_) {}
  }

  function restoreSession() {
    state.sessionId = "";
    try {
      const saved = localStorage.getItem(sessionStorageKey());
      if (saved) state.sessionId = saved;
    } catch (_) {}
  }

  async function refreshAll() {
    let refreshFailed = false;
    const previousTargetKey = String(state.sessionTargetKey || "");
    state.loading = true;
    setButtonState(true);
    renderStatus();
    try {
      const [config, runtime, skillStatus] = await Promise.all([
        fetchJson("/api/agent/config"),
        fetchJson("/api/agent/runtime"),
        fetchJson("/api/agent/skill"),
      ]);
      state.config = config;
      state.runtime = runtime;
      state.skillStatus = skillStatus;
      state.sessionTargetKey = currentTargetKey();
      if (state.sessionTargetKey !== previousTargetKey) {
        state._meiAutoSessionOnce = false;
      }
      let runtimeRef = runtime;
      if (runtimeRef && runtimeRef.running) {
        try {
          state.health = await fetchJson("/api/agent/health");
        } catch (_) {
          state.health = null;
        }
        try {
          state.sessions = await fetchAllSessions({ skipCache: true });
          if (
            state.sessionId &&
            state.sessions.length > 0 &&
            !sessionIdInList(state.sessions, state.sessionId)
          ) {
            state.sessions = await fetchAllSessions({ skipCache: true });
          }
        } catch (_) {
          state.sessions = [];
        }
      } else {
        state.health = null;
        state.sessions = [];
      }
    } catch (error) {
      refreshFailed = true;
      state.health = null;
      state.sessions = [];
      state.skillStatus = null;
      setInlineNote("读取助手状态失败：" + String(error.message || error));
    } finally {
      state.loading = false;
      setButtonState(false);
      renderStatus();
      renderConfig();
      renderRuntime();
      renderSkillStatus();
      await refreshModelProbe(true).catch(function () {});
      await refreshContextPreview().catch(function () {});
      const boundSessions = listBoundSessionsForTarget(state.sessions, state.sessionTargetKey);
      if (state.sessionId && !sessionIdInList(state.sessions, state.sessionId)) {
        state.sessionId = "";
        state.messages = [];
        state.lastMessagesFingerprint = "";
        clearDeltaDebugLog();
        resetPendingPermissionState();
        rememberSession();
      }
      if (!state.sessionId && boundSessions.length > 0) {
        const savedId = String(localStorage.getItem(sessionStorageKey()) || "").trim();
        const saved = savedId ? boundSessions.find(function (item) { return item.id === savedId; }) : null;
        const preferred = saved || boundSessions[0];
        state.sessionId = preferred ? preferred.id : "";
        resetPendingPermissionState();
      }
      if (
        !state._meiAutoSessionOnce &&
        state.health &&
        state.health.healthy &&
        !state.sessionId
      ) {
        const forTarget = listBoundSessionsForTarget(state.sessions, state.sessionTargetKey);
        if (forTarget.length === 0) {
          state._meiAutoSessionOnce = true;
          await postNewBoundSession().catch(function () {});
          return;
        }
      }
      renderSessions();
      syncSourceDiffEntry();
      restoreDeltaDebugLog(state.sessionId);
      if (state.health && state.health.healthy && state.sessionId) {
        try {
          await refreshMessages({ forcePendingPermissions: true });
        } catch (_) {
          refreshFailed = true;
        }
        connectEvents(false);
      } else {
        closeEventStream();
        renderMessages();
      }
    }
    return !refreshFailed;
  }

  async function startServer() {
    setInlineNote("");
    await refreshAll();
  }

  function buildSessionTitle() {
    return buildBoundSessionTitle(currentTargetKey());
  }

  async function postNewBoundSession() {
    state.sessionTargetKey = currentTargetKey();
    const session = await fetchJson("/api/agent/session", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ title: buildSessionTitle() }),
    });
    state.sessionId = session.id || "";
    clearDeltaDebugLog({ dropPersisted: true });
    resetPendingPermissionState();
    rememberSession();
    invalidateSessionCache();
    await refreshAll();
  }

  async function createSession() {
    const healthy = !!(state.health && state.health.healthy);
    if (!healthy) {
      setInlineNote("助手暂不可用；请检查服务端 QWEN_BASE_URL、QWEN_API_KEY、QWEN_COMPLETION_MODEL 等配置。");
      return;
    }
    await postNewBoundSession();
  }

  async function refreshMessages(options) {
    const opts = options || {};
    if (!state.sessionId || !(state.health && state.health.healthy)) {
      closeEventStream();
      clearDeltaDebugLog();
      state.lastMessagesFingerprint = "";
      resetPendingPermissionState();
      state.progress = {
        visible: false,
        label: "",
        detail: "",
        items: [],
      };
      renderProgressStrip();
      renderMessages();
      return;
    }
    const payload = await fetchJson(
      "/api/agent/session/" +
        encodeURIComponent(state.sessionId) +
        "/messages?limit=80",
    );
    const list = payload && Array.isArray(payload.messages) ? payload.messages : [];
    const nextFingerprint = String(state.sessionId) + "|" + JSON.stringify(list);
    state.progress = deriveProgressFromMessages(list);
    renderProgressStrip();
    const runningBlocked = deriveBlockedNoticesFromRawMessages(list);
    const runningBlockedFingerprint = blockedPermissionFingerprint(runningBlocked);
    const shouldBootstrapPendingPermissions =
      opts.forcePendingPermissions === true &&
      state.pendingPermissionsBootstrappedSessionId !== String(state.sessionId || "");
    const shouldRefreshPendingPermissions =
      shouldBootstrapPendingPermissions ||
      (
        !!runningBlockedFingerprint &&
        runningBlockedFingerprint !== state.pendingPermissionsFingerprint
      );
    if (nextFingerprint === state.lastMessagesFingerprint && !shouldRefreshPendingPermissions) {
      state.pendingPermissionsFingerprint = runningBlockedFingerprint;
      return;
    }
    let pendingBlocked = Array.isArray(state.pendingPermissionNotices)
      ? state.pendingPermissionNotices.slice()
      : [];
    if (shouldRefreshPendingPermissions) {
      try {
        const pendingPayload = await fetchJson(
          "/api/agent/session/" +
            encodeURIComponent(state.sessionId) +
            "/permissions/pending",
        );
        const pending = pendingPayload && Array.isArray(pendingPayload.pending)
          ? pendingPayload.pending
          : [];
        pendingBlocked = pending.map(blockedPermissionNoticeFromData);
      } catch (_) {
        pendingBlocked = [];
      }
      state.pendingPermissionsFetchedAt = Date.now();
      state.pendingPermissionNotices = pendingBlocked.slice();
      if (shouldBootstrapPendingPermissions) {
        state.pendingPermissionsBootstrappedSessionId = String(state.sessionId || "");
      }
    }
    state.pendingPermissionsFingerprint = runningBlockedFingerprint;
    state.lastMessagesFingerprint = nextFingerprint;
    list.forEach(function (raw) {
      const inferred = inferAgentModeFromRawMessage(raw);
      const messageId = String(raw && raw.message_id ? raw.message_id : "");
      if (!inferred || !messageId) return;
      const meta = getMessageMeta(state.sessionId, messageId);
      if (!meta || !meta.agent) {
        setMessageMeta(messageId, { agent: inferred, hasDiff: null, reverted: false });
      }
    });
    state.messages = list.map(normalizeMessage);
    syncSourceDiffEntry();
    const mergedBlocked = mergeBlockedPermissionNotices(pendingBlocked, runningBlocked);
    applyBlockedPermissionNotices(mergedBlocked);
    decorateMessageActions();
    renderMessages();
    await hydrateBuildDiffMeta(state.messages);
    if (activeGenerationFinished(list)) {
      finishSending();
    }
  }

  function summarizePromptError(error) {
    if (!error) return "";
    if (typeof error === "string") return error;
    if (error && typeof error === "object") {
      if (error.data && typeof error.data.message === "string") {
        return error.data.message;
      }
      if (typeof error.message === "string") {
        return error.message;
      }
    }
    try {
      return JSON.stringify(error);
    } catch (_) {
      return String(error);
    }
  }

  async function stopSending() {
    if (!state.sending || state.aborting) return;
    state.aborting = true;
    setButtonState(false);
    try {
      if (state.sendAbortController) {
        state.sendAbortController.abort();
      }
      if (state.sessionId) {
        await fetchJson(
          "/api/agent/session/" + encodeURIComponent(state.sessionId) + "/abort",
          {
            method: "POST",
            headers: { "content-type": "application/json" },
          },
        );
      }
      await refreshMessages().catch(function () {});
      finishSending({ restoreDraft: true });
    } catch (error) {
      setInlineNote("停止失败：" + String(error.message || error));
      state.aborting = false;
      setButtonState(false);
    }
  }

  async function postPromptWithCurrentSession(text, controller) {
    const body = {
      text: text,
      app_id: String(root.dataset.app || ""),
      scene_id: currentSceneId(),
      entry_id: String(root.dataset.entry || ""),
      target_file: currentTargetKey(),
      mode: normalizeAgentMode(state.agentMode),
      route_mode: normalizeRouteMode(root.dataset.mode),
      agent: normalizeAgentMode(state.agentMode),
    };
    const mref = getSelectedCompletionModelRef();
    if (mref) {
      body.model = { providerID: mref.provider_id, modelID: mref.model_id };
    }
    return fetchJson(
      "/api/agent/session/" + encodeURIComponent(state.sessionId) + "/message",
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
        signal: controller.signal,
      },
    );
  }

  async function sendPrompt() {
    if (state.sending) {
      await stopSending();
      return;
    }
    const draftText = els.input && els.input.value ? String(els.input.value) : "";
    const text = draftText.trim();
    if (!text) {
      if (els.input) els.input.focus();
      return;
    }
    const healthy = !!(state.health && state.health.healthy);
    if (!healthy) {
      setInlineNote("助手未就绪；请检查 QWEN_* 配置或点击“重连”。");
      return;
    }
    if (!state.sessionId) {
      await createSession();
      if (!state.sessionId) {
        return;
      }
    }
    state.sessionTargetKey = currentTargetKey();
    state.sending = true;
    state.aborting = false;
    state.pendingPromptDraft = draftText;
    state.progress = {
      visible: true,
      label: normalizeAgentMode(state.agentMode) === "ask" ? "问答处理中" : "脚本生成中",
      detail: normalizeAgentMode(state.agentMode) === "ask" ? "等待回答输出" : "等待执行输出",
      items: [
        {
          label: normalizeAgentMode(state.agentMode) === "ask" ? "问答中" : "生成中",
          status: "running",
        },
      ],
    };
    renderProgressStrip();
    clearGenerationSettleTimer();
    if (els.input) {
      els.input.value = "";
      autoResizeComposerInput();
      els.input.focus();
    }
    setButtonState(false);
    try {
      const controller = new AbortController();
      state.sendAbortController = controller;
      let summary;
      try {
        summary = await postPromptWithCurrentSession(text, controller);
      } catch (error) {
        if (!isNotFoundError(error)) {
          throw error;
        }
        state.sendAbortController = null;
        state.sessionId = "";
        state.messages = [];
        state.lastMessagesFingerprint = "";
        resetPendingPermissionState();
        rememberSession();
        invalidateSessionCache();
        setInlineNote("检测到旧会话已失效，正在自动重建会话后重试…");
        await createSession();
        if (!state.sessionId) {
          throw error;
        }
        const retryController = new AbortController();
        state.sendAbortController = retryController;
        summary = await postPromptWithCurrentSession(text, retryController);
      }
      state.sendAbortController = null;
      if (summary && summary.error) {
        const detail = summarizePromptError(summary.error);
        setInlineNote("发送失败：" + (detail || "上游模型返回错误"));
        await refreshMessages().catch(function () {});
        finishSending({ restoreDraft: true });
        return;
      }
      if (summary && summary.message_id) {
        state.activeGenerationMessageId = String(summary.message_id);
        setMessageMeta(summary.message_id, {
          agent: normalizeAgentMode(state.agentMode),
          hasDiff: normalizeAgentMode(state.agentMode) === "build" ? null : false,
          reverted: false,
        });
        if (normalizeAgentMode(state.agentMode) === "build") {
          state.pendingReloadMessageId = String(summary.message_id);
        }
      }
      await refreshMessages();
      finishSending();
      if (summary && summary.message_id && normalizeAgentMode(state.agentMode) === "build") {
        try {
          const diff = await fetchSessionDiff(summary.message_id);
          const hasDiff = sessionDiffHasMaterialChanges(diff);
          setMessageMeta(summary.message_id, { hasDiff: hasDiff });
          if (hasDiff) {
            state.messageDiffCache[diffCacheKey(state.sessionId, summary.message_id)] = diff;
            decorateMessageActions();
            renderMessages();
            scheduleHostReload("Build 已修改文件，正在刷新预览与源码…");
          }
        } catch (_) {}
      }
      renderHistoryButtons();
      connectEvents(false);
      markGenerationActivity();
    } catch (error) {
      const aborted = state.aborting || (error && error.name === "AbortError");
      state.sendAbortController = null;
      if (aborted) {
        return;
      }
      setInlineNote("发送失败：" + String(error.message || error));
      finishSending({ restoreDraft: true });
    }
  }

  if (els.reconnect) {
    els.reconnect.addEventListener("click", function () {
      refreshAll().catch(function (error) {
        setInlineNote("重连失败：" + String(error.message || error));
      });
    });
  }

  if (els.newSession) {
    els.newSession.addEventListener("click", function () {
      createSession().catch(function (error) {
        setInlineNote("创建会话失败：" + String(error.message || error));
      });
    });
  }

  if (els.sessionSelect) {
    const onSessionSelectChange = function () {
      state.sessionId = String(els.sessionSelect.value || "");
      restoreDeltaDebugLog(state.sessionId);
      state.sessionTargetKey = currentTargetKey();
      resetPendingPermissionState();
      rememberSession();
      refreshMessages().catch(function (error) {
        setInlineNote("读取会话失败：" + String(error.message || error));
      });
      connectEvents(true);
    };
    els.sessionSelect.addEventListener("sl-change", onSessionSelectChange);
    els.sessionSelect.addEventListener("change", onSessionSelectChange);
  }

  if (els.run) {
    els.run.addEventListener("click", function () {
      sendPrompt().catch(function (error) {
        setInlineNote("发送失败：" + String(error.message || error));
      });
    });
  }

  if (els.contextRefresh) {
    els.contextRefresh.addEventListener("click", function () {
      refreshContextPreview(true).catch(function (error) {
        setInlineNote("刷新上下文预览失败：" + String(error.message || error));
      });
    });
  }

  if (els.input) {
    els.input.addEventListener("input", function () {
      autoResizeComposerInput();
      renderRunButton(state.loading);
    });
    els.input.addEventListener("keydown", function (event) {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        sendPrompt().catch(function (error) {
          setInlineNote("发送失败：" + String(error.message || error));
        });
      }
    });
    autoResizeComposerInput();
  }

  const onComposerInputWindowResize = function () {
    autoResizeComposerInput();
    sizeCompletionModelSelectWidth();
    if (
      isAccessFloatingMode() &&
      els.accessFloatingRoot &&
      els.accessFloatingRoot.dataset.positioned === "true"
    ) {
      const rect = els.accessFloatingRoot.getBoundingClientRect();
      const pos = applyAccessFloatingPosition(rect.left, rect.top);
      if (pos) rememberAccessFloatingPosition(pos.left, pos.top);
    }
  };
  window.addEventListener("resize", onComposerInputWindowResize);

  if (els.modeAsk) {
    els.modeAsk.addEventListener("click", function () {
      switchAgentMode("ask");
    });
  }

  if (els.modeBuild) {
    els.modeBuild.addEventListener("click", function () {
      switchAgentMode("build");
    });
  }

  if (els.completionModelSelect) {
    els.completionModelSelect.addEventListener("change", function () {
      rememberSelectedCompletionModel(els.completionModelSelect.value);
      syncModelLabelFromCompletionSelect();
      sizeCompletionModelSelectWidth();
      refreshModelProbe(true).catch(function () {});
    });
  }

  if (els.accessFab) {
    els.accessFab.addEventListener("click", function () {
      if (state.accessFloatingDragMoved) {
        state.accessFloatingDragMoved = false;
        return;
      }
      toggleAccessFloatingPanel();
    });
    els.accessFab.addEventListener("pointerdown", beginAccessFloatingDrag);
  }

  if (els.accessClose) {
    els.accessClose.addEventListener("click", function () {
      toggleAccessFloatingPanel(false);
    });
  }

  const onAccessFloatingEscape = function (event) {
    if (!isAccessFloatingMode()) return;
    if (event && event.key === "Escape" && state.accessFloatingOpen) {
      toggleAccessFloatingPanel(false);
    }
  };
  document.addEventListener("keydown", onAccessFloatingEscape);
  document.addEventListener("pointermove", continueAccessFloatingDrag);
  document.addEventListener("pointerup", endAccessFloatingDrag);
  document.addEventListener("pointercancel", endAccessFloatingDrag);

  if (els.sourceViewDiffBtn) {
    els.sourceViewDiffBtn.addEventListener("click", function () {
      if (currentManageTab() !== "diff") {
        setManageTab("diff");
        return;
      }
      if (!state.latestDiffMessageId) {
        setInlineNote("最后一轮 Build 生成改动后才可查看差异。");
        return;
      }
      inspectDiffForMessage(state.latestDiffMessageId).catch(function (error) {
        setInlineNote("读取差异失败：" + String(error.message || error));
      });
    });
  }

  if (els.undo) {
    els.undo.addEventListener("click", function () {
      const messageId = latestUndoMessageId();
      if (!messageId) return;
      applyRevertForMessage(messageId).catch(function (error) {
        setInlineNote("撤回失败：" + String(error.message || error));
      });
    });
  }

  if (els.redo) {
    els.redo.addEventListener("click", function () {
      if (!canRedo()) return;
      applyUnrevertForSession().catch(function (error) {
        setInlineNote("恢复失败：" + String(error.message || error));
      });
    });
  }

  const onManageTabChange = function (event) {
    const nextTab =
      event && event.detail && typeof event.detail.tab === "string"
        ? event.detail.tab
        : currentManageTab();
    applyManageTabMode(nextTab);
  };
  document.addEventListener("mei:manage-tab-change", onManageTabChange);

  const onManageContextChange = function (event) {
    const detail = event && event.detail && typeof event.detail === "object"
      ? event.detail
      : {};
    if (detail && typeof detail.app === "string") {
      root.dataset.app = detail.app;
    }
    if (detail && typeof detail.scene === "string") {
      root.dataset.scene = detail.scene;
    }
    if (detail && typeof detail.target === "string") {
      root.dataset.target = detail.target;
    }
    if (detail && typeof detail.entry === "string") {
      root.dataset.entry = detail.entry;
    }
    if (detail && typeof detail.entryTarget === "string") {
      root.dataset.entryTarget = detail.entryTarget;
    }
    if (detail && typeof detail.mode === "string") {
      root.dataset.mode = detail.mode;
    }
    if (detail && typeof detail.sourceViews === "string") {
      root.dataset.sourceViews = detail.sourceViews;
    }
    if (detail && typeof detail.viewTab === "string") {
      root.dataset.viewTab = detail.viewTab;
    }
    state.contextPreview = null;
    state.contextPreviewBackoffUntilMs = 0;
    state.contextPreviewScopeKey = "";
    state.contextPreviewFetchedAtMs = 0;
    state.modelProbe = null;
    state.modelProbeFetchedAtMs = 0;
    state._meiAutoSessionOnce = false;
    renderContextPreview();
    destroySourceDiffView();
    destroySourceEditor();
    refreshLinkedViewRefs();
    restoreAccessFloatingPanel();
    ensureSourceEditor();
    applyManageTabMode(currentManageTab());
    root.classList.add("is-soft-refresh");
    restoreRevertedState();
    restoreAgentMode();
    restoreSession();
    restoreDeltaDebugLog(state.sessionId);
    refreshAll().catch(function (error) {
      setInlineNote("刷新作者助手面板失败：" + String(error.message || error));
    }).finally(function () {
      renderDeltaDebugLog();
      window.setTimeout(function () {
        root.classList.remove("is-soft-refresh");
      }, 80);
    });
  };
  document.addEventListener("mei:manage-context-change", onManageContextChange);

  restoreRevertedState();
  restoreAgentMode();
  restoreAccessFloatingPanel();
  restoreSession();
  restoreDeltaDebugLog(state.sessionId);
  const initialTab = currentManageTab();
  initSourceEditor();
  renderSourceViewMode(initialTab === "diff" ? "diff" : "source");
  renderProgressStrip();
  renderContextPreview();
  syncSourceDiffEntry();
  refreshAll()
    .then(function () {
      if (initialTab !== "diff") return;
      if (!state.latestDiffMessageId) {
        return;
      }
      inspectDiffForMessage(state.latestDiffMessageId).catch(function (error) {
        setInlineNote("读取差异失败：" + String(error.message || error));
      });
    })
    .catch(function () {})
    .finally(function () {
      renderDeltaDebugLog();
    });
  const beforeUnloadHandler = function () {
    closeEventStream();
  };
  window.addEventListener("beforeunload", beforeUnloadHandler);
  const POLL_ACTIVE_MS = 30000;
  const POLL_IDLE_MS = 120000;
  const POLL_STREAM_HEALTHY_MS = 180000;
  const POLL_MAX_MS = 300000;
  let refreshTimerId = 0;
  let refreshPollFailureCount = 0;
  let refreshPollInFlight = false;
  function currentBasePollDelayMs() {
    const hasActiveGeneration = Boolean(
      state.sending || state.loading || state.streamConnected || state.activeGenerationMessageId,
    );
    return hasActiveGeneration ? POLL_ACTIVE_MS : POLL_IDLE_MS;
  }
  function nextRefreshPollDelayMs() {
    const base = currentBasePollDelayMs();
    return Math.min(POLL_MAX_MS, base * Math.pow(2, refreshPollFailureCount));
  }
  function rightSidebarCollapsed() {
    const workspaceRoot = document.getElementById("workspace-root");
    return !!(workspaceRoot && workspaceRoot.dataset.rightCollapsed === "true");
  }
  function shouldPausePolling() {
    if (document.visibilityState === "hidden") return true;
    if (rightSidebarCollapsed()) return true;
    return false;
  }
  function scheduleRefreshPoll(delayMs) {
    if (refreshTimerId) {
      window.clearTimeout(refreshTimerId);
    }
    refreshTimerId = window.setTimeout(
      runRefreshPoll,
      Math.max(1000, Number(delayMs) || currentBasePollDelayMs()),
    );
  }
  async function runRefreshPoll() {
    if (refreshPollInFlight) {
      scheduleRefreshPoll(nextRefreshPollDelayMs());
      return;
    }
    if (shouldPausePolling()) {
      scheduleRefreshPoll(Math.max(currentBasePollDelayMs(), nextRefreshPollDelayMs()));
      return;
    }
    if (
      state.streamConnected &&
      state.health &&
      state.health.healthy &&
      !state.sending &&
      !state.loading
    ) {
      scheduleRefreshPoll(POLL_STREAM_HEALTHY_MS);
      return;
    }
    refreshPollInFlight = true;
    try {
      const ok = await refreshAll().catch(function () { return false; });
      if (ok) {
        refreshPollFailureCount = 0;
      } else {
        refreshPollFailureCount = Math.min(refreshPollFailureCount + 1, 4);
      }
    } finally {
      refreshPollInFlight = false;
      scheduleRefreshPoll(nextRefreshPollDelayMs());
    }
  }
  scheduleRefreshPoll(currentBasePollDelayMs());
  boot.disposeAgentPanel = function () {
    closeEventStream();
    document.removeEventListener("mei:manage-tab-change", onManageTabChange);
    document.removeEventListener("mei:manage-context-change", onManageContextChange);
    document.removeEventListener("keydown", onAccessFloatingEscape);
    document.removeEventListener("pointermove", continueAccessFloatingDrag);
    document.removeEventListener("pointerup", endAccessFloatingDrag);
    document.removeEventListener("pointercancel", endAccessFloatingDrag);
    window.removeEventListener("beforeunload", beforeUnloadHandler);
    window.removeEventListener("resize", onComposerInputWindowResize);
    if (els.accessFab) {
      els.accessFab.removeEventListener("pointerdown", beginAccessFloatingDrag);
    }
    if (refreshTimerId) window.clearTimeout(refreshTimerId);
    if (state._completionModelMeasure && state._completionModelMeasure.parentNode) {
      try {
        state._completionModelMeasure.parentNode.removeChild(state._completionModelMeasure);
      } catch (_) {}
    }
    state._completionModelMeasure = null;
  };
})();
