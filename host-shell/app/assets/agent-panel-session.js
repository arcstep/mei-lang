/**
 * 会话传输层：EventSource、SSE 事件分发、后台刷新轮询。由 agent-panel 主文件装配 `SES`。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallSession = function (api) {
    const POLL_ACTIVE_MS = 30000;
    const POLL_IDLE_MS = 120000;
    const POLL_STREAM_HEALTHY_MS = 180000;
    const POLL_MAX_MS = 300000;

    let refreshTimerId = 0;
    let refreshPollFailureCount = 0;
    let refreshPollInFlight = false;

    function closeEventStream() {
      api.clearGenerationSettleTimer();
      if (api.state.eventSource) {
        try {
          api.state.eventSource.close();
        } catch (_) {}
      }
      api.state.eventSource = null;
      api.state.eventSourceSessionId = "";
      api.state.streamConnected = false;
    }

    function connectEvents(forceReconnect) {
      const sessionId = String(api.state.sessionId || "").trim();
      if (!(api.state.health && api.state.health.healthy) || !sessionId) {
        closeEventStream();
        return;
      }
      if (
        api.state.eventSource &&
        api.state.eventSourceSessionId === sessionId &&
        !forceReconnect
      ) {
        return;
      }
      closeEventStream();
      try {
        const source = new EventSource(
          "/api/agent/session/" + encodeURIComponent(sessionId) + "/events",
        );
        source.onopen = function () {
          api.state.streamConnected = true;
          api.renderStatus();
        };
        source.onerror = function () {
          api.state.streamConnected = false;
          api.renderStatus();
        };
        source.onmessage = function (event) {
          try {
            applyHostEvent(JSON.parse(String(event.data || "{}")));
          } catch (_) {}
        };
        api.state.eventSource = source;
        api.state.eventSourceSessionId = sessionId;
      } catch (_) {
        api.state.streamConnected = false;
        api.renderStatus();
      }
    }

    function applyHostEvent(event) {
      if (!event || typeof event !== "object") return;
      const kind = String(event.kind || "");
      if (!kind) return;
      if (kind === "session_status") {
        const st = String(event.status || "");
        if (st === "connected") {
          api.state.streamConnected = true;
        }
        if (api.state.sending && (st === "connected" || st === "heartbeat")) {
          api.markGenerationActivity();
        }
        if (st === "agent_unavailable" || st === "upstream_unavailable") {
          api.state.streamConnected = false;
          closeEventStream();
          if (api.state.sending) {
            api.finishSending({ restoreDraft: true });
          }
        }
        api.renderStatus();
        return;
      }
      if (
        kind === "message_info" ||
        kind === "message_part_upsert" ||
        kind === "message_part_delta" ||
        kind === "message_part_removed"
      ) {
        if (kind === "message_part_delta") {
          api.recordDeltaDebugEvent(event);
        }
        api.markGenerationActivity();
        api.refreshMessages().catch(function () {});
        return;
      }
      if (kind === "permission_requested") {
        api.markGenerationActivity();
        const notice = api.blockedPermissionNoticeFromData(event);
        api.rememberBlockedPermissionNotice(notice);
        api.setInlineNote(
          "内置助手请求目录访问权限：" +
            String(notice.path || notice.permission || "unknown") +
            "（请在管理页批准或拒绝）",
        );
        return;
      }
      if (kind === "permission_blocked") {
        api.markGenerationActivity();
        const notice = api.blockedPermissionNoticeFromData(event);
        api.rememberBlockedPermissionNotice(notice);
        api.setInlineNote(String(notice.message || "会话触发了未授权访问，已自动拒绝。"));
        return;
      }
      if (kind === "permission_resolved") {
        api.markGenerationActivity();
        api.setInlineNote(
          "权限请求已自动处理：permission_id=" +
            String(event.permission_id || "") +
            "，response=" +
            String(event.response || ""),
        );
      }
    }

    function currentBasePollDelayMs() {
      const hasActiveGeneration = Boolean(
        api.state.sending ||
          api.state.loading ||
          api.state.streamConnected ||
          api.state.activeGenerationMessageId,
      );
      return hasActiveGeneration ? POLL_ACTIVE_MS : POLL_IDLE_MS;
    }

    function nextRefreshPollDelayMs() {
      const base = currentBasePollDelayMs();
      return Math.min(POLL_MAX_MS, base * Math.pow(2, refreshPollFailureCount));
    }

    function rightSidebarCollapsed() {
      const workspaceRoot = document.getElementById("workspace-root");
      return !!(workspaceRoot && workspaceRoot.dataset.rightCollapsed === "true");
    }

    function isAgentPollingStopped() {
      return typeof api.areAgentRequestsBlocked === "function" && api.areAgentRequestsBlocked();
    }

    function shouldPausePolling() {
      if (isAgentPollingStopped()) return true;
      if (document.visibilityState === "hidden") return true;
      if (rightSidebarCollapsed()) return true;
      return false;
    }

    function scheduleRefreshPoll(delayMs) {
      if (isAgentPollingStopped()) {
        return;
      }
      if (refreshTimerId) {
        global.clearTimeout(refreshTimerId);
      }
      refreshTimerId = global.setTimeout(
        runRefreshPoll,
        Math.max(1000, Number(delayMs) || currentBasePollDelayMs()),
      );
    }

    async function runRefreshPoll() {
      if (isAgentPollingStopped()) {
        return;
      }
      if (refreshPollInFlight) {
        scheduleRefreshPoll(nextRefreshPollDelayMs());
        return;
      }
      if (shouldPausePolling()) {
        scheduleRefreshPoll(Math.max(currentBasePollDelayMs(), nextRefreshPollDelayMs()));
        return;
      }
      if (
        api.state.streamConnected &&
        api.state.health &&
        api.state.health.healthy &&
        !api.state.sending &&
        !api.state.loading
      ) {
        scheduleRefreshPoll(POLL_STREAM_HEALTHY_MS);
        return;
      }
      refreshPollInFlight = true;
      try {
        const ok = await api.refreshAll().catch(function () {
          return false;
        });
        if (ok) {
          refreshPollFailureCount = 0;
        } else {
          refreshPollFailureCount = Math.min(refreshPollFailureCount + 1, 4);
        }
      } finally {
        refreshPollInFlight = false;
        if (!isAgentPollingStopped()) {
          scheduleRefreshPoll(nextRefreshPollDelayMs());
        }
      }
    }

    function startPolling() {
      if (isAgentPollingStopped()) {
        return;
      }
      scheduleRefreshPoll(currentBasePollDelayMs());
    }

    function dispose() {
      if (refreshTimerId) {
        global.clearTimeout(refreshTimerId);
        refreshTimerId = 0;
      }
      closeEventStream();
    }

    function onAgentAuthBlocked() {
      refreshPollFailureCount = 0;
      refreshPollInFlight = false;
      if (refreshTimerId) {
        global.clearTimeout(refreshTimerId);
        refreshTimerId = 0;
      }
      closeEventStream();
      api.renderStatus();
    }

    document.addEventListener("mei:agent-auth-blocked", onAgentAuthBlocked);

    return {
      closeEventStream: closeEventStream,
      connectEvents: connectEvents,
      applyHostEvent: applyHostEvent,
      startPolling: startPolling,
      dispose: function () {
        document.removeEventListener("mei:agent-auth-blocked", onAgentAuthBlocked);
        dispose();
      },
    };
  };
})(window);
