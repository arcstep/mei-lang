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

  if (
    typeof window.__meiAgentPanelCollectElements !== "function" ||
    typeof window.__meiAgentPanelCreateInitialState !== "function"
  ) {
    console.error(
      "MeiAgentPanelLayout missing: ensure agent-panel-layout.js is bundled before agent-panel.js",
    );
    return;
  }

  const els = window.__meiAgentPanelCollectElements();
  const state = window.__meiAgentPanelCreateInitialState();

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

  if (typeof window.__meiAgentPanelInstallDeltaDebug !== "function") {
    console.error(
      "MeiAgentPanelDeltaDebug missing: ensure agent-panel-delta-debug.js is bundled before agent-panel.js",
    );
    return;
  }
  const DD = window.__meiAgentPanelInstallDeltaDebug({ els: els, state: state, RT: RT, $U: $U });
  const writeDeltaDebugLogToStorage = DD.writeDeltaDebugLogToStorage;
  const restoreDeltaDebugLog = DD.restoreDeltaDebugLog;
  const renderDeltaDebugLog = DD.renderDeltaDebugLog;
  const recordDeltaDebugEvent = DD.recordDeltaDebugEvent;

  if (typeof window.__meiAgentPanelInstallBindings !== "function") {
    console.error(
      "MeiAgentPanelBindings missing: ensure agent-panel-bindings.js is bundled before agent-panel.js",
    );
    return;
  }
  const B = window.__meiAgentPanelInstallBindings({
    els: els,
    state: state,
    $U: $U,
    RT: RT,
    COMPOSER_MIN_ROWS: COMPOSER_MIN_ROWS,
    COMPOSER_MAX_ROWS: COMPOSER_MAX_ROWS,
  });
  const composerDraftText = B.composerDraftText;
  const refreshLinkedViewRefs = B.refreshLinkedViewRefs;
  const autoResizeComposerInput = B.autoResizeComposerInput;
  const canSubmitPrompt = B.canSubmitPrompt;
  const normalizeFilePath = B.normalizeFilePath;
  const sourceTargetKey = B.sourceTargetKey;
  const sourceLanguage = B.sourceLanguage;
  const sourceRawText = B.sourceRawText;
  const latestRoundAssistantMessageId = B.latestRoundAssistantMessageId;
  const latestDiffEligibleMessageId = B.latestDiffEligibleMessageId;
  const diffCacheKey = B.diffCacheKey;
  const setMessageMeta = B.setMessageMeta;
  const getMessageMeta = B.getMessageMeta;
  const setSessionRevertedFlag = B.setSessionRevertedFlag;
  const hasSessionRevertedChanges = B.hasSessionRevertedChanges;
  const persistRevertedState = B.persistRevertedState;
  const restoreRevertedState = B.restoreRevertedState;
  const revertedIdsForSession = B.revertedIdsForSession;
  const setRevertedIdsForSession = B.setRevertedIdsForSession;
  const isMessageReverted = B.isMessageReverted;
  const latestUndoMessageId = B.latestUndoMessageId;
  const canUndo = B.canUndo;
  const canRedo = B.canRedo;
  const deriveProgressFromMessages = B.deriveProgressFromMessages;
  const fetchSessionDiff = B.fetchSessionDiff;
  const sessionDiffHasMaterialChanges = B.sessionDiffHasMaterialChanges;

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
  boot.toggleAccessFloatingPanel = AF.toggleAccessFloatingPanel.bind(AF);
  boot.syncAccessFloatingViewportMount = AF.syncAccessFloatingViewportMount.bind(AF);
  boot.reclampAccessFloatingInViewport = AF.reclampAccessFloatingInViewport.bind(AF);
  boot.activateAccessFabTap = AF.activateAccessFabTap.bind(AF);
  boot.agentPanelState = state;

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
    if (!RT.panelAuthoringEnabled()) return "";
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
    panelAuthoringEnabled: RT.panelAuthoringEnabled,
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
    collectBrowserContext: CTX.collectBrowserContext,
  };
  const MSG =
    typeof window.__meiAgentPanelInstallMessages === "function"
      ? window.__meiAgentPanelInstallMessages(msgApi)
      : null;
  if (!MSG || typeof MSG.refreshMessages !== "function") {
    console.error(
      "MeiAgentPanelMessages missing: ensure agent-panel-messages-model.js, agent-panel-messages.js, agent-panel-layout.js are bundled before agent-panel.js",
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
          areAgentRequestsBlocked: function () {
            return $U.areAgentRequestsBlocked();
          },
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
      $U.resolveAgentAuthGate()
        .then(function (gate) {
          if (!gate.allowed) {
            $U.blockAgentRequests(gate.reason);
            CHR.setInlineNote($U.agentRequestsBlockMessage(gate.reason));
            return;
          }
          $U.unblockAgentRequests();
