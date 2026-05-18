/**
 * 会话列表缓存、消息渲染、权限提示与 Prompt 发送链。由 agent-panel 主文件装配 `MSG`。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallMessages = function (api) {
    const root = api.root;
    const els = api.els;
    const state = api.state;
    const $U = api.$U;
    const CHR = api.CHR;
    const CTX = api.CTX;
    const SRC = api.SRC;
    const SESSION_CACHE_KEY = api.SESSION_CACHE_KEY;
    const SESSION_CACHE_TTL_MS = api.SESSION_CACHE_TTL_MS;
    const CHAT_BOTTOM_STICKY_THRESHOLD_PX = api.CHAT_BOTTOM_STICKY_THRESHOLD_PX;

    function __meiSes() {
      return api.transport.ses;
    }

    async function fetchSessionDiff(messageId) {
      return api.fetchSessionDiff(messageId);
    }

    function sessionDiffHasMaterialChanges(diff) {
      return api.sessionDiffHasMaterialChanges(diff);
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
    const payload = await $U.fetchJson("/api/agent/session");
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
    const kind = api.sessionBindingKind();
    const scene = api.currentSceneId();
    const target = api.normalizeTargetKey(targetKey);
    return (Array.isArray(sessions) ? sessions : [])
      .filter(function (session) {
        if (!session || typeof session !== "object") return false;
        const meta = api.parseBoundSessionTitle(session.title);
        if (!meta) return false;
        if (meta.app !== app) return false;
        if (kind === "scene") {
          if (meta.bind !== "scene") return false;
          return String(meta.scene || "") === String(scene || "");
        }
        if (meta.bind === "scene") return false;
        if (meta.target !== target) return false;
        if (scene && meta.scene && meta.scene !== scene) return false;
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
    const desiredTarget = api.normalizeTargetKey(targetKey || api.currentTargetKey());
    const sessions = listBoundSessionsForTarget(
      await fetchAllSessions({ preferCache: true }),
      desiredTarget,
    );
    const current = String(selectedId || state.sessionId || "");
    els.sessionSelect.innerHTML = "";
    const placeholder = document.createElement("sl-option");
    placeholder.value = "";
    placeholder.textContent =
      api.normalizeRouteMode(root.dataset.mode) === "access"
        ? "历史（当前场景）"
        : "历史（当前文件）";
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
    refreshSessionPicker(state.sessionId, api.currentTargetKey()).catch(function () {});
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
      SRC.leaveDiffView();
    } else {
      SRC.destroySourceDiffView();
    }
    state.progress = {
      visible: false,
      label: "",
      detail: "",
      items: [],
    };
    CHR.renderProgressStrip();
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
      CHR.setInlineNote(summary);
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
      '<div class="' + $U.CHAT_CLASS.inlineActions + '">' +
      actions
        .map(function (action, index) {
          return (
            '<button type="button" class="' + $U.CHAT_CLASS.actionButton + '" data-message-id="' +
            $U.escapeHtml(messageId) +
            '" data-action-index="' +
            String(index) +
            '">' +
            $U.escapeHtml(action && action.label ? action.label : "执行") +
            "</button>"
          );
        })
        .join("") +
      "</div>"
    );
  }
  async function applyRevertForMessage(messageId) {
    const sid = String(state.sessionId || "").trim();
    const mid = String(messageId || "").trim();
    if (!sid || !mid) return;
    await $U.fetchJson("/api/agent/session/" + encodeURIComponent(sid) + "/revert", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ message_id: mid }),
    });
    api.setSessionRevertedFlag(sid, true);
    api.setMessageMeta(mid, { reverted: true });
    const revertedIds = api.revertedIdsForSession(sid);
    revertedIds.push(mid);
    api.setRevertedIdsForSession(sid, revertedIds);
    CHR.setInlineNote("已撤回上一轮代码修改。");
    await refreshMessages();
    api.scheduleHostReload("已撤回修改，正在刷新预览与源码…");
  }

  async function applyUnrevertForSession() {
    const sid = String(state.sessionId || "").trim();
    if (!sid) return;
    await $U.fetchJson("/api/agent/session/" + encodeURIComponent(sid) + "/unrevert", {
      method: "POST",
      headers: { "content-type": "application/json" },
    });
    api.setSessionRevertedFlag(sid, false);
    api.setRevertedIdsForSession(sid, []);
    Object.keys(state.messageMeta).forEach(function (key) {
      if (key.startsWith(sid + "::")) {
        state.messageMeta[key] = Object.assign({}, state.messageMeta[key], {
          reverted: false,
        });
      }
    });
    CHR.setInlineNote("已恢复最近撤回的代码修改。");
    await refreshMessages();
    api.scheduleHostReload("已恢复撤回修改，正在刷新预览与源码…");
  }

  function actionsForAssistantMessage(_message) {
    return [];
  }

  async function hydrateBuildDiffMeta(messages) {
    if (!Array.isArray(messages) || !state.sessionId) return;
    if (api.historyUnavailableReason()) {
      CHR.renderHistoryButtons();
      SRC.setDiffTabBadge(0, 0);
      return;
    }
    let changed = false;
    let shouldReloadForPendingBuild = false;
    for (const message of messages) {
      if (!message || String(message.role || "") !== "assistant") continue;
      const messageId = String(message.id || "");
      if (!messageId) continue;
      const meta = api.getMessageMeta(state.sessionId, messageId) || {};
      if (typeof meta.hasDiff === "boolean") continue;
      try {
        const diff = await fetchSessionDiff(messageId);
        const hasDiff = api.sessionDiffHasMaterialChanges(diff);
        api.setMessageMeta(messageId, { hasDiff: hasDiff });
        if (hasDiff) {
          state.messageDiffCache[api.diffCacheKey(state.sessionId, messageId)] = diff;
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
    CHR.renderHistoryButtons();
    SRC.syncSourceDiffEntry();
    void SRC.refreshDiffTabBadge();
    if (shouldReloadForPendingBuild) {
      api.scheduleHostReload("Build 已修改文件，正在刷新预览与源码…");
    }
  }

  function decorateMessageActions() {
    SRC.syncSourceDiffEntry();
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
    CHR.renderHistoryButtons();
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
    const role = $U.escapeHtml(roleRaw);
    const messageId = String(message && message.id ? message.id : "");
    const reverted = roleRaw === "assistant" && api.isMessageReverted(state.sessionId, messageId);
    const classList = [
      $U.CHAT_CLASS.messageBase,
      $U.chatMessageRoleClass(roleRaw, reverted),
    ];
    if (extraClass) classList.push(extraClass);
    const cls = classList.join(" ");
    const roleTextClass = $U.chatRoleTextClass(roleRaw, reverted);
    const blocks = Array.isArray(message && message.blocks) ? message.blocks : [];
    const time = $U.escapeHtml(String(message && message.time ? message.time : ""));
    function blockBodyHtml(block) {
      const content = String(block.content || "");
      const blockType = String(block.type || "text");
      const collapsed = !!block.collapsed;
      if (collapsed) {
        return $U.escapeHtml(content);
      }
      if (blockType !== "text") {
        return $U.escapeHtml(content);
      }
      if (roleRaw === "assistant") {
        return CHR.renderMarkdownToSafeHtml(content);
      }
      return $U.escapeHtml(content);
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
        return $U.CHAT_CLASS.body;
      }
      return $U.CHAT_CLASS.bodyMarkdown;
    }
    const bodyHtml =
      blocks.length > 0
        ? blocks
            .map(function (block) {
              const label = String(block.label || "").trim();
              const blockType = String(block.type || "text");
              const labelToneClass = $U.chatBlockLabelToneClass(blockType);
              const inner = blockBodyHtml(block);
              const tag = blockBodyTag(block);
              const bodyClass = blockBodyClass(block);
              if (block.collapsed) {
                return (
                  '<details class="' +
                  $U.CHAT_CLASS.block +
                  " " +
                  $U.CHAT_CLASS.blockDetails +
                  " author-chat-block-" +
                  $U.escapeHtml(blockType) +
                  '"><summary class="' +
                  $U.CHAT_CLASS.blockSummary +
                  " " +
                  labelToneClass +
                  '">' +
                  $U.escapeHtml(label || "展开") +
                  '</summary><pre class="' +
                  bodyClass +
                  '">' +
                  inner +
                  "</pre></details>"
                );
              }
              return (
                '<section class="' +
                $U.CHAT_CLASS.block +
                " author-chat-block-" +
                $U.escapeHtml(blockType) +
                '">' +
                (label
                  ? '<div class="' + $U.CHAT_CLASS.blockLabel + " " + labelToneClass + '">' + $U.escapeHtml(label) + "</div>"
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
                '<div class="' + $U.CHAT_CLASS.bodyMarkdown + '">' + CHR.renderMarkdownToSafeHtml(fallback) + "</div>"
              );
            }
            return '<pre class="' + $U.CHAT_CLASS.body + '">' + $U.escapeHtml(fallback) + "</pre>";
          })();
    const actions = roleRaw === "assistant" ? renderMessageActions(message, messageId) : "";
    return (
      '<div class="' +
      cls +
      '" data-message-id="' +
      $U.escapeHtml(messageId) +
      '">' +
      '<div class="' + $U.CHAT_CLASS.head + '"><div class="' + $U.CHAT_CLASS.roleBase + " author-chat-role-" +
      role +
      " " +
      roleTextClass +
      '">' +
      (roleRaw === "user" ? "我" : roleRaw === "assistant" ? $U.escapeHtml(state.modelLabel || "模型") : "系统") +
      '</div><div class="' + $U.CHAT_CLASS.meta + '"><span class="' + $U.CHAT_CLASS.time + '">' +
      time +
      '</span><button type="button" class="' +
      $U.CHAT_CLASS.copyButton +
      '" title="复制对话内容（Markdown 原文）" data-message-id="' +
      $U.escapeHtml(messageId) +
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
        '<div class="' + $U.CHAT_CLASS.empty + '">未选择会话。可先点击“新建对话”，或等待宿主自动创建/恢复会话。</div>';
      restoreChatScroll(scrollSnapshot, shouldStickBottom);
      return;
    }
    if (!state.messages.length) {
      els.chatLog.innerHTML =
        '<div class="' + $U.CHAT_CLASS.empty + '">发送任务后，这里会连续显示输入、参考信息和模型回复。</div>';
      restoreChatScroll(scrollSnapshot, shouldStickBottom);
      return;
    }
    const rounds = api.conversationRounds(state.messages);
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
          '<section class="' + $U.CHAT_CLASS.round + '">' +
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

  async function respondPermissionRequest(permissionId, responseKind) {
    const sid = String(state.sessionId || "").trim();
    const pid = String(permissionId || "").trim();
    const reply = String(responseKind || "").trim();
    if (!sid || !pid || !reply) return;
    await $U.fetchJson(
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
    CHR.setInlineNote("权限请求已处理：permission_id=" + pid + "，response=" + reply);
  }

  function rememberSession() {
    try {
      if (state.sessionId) {
        localStorage.setItem(api.sessionStorageKey(), state.sessionId);
      } else {
        localStorage.removeItem(api.sessionStorageKey());
      }
    } catch (_) {}
  }

  function restoreSession() {
    state.sessionId = "";
    try {
      const saved = localStorage.getItem(api.sessionStorageKey());
      if (saved) state.sessionId = saved;
    } catch (_) {}
  }

  async function refreshAll() {
    let refreshFailed = false;
    const previousTargetKey = String(state.sessionTargetKey || "");
    state.loading = true;
    CHR.setButtonState(true);
    CHR.renderStatus();
    try {
      const [config, runtime, skillStatus] = await Promise.all([
        $U.fetchJson("/api/agent/config"),
        $U.fetchJson("/api/agent/runtime"),
        $U.fetchJson("/api/agent/skill"),
      ]);
      state.config = config;
      state.runtime = runtime;
      state.skillStatus = skillStatus;
      state.sessionTargetKey = api.currentSessionBindingFingerprint();
      if (state.sessionTargetKey !== previousTargetKey) {
        state._meiAutoSessionOnce = false;
      }
      let runtimeRef = runtime;
      if (runtimeRef && runtimeRef.running) {
        try {
          state.health = await $U.fetchJson("/api/agent/health");
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
      CHR.setInlineNote("读取助手状态失败：" + String(error.message || error));
    } finally {
      state.loading = false;
      CHR.setButtonState(false);
      CHR.renderStatus();
      CHR.renderConfig();
      CHR.renderRuntime();
      CHR.renderSkillStatus();
      await CTX.refreshModelProbe(true).catch(function () {});
      await CTX.refreshContextPreview().catch(function () {});
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
      if (state.health && state.health.healthy && state.sessionId) {
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
    const nextFingerprint = String(state.sessionId) + "|" + JSON.stringify(list);
    state.progress = api.deriveProgressFromMessages(list);
    CHR.renderProgressStrip();
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
        const pendingPayload = await $U.fetchJson(
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
      const meta = api.getMessageMeta(state.sessionId, messageId);
      if (!meta || !meta.agent) {
        api.setMessageMeta(messageId, { agent: inferred, hasDiff: null, reverted: false });
      }
    });
    state.messages = list.map(normalizeMessage);
    SRC.syncSourceDiffEntry();
    const mergedBlocked = mergeBlockedPermissionNotices(pendingBlocked, runningBlocked);
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
    const body = {
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
      normalizeMessage: normalizeMessage,
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
      blockedPermissionNoticeFromData: blockedPermissionNoticeFromData,
      rememberBlockedPermissionNotice: rememberBlockedPermissionNotice,
    };
  };
})(window);
