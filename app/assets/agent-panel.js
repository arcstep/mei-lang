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

  const onManageSourceBundleReady = function () {
    if (!SRC || typeof SRC.ensureSourceEditor !== "function") return;
    const nextTab = RT.currentManageTab();
    SRC.ensureSourceEditor();
    if (typeof SRC.applyManageTabMode === "function") {
      SRC.applyManageTabMode(nextTab);
    }
  };
  document.addEventListener("mei:manage-source-bundle-ready", onManageSourceBundleReady);

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

  const onBrowserQueryStateChange = function () {
    state.contextPreviewScopeKey = "";
    state.contextPreviewFetchedAtMs = 0;
    state.contextPreviewBackoffUntilMs = 0;
    CTX.refreshContextPreview(true).catch(function () {});
  };
  document.addEventListener("mei:query-state-change", onBrowserQueryStateChange);

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
    document.removeEventListener("mei:manage-source-bundle-ready", onManageSourceBundleReady);
    document.removeEventListener("mei:manage-context-change", onManageContextChange);
    document.removeEventListener("mei:query-state-change", onBrowserQueryStateChange);
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
