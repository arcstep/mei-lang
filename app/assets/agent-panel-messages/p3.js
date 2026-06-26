      state.sessions = [];
      state.skillStatus = null;
      if (error && error.agentBlocked) {
        CHR.setInlineNote($U.agentRequestsBlockMessage());
      } else if (Number(error && error.status) === 403) {
        CHR.setInlineNote($U.agentRequestsBlockMessage("capability"));
      } else if (Number(error && error.status) === 401) {
        CHR.setInlineNote($U.agentRequestsBlockMessage("session_expired"));
      } else {
        CHR.setInlineNote("读取助手状态失败：" + String(error.message || error));
      }
    } finally {
      state.loading = false;
      CHR.setButtonState(false);
      CHR.renderStatus();
      CHR.renderConfig();
      CHR.renderRuntime();
      CHR.renderSkillStatus();
      const skipAgentFollowups = refreshFailed || $U.areAgentRequestsBlocked();
      if (!skipAgentFollowups) {
        await CTX.refreshModelProbe(true).catch(function () {});
        await CTX.refreshContextPreview().catch(function () {});
      }
      const boundSessions = listBoundSessionsForTarget(state.sessions, state.sessionTargetKey);
      if (state.sessionId && !sessionIdInList(state.sessions, state.sessionId)) {
        state.sessionId = "";
        state.messages = [];
        state.lastMessagesFingerprint = "";
        CHR.clearDeltaDebugLog();
        resetPendingPermissionState();
        rememberSession();
      }
      if (!state.sessionId && boundSessions.length > 0) {
        const savedId = String(localStorage.getItem(api.sessionStorageKey()) || "").trim();
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
      SRC.syncSourceDiffEntry();
      api.restoreDeltaDebugLog(state.sessionId);
      if (!skipAgentFollowups && state.health && state.health.healthy && state.sessionId) {
        try {
          await refreshMessages({ forcePendingPermissions: true });
        } catch (_) {
          refreshFailed = true;
        }
        __meiSes().connectEvents(false);
      } else {
        __meiSes().closeEventStream();
        renderMessages();
      }
    }
    return !refreshFailed;
  }

  async function startServer() {
    CHR.setInlineNote("");
    await refreshAll();
  }

  function buildSessionTitle() {
    return api.buildBoundSessionTitle(api.currentTargetKey());
  }

  async function postNewBoundSession() {
    state.sessionTargetKey = api.currentSessionBindingFingerprint();
    const session = await $U.fetchJson("/api/agent/session", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ title: buildSessionTitle() }),
    });
    state.sessionId = session.id || "";
    CHR.clearDeltaDebugLog({ dropPersisted: true });
    resetPendingPermissionState();
    rememberSession();
    invalidateSessionCache();
    await refreshAll();
  }

  async function createSession() {
    const healthy = !!(state.health && state.health.healthy);
    if (!healthy) {
      CHR.setInlineNote("助手暂不可用；请检查服务端 QWEN_BASE_URL、QWEN_API_KEY、QWEN_COMPLETION_MODEL 等配置。");
      return;
    }
    await postNewBoundSession();
  }

  async function refreshMessages(options) {
    const opts = options || {};
    if (!state.sessionId || !(state.health && state.health.healthy)) {
      __meiSes().closeEventStream();
      CHR.clearDeltaDebugLog();
      state.lastMessagesFingerprint = "";
      state.sessionPatches = [];
      state.sessionPatchFingerprint = "";
      saveSessionPatchesToStorage([]);
      applySessionPatchesToDom([]);
      resetPendingPermissionState();
      state.progress = {
        visible: false,
        label: "",
        detail: "",
        items: [],
      };
      CHR.renderProgressStrip();
      renderMessages();
      return;
    }
    const payload = await $U.fetchJson(
      "/api/agent/session/" +
        encodeURIComponent(state.sessionId) +
        "/messages?limit=80",
    );
    const list = payload && Array.isArray(payload.messages) ? payload.messages : [];
    let sessionPatches = extractSessionPatchOffers(list);
    if (!sessionPatches.length) {
      sessionPatches = loadSessionPatchesFromStorage();
    }
    const patchFingerprint = JSON.stringify(sessionPatches);
    if (patchFingerprint !== String(state.sessionPatchFingerprint || "")) {
      state.sessionPatches = sessionPatches.slice();
      state.sessionPatchFingerprint = patchFingerprint;
      saveSessionPatchesToStorage(sessionPatches);
      applySessionPatchesToDom(sessionPatches);
      state.contextPreviewScopeKey = "";
      state.contextPreviewFetchedAtMs = 0;
      CTX.refreshContextPreview(true).catch(function () {});
    }
    const nextFingerprint = String(state.sessionId) + "|" + JSON.stringify(list);
    state.progress = api.deriveProgressFromMessages(list);
    CHR.renderProgressStrip();
    const runningBlocked = M.deriveBlockedNoticesFromRawMessages(list);
    const runningBlockedFingerprint = M.blockedPermissionFingerprint(runningBlocked);
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
        const pendingPayload = await $U.fetchJson(
          "/api/agent/session/" +
            encodeURIComponent(state.sessionId) +
            "/permissions/pending",
        );
        const pending = pendingPayload && Array.isArray(pendingPayload.pending)
          ? pendingPayload.pending
          : [];
        pendingBlocked = pending.map(M.blockedPermissionNoticeFromData);
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
      const inferred = M.inferAgentModeFromRawMessage(raw);
      const messageId = String(raw && raw.message_id ? raw.message_id : "");
      if (!inferred || !messageId) return;
      const meta = api.getMessageMeta(state.sessionId, messageId);
      if (!meta || !meta.agent) {
        api.setMessageMeta(messageId, { agent: inferred, hasDiff: null, reverted: false });
      }
    });
    state.messages = list.map(M.normalizeMessage);
    SRC.syncSourceDiffEntry();
    const mergedBlocked = M.mergeBlockedPermissionNotices(pendingBlocked, runningBlocked);
    applyBlockedPermissionNotices(mergedBlocked);
    decorateMessageActions();
    renderMessages();
    await hydrateBuildDiffMeta(state.messages);
    if (CHR.activeGenerationFinished(list)) {
      CHR.finishSending();
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
    CHR.setButtonState(false);
    try {
      if (state.sendAbortController) {
        state.sendAbortController.abort();
      }
      if (state.sessionId) {
        await $U.fetchJson(
          "/api/agent/session/" + encodeURIComponent(state.sessionId) + "/abort",
          {
            method: "POST",
            headers: { "content-type": "application/json" },
          },
        );
      }
      await refreshMessages().catch(function () {});
      CHR.finishSending({ restoreDraft: true });
    } catch (error) {
      CHR.setInlineNote("停止失败：" + String(error.message || error));
      state.aborting = false;
      CHR.setButtonState(false);
    }
  }

  async function postPromptWithCurrentSession(text, controller) {
    const host =
      typeof globalThis !== "undefined" && globalThis.MeiAgentHostCoordinates;
    const body =
      host && typeof host.buildPromptRequestBody === "function"
        ? host.buildPromptRequestBody(api, text, {
            resourceVisibility: CTX.currentResourceVisibility(),
            browserContext:
              typeof api.collectBrowserContext === "function"
                ? api.collectBrowserContext()
                : null,
          })
        : {
            text: text,
            app_id: String(root.dataset.app || ""),
            scene_id: api.currentSceneId(),
            target_file: api.currentTargetKey(),
            mode: api.normalizeAgentMode(state.agentMode),
            route_mode: api.normalizeRouteMode(root.dataset.mode),
            agent: api.normalizeAgentMode(state.agentMode),
            resource_visibility: CTX.currentResourceVisibility(),
          };
    const mref = CHR.getSelectedCompletionModelRef();
    if (mref) {
      body.model = { providerID: mref.provider_id, modelID: mref.model_id };
    }
    return $U.fetchJson(
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
      CHR.setInlineNote("助手未就绪；请检查 QWEN_* 配置或点击“重连”。");
      return;
    }
    if (!state.sessionId) {
      await createSession();
      if (!state.sessionId) {
        return;
      }
    }
    state.sessionTargetKey = api.currentSessionBindingFingerprint();
    state.sending = true;
    state.aborting = false;
    state.pendingPromptDraft = draftText;
    state.progress = {
      visible: true,
      label: api.normalizeAgentMode(state.agentMode) === "ask" ? "问答处理中" : "脚本生成中",
      detail: api.normalizeAgentMode(state.agentMode) === "ask" ? "等待回答输出" : "等待执行输出",
      items: [
        {
          label: api.normalizeAgentMode(state.agentMode) === "ask" ? "问答中" : "生成中",
          status: "running",
        },
      ],
    };
    CHR.renderProgressStrip();
    CHR.clearGenerationSettleTimer();
    if (els.input) {
      els.input.value = "";
      api.autoResizeComposerInput();
      els.input.focus();
    }
    CHR.setButtonState(false);
    try {
      const controller = new AbortController();
      state.sendAbortController = controller;
      let summary;
      try {
        summary = await postPromptWithCurrentSession(text, controller);
      } catch (error) {
        if (!api.isNotFoundError(error)) {
          throw error;
        }
        state.sendAbortController = null;
        state.sessionId = "";
        state.messages = [];
        state.lastMessagesFingerprint = "";
        resetPendingPermissionState();
        rememberSession();
        invalidateSessionCache();
        CHR.setInlineNote("检测到旧会话已失效，正在自动重建会话后重试…");
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
        CHR.setInlineNote("发送失败：" + (detail || "上游模型返回错误"));
        await refreshMessages().catch(function () {});
        CHR.finishSending({ restoreDraft: true });
        return;
      }
      if (summary && summary.message_id) {
        state.activeGenerationMessageId = String(summary.message_id);
        api.setMessageMeta(summary.message_id, {
          agent: api.normalizeAgentMode(state.agentMode),
          hasDiff: api.normalizeAgentMode(state.agentMode) === "build" ? null : false,
          reverted: false,
        });
        if (api.normalizeAgentMode(state.agentMode) === "build") {
          state.pendingReloadMessageId = String(summary.message_id);
        }
      }
      await refreshMessages();
      CHR.finishSending();
      if (summary && (summary.scope_digest || summary.profile_summary)) {
        var bits = [];
        if (summary.scope_digest) {
          bits.push("scope_digest=" + String(summary.scope_digest));
        }
        if (summary.profile_summary) {
          bits.push(String(summary.profile_summary));
        }
        CHR.setInlineNote("发送完成 · " + bits.join(" | "));
      }
      if (summary && summary.message_id && api.normalizeAgentMode(state.agentMode) === "build") {
        try {
          const diff = await fetchSessionDiff(summary.message_id);
          const hasDiff = api.sessionDiffHasMaterialChanges(diff);
          api.setMessageMeta(summary.message_id, { hasDiff: hasDiff });
          if (hasDiff) {
            state.messageDiffCache[api.diffCacheKey(state.sessionId, summary.message_id)] = diff;
            decorateMessageActions();
            renderMessages();
            api.scheduleHostReload("Build 已修改文件，正在刷新预览与源码…");
          }
        } catch (_) {}
      }
      CHR.renderHistoryButtons();
      __meiSes().connectEvents(false);
      CHR.markGenerationActivity();
    } catch (error) {
      const aborted = state.aborting || (error && error.name === "AbortError");
      state.sendAbortController = null;
      if (aborted) {
        return;
      }
      CHR.setInlineNote("发送失败：" + String(error.message || error));
      CHR.finishSending({ restoreDraft: true });
    }
  }

    return {
      readSessionCache: readSessionCache,
      writeSessionCache: writeSessionCache,
      invalidateSessionCache: invalidateSessionCache,
      fetchAllSessions: fetchAllSessions,
      renderSessions: renderSessions,
      resetPendingPermissionState: resetPendingPermissionState,
      normalizeMessage: M.normalizeMessage,
      respondPermissionRequest: respondPermissionRequest,
      rememberSession: rememberSession,
      restoreSession: restoreSession,
      refreshAll: refreshAll,
      startServer: startServer,
      buildSessionTitle: buildSessionTitle,
      postNewBoundSession: postNewBoundSession,
      createSession: createSession,
      refreshMessages: refreshMessages,
      summarizePromptError: summarizePromptError,
      stopSending: stopSending,
      postPromptWithCurrentSession: postPromptWithCurrentSession,
      sendPrompt: sendPrompt,
      applyRevertForMessage: applyRevertForMessage,
      applyUnrevertForSession: applyUnrevertForSession,
      blockedPermissionNoticeFromData: M.blockedPermissionNoticeFromData,
      rememberBlockedPermissionNotice: rememberBlockedPermissionNotice,
    };
  };
})(window);
