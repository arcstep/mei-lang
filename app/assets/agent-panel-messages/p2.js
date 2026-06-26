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
    if (!api.panelAuthoringEnabled()) {
      CHR.renderHistoryButtons();
      SRC.setDiffTabBadge(0, 0);
      return;
    }
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
      } catch (_) {
        api.setMessageMeta(messageId, { hasDiff: false });
        changed = true;
      }
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
        M.copyText(text)
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
    if ($U.areAgentRequestsBlocked()) {
      return false;
    }
    let refreshFailed = false;
    const previousTargetKey = String(state.sessionTargetKey || "");
    state.loading = true;
    CHR.setButtonState(true);
    CHR.renderStatus();
    try {
      const config = await $U.fetchJson("/api/agent/config");
      if ($U.areAgentRequestsBlocked()) {
        refreshFailed = true;
        throw new Error("agent auth blocked");
      }
      const [runtime, skillStatus] = await Promise.all([
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
