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
    input: document.getElementById("author-intent-input"),
    run: document.getElementById("author-run-btn"),
    modePlan: document.getElementById("author-mode-plan-btn"),
    modeBuild: document.getElementById("author-mode-build-btn"),
    undo: document.getElementById("author-undo-btn"),
    redo: document.getElementById("author-redo-btn"),
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
    for (let index = state.messages.length - 1; index >= 0; index -= 1) {
      const message = state.messages[index];
      if (!message || String(message.role || "") !== "assistant") continue;
      const messageId = String(message.id || "").trim();
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

  function renderHistoryButtons() {
    const unavailableReason = historyUnavailableReason();
    const undoEnabled =
      !unavailableReason && !state.loading && !state.sending && !state.aborting && canUndo();
    const redoEnabled =
      !unavailableReason && !state.loading && !state.sending && !state.aborting && canRedo();
    if (els.undo) {
      els.undo.disabled = !undoEnabled;
      els.undo.classList.toggle("is-active", undoEnabled);
      els.undo.title = unavailableReason || "撤回上一轮消息及其代码影响";
    }
    if (els.redo) {
      els.redo.disabled = !redoEnabled;
      els.redo.classList.toggle("is-active", redoEnabled);
      els.redo.title = unavailableReason || "恢复最近撤回的消息及其代码影响";
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
    els.run.disabled = isStopping || (disabled && !isSending);
    els.run.textContent = isSending ? "■" : "➤";
    els.run.title = isSending ? (isStopping ? "停止中" : "停止发送") : "发送";
    els.run.classList.toggle("author-btn-danger", isSending);
    els.run.classList.toggle("author-btn-primary", !isSending);
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
    if (opts.restoreDraft) {
      mergeDraftBackIntoInput();
    } else {
      state.pendingPromptDraft = "";
    }
    setButtonState(false);
  }

  function markGenerationActivity() {
    if (!state.sending) return;
    clearGenerationSettleTimer();
    state.generationSettleTimer = window.setTimeout(function () {
      finishSending();
    }, 1800);
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
    if (tool.title) lines.push("标题: " + String(tool.title));
    if (tool.output) lines.push("输出:\n" + String(tool.output));
    if (tool.error) lines.push("错误:\n" + String(tool.error));
    return lines.join("\n");
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

  function formatDiffSummary(diff) {
    if (!diff) return "";
    return (
      "消息: " +
      String(diff.message_id || "-") +
      "\n文件: " +
      String(Array.isArray(diff.files) ? diff.files.length : 0) +
      "\n新增: +" +
      String(diff.additions || 0) +
      "\n删除: -" +
      String(diff.deletions || 0)
    );
  }

  function buildDiffBlocks(diff) {
    const blocks = [];
    const summary = makeTextBlock("差异概览", formatDiffSummary(diff), "diff");
    if (summary) blocks.push(summary);
    const files = Array.isArray(diff && diff.files) ? diff.files : [];
    files.forEach(function (file, index) {
      const body =
        "文件: " +
        String(file.file || "") +
        "\n新增: +" +
        String(file.additions || 0) +
        "\n删除: -" +
        String(file.deletions || 0) +
        "\n\n[before]\n" +
        String(file.before || "") +
        "\n\n[after]\n" +
        String(file.after || "");
      const block = makeTextBlock(
        "变更 #" + String(index + 1),
        body,
        "diff",
        true,
      );
      if (block) blocks.push(block);
    });
    return blocks;
  }

  function buildCurrentCodeBlocks(messageId, diff, reverted) {
    const blocks = [];
    const summary = makeTextBlock(
      "当前代码",
      "消息: " +
        String(messageId || "") +
        "\n来源: " +
        (reverted ? "revert 后（before）" : "build 后（after）"),
      "code",
    );
    if (summary) blocks.push(summary);
    const files = Array.isArray(diff && diff.files) ? diff.files : [];
    files.forEach(function (file) {
      const code = reverted ? String(file.before || "") : String(file.after || "");
      const block = makeTextBlock(
        "文件: " + String(file.file || ""),
        code || "(空文件)",
        "code",
        true,
      );
      if (block) blocks.push(block);
    });
    return blocks;
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
    pushMessage("system", "已撤回上一轮消息及其代码影响。", {
      id: "revert:" + mid,
    });
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
    pushMessage("system", "已恢复最近撤回的消息及其代码影响。", {
      id: "unrevert:" + sid,
    });
    await refreshMessages();
    scheduleHostReload("已恢复撤回修改，正在刷新预览与源码…");
  }

  async function showDiffForMessage(messageId) {
    const sid = String(state.sessionId || "").trim();
    const mid = String(messageId || "").trim();
    if (!sid || !mid) return;
    const diff = await fetchSessionDiff(mid);
    setMessageMeta(mid, { hasDiff: !!(diff && Array.isArray(diff.files) && diff.files.length > 0) });
    state.messageDiffCache[messageKey(sid, mid)] = diff;
    if (!(diff && Array.isArray(diff.files) && diff.files.length > 0)) {
      pushMessage("system", "该轮未产生可回退的文件修改。", {
        id: "diff:" + mid,
      });
      renderMessages();
      return;
    }
    pushMessage("system", "已加载该轮修改差异。", {
      id: "diff:" + mid,
      blocks: buildDiffBlocks(diff),
      actions: [
        {
          label: "查看当前代码结果",
          onClick: function () {
            showCurrentCodeForMessage(mid).catch(function (error) {
              setInlineNote("读取当前代码失败：" + String(error.message || error));
            });
          },
        },
      ],
    });
    renderMessages();
  }

  async function showCurrentCodeForMessage(messageId) {
    const sid = String(state.sessionId || "").trim();
    const mid = String(messageId || "").trim();
    if (!sid || !mid) return;
    const cacheKey = messageKey(sid, mid);
    const diff = state.messageDiffCache[cacheKey] || (await fetchSessionDiff(mid));
    state.messageDiffCache[cacheKey] = diff;
    if (!(diff && Array.isArray(diff.files) && diff.files.length > 0)) {
      pushMessage("system", "该轮没有可展示的代码结果。", {
        id: "current:" + mid,
      });
      return;
    }
    const meta = getMessageMeta(sid, mid) || {};
    const reverted = !!meta.reverted;
    pushMessage("system", "当前代码结果已刷新。", {
      id: "current:" + mid,
      blocks: buildCurrentCodeBlocks(mid, diff, reverted),
    });
  }

  function actionsForAssistantMessage(message) {
    if (historyUnavailableReason()) return [];
    const messageId = String(message && message.id ? message.id : "");
    if (!messageId || String(message.role || "") !== "assistant") return [];
    const meta = getMessageMeta(state.sessionId, messageId);
    if (!meta || meta.hasDiff !== true) {
      return [];
    }
    return [
      {
        label: "查看差异",
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
    els.chatLog.innerHTML = state.messages
      .map(function (message) {
        const role = escapeHtml(message.role || "assistant");
        const cls =
          role === "user"
            ? "author-chat-message author-chat-user"
            : role === "assistant"
              ? "author-chat-message author-chat-assistant"
              : "author-chat-message author-chat-system";
        const blocks = Array.isArray(message.blocks) ? message.blocks : [];
        const time = escapeHtml(message.time || "");
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
            : '<pre class="author-chat-body">' + escapeHtml(message.body || "") + "</pre>";
        return (
          '<div class="' +
          cls +
          '">' +
          '<div class="author-chat-head"><div class="author-chat-role author-chat-role-' +
          role +
          '">' +
          (role === "user" ? "我" : role === "assistant" ? escapeHtml(state.modelLabel || "模型") : "系统") +
          '</div><div class="author-chat-meta"><span class="author-chat-time">' +
          time +
          '</span><button type="button" class="author-chat-copy-btn opencode-copy-btn" data-message-id="' +
          escapeHtml(message.id || "") +
          '">⧉</button></div></div>' +
          bodyHtml +
          renderMessageActions(message, message.id || "") +
          "</div>"
        );
      })
      .join("");

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
          blocks.length > 0
            ? blocks
                .map(function (block) {
                  return (block.label ? "[" + String(block.label) + "]\n" : "") + String(block.content || "");
                })
                .join("\n\n")
            : String(message.body || "");
        copyText(text).catch(function () {});
      });
    });

    restoreChatScroll(scrollSnapshot, shouldStickBottom);
  }

  function pushMessage(role, body, options) {
    const opts = options || {};
    const id = String(opts.id || "local:" + Date.now() + ":" + Math.random());
    const existing = state.messages.find(function (item) {
      return String(item && item.id ? item.id : "") === id;
    });
    const next = {
      id: id,
      role: role,
      body: String(body || "").trim(),
      blocks: Array.isArray(opts.blocks) ? opts.blocks : [],
      actions: Array.isArray(opts.actions) ? opts.actions : [],
      time: new Date().toLocaleTimeString("zh-CN", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      }),
    };
    if (existing) {
      existing.body = next.body;
      existing.blocks = next.blocks;
      existing.actions = next.actions;
      existing.time = next.time;
    } else {
      state.messages.push(next);
      if (state.messages.length > 120) {
        state.messages = state.messages.slice(-120);
      }
    }
    renderMessages();
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
    pushMessage("system", "permission_id: " + pid + "\nresponse: " + reply, {
      id: "permission-result:" + pid,
    });
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
      const metadata = event.metadata ? JSON.stringify(event.metadata, null, 2) : "{}";
      const permissionId = String(event.permission_id || "");
      pushMessage(
        "system",
        "permission_id: " +
          permissionId +
          "\npermission: " +
          String(event.permission || "") +
          "\nmetadata:\n" +
          metadata,
        {
          id: "permission:" + permissionId,
          actions: permissionId
            ? [
                {
                  label: "允许一次",
                  onClick: function () {
                    respondPermissionRequest(permissionId, "once").catch(function () {});
                  },
                },
                {
                  label: "始终允许",
                  onClick: function () {
                    respondPermissionRequest(permissionId, "always").catch(function () {});
                  },
                },
                {
                  label: "拒绝",
                  onClick: function () {
                    respondPermissionRequest(permissionId, "reject").catch(function () {});
                  },
                },
              ]
            : [],
        },
      );
      return;
    }
    if (kind === "permission_resolved") {
      markGenerationActivity();
      pushMessage(
        "system",
        "permission_id: " +
          String(event.permission_id || "") +
          "\nresponse: " +
          String(event.response || ""),
        {
          id: "permission-result:" + String(event.permission_id || ""),
        },
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
        rememberSession();
      }
      if (!state.sessionId && boundSessions.length > 0) {
        const savedId = String(localStorage.getItem(sessionStorageKey) || "").trim();
        const saved = savedId ? boundSessions.find(function (item) { return item.id === savedId; }) : null;
        const preferred = saved || boundSessions[0];
        state.sessionId = preferred ? preferred.id : "";
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
      if (state.health && state.health.healthy && state.sessionId) {
        await refreshMessages();
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

  async function refreshMessages() {
    if (!state.sessionId || !(state.health && state.health.healthy)) {
      closeEventStream();
      state.lastMessagesFingerprint = "";
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
    if (nextFingerprint === state.lastMessagesFingerprint) {
      return;
    }
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
    decorateMessageActions();
    renderMessages();
    await hydrateBuildDiffMeta(state.messages);
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
  refreshAll();
  window.addEventListener("beforeunload", closeEventStream);
  window.setInterval(function () {
    refreshAll().catch(function () {});
  }, 8000);
})();
