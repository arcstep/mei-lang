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
  const CHAT_BOTTOM_STICKY_THRESHOLD_PX = 28;
  const COMPOSER_MIN_ROWS = 2;
  const COMPOSER_MAX_ROWS = 12;

  const $U = window.MeiAgentPanelUtils;
  if (
    !$U ||
    typeof $U.escapeHtml !== "function" ||
    typeof $U.fetchJson !== "function" ||
    typeof $U.CHAT_CLASS !== "object" ||
    typeof $U.chatMessageRoleClass !== "function"
  ) {
    console.error(
      "MeiAgentPanelUtils missing: ensure agent-panel-utils.js is bundled before agent-panel.js",
    );
    return;
  }

  const RT =
    typeof window.__meiAgentPanelInstallRouting === "function"
      ? window.__meiAgentPanelInstallRouting({ root: root, boot: boot, $U: $U })
      : null;
  if (!RT || typeof RT.currentTargetKey !== "function") {
    console.error(
      "MeiAgentPanelRouting missing: ensure agent-panel-routing.js is bundled before agent-panel.js",
    );
    return;
  }

  function normalizeDeltaDebugRows(rows) {
    return $U.normalizeDeltaDebugRows(rows);
  }

  function writeDeltaDebugLogToStorage(sessionId, rows) {
    if (!window.sessionStorage) return;
    const key = RT.deltaDebugStorageKey(sessionId);
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
    const key = RT.deltaDebugStorageKey(sessionId);
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

  const AF =
    typeof window.__meiAgentPanelInstallAccessFloat === "function"
      ? window.__meiAgentPanelInstallAccessFloat({
          root,
          els,
          state,
          normalizeRouteMode: RT.normalizeRouteMode,
          accessFloatingStorageKey: RT.accessFloatingStorageKey,
          accessFloatingPositionStorageKey: RT.accessFloatingPositionStorageKey,
        })
      : null;
  if (!AF || typeof AF.beginAccessFloatingDrag !== "function") {
    console.error(
      "MeiAgentPanelAccessFloat missing: ensure agent-panel-access-float.js is bundled before agent-panel.js",
    );
    return;
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
    return $U.normalizeFilePath(value);
  }

  function sourceTargetKey() {
    refreshLinkedViewRefs();
    const targetNode = els.sourceViewSourceRaw || els.sourceViewSourcePanel;
    if (targetNode && targetNode.dataset && targetNode.dataset.sourceTarget) {
      return normalizeFilePath(targetNode.dataset.sourceTarget);
    }
    return RT.currentTargetKey();
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

  function latestRoundAssistantMessageId() {
    const rounds = $U.conversationRounds(state.messages);
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
      localStorage.setItem(RT.revertedStorageKey(), JSON.stringify(state.revertedMessageIds));
    } catch (_) {}
  }

  function restoreRevertedState() {
    state.revertedMessageIds = {};
    state.sessionHasRevertedChanges = {};
    try {
      const raw = localStorage.getItem(RT.revertedStorageKey());
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
    const rounds = $U.conversationRounds(state.messages);
    for (let index = rounds.length - 1; index >= 0; index -= 1) {
      const round = rounds[index];
      const assistants = round && Array.isArray(round.assistants) ? round.assistants : [];
      const message = assistants.length ? assistants[assistants.length - 1] : null;
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
    const agent = RT.normalizeAgentMode(meta.agent || state.agentMode);
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

  let SES = null;

  const chromeApi = {
    root: root,
    els: els,
    state: state,
    $U: $U,
    historyUnavailableReason: historyUnavailableReason,
    canUndo: canUndo,
    canRedo: canRedo,
    canSubmitPrompt: canSubmitPrompt,
    autoResizeComposerInput: autoResizeComposerInput,
    normalizeAgentMode: RT.normalizeAgentMode,
    modeStorageKey: RT.modeStorageKey,
    writeDeltaDebugLogToStorage: writeDeltaDebugLogToStorage,
    renderDeltaDebugLog: renderDeltaDebugLog,
    MODEL_PROBE_RED_AFTER_MS: MODEL_PROBE_RED_AFTER_MS,
    MODEL_PROBE_RED_AFTER_STREAK: MODEL_PROBE_RED_AFTER_STREAK,
    MODEL_PROBE_COLD_START_RED_AFTER_STREAK: MODEL_PROBE_COLD_START_RED_AFTER_STREAK,
    formatMsTimeForSkill: function (v) {
      return chromeApi._fmtSkillMs(v);
    },
    _fmtSkillMs: function () {
      return "";
    },
  };

  const CHR =
    typeof window.__meiAgentPanelInstallChrome === "function"
      ? window.__meiAgentPanelInstallChrome(chromeApi)
      : null;
  if (!CHR || typeof CHR.renderStatus !== "function") {
    console.error(
      "MeiAgentPanelChrome missing: ensure agent-panel-chrome.js is bundled before agent-panel.js",
    );
    return;
  }


  const SRC =
    typeof window.__meiAgentPanelInstallSourceView === "function"
      ? window.__meiAgentPanelInstallSourceView({
          root,
          els,
          state,
          refreshLinkedViewRefs,
          setInlineNote: CHR.setInlineNote,
          currentManageTab: RT.currentManageTab,
          renderDeltaDebugLog,
          fetchSessionDiff,
          sessionDiffHasMaterialChanges,
          setMessageMeta,
          diffCacheKey,
          latestRoundAssistantMessageId,
          latestDiffEligibleMessageId,
          historyUnavailableReason,
          normalizeFilePath,
          sourceTargetKey,
          sourceLanguage,
          sourceRawText,
        })
      : null;
  if (!SRC || typeof SRC.ensureSourceEditor !== "function") {
    console.error(
      "MeiAgentPanelSourceView missing: ensure agent-panel-source.js is bundled before agent-panel.js",
    );
    return;
  }

  function scheduleHostReload(reason) {
    const text = String(reason || "").trim();
    if (text) CHR.setInlineNote(text);
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
  const CTX =
    typeof window.__meiAgentPanelInstallContextPreview === "function"
      ? window.__meiAgentPanelInstallContextPreview({
          root: root,
          els: els,
          state: state,
          $U: $U,
          setInlineNote: CHR.setInlineNote,
          currentAppKey: RT.currentAppKey,
          currentSceneId: RT.currentSceneId,
          currentTargetKey: RT.currentTargetKey,
          normalizeTargetKey: RT.normalizeTargetKey,
          normalizeRouteMode: RT.normalizeRouteMode,
          normalizeAgentMode: RT.normalizeAgentMode,
          getSelectedCompletionModelRef: CHR.getSelectedCompletionModelRef,
          renderDeltaDebugLog: renderDeltaDebugLog,
          renderStatusBarOpenCode: CHR.renderStatusBarOpenCode,
        })
      : null;
  if (!CTX || typeof CTX.renderContextPreview !== "function") {
    console.error(
      "MeiAgentPanelContextPreview missing: ensure agent-panel-context.js is bundled before agent-panel.js",
    );
    return;
  }

  chromeApi._fmtSkillMs = function (v) {
    return CTX.formatMsTime(v);
  };

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


  async function fetchSessionDiff(messageId) {
    if (!state.sessionId) return null;
    const params = new URLSearchParams();
    const mid = String(messageId || "").trim();
    if (mid) params.set("message_id", mid);
    const pathKey = sourceTargetKey();
    if (pathKey) params.set("path", pathKey);
    const qs = params.toString();
    return $U.fetchJson(
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


  const transport = { ses: null };
  const msgApi = {
    transport: transport,
    root: root,
    els: els,
    state: state,
    $U: $U,
    CHR: CHR,
    CTX: CTX,
    SRC: SRC,
    SESSION_CACHE_KEY: SESSION_CACHE_KEY,
    SESSION_CACHE_TTL_MS: SESSION_CACHE_TTL_MS,
    CHAT_BOTTOM_STICKY_THRESHOLD_PX: CHAT_BOTTOM_STICKY_THRESHOLD_PX,
    fetchSessionDiff: fetchSessionDiff,
    sessionDiffHasMaterialChanges: sessionDiffHasMaterialChanges,
    deriveProgressFromMessages: deriveProgressFromMessages,
    conversationRounds: $U.conversationRounds,
    sessionStorageKey: RT.sessionStorageKey,
    currentSessionBindingFingerprint: RT.currentSessionBindingFingerprint,
    normalizeRouteMode: RT.normalizeRouteMode,
    normalizeAgentMode: RT.normalizeAgentMode,
    buildBoundSessionTitle: RT.buildBoundSessionTitle,
    parseBoundSessionTitle: RT.parseBoundSessionTitle,
    historyUnavailableReason: historyUnavailableReason,
    scheduleHostReload: scheduleHostReload,
    autoResizeComposerInput: autoResizeComposerInput,
    isNotFoundError: isNotFoundError,
    restoreDeltaDebugLog: restoreDeltaDebugLog,
    revertedIdsForSession: revertedIdsForSession,
    setRevertedIdsForSession: setRevertedIdsForSession,
    setSessionRevertedFlag: setSessionRevertedFlag,
    isMessageReverted: isMessageReverted,
    getMessageMeta: getMessageMeta,
    setMessageMeta: setMessageMeta,
    diffCacheKey: diffCacheKey,
    normalizeTargetKey: RT.normalizeTargetKey,
    sessionBindingKind: RT.sessionBindingKind,
    currentSceneId: RT.currentSceneId,
    currentTargetKey: RT.currentTargetKey,
  };
  const MSG =
    typeof window.__meiAgentPanelInstallMessages === "function"
      ? window.__meiAgentPanelInstallMessages(msgApi)
      : null;
  if (!MSG || typeof MSG.refreshMessages !== "function") {
    console.error(
      "MeiAgentPanelMessages missing: ensure agent-panel-messages.js is bundled before agent-panel.js",
    );
    return;
  }

  SES =
    typeof window.__meiAgentPanelInstallSession === "function"
      ? window.__meiAgentPanelInstallSession({
          state: state,
          renderStatus: CHR.renderStatus,
          clearGenerationSettleTimer: CHR.clearGenerationSettleTimer,
          markGenerationActivity: CHR.markGenerationActivity,
          finishSending: CHR.finishSending,
          recordDeltaDebugEvent: recordDeltaDebugEvent,
          refreshMessages: MSG.refreshMessages,
          blockedPermissionNoticeFromData: MSG.blockedPermissionNoticeFromData,
          rememberBlockedPermissionNotice: MSG.rememberBlockedPermissionNotice,
          setInlineNote: CHR.setInlineNote,
          refreshAll: MSG.refreshAll,
        })
      : null;
  if (!SES || typeof SES.closeEventStream !== "function") {
    console.error(
      "MeiAgentPanelSession missing: ensure agent-panel-session.js is bundled before agent-panel.js",
    );
    return;
  }


  transport.ses = SES;


  if (els.reconnect) {
    els.reconnect.addEventListener("click", function () {
      MSG.refreshAll().catch(function (error) {
        CHR.setInlineNote("重连失败：" + String(error.message || error));
      });
    });
  }

  if (els.newSession) {
    els.newSession.addEventListener("click", function () {
      MSG.createSession().catch(function (error) {
        CHR.setInlineNote("创建会话失败：" + String(error.message || error));
      });
    });
  }

  if (els.sessionSelect) {
    const onSessionSelectChange = function () {
      state.sessionId = String(els.sessionSelect.value || "");
      restoreDeltaDebugLog(state.sessionId);
      state.sessionTargetKey = RT.currentSessionBindingFingerprint();
      MSG.resetPendingPermissionState();
      MSG.rememberSession();
      MSG.refreshMessages().catch(function (error) {
        CHR.setInlineNote("读取会话失败：" + String(error.message || error));
      });
      SES.connectEvents(true);
    };
    els.sessionSelect.addEventListener("sl-change", onSessionSelectChange);
    els.sessionSelect.addEventListener("change", onSessionSelectChange);
  }

  if (els.run) {
    els.run.addEventListener("click", function () {
      MSG.sendPrompt().catch(function (error) {
        CHR.setInlineNote("发送失败：" + String(error.message || error));
      });
    });
  }

  if (els.contextRefresh) {
    els.contextRefresh.addEventListener("click", function () {
      CTX.refreshContextPreview(true).catch(function (error) {
        CHR.setInlineNote("刷新上下文预览失败：" + String(error.message || error));
      });
    });
  }

  const resourceVisibilitySelect = document.getElementById("author-resource-visibility-select");
  if (resourceVisibilitySelect) {
    resourceVisibilitySelect.addEventListener("sl-change", function () {
      state.contextPreviewFetchedAtMs = 0;
      state.contextPreviewScopeKey = "";
      CTX.refreshContextPreview(true).catch(function (error) {
        CHR.setInlineNote("刷新上下文预览失败：" + String(error.message || error));
      });
    });
  }

  if (els.input) {
    els.input.addEventListener("input", function () {
      autoResizeComposerInput();
      CHR.renderRunButton(state.loading);
    });
    els.input.addEventListener("keydown", function (event) {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        MSG.sendPrompt().catch(function (error) {
          CHR.setInlineNote("发送失败：" + String(error.message || error));
        });
      }
    });
    autoResizeComposerInput();
  }

  const onComposerInputWindowResize = function () {
    autoResizeComposerInput();
    CHR.sizeCompletionModelSelectWidth();
    if (
      AF.isAccessFloatingMode() &&
      els.accessFloatingRoot &&
      els.accessFloatingRoot.dataset.positioned === "true"
    ) {
      const rect = els.accessFloatingRoot.getBoundingClientRect();
      const pos = AF.applyAccessFloatingPosition(rect.left, rect.top);
      if (pos) AF.rememberAccessFloatingPosition(pos.left, pos.top);
    }
  };
  window.addEventListener("resize", onComposerInputWindowResize);

  if (els.modeAsk) {
    els.modeAsk.addEventListener("click", function () {
      CHR.switchAgentMode("ask");
    });
  }

  if (els.modeBuild) {
    els.modeBuild.addEventListener("click", function () {
      CHR.switchAgentMode("build");
    });
  }

  if (els.completionModelSelect) {
    els.completionModelSelect.addEventListener("change", function () {
      CHR.rememberSelectedCompletionModel(els.completionModelSelect.value);
      CHR.syncModelLabelFromCompletionSelect();
      CHR.sizeCompletionModelSelectWidth();
      CTX.refreshModelProbe(true).catch(function () {});
    });
  }

  if (els.accessFab) {
    els.accessFab.addEventListener("click", function () {
      if (state.accessFloatingDragMoved) {
        state.accessFloatingDragMoved = false;
        return;
      }
      AF.toggleAccessFloatingPanel();
    });
    els.accessFab.addEventListener("pointerdown", AF.beginAccessFloatingDrag);
  }

  if (els.accessClose) {
    els.accessClose.addEventListener("click", function () {
      AF.toggleAccessFloatingPanel(false);
    });
  }

  const onAccessFloatingEscape = function (event) {
    if (!AF.isAccessFloatingMode()) return;
    if (event && event.key === "Escape" && state.accessFloatingOpen) {
      AF.toggleAccessFloatingPanel(false);
    }
  };
  document.addEventListener("keydown", onAccessFloatingEscape);
  document.addEventListener("pointermove", AF.continueAccessFloatingDrag);
  document.addEventListener("pointerup", AF.endAccessFloatingDrag);
  document.addEventListener("pointercancel", AF.endAccessFloatingDrag);

  if (els.sourceViewDiffBtn) {
    els.sourceViewDiffBtn.addEventListener("click", function () {
      if (RT.currentManageTab() !== "diff") {
        RT.setManageTab("diff");
        return;
      }
      if (!state.latestDiffMessageId) {
        CHR.setInlineNote("最后一轮 Build 生成改动后才可查看差异。");
        return;
      }
      SRC.inspectDiffForMessage(state.latestDiffMessageId).catch(function (error) {
        CHR.setInlineNote("读取差异失败：" + String(error.message || error));
      });
    });
  }

  if (els.undo) {
    els.undo.addEventListener("click", function () {
      const messageId = latestUndoMessageId();
      if (!messageId) return;
      MSG.applyRevertForMessage(messageId).catch(function (error) {
        CHR.setInlineNote("撤回失败：" + String(error.message || error));
      });
    });
  }

  if (els.redo) {
    els.redo.addEventListener("click", function () {
      if (!canRedo()) return;
      MSG.applyUnrevertForSession().catch(function (error) {
        CHR.setInlineNote("恢复失败：" + String(error.message || error));
      });
    });
  }

  const onManageTabChange = function (event) {
    const nextTab =
      event && event.detail && typeof event.detail.tab === "string"
        ? event.detail.tab
        : RT.currentManageTab();
    SRC.applyManageTabMode(nextTab);
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
    const nextFile =
      detail && typeof detail.file === "string"
        ? detail.file
        : detail && typeof detail.target === "string"
          ? detail.target
          : "";
    if (nextFile) {
      root.dataset.file = nextFile;
    }
    if (detail && typeof detail.sceneTarget === "string") {
      root.dataset.sceneTarget = detail.sceneTarget;
    }
    if (detail && typeof detail.entryTarget === "string") {
      root.dataset.sceneTarget = detail.entryTarget;
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
    CTX.renderContextPreview();
    SRC.destroySourceDiffView();
    SRC.destroySourceEditor();
    refreshLinkedViewRefs();
    AF.restoreAccessFloatingPanel();
    SRC.ensureSourceEditor();
    SRC.applyManageTabMode(RT.currentManageTab());
    root.classList.add("is-soft-refresh");
    restoreRevertedState();
    CHR.restoreAgentMode();
    MSG.restoreSession();
    restoreDeltaDebugLog(state.sessionId);
    MSG.refreshAll().catch(function (error) {
      CHR.setInlineNote("刷新作者助手面板失败：" + String(error.message || error));
    }).finally(function () {
      renderDeltaDebugLog();
      window.setTimeout(function () {
        root.classList.remove("is-soft-refresh");
      }, 80);
    });
  };
  document.addEventListener("mei:manage-context-change", onManageContextChange);

  restoreRevertedState();
  CHR.restoreAgentMode();
  AF.restoreAccessFloatingPanel();
  MSG.restoreSession();
  restoreDeltaDebugLog(state.sessionId);
  const initialTab = RT.currentManageTab();
  SRC.initSourceEditor();
  SRC.renderSourceViewMode(initialTab === "diff" ? "diff" : "source");
  CHR.renderProgressStrip();
  CTX.renderContextPreview();
  SRC.syncSourceDiffEntry();
  MSG.refreshAll()
    .then(function () {
      if (initialTab !== "diff") return;
      if (!state.latestDiffMessageId) {
        return;
      }
      SRC.inspectDiffForMessage(state.latestDiffMessageId).catch(function (error) {
        CHR.setInlineNote("读取差异失败：" + String(error.message || error));
      });
    })
    .catch(function () {})
    .finally(function () {
      renderDeltaDebugLog();
    });
  const beforeUnloadHandler = function () {
    SES.closeEventStream();
  };
  window.addEventListener("beforeunload", beforeUnloadHandler);
  SES.startPolling();
  boot.disposeAgentPanel = function () {
    SES.dispose();
    document.removeEventListener("mei:manage-tab-change", onManageTabChange);
    document.removeEventListener("mei:manage-context-change", onManageContextChange);
    document.removeEventListener("keydown", onAccessFloatingEscape);
    document.removeEventListener("pointermove", AF.continueAccessFloatingDrag);
    document.removeEventListener("pointerup", AF.endAccessFloatingDrag);
    document.removeEventListener("pointercancel", AF.endAccessFloatingDrag);
    window.removeEventListener("beforeunload", beforeUnloadHandler);
    window.removeEventListener("resize", onComposerInputWindowResize);
    if (els.accessFab) {
      els.accessFab.removeEventListener("pointerdown", AF.beginAccessFloatingDrag);
    }
    if (state._completionModelMeasure && state._completionModelMeasure.parentNode) {
      try {
        state._completionModelMeasure.parentNode.removeChild(state._completionModelMeasure);
      } catch (_) {}
    }
    state._completionModelMeasure = null;
  };
})();
