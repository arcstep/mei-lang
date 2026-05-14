(function () {
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
    input: document.getElementById("author-intent-input"),
    run: document.getElementById("author-run-btn"),
    modePlan: document.getElementById("author-mode-plan-btn"),
    modeBuild: document.getElementById("author-mode-build-btn"),
    undo: document.getElementById("author-undo-btn"),
    redo: document.getElementById("author-redo-btn"),
    sourceViewSourceBtn: document.getElementById("source-view-source-btn"),
    sourceViewDiffBtn: document.getElementById("source-view-diff-btn"),
    sourceViewStatus: document.getElementById("source-view-status"),
    sourceViewHost: document.getElementById("source-view-host"),
    sourceViewSourcePanel: document.getElementById("source-view-source-panel"),
    sourceViewSourceRaw: document.getElementById("source-view-source-raw"),
    sourceViewDiffPanel: document.getElementById("source-view-diff-panel"),
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
    sourceDiffResizeObserver: null,
    sourceViewResizeObserver: null,
    progress: {
      visible: false,
      label: "",
      detail: "",
      items: [],
    },
  };

  const sessionStorageKey =
    "mei-lang.opencode.session." +
    String(root.dataset.app || "") +
    "." +
    String(root.dataset.target || "");
  const modeStorageKey =
    "mei-lang.opencode.mode." +
    String(root.dataset.app || "") +
    "." +
    String(root.dataset.target || "");
  const revertedStorageKey =
    "mei-lang.opencode.reverted." +
    String(root.dataset.app || "") +
    "." +
    String(root.dataset.target || "");
  const SESSION_CACHE_KEY = "mei.author.sessions.v1";
  const SESSION_CACHE_TTL_MS = 30000;
  const CHAT_BOTTOM_STICKY_THRESHOLD_PX = 28;

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
    return params.get("target") || String(root.dataset.target || "");
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

  function normalizeAgentMode(value) {
    return String(value || "").toLowerCase() === "plan" ? "plan" : "build";
  }

  function composerDraftText() {
    return els.input && typeof els.input.value === "string" ? String(els.input.value) : "";
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
    const targetNode = els.sourceViewSourceRaw || els.sourceViewSourcePanel;
    if (targetNode && targetNode.dataset && targetNode.dataset.sourceTarget) {
      return normalizeFilePath(targetNode.dataset.sourceTarget);
    }
    return currentTargetKey();
  }

  function sourceLanguage() {
    const targetNode = els.sourceViewSourceRaw || els.sourceViewSourcePanel;
    if (targetNode && targetNode.dataset && targetNode.dataset.sourceLang) {
      return String(targetNode.dataset.sourceLang || "").trim().toLowerCase() || "plain";
    }
    return "plain";
  }

  function sourceRawText() {
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
      localStorage.setItem(revertedStorageKey, JSON.stringify(state.revertedMessageIds));
    } catch (_) {}
  }

  function restoreRevertedState() {
    try {
      const raw = localStorage.getItem(revertedStorageKey);
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

    let label = agent === "plan" ? "正在规划" : "正在执行";
    if (runningTools.length > 0) {
      label = (agent === "plan" ? "正在规划" : "正在执行") + " · 工具运行中";
    } else if (stepStarts > stepFinishes) {
      label = (agent === "plan" ? "正在规划" : "正在执行") + " · 步骤处理中";
    } else if (parts.length > 0) {
      label = agent === "plan" ? "正在生成计划" : "正在生成结果";
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
        label: agent === "plan" ? "等待规划输出" : "等待执行输出",
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

  function renderSourceViewStatus(text) {
    if (!els.sourceViewStatus) return;
    const message = String(text || "").trim();
    els.sourceViewStatus.textContent = message || "仅支持最后一轮 Build";
  }

  function destroySourceEditor() {
    if (state.sourceViewResizeObserver) {
      try {
        state.sourceViewResizeObserver.disconnect();
      } catch (_) {}
      state.sourceViewResizeObserver = null;
    }
    state.sourceCodeMirror = null;
    if (els.sourceViewSourcePanel) {
      els.sourceViewSourcePanel.innerHTML = "";
    }
  }

  function destroySourceDiffView() {
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

  function initSourceEditor() {
    if (!els.sourceViewSourcePanel || !window.CodeMirror) {
      return;
    }
    destroySourceEditor();
    state.sourceCodeMirror = window.CodeMirror(els.sourceViewSourcePanel, {
      value: sourceRawText(),
      lineNumbers: true,
      readOnly: true,
      mode: sourceLanguage() === "mei" ? "mei" : null,
      theme: "default",
      lineWrapping: false,
      scrollbarStyle: "native",
    });
    bindSourceViewResizeRefresh();
    scheduleSourceDiffRefresh();
  }

  function renderSourceViewMode(mode) {
    const nextMode = mode === "diff" ? "diff" : "source";
    state.sourceViewMode = nextMode;
    if (els.sourceViewSourcePanel) {
      els.sourceViewSourcePanel.hidden = nextMode !== "source";
    }
    if (els.sourceViewDiffPanel) {
      els.sourceViewDiffPanel.hidden = nextMode !== "diff";
    }
    if (els.sourceViewSourceBtn) {
      const active = nextMode === "source";
      els.sourceViewSourceBtn.classList.toggle("is-active", active);
      els.sourceViewSourceBtn.setAttribute("aria-pressed", active ? "true" : "false");
    }
    if (els.sourceViewDiffBtn) {
      const active = nextMode === "diff";
      els.sourceViewDiffBtn.classList.toggle("is-active", active);
      els.sourceViewDiffBtn.setAttribute("aria-pressed", active ? "true" : "false");
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
    renderSourceViewStatus("差异文件：" + String(fileDiff && fileDiff.file ? fileDiff.file : ""));
    bindSourceDiffResizeRefresh();
    scheduleSourceDiffRefresh();
    return true;
  }

  function leaveDiffView() {
    state.sourceDiffMessageId = "";
    destroySourceDiffView();
    renderSourceViewMode("source");
    const message = state.latestDiffMessageId
      ? "仅支持最后一轮 Build"
      : "最后一轮 Build 生成改动后可查看差异";
    renderSourceViewStatus(message);
  }

  async function inspectDiffForMessage(messageId) {
    const sid = String(state.sessionId || "").trim();
    const mid = String(messageId || "").trim();
    if (!sid || !mid) return false;
    if (mid !== String(state.latestDiffMessageId || "")) {
      setInlineNote("仅支持查看最后一轮 Build 的差异。");
      return false;
    }
    const cacheKey = messageKey(sid, mid);
    const diff = state.messageDiffCache[cacheKey] || (await fetchSessionDiff(mid));
    state.messageDiffCache[cacheKey] = diff;
    const hasFiles = !!(diff && Array.isArray(diff.files) && diff.files.length > 0);
    setMessageMeta(mid, { hasDiff: hasFiles });
    if (!hasFiles) {
      setInlineNote("最后一轮未产生可显示的文件差异。");
      leaveDiffView();
      return false;
    }
    const fileDiff = pickDiffFileForTarget(diff);
    if (!fileDiff) {
      setInlineNote("当前目标文件没有可显示差异。");
      leaveDiffView();
      return false;
    }
    return renderSourceDiff(fileDiff, mid);
  }

  function syncSourceDiffEntry() {
    state.latestRoundAssistantId = latestRoundAssistantMessageId();
    state.latestDiffMessageId = latestDiffEligibleMessageId();
    if (els.sourceViewDiffBtn) {
      const enabled = !!state.latestDiffMessageId && !historyUnavailableReason();
      els.sourceViewDiffBtn.disabled = !enabled;
      els.sourceViewDiffBtn.title = enabled
        ? "查看最后一轮 Build 差异"
        : (historyUnavailableReason() || "最后一轮 Build 生成改动后可查看 Diff");
    }
    if (
      state.sourceViewMode === "diff" &&
      state.sourceDiffMessageId &&
      state.sourceDiffMessageId !== state.latestDiffMessageId
    ) {
      leaveDiffView();
    } else if (!state.latestDiffMessageId && state.sourceViewMode === "diff") {
      leaveDiffView();
    } else if (state.sourceViewMode !== "diff") {
      renderSourceViewStatus(
        state.latestDiffMessageId
          ? "仅支持最后一轮 Build"
          : "最后一轮 Build 生成改动后可查看差异",
      );
    }
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
        const status = escapeHtml(String(item && item.status ? item.status : "pending").trim());
        return '<span class="author-progress-chip is-' + status + '">' + label + "</span>";
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

  function hasServerTarget() {
    return !!(state.runtime && state.runtime.server_url);
  }

  function canStartManaged() {
    return !!(
      state.config &&
      state.config.preferred_mode === "managed" &&
      state.config.managed_start_available
    );
  }

  function buildBoundSessionTitle(targetKey) {
    const params = new URLSearchParams();
    params.set("app", String(root.dataset.app || ""));
    params.set("target", String(targetKey || ""));
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
      const entry = String(params.get("entry") || "").trim();
      if (!app || !target) return null;
      return { app: app, target: target, entry: entry };
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
    if (els.modePlan) els.modePlan.disabled = controlsDisabled;
    if (els.modeBuild) els.modeBuild.disabled = controlsDisabled;
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
    if (!els.serverStatus || !els.serverDot) return;
    const runtime = state.runtime;
    const health = state.health;
    const config = state.config;
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
    } else if (runtime && runtime.connection_source === "managed" && runtime.running) {
      label = "启动中";
    } else if (runtime && runtime.connection_source === "external" && runtime.running) {
      label = "未连接";
    } else if (canStartManaged()) {
      label = "可启动";
    }
    els.serverStatus.textContent = label;
    els.serverDot.className = dotClass;
    if (els.reconnect) {
      const shouldShowReconnect =
        !state.loading &&
        !!(runtime && runtime.running) &&
        !(health && health.healthy);
      els.reconnect.hidden = !shouldShowReconnect;
    }
  }

  function renderConfig() {
    const config = state.config;
    if (!config) {
      state.modelLabel = "模型";
      if (els.modelLabel) els.modelLabel.textContent = state.modelLabel;
      return;
    }
    state.modelLabel =
      String(config.completion_model || config.provider_name || config.provider_id || "模型").trim() ||
      "模型";
    if (els.modelLabel) els.modelLabel.textContent = state.modelLabel;
  }

  function renderAgentMode() {
    const mode = normalizeAgentMode(state.agentMode);
    state.agentMode = mode;
    if (els.modePlan) {
      const active = mode === "plan";
      els.modePlan.classList.toggle("is-active", active);
      els.modePlan.setAttribute("aria-pressed", active ? "true" : "false");
    }
    if (els.modeBuild) {
      const active = mode === "build";
      els.modeBuild.classList.toggle("is-active", active);
      els.modeBuild.setAttribute("aria-pressed", active ? "true" : "false");
    }
  }

  function rememberAgentMode() {
    try {
      localStorage.setItem(modeStorageKey, normalizeAgentMode(state.agentMode));
    } catch (_) {}
  }

  function restoreAgentMode() {
    try {
      const saved = localStorage.getItem(modeStorageKey);
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
      state.agentMode === "plan"
        ? "已切换到 Plan（分析为主，不主动生成代码改动）"
        : "已切换到 Build（可直接改代码）",
    );
  }

  function renderRuntime() {
    renderStatus();
    renderInlineNote();
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

  function renderSkillStatus() {
    if (!els.skillLine) return;
    const skill = state.skillStatus;
    if (!skill || !skill.source_present) {
      els.skillLine.textContent = "Skill: 未发现 MeiLang skill 源目录";
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
    els.skillLine.textContent = summary.join(" · ");
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
    const payload = await fetchJson("/api/opencode/session");
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
    const entry = String(root.dataset.entry || "");
    const target = normalizeTargetKey(targetKey);
    return (Array.isArray(sessions) ? sessions : [])
      .filter(function (session) {
        if (!session || typeof session !== "object") return false;
        const meta = parseBoundSessionTitle(session.title);
        if (!meta) return false;
        if (meta.app !== app) return false;
        if (meta.target !== target) return false;
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
    const placeholder = document.createElement("option");
    placeholder.value = "";
    placeholder.textContent = "历史（当前文件）";
    els.sessionSelect.appendChild(placeholder);
    sessions.forEach(function (session) {
      if (!session || typeof session !== "object") return;
      const id = String(session.id || "");
      if (!id) return;
      const option = document.createElement("option");
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

  function formatToolPart(part) {
    const tool = part && part.tool ? part.tool : null;
    if (!tool) return null;
    const lines = [];
    lines.push("工具: " + String(tool.tool || "unknown"));
    lines.push("状态: " + String(tool.status || "pending"));
    if (tool.input_path) lines.push("路径: " + String(tool.input_path));
    if (tool.title) lines.push("标题: " + String(tool.title));
    if (tool.output) lines.push("输出:\n" + String(tool.output));
    if (tool.error) lines.push("错误:\n" + String(tool.error));
    return lines.join("\n");
  }

  function looksLikeSkillPath(path) {
    return String(path || "").replaceAll("\\", "/").includes("/.mei/opencode/skills/meilang-author");
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
          "系统尝试读取 MeiLang skill 目录但当前未获授权。请联系管理员检查 OpenCode external_directory 白名单配置。",
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
      renderSourceViewStatus("最后一轮 Build 生成改动后可查看差异");
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
    const parts = Array.isArray(raw && raw.parts) ? raw.parts : [];
    const role = String((raw && raw.role) || "assistant");
    const textParts = [];
    const reasoningParts = [];
    const toolParts = [];
    const debugParts = [];
    parts.forEach(function (part) {
      const type = String((part && part.part_type) || "");
      if (type === "text" && part.text) {
        textParts.push(String(part.text));
        return;
      }
      if (type === "reasoning" && part.text) {
        reasoningParts.push(String(part.text));
        return;
      }
      if (type === "tool") {
        const toolSummary = formatToolPart(part);
        if (toolSummary) toolParts.push(toolSummary);
        return;
      }
      if (part && part.raw) {
        debugParts.push(JSON.stringify(part.raw, null, 2));
      }
    });
    const blocks = [];
    const textBlock = makeTextBlock("", textParts.join("\n\n"), "text");
    const reasoningBlock = makeTextBlock("思考（可折叠调试）", reasoningParts.join("\n\n"), "reasoning", true);
    if (textBlock) blocks.push(textBlock);
    if (reasoningBlock) blocks.push(reasoningBlock);
    toolParts.forEach(function (toolBody, idx) {
      const block = makeTextBlock("工具调用 #" + String(idx + 1), toolBody, "tool", true);
      if (block) blocks.push(block);
    });
    debugParts.forEach(function (debugBody, idx) {
      const block = makeTextBlock("结构化片段 #" + String(idx + 1), debugBody, "debug", true);
      if (block) blocks.push(block);
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
      '<div class="author-chat-inline-actions">' +
      actions
        .map(function (action, index) {
          return (
            '<button type="button" class="author-chat-action-btn opencode-action-btn" data-message-id="' +
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
    const query = messageId
      ? "?message_id=" + encodeURIComponent(String(messageId))
      : "";
    return fetchJson(
      "/api/opencode/session/" +
        encodeURIComponent(state.sessionId) +
        "/diff" +
        query,
    );
  }

  async function applyRevertForMessage(messageId) {
    const sid = String(state.sessionId || "").trim();
    const mid = String(messageId || "").trim();
    if (!sid || !mid) return;
    await fetchJson("/api/opencode/session/" + encodeURIComponent(sid) + "/revert", {
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
    await fetchJson("/api/opencode/session/" + encodeURIComponent(sid) + "/unrevert", {
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

  async function showDiffForMessage(messageId) {
    if (!(await inspectDiffForMessage(messageId))) {
      return;
    }
    setInlineNote("差异已加载到左侧源码区。");
  }

  function actionsForAssistantMessage(message) {
    if (historyUnavailableReason()) return [];
    const messageId = String(message && message.id ? message.id : "");
    if (!messageId || String(message.role || "") !== "assistant") return [];
    if (messageId !== String(state.latestRoundAssistantId || "")) {
      return [];
    }
    const meta = getMessageMeta(state.sessionId, messageId);
    if (!meta || meta.hasDiff !== true) {
      return [];
    }
    return [
      {
        label: "最近差异",
        onClick: function () {
          showDiffForMessage(messageId).catch(function (error) {
            setInlineNote("读取差异失败：" + String(error.message || error));
          });
        },
      },
    ];
  }

  async function hydrateBuildDiffMeta(messages) {
    if (!Array.isArray(messages) || !state.sessionId) return;
    if (historyUnavailableReason()) {
      renderHistoryButtons();
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
        const hasDiff = !!(diff && Array.isArray(diff.files) && diff.files.length > 0);
        setMessageMeta(messageId, { hasDiff: hasDiff });
        if (hasDiff) {
          state.messageDiffCache[messageKey(state.sessionId, messageId)] = diff;
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
      syncSourceDiffEntry();
      state.messages = state.messages.map(function (row) {
        return Object.assign({}, row, { actions: actionsForAssistantMessage(row) });
      });
      renderMessages();
    }
    renderHistoryButtons();
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
      "author-chat-message",
      roleRaw === "user"
        ? "author-chat-user"
        : roleRaw === "assistant"
          ? "author-chat-assistant"
          : "author-chat-system",
    ];
    if (reverted) classList.push("author-chat-assistant-reverted");
    if (extraClass) classList.push(extraClass);
    const cls = classList.join(" ");
    const blocks = Array.isArray(message && message.blocks) ? message.blocks : [];
    const time = escapeHtml(String(message && message.time ? message.time : ""));
    const bodyHtml =
      blocks.length > 0
        ? blocks
            .map(function (block) {
              const label = String(block.label || "").trim();
              const content = escapeHtml(block.content || "");
              if (block.collapsed) {
                return (
                  '<details class="author-chat-block author-chat-block-details author-chat-block-' +
                  escapeHtml(block.type || "text") +
                  '"><summary class="author-chat-block-label">' +
                  escapeHtml(label || "展开") +
                  '</summary><pre class="author-chat-body">' +
                  content +
                  "</pre></details>"
                );
              }
              return (
                '<section class="author-chat-block author-chat-block-' +
                escapeHtml(block.type || "text") +
                '">' +
                (label
                  ? '<div class="author-chat-block-label">' + escapeHtml(label) + "</div>"
                  : "") +
                '<pre class="author-chat-body">' +
                content +
                "</pre></section>"
              );
            })
            .join("")
        : '<pre class="author-chat-body">' + escapeHtml(message && message.body ? message.body : "") + "</pre>";
    const actions = roleRaw === "assistant" ? renderMessageActions(message, messageId) : "";
    return (
      '<div class="' +
      cls +
      '" data-message-id="' +
      escapeHtml(messageId) +
      '">' +
      '<div class="author-chat-head"><div class="author-chat-role author-chat-role-' +
      role +
      '">' +
      (roleRaw === "user" ? "我" : roleRaw === "assistant" ? escapeHtml(state.modelLabel || "模型") : "系统") +
      '</div><div class="author-chat-meta"><span class="author-chat-time">' +
      time +
      '</span><button type="button" class="author-chat-copy-btn opencode-copy-btn" data-message-id="' +
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
        '<div class="author-chat-empty">未选择会话。可先点击“新建对话”，或等待宿主自动创建/恢复会话。</div>';
      restoreChatScroll(scrollSnapshot, shouldStickBottom);
      return;
    }
    if (!state.messages.length) {
      els.chatLog.innerHTML =
        '<div class="author-chat-empty">发送任务后，这里会连续显示输入、参考信息和模型回复。</div>';
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
          '<section class="author-chat-round">' +
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

    Array.from(els.chatLog.querySelectorAll(".opencode-action-btn")).forEach(function (button) {
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

    Array.from(els.chatLog.querySelectorAll(".opencode-copy-btn")).forEach(function (button) {
      button.addEventListener("click", function () {
        const messageId = String(button.getAttribute("data-message-id") || "");
        const message = state.messages.find(function (item) {
          return String(item && item.id ? item.id : "") === messageId;
        });
        if (!message) return;
        const blocks = Array.isArray(message.blocks) ? message.blocks : [];
        const text =
          debugCopyTextForMessage(message);
        copyText(text).catch(function () {});
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
        "/api/opencode/session/" + encodeURIComponent(sessionId) + "/events",
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
      "/api/opencode/session/" +
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
      if (st === "opencode_unavailable" || st === "upstream_unavailable") {
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
      markGenerationActivity();
      refreshMessages().catch(function () {});
      return;
    }
    if (kind === "permission_requested") {
      markGenerationActivity();
      const notice = blockedPermissionNoticeFromData(event);
      rememberBlockedPermissionNotice(notice);
      setInlineNote("已拒绝未授权权限请求：" + String(notice.path || notice.permission || "unknown"));
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
        localStorage.setItem(sessionStorageKey, state.sessionId);
      } else {
        localStorage.removeItem(sessionStorageKey);
      }
    } catch (_) {}
  }

  function restoreSession() {
    try {
      const saved = localStorage.getItem(sessionStorageKey);
      if (saved) state.sessionId = saved;
    } catch (_) {}
  }

  async function refreshAll() {
    state.loading = true;
    setButtonState(true);
    renderStatus();
    try {
      const [config, runtime, skillStatus] = await Promise.all([
        fetchJson("/api/opencode/config"),
        fetchJson("/api/opencode/runtime"),
        fetchJson("/api/opencode/skill"),
      ]);
      state.config = config;
      state.runtime = runtime;
      state.skillStatus = skillStatus;
      state.sessionTargetKey = currentTargetKey();
      let runtimeRef = runtime;
      if (runtimeRef && runtimeRef.running) {
        try {
          state.health = await fetchJson("/api/opencode/health");
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
      state.health = null;
      state.sessions = [];
      state.skillStatus = null;
      setInlineNote("读取 OpenCode 状态失败：" + String(error.message || error));
    } finally {
      state.loading = false;
      setButtonState(false);
      renderStatus();
      renderConfig();
      renderRuntime();
      renderSkillStatus();
      const boundSessions = listBoundSessionsForTarget(state.sessions, state.sessionTargetKey);
      if (state.sessionId && !sessionIdInList(state.sessions, state.sessionId)) {
        state.sessionId = "";
        state.messages = [];
        state.lastMessagesFingerprint = "";
        resetPendingPermissionState();
        rememberSession();
      }
      if (!state.sessionId && boundSessions.length > 0) {
        const savedId = String(localStorage.getItem(sessionStorageKey) || "").trim();
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
      if (state.health && state.health.healthy && state.sessionId) {
        await refreshMessages({ forcePendingPermissions: true });
        connectEvents(false);
      } else {
        closeEventStream();
        renderMessages();
      }
    }
  }

  async function startServer() {
    if (!canStartManaged()) {
      setInlineNote("当前默认使用 external OpenCode 服务；请先独立启动 opencode-server。");
      await refreshAll();
      return;
    }
    setButtonState(true);
    setInlineNote("正在启动 OpenCode 服务...");
    try {
      await fetchJson("/api/opencode/start", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ port: 4099 }),
      });
    } catch (error) {
      setInlineNote("启动失败：" + String(error.message || error));
    }
    invalidateSessionCache();
    await refreshAll();
  }

  function buildSessionTitle() {
    return buildBoundSessionTitle(currentTargetKey());
  }

  async function postNewBoundSession() {
    state.sessionTargetKey = currentTargetKey();
    const session = await fetchJson("/api/opencode/session", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ title: buildSessionTitle() }),
    });
    state.sessionId = session.id || "";
    resetPendingPermissionState();
    rememberSession();
    invalidateSessionCache();
    await refreshAll();
  }

  async function createSession() {
    const healthy = !!(state.health && state.health.healthy);
    if (!healthy) {
      setInlineNote("OpenCode 服务未连接；请先独立启动服务并点击“重连”。");
      return;
    }
    await postNewBoundSession();
  }

  async function refreshMessages(options) {
    const opts = options || {};
    if (!state.sessionId || !(state.health && state.health.healthy)) {
      closeEventStream();
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
      "/api/opencode/session/" +
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
          "/api/opencode/session/" +
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
          "/api/opencode/session/" + encodeURIComponent(state.sessionId) + "/abort",
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
    return fetchJson(
      "/api/opencode/session/" + encodeURIComponent(state.sessionId) + "/message",
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          text: text,
          app_id: String(root.dataset.app || ""),
          entry_id: String(root.dataset.entry || ""),
          target_file: currentTargetKey(),
          agent: normalizeAgentMode(state.agentMode),
        }),
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
      setInlineNote("OpenCode 服务未连接；请先独立启动服务并点击“重连”。");
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
      label: normalizeAgentMode(state.agentMode) === "plan" ? "正在规划" : "正在执行",
      detail: normalizeAgentMode(state.agentMode) === "plan" ? "等待规划输出" : "等待执行输出",
      items: [
        {
          label: normalizeAgentMode(state.agentMode) === "plan" ? "规划中" : "执行中",
          status: "running",
        },
      ],
    };
    renderProgressStrip();
    clearGenerationSettleTimer();
    if (els.input) {
      els.input.value = "";
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
          hasDiff: state.agentMode === "build" ? null : false,
          reverted: false,
        });
        if (normalizeAgentMode(state.agentMode) === "build") {
          state.pendingReloadMessageId = String(summary.message_id);
        }
      }
      await refreshMessages();
      if (summary && summary.finish) {
        finishSending();
      }
      if (summary && summary.message_id && normalizeAgentMode(state.agentMode) === "build") {
        try {
          const diff = await fetchSessionDiff(summary.message_id);
          const hasDiff = !!(diff && Array.isArray(diff.files) && diff.files.length > 0);
          setMessageMeta(summary.message_id, { hasDiff: hasDiff });
          if (hasDiff) {
            state.messageDiffCache[messageKey(state.sessionId, summary.message_id)] = diff;
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
      const action = canStartManaged() && !hasServerTarget() ? startServer : refreshAll;
      action().catch(function (error) {
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
    els.sessionSelect.addEventListener("change", function () {
      state.sessionId = String(els.sessionSelect.value || "");
      state.sessionTargetKey = currentTargetKey();
      resetPendingPermissionState();
      rememberSession();
      refreshMessages().catch(function (error) {
        setInlineNote("读取会话失败：" + String(error.message || error));
      });
      connectEvents(true);
    });
  }

  if (els.run) {
    els.run.addEventListener("click", function () {
      sendPrompt().catch(function (error) {
        setInlineNote("发送失败：" + String(error.message || error));
      });
    });
  }

  if (els.input) {
    els.input.addEventListener("input", function () {
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
  }

  if (els.modePlan) {
    els.modePlan.addEventListener("click", function () {
      switchAgentMode("plan");
    });
  }

  if (els.modeBuild) {
    els.modeBuild.addEventListener("click", function () {
      switchAgentMode("build");
    });
  }

  if (els.sourceViewSourceBtn) {
    els.sourceViewSourceBtn.addEventListener("click", function () {
      leaveDiffView();
    });
  }

  if (els.sourceViewDiffBtn) {
    els.sourceViewDiffBtn.addEventListener("click", function () {
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

  restoreRevertedState();
  restoreAgentMode();
  restoreSession();
  initSourceEditor();
  renderSourceViewMode("source");
  renderSourceViewStatus("最后一轮 Build 生成改动后可查看差异");
  renderProgressStrip();
  syncSourceDiffEntry();
  refreshAll();
  window.addEventListener("beforeunload", closeEventStream);
  window.setInterval(function () {
    refreshAll().catch(function () {});
  }, 8000);
})();
