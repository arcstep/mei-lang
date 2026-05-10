(function () {
  const root = document.getElementById("meilang-author-panel");
  if (!root) return;

  const els = {
    serverDot: document.getElementById("author-server-dot"),
    serverStatus: document.getElementById("author-server-status"),
    config: document.getElementById("author-config-line"),
    reconnect: document.getElementById("author-reconnect-btn"),
    newSession: document.getElementById("author-session-btn"),
    sessionSelect: document.getElementById("author-session-select"),
    chatLog: document.getElementById("author-chat-log"),
    input: document.getElementById("author-intent-input"),
    run: document.getElementById("author-run-btn"),
  };

  const state = {
    config: null,
    runtime: null,
    health: null,
    sessions: [],
    sessionId: "",
    sessionTargetKey: "",
    messages: [],
    loading: false,
    sending: false,
    eventSource: null,
    eventSourceSessionId: "",
    streamConnected: false,
    modelLabel: "模型",
    sessionsCacheAtMs: 0,
    sessionsFetchInFlight: null,
    _meiAutoSessionOnce: false,
    _meiClientAutoOpencodeOnce: false,
  };

  const sessionStorageKey =
    "mei-lang.opencode.session." +
    String(root.dataset.app || "") +
    "." +
    String(root.dataset.target || "");
  const SESSION_CACHE_KEY = "mei.author.sessions.v1";
  const SESSION_CACHE_TTL_MS = 30000;

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
      throw new Error(url + " -> " + response.status);
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

  function setButtonState(disabled) {
    if (els.reconnect) els.reconnect.disabled = disabled;
    if (els.newSession) els.newSession.disabled = disabled;
    if (els.sessionSelect) els.sessionSelect.disabled = disabled;
    if (els.run) els.run.disabled = disabled || state.sending;
  }

  function renderStatus() {
    if (!els.serverStatus || !els.serverDot) return;
    const runtime = state.runtime;
    const health = state.health;
    const config = state.config;
    let label = "已断开";
    let dotClass = "author-server-dot author-server-dot-off";
    if (state.loading) {
      label = "刷新中";
    } else if (runtime && runtime.running && state.streamConnected) {
      label = "会话中";
      dotClass = "author-server-dot author-server-dot-on";
    } else if (runtime && runtime.running && health && health.healthy) {
      label = "已连接";
      dotClass = "author-server-dot author-server-dot-on";
    } else if (config && config.runtime_env_ready) {
      label = "可启动";
    }
    els.serverStatus.textContent = label;
    els.serverDot.className = dotClass;

    if (!els.config) return;
    const parts = [];
    parts.push("app=" + String(root.dataset.app || ""));
    parts.push("target=" + currentTargetKey());
    if (config && config.provider_id) parts.push(String(config.provider_id));
    if (config && config.completion_model) parts.push(String(config.completion_model));
    if (runtime && runtime.server_url) parts.push("server=" + runtime.server_url);
    if (config && Array.isArray(config.missing_env) && config.missing_env.length > 0) {
      parts.push("缺少环境变量：" + config.missing_env.join(", "));
    }
    els.config.textContent = parts.join(" · ");
  }

  function renderConfig() {
    const config = state.config;
    if (!config) {
      state.modelLabel = "模型";
      return;
    }
    state.modelLabel =
      String(config.completion_model || config.provider_name || config.provider_id || "模型").trim() ||
      "模型";
  }

  function renderRuntime() {
    renderStatus();
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

  function renderMessages() {
    if (!els.chatLog) return;
    if (!state.sessionId) {
      els.chatLog.innerHTML =
        '<div class="author-chat-empty">未选择会话。可先点击“新建对话”，或等待宿主自动创建/恢复会话。</div>';
      return;
    }
    if (!state.messages.length) {
      els.chatLog.innerHTML =
        '<div class="author-chat-empty">发送任务后，这里会连续显示输入、参考信息和模型回复。</div>';
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

    els.chatLog.scrollTop = els.chatLog.scrollHeight;
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
    if (!(state.runtime && state.runtime.running) || !sessionId) {
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
      if (st === "opencode_unavailable" || st === "upstream_unavailable") {
        state.streamConnected = false;
        closeEventStream();
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
      refreshMessages().catch(function () {});
      return;
    }
    if (kind === "permission_requested") {
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
      const [config, runtime] = await Promise.all([
        fetchJson("/api/opencode/config"),
        fetchJson("/api/opencode/runtime"),
      ]);
      state.config = config;
      state.runtime = runtime;
      state.sessionTargetKey = currentTargetKey();
      let runtimeRef = runtime;
      if (
        !state._meiClientAutoOpencodeOnce &&
        config &&
        config.runtime_env_ready &&
        config.config_content_ready &&
        runtimeRef &&
        !runtimeRef.running
      ) {
        state._meiClientAutoOpencodeOnce = true;
        try {
          await fetchJson("/api/opencode/start", {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ port: 4099 }),
          });
          state.runtime = await fetchJson("/api/opencode/runtime");
          runtimeRef = state.runtime;
        } catch (_) {
          try {
            state.runtime = await fetchJson("/api/opencode/runtime");
            runtimeRef = state.runtime;
          } catch (_) {}
        }
      }
      if (runtimeRef && runtimeRef.running) {
        try {
          state.health = await fetchJson("/api/opencode/health");
        } catch (_) {
          state.health = null;
        }
        try {
          state.sessions = await fetchAllSessions({ preferCache: true });
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
      if (els.config) {
        els.config.textContent = "读取 OpenCode 状态失败：" + String(error.message || error);
      }
    } finally {
      state.loading = false;
      setButtonState(false);
      renderStatus();
      renderConfig();
      renderRuntime();
      const boundSessions = listBoundSessionsForTarget(state.sessions, state.sessionTargetKey);
      if (state.sessionId && !sessionIdInList(state.sessions, state.sessionId)) {
        state.sessionId = "";
        state.messages = [];
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
        state.runtime &&
        state.runtime.running &&
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
      if (state.runtime && state.runtime.running && state.sessionId) {
        await refreshMessages();
        connectEvents(false);
      } else {
        closeEventStream();
        renderMessages();
      }
    }
  }

  async function startServer() {
    setButtonState(true);
    if (els.config) els.config.textContent = "正在启动 OpenCode 服务...";
    try {
      await fetchJson("/api/opencode/start", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ port: 4099 }),
      });
    } catch (error) {
      if (els.config) {
        els.config.textContent = "启动失败：" + String(error.message || error);
      }
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
    const wasRunning = state.runtime && state.runtime.running;
    if (!wasRunning) {
      await startServer();
      if (state.sessionId) {
        return;
      }
    }
    await postNewBoundSession();
  }

  async function refreshMessages() {
    if (!state.sessionId || !(state.runtime && state.runtime.running)) {
      closeEventStream();
      renderMessages();
      return;
    }
    const payload = await fetchJson(
      "/api/opencode/session/" +
        encodeURIComponent(state.sessionId) +
        "/messages?limit=80",
    );
    const list = payload && Array.isArray(payload.messages) ? payload.messages : [];
    state.messages = list.map(normalizeMessage);
    renderMessages();
  }

  async function sendPrompt() {
    const text = (els.input && els.input.value ? els.input.value : "").trim();
    if (!text) {
      if (els.input) els.input.focus();
      return;
    }
    if (!(state.runtime && state.runtime.running)) {
      await startServer();
    }
    if (!state.sessionId) {
      await createSession();
    }
    state.sessionTargetKey = currentTargetKey();
    state.sending = true;
    setButtonState(true);
    try {
      await fetchJson("/api/opencode/session/" + encodeURIComponent(state.sessionId) + "/message", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ text: text }),
      });
      if (els.input) {
        els.input.value = "";
      }
      await refreshMessages();
      connectEvents(false);
    } catch (error) {
      if (els.config) {
        els.config.textContent = "发送失败：" + String(error.message || error);
      }
    } finally {
      state.sending = false;
      setButtonState(false);
    }
  }

  if (els.reconnect) {
    els.reconnect.addEventListener("click", function () {
      const action = state.runtime && state.runtime.running ? refreshAll : startServer;
      action().catch(function (error) {
        if (els.config) {
          els.config.textContent = "重连失败：" + String(error.message || error);
        }
      });
    });
  }

  if (els.newSession) {
    els.newSession.addEventListener("click", function () {
      createSession().catch(function (error) {
        if (els.config) {
          els.config.textContent = "创建会话失败：" + String(error.message || error);
        }
      });
    });
  }

  if (els.sessionSelect) {
    els.sessionSelect.addEventListener("change", function () {
      state.sessionId = String(els.sessionSelect.value || "");
      state.sessionTargetKey = currentTargetKey();
      rememberSession();
      refreshMessages().catch(function (error) {
        if (els.config) {
          els.config.textContent = "读取会话失败：" + String(error.message || error);
        }
      });
      connectEvents(true);
    });
  }

  if (els.run) {
    els.run.addEventListener("click", function () {
      sendPrompt().catch(function (error) {
        if (els.config) {
          els.config.textContent = "发送失败：" + String(error.message || error);
        }
      });
    });
  }

  if (els.input) {
    els.input.addEventListener("keydown", function (event) {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        sendPrompt().catch(function (error) {
          if (els.config) {
            els.config.textContent = "发送失败：" + String(error.message || error);
          }
        });
      }
    });
  }

  restoreSession();
  refreshAll();
  window.addEventListener("beforeunload", closeEventStream);
  window.setInterval(function () {
    refreshAll().catch(function () {});
  }, 8000);
})();
