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

    const M = global.MeiAgentPanelMessagesModel;
    if (!M || typeof M.normalizeMessage !== "function") {
      console.error(
        "MeiAgentPanelMessagesModel missing: ensure agent-panel-messages-model.js is bundled before agent-panel-messages.js",
      );
      return {};
    }

    const SESSION_PATCH_STORAGE_PREFIX = "mei.agent.session_patch.v1";

    function cssEscape(value) {
      if (window.CSS && typeof window.CSS.escape === "function") {
        return window.CSS.escape(String(value || ""));
      }
      return String(value || "").replace(/["\\]/g, "\\$&");
    }

    function safeParseJson(raw) {
      const text = String(raw || "").trim();
      if (!text) return null;
      try {
        return JSON.parse(text);
      } catch (_) {
        return null;
      }
    }

    function sessionPatchStorageKey() {
      const app = String(root.dataset.app || api.currentAppKey() || "").trim() || "unknown";
      const scene = String(api.currentSceneId() || "").trim() || "scene";
      const sid = String(state.sessionId || "").trim() || "session";
      return [SESSION_PATCH_STORAGE_PREFIX, app, scene, sid].join(":");
    }

    function clearSessionPatchDomEffects() {
      document
        .querySelectorAll("[data-mei-session-patch-hidden='1'],[data-mei-session-patch-highlight='1']")
        .forEach(function (node) {
          node.removeAttribute("data-mei-session-patch-hidden");
          node.removeAttribute("data-mei-session-patch-highlight");
        });
    }

    function loadSessionPatchesFromStorage() {
      if (!window.sessionStorage) return [];
      try {
        const raw = window.sessionStorage.getItem(sessionPatchStorageKey());
        const parsed = safeParseJson(raw);
        return Array.isArray(parsed) ? parsed : [];
      } catch (_) {
        return [];
      }
    }

    function saveSessionPatchesToStorage(offers) {
      if (!window.sessionStorage) return;
      try {
        const rows = Array.isArray(offers) ? offers : [];
        if (!rows.length) {
          window.sessionStorage.removeItem(sessionPatchStorageKey());
          return;
        }
        window.sessionStorage.setItem(sessionPatchStorageKey(), JSON.stringify(rows));
      } catch (_) {}
    }

    function extractSessionPatchOffers(rawMessages) {
      const offers = [];
      (Array.isArray(rawMessages) ? rawMessages : []).forEach(function (raw) {
        if (!raw || String(raw.role || "") !== "assistant") return;
        const parts = Array.isArray(raw.parts) ? raw.parts : [];
        parts.forEach(function (part) {
          if (!part || String(part.part_type || "") !== "tool") return;
          const tool = part.tool || null;
          if (!tool || String(tool.tool || "") !== "propose_session_patch") return;
          if (String(tool.status || "") !== "completed") return;
          const payload = safeParseJson(tool.output);
          const patch = payload && payload.patch && typeof payload.patch === "object" ? payload.patch : null;
          const ops = Array.isArray(patch && patch.ops) ? patch.ops : [];
          if (!patch || !ops.length) return;
          offers.push({
            offer_id: String(payload.call_id || tool.call_id || part.part_id || "").trim(),
            summary: String(payload.summary || "").trim(),
            patch: {
              schema: String(patch.schema || "mei_session_patch_v1"),
              patch_id: String(patch.patch_id || "").trim(),
              ops: ops,
            },
          });
        });
      });
      return offers;
    }

    function applySessionPatchesToDom(offers) {
      clearSessionPatchDomEffects();
      let opCount = 0;
      (Array.isArray(offers) ? offers : []).forEach(function (offer) {
        const ops = Array.isArray(offer && offer.patch && offer.patch.ops) ? offer.patch.ops : [];
        ops.forEach(function (rawOp) {
          const op = rawOp && typeof rawOp === "object" ? rawOp : {};
          const type = String(op.type || "").trim();
          if (!type) return;
          if (type === "focus_query_state") {
            opCount += 1;
            return;
          }
          const panelId = String(op.panel_id || "").trim();
          if (!panelId) return;
          const selector = "[data-mei-panel-id='" + cssEscape(panelId) + "']";
          const nodes = Array.from(document.querySelectorAll(selector));
          if (!nodes.length) return;
          if (type === "hide_panel") {
            nodes.forEach(function (node) {
              node.setAttribute("data-mei-session-patch-hidden", "1");
            });
            opCount += 1;
            return;
          }
          if (type === "highlight_panel") {
            nodes.forEach(function (node) {
              node.setAttribute("data-mei-session-patch-highlight", "1");
            });
            opCount += 1;
            return;
          }
          if (type === "move_panel_front") {
            nodes.forEach(function (node) {
              const parent = node.parentElement;
              if (parent && parent.firstElementChild !== node) {
                parent.insertBefore(node, parent.firstElementChild);
              }
            });
            opCount += 1;
          }
        });
      });
      window.__meiAccessSessionPatchState = {
        schema: "mei_session_patch_state_v1",
        offer_count: Array.isArray(offers) ? offers.length : 0,
        op_count: opCount,
      };
      return opCount;
    }

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

  function rememberBlockedPermissionNotice(notice) {
    state.pendingPermissionNotices = M.mergeBlockedPermissionNotices(
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
