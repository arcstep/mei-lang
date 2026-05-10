use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::UiRouteMode;

pub(super) fn panel_view(compiled: &CompiledApp, route_mode: UiRouteMode) -> impl IntoView {
    view! {
        <div
            id="meilang-opencode-root"
            class="opencode-panel"
            data-app-id=compiled.app_id.clone()
            data-entry-target=compiled.entry_target.clone()
            data-mode=route_mode.slug()
        >
            <div class="opencode-section">
                <div class="opencode-line">
                    <span class="opencode-label">"状态"</span>
                    <strong id="meilang-opencode-status" class="opencode-badge opencode-badge-idle">"加载中"</strong>
                </div>
                <p id="meilang-opencode-summary" class="opencode-summary">
                    "正在读取 OpenCode 配置与运行态..."
                </p>
            </div>
            <div class="opencode-actions">
                <button id="meilang-opencode-refresh" class="opencode-btn opencode-btn-muted" type="button">
                    "刷新"
                </button>
                <button id="meilang-opencode-start" class="opencode-btn" type="button">
                    "启动"
                </button>
                <button id="meilang-opencode-stop" class="opencode-btn opencode-btn-danger" type="button">
                    "停止"
                </button>
            </div>
            <div class="opencode-section">
                <div class="opencode-line">
                    <div class="panel-heading">
                        <h3>"会话"</h3>
                        <p>"session bridge"</p>
                    </div>
                    <button id="meilang-opencode-new-session" class="opencode-btn opencode-btn-muted" type="button">
                        "新会话"
                    </button>
                </div>
                <select id="meilang-opencode-session-select" class="opencode-select">
                    <option value="">"未选择会话"</option>
                </select>
            </div>
            <div class="opencode-section">
                <div class="panel-heading">
                    <h3>"配置"</h3>
                    <p>"provider / model / env"</p>
                </div>
                <ul id="meilang-opencode-config" class="opencode-list">
                    <li>"正在读取配置..."</li>
                </ul>
            </div>
            <div class="opencode-section">
                <div class="panel-heading">
                    <h3>"运行态"</h3>
                    <p>"managed server health"</p>
                </div>
                <ul id="meilang-opencode-runtime" class="opencode-list">
                    <li>"正在读取运行态..."</li>
                </ul>
            </div>
            <div class="opencode-section">
                <div class="panel-heading">
                    <h3>"当前上下文"</h3>
                    <p>"host shell context"</p>
                </div>
                <ul class="opencode-list">
                    <li>{format!("当前应用：{}", compiled.app_id)}</li>
                    <li>{format!("入口脚本：{}", compiled.entry_target)}</li>
                    <li>{format!("模式：{}", route_mode.slug())}</li>
                </ul>
            </div>
            <div class="opencode-section">
                <div class="panel-heading">
                    <h3>"对话"</h3>
                    <p>"minimal session chat"</p>
                </div>
                <div id="meilang-opencode-messages" class="opencode-messages">
                    <div class="opencode-message opencode-message-system">
                        <div class="opencode-message-role">"system"</div>
                        <div class="opencode-message-body">"等待会话初始化..."</div>
                    </div>
                </div>
                <textarea
                    id="meilang-opencode-input"
                    class="opencode-input"
                    rows="5"
                    placeholder="输入消息；Cmd/Ctrl+Enter 发送"
                ></textarea>
                <div class="opencode-actions">
                    <button id="meilang-opencode-send" class="opencode-btn" type="button">
                        "发送"
                    </button>
                </div>
            </div>
        </div>
    }
}

pub(super) const BOOTSTRAP_SCRIPT: &str = r#"
(function () {
  const root = document.getElementById("meilang-opencode-root");
  if (!root) return;

  const els = {
    status: document.getElementById("meilang-opencode-status"),
    summary: document.getElementById("meilang-opencode-summary"),
    config: document.getElementById("meilang-opencode-config"),
    runtime: document.getElementById("meilang-opencode-runtime"),
    refresh: document.getElementById("meilang-opencode-refresh"),
    start: document.getElementById("meilang-opencode-start"),
    stop: document.getElementById("meilang-opencode-stop"),
    newSession: document.getElementById("meilang-opencode-new-session"),
    sessionSelect: document.getElementById("meilang-opencode-session-select"),
    messages: document.getElementById("meilang-opencode-messages"),
    input: document.getElementById("meilang-opencode-input"),
    send: document.getElementById("meilang-opencode-send"),
  };

  const state = {
    config: null,
    runtime: null,
    health: null,
    sessions: [],
    sessionId: "",
    messages: [],
    loading: false,
    sending: false,
    eventSource: null,
    eventSourceSessionId: "",
    streamConnected: false,
    modelLabel: "模型",
  };

  const sessionStorageKey =
    "mei-lang.opencode.session." +
    String(root.dataset.appId || "") +
    "." +
    String(root.dataset.entryTarget || "");

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

  function setButtonState(disabled) {
    if (els.refresh) els.refresh.disabled = disabled;
    if (els.start) els.start.disabled = disabled;
    if (els.stop) els.stop.disabled = disabled;
    if (els.newSession) els.newSession.disabled = disabled;
    if (els.sessionSelect) els.sessionSelect.disabled = disabled;
    if (els.send) els.send.disabled = disabled || state.sending;
  }

  function renderList(target, items) {
    if (!target) return;
    target.innerHTML = items.map((item) => "<li>" + item + "</li>").join("");
  }

  function renderStatus() {
    if (!els.status || !els.summary) return;
    const runtime = state.runtime;
    const health = state.health;
    const config = state.config;
    let label = "未配置";
    let badge = "opencode-badge opencode-badge-idle";
    if (state.loading) {
      label = "刷新中";
      badge = "opencode-badge opencode-badge-busy";
    } else if (runtime && runtime.running && health && health.healthy) {
      label = "运行中";
      badge = "opencode-badge opencode-badge-ok";
    } else if (runtime && runtime.running) {
      label = "已启动";
      badge = "opencode-badge opencode-badge-warn";
    } else if (config && config.runtime_env_ready) {
      label = "可启动";
      badge = "opencode-badge opencode-badge-warn";
    }
    els.status.className = badge;
    els.status.textContent = label;

    const parts = [];
    if (runtime && runtime.server_url) parts.push("server=" + runtime.server_url);
    if (health && health.version) parts.push("health=v" + health.version);
    if (config && config.default_model) parts.push("model=" + config.default_model);
    if (config && Array.isArray(config.missing_env) && config.missing_env.length > 0) {
      parts.push("缺少环境变量：" + config.missing_env.join(", "));
    }
    els.summary.textContent = parts.length > 0 ? parts.join(" · ") : "OpenCode 宿主桥接已接入，等待进一步配置。";
  }

  function renderConfig() {
    const config = state.config;
    if (!config) {
      state.modelLabel = "模型";
      renderList(els.config, ["尚未读取到配置"]);
      return;
    }
    state.modelLabel = String(config.default_model || config.provider_name || "模型").trim() || "模型";
    const items = [
      "provider: " + escapeHtml(config.provider_name || config.provider_id || "-"),
      "default_model: " + escapeHtml(config.default_model || "-"),
      "base_url: " + escapeHtml(config.base_url || "-"),
      "config_root: " + escapeHtml(config.config_root || "-"),
      "dotenv_path: " + escapeHtml(config.dotenv_path || "-"),
      "opencode.json: " + escapeHtml(config.project_config_path || "-"),
      "project_config_present: " + escapeHtml(String(config.project_config_present === true)),
      "runtime_env_ready: " + escapeHtml(String(config.runtime_env_ready === true)),
    ];
    if (Array.isArray(config.missing_env) && config.missing_env.length > 0) {
      items.push("missing_env: " + escapeHtml(config.missing_env.join(", ")));
    }
    renderList(els.config, items);
  }

  function renderRuntime() {
    const runtime = state.runtime;
    const health = state.health;
    if (!runtime) {
      renderList(els.runtime, ["尚未读取到运行态"]);
      return;
    }
    const items = [
      "running: " + escapeHtml(String(runtime.running === true)),
      "server_url: " + escapeHtml(runtime.server_url || "-"),
      "pid: " + escapeHtml(runtime.pid != null ? String(runtime.pid) : "-"),
      "working_directory: " + escapeHtml(runtime.working_directory || "-"),
      "health: " + escapeHtml(health && health.healthy ? "ok" : "offline"),
    ];
    if (runtime.last_exit) {
      items.push(
        "last_exit: " +
          escapeHtml(
            String(runtime.last_exit.kind || "-") +
              " / " +
              String(runtime.last_exit.code != null ? runtime.last_exit.code : "-")
          )
      );
    }
    renderList(els.runtime, items);
  }

  function renderSessions() {
    if (!els.sessionSelect) return;
    const options = ['<option value="">未选择会话</option>'];
    state.sessions.forEach((session) => {
      const selected = session.id === state.sessionId ? ' selected' : '';
      const title = escapeHtml(session.title || session.id || "session");
      options.push(
        '<option value="' +
          escapeHtml(session.id) +
          '"' +
          selected +
          '>' +
          title +
          '</option>'
      );
    });
    els.sessionSelect.innerHTML = options.join("");
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

  function normalizeMessage(raw) {
    const parts = Array.isArray(raw && raw.parts) ? raw.parts : [];
    const role = String((raw && raw.role) || "assistant");
    const textParts = [];
    const reasoningParts = [];
    const toolParts = [];
    const debugParts = [];
    parts.forEach((part) => {
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
    toolParts.forEach((toolBody, idx) => {
      const block = makeTextBlock("工具调用 #" + String(idx + 1), toolBody, "tool", true);
      if (block) blocks.push(block);
    });
    debugParts.forEach((debugBody, idx) => {
      const block = makeTextBlock("结构化片段 #" + String(idx + 1), debugBody, "debug", true);
      if (block) blocks.push(block);
    });
    const body =
      blocks.length > 0
        ? blocks
            .map((block) => (block.label ? "[" + block.label + "]\n" : "") + block.content)
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
      '<div class="opencode-message-actions">' +
      actions
        .map(function (action, index) {
          return (
            '<button type="button" class="opencode-btn opencode-btn-muted opencode-action-btn" data-message-id="' +
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
    if (!els.messages) return;
    if (!state.sessionId) {
      els.messages.innerHTML =
        '<div class="opencode-message opencode-message-system"><div class="opencode-message-role">system</div><div class="opencode-message-body">未选择会话。可先点击“新会话”，或启动服务后选择已有会话。</div></div>';
      return;
    }
    if (!state.messages.length) {
      els.messages.innerHTML =
        '<div class="opencode-message opencode-message-system"><div class="opencode-message-role">system</div><div class="opencode-message-body">当前会话暂无消息。</div></div>';
      return;
    }
    els.messages.innerHTML = state.messages
      .map((message) => {
        const role = escapeHtml(message.role || "assistant");
        const cls =
          role === "user"
            ? "opencode-message opencode-message-user"
            : role === "assistant"
              ? "opencode-message opencode-message-assistant"
              : "opencode-message opencode-message-system";
        const blocks = Array.isArray(message.blocks) ? message.blocks : [];
        const bodyHtml =
          blocks.length > 0
            ? blocks
                .map(function (block) {
                  const label = String(block.label || "").trim();
                  const content = escapeHtml(block.content || "");
                  if (block.collapsed) {
                    return (
                      '<details class="opencode-message-block opencode-message-block-' +
                      escapeHtml(block.type || "text") +
                      '"><summary class="opencode-message-block-label">' +
                      escapeHtml(label || "展开") +
                      '</summary><pre class="opencode-message-body">' +
                      content +
                      "</pre></details>"
                    );
                  }
                  return (
                    '<section class="opencode-message-block opencode-message-block-' +
                    escapeHtml(block.type || "text") +
                    '">' +
                    (label
                      ? '<div class="opencode-message-block-label">' + escapeHtml(label) + "</div>"
                      : "") +
                    '<pre class="opencode-message-body">' +
                    content +
                    "</pre></section>"
                  );
                })
                .join("")
            : '<pre class="opencode-message-body">' + escapeHtml(message.body || "") + "</pre>";
        return (
          '<div class="' +
          cls +
          '">' +
          '<div class="opencode-message-role">' +
          role +
          '</div>' +
          bodyHtml +
          renderMessageActions(message, message.id || "") +
          '</div>'
        );
      })
      .join("");
    Array.from(els.messages.querySelectorAll(".opencode-action-btn")).forEach(function (button) {
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
    els.messages.scrollTop = els.messages.scrollHeight;
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
    if (
      state.eventSource &&
      state.eventSourceSessionId === sessionId &&
      !forceReconnect
    ) {
      return;
    }
    closeEventStream();
    try {
      const source = new EventSource(
        "/api/opencode/session/" +
          encodeURIComponent(sessionId) +
          "/events"
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
      }
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
      if (String(event.status || "") === "connected") {
        state.streamConnected = true;
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
        }
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
        }
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
      if (runtime && runtime.running) {
        try {
          state.health = await fetchJson("/api/opencode/health");
        } catch (_) {
          state.health = null;
        }
        try {
          state.sessions = await fetchJson("/api/opencode/session");
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
      if (els.summary) {
        els.summary.textContent = "读取 OpenCode 状态失败：" + String(error.message || error);
      }
    } finally {
      state.loading = false;
      setButtonState(false);
      renderStatus();
      renderConfig();
      renderRuntime();
      if (state.sessionId && !state.sessions.some((item) => item.id === state.sessionId)) {
        state.sessionId = "";
        state.messages = [];
        rememberSession();
      }
      if (!state.sessionId && state.sessions.length > 0) {
        const savedId = String(state.sessionId || "").trim();
        const saved = savedId
          ? state.sessions.find((item) => item.id === savedId)
          : null;
        const preferred = saved || state.sessions[0];
        state.sessionId = preferred ? preferred.id : "";
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
    if (els.summary) els.summary.textContent = "正在启动 OpenCode 服务...";
    try {
      await fetchJson("/api/opencode/start", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ port: 4099 }),
      });
    } catch (error) {
      if (els.summary) {
        els.summary.textContent = "启动失败：" + String(error.message || error);
      }
    }
    await refreshAll();
  }

  async function stopServer() {
    closeEventStream();
    setButtonState(true);
    if (els.summary) els.summary.textContent = "正在停止 OpenCode 服务...";
    try {
      await fetchJson("/api/opencode/stop", {
        method: "POST",
      });
    } catch (error) {
      if (els.summary) {
        els.summary.textContent = "停止失败：" + String(error.message || error);
      }
    }
    await refreshAll();
  }

  function buildSessionTitle() {
    const app = String(root.dataset.appId || "").trim();
    const target = String(root.dataset.entryTarget || "").trim();
    return [app, target].filter(Boolean).join(" · ") || "MeiLang Session";
  }

  async function createSession() {
    if (!(state.runtime && state.runtime.running)) {
      await startServer();
    }
    const session = await fetchJson("/api/opencode/session", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ title: buildSessionTitle() }),
    });
    state.sessionId = session.id || "";
    rememberSession();
    await refreshAll();
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
        "/messages?limit=80"
    );
    const list =
      payload && Array.isArray(payload.messages) ? payload.messages : [];
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
    state.sending = true;
    setButtonState(true);
    try {
      await fetchJson(
        "/api/opencode/session/" +
          encodeURIComponent(state.sessionId) +
          "/message",
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ text: text }),
        }
      );
      if (els.input) {
        els.input.value = "";
      }
      await refreshMessages();
      connectEvents(false);
    } catch (error) {
      if (els.summary) {
        els.summary.textContent = "发送失败：" + String(error.message || error);
      }
    } finally {
      state.sending = false;
      setButtonState(false);
    }
  }

  if (els.refresh) els.refresh.addEventListener("click", refreshAll);
  if (els.start) els.start.addEventListener("click", startServer);
  if (els.stop) els.stop.addEventListener("click", stopServer);
  if (els.newSession) els.newSession.addEventListener("click", function () {
    createSession().catch(function (error) {
      if (els.summary) {
        els.summary.textContent = "创建会话失败：" + String(error.message || error);
      }
    });
  });
  if (els.sessionSelect) {
    els.sessionSelect.addEventListener("change", function () {
      state.sessionId = String(els.sessionSelect.value || "");
      rememberSession();
      refreshMessages().catch(function (error) {
        if (els.summary) {
          els.summary.textContent = "读取会话失败：" + String(error.message || error);
        }
      });
      connectEvents(true);
    });
  }
  if (els.send) {
    els.send.addEventListener("click", function () {
      sendPrompt().catch(function (error) {
        if (els.summary) {
          els.summary.textContent = "发送失败：" + String(error.message || error);
        }
      });
    });
  }
  if (els.input) {
    els.input.addEventListener("keydown", function (event) {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        sendPrompt().catch(function (error) {
          if (els.summary) {
            els.summary.textContent = "发送失败：" + String(error.message || error);
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
"#;
