/**
 * Delta 调试日志：sessionStorage 读写与 UI 渲染。
 * 由 agent-panel 在具备 `els, state, RT, $U` 后调用 `__meiAgentPanelInstallDeltaDebug(api)`。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallDeltaDebug = function (api) {
    const els = api.els;
    const state = api.state;
    const RT = api.RT;
    const $U = api.$U;

    function normalizeDeltaDebugRows(rows) {
      return $U.normalizeDeltaDebugRows(rows);
    }

    function writeDeltaDebugLogToStorage(sessionId, rows) {
      if (!window.sessionStorage) return;
      const key = RT.deltaDebugStorageKey(sessionId);
      if (!key) return;
      try {
        window.sessionStorage.setItem(
          key,
          JSON.stringify({
            updatedAtMs: Date.now(),
            rows: normalizeDeltaDebugRows(rows),
          }),
        );
      } catch (_) {}
    }

    function readDeltaDebugLogFromStorage(sessionId) {
      if (!window.sessionStorage) return [];
      const key = RT.deltaDebugStorageKey(sessionId);
      if (!key) return [];
      try {
        const raw = window.sessionStorage.getItem(key);
        if (!raw) return [];
        const parsed = JSON.parse(raw);
        return normalizeDeltaDebugRows(parsed && parsed.rows);
      } catch (_) {
        return [];
      }
    }

    function restoreDeltaDebugLog(sessionId) {
      state.deltaDebugLog = readDeltaDebugLogFromStorage(sessionId);
      renderDeltaDebugLog();
    }

    function trimDeltaPreview(text, maxChars) {
      const raw = String(text || "");
      if (!raw) return "";
      const normalized = raw.replace(/\s+/g, " ").trim();
      if (!normalized) return "";
      if (normalized.length <= maxChars) return normalized;
      return normalized.slice(0, Math.max(0, maxChars - 1)) + "…";
    }

    function formatDeltaDebugTs(stamp) {
      const ms = Number(stamp || 0);
      if (!Number.isFinite(ms) || ms <= 0) return "-";
      const d = new Date(ms);
      const pad = function (n, w) {
        const s = String(Number(n) || 0);
        return s.length >= w ? s : "0".repeat(w - s.length) + s;
      };
      return (
        pad(d.getHours(), 2) +
        ":" +
        pad(d.getMinutes(), 2) +
        ":" +
        pad(d.getSeconds(), 2) +
        "." +
        pad(d.getMilliseconds(), 3)
      );
    }

    function recordDeltaDebugEvent(event) {
      const serverTs = Number(event && event.server_ts_ms ? event.server_ts_ms : 0);
      const clientRxTs = Date.now();
      const deltaRaw = event && typeof event.delta === "string" ? event.delta : "";
      const preview = trimDeltaPreview(deltaRaw, 48);
      const gapRxMs =
        Number.isFinite(serverTs) && serverTs > 0 ? clientRxTs - serverTs : null;
      const row = {
        serverTs: Number.isFinite(serverTs) ? serverTs : 0,
        clientRxTs: clientRxTs,
        paintTs: null,
        partId: String(event && event.part_id ? event.part_id : ""),
        messageId: String(event && event.message_id ? event.message_id : ""),
        chars: deltaRaw.length,
        preview: preview,
        gapRxMs: gapRxMs,
        gapPaintMs: null,
      };
      state.deltaDebugLog.unshift(row);
      if (state.deltaDebugLog.length > 120) {
        state.deltaDebugLog.length = 120;
      }
      writeDeltaDebugLogToStorage(String(state.sessionId || ""), state.deltaDebugLog);
      renderDeltaDebugLog();
      requestAnimationFrame(function () {
        requestAnimationFrame(function () {
          const paintTs = Date.now();
          row.paintTs = paintTs;
          row.gapPaintMs =
            row.serverTs > 0 && Number.isFinite(row.serverTs) ? paintTs - row.serverTs : null;
          writeDeltaDebugLogToStorage(String(state.sessionId || ""), state.deltaDebugLog);
          renderDeltaDebugLog();
        });
      });
    }

    function renderDeltaDebugLog() {
      const log = Array.isArray(state.deltaDebugLog) ? state.deltaDebugLog : [];
      const manageEl = document.getElementById("mei-manage-debug-agent-sse-delta");
      const emptyManageHint =
        "尚无助手流式 delta 记录。请在右侧「作者」连接会话并发消息；出现 srv/cli_rx/gap_rx 与 cli_paint/gap_paint（后者为连续两次 requestAnimationFrame 后的墙钟，近似「排帧后」与首绘间隔）。SPA 换文件后若曾收过 delta，请再点一次「调试」页签或发新消息以刷新本区。";
      if (!log.length) {
        if (els.contextDeltaDebug) els.contextDeltaDebug.textContent = "(empty)";
        if (manageEl) manageEl.textContent = emptyManageHint;
        return;
      }
      const lines = log.slice(0, 60).map(function (item, index) {
        const rxTs =
          item && item.clientRxTs != null
            ? item.clientRxTs
            : item && item.clientTs != null
              ? item.clientTs
              : 0;
        const gapRxLabel =
          item && item.gapRxMs != null && Number.isFinite(item.gapRxMs)
            ? String(item.gapRxMs) + "ms"
            : item && item.gapMs != null && Number.isFinite(item.gapMs)
              ? String(item.gapMs) + "ms"
              : "-";
        const paintTs = item && item.paintTs != null ? item.paintTs : null;
        const cliPaintStr =
          paintTs != null && Number.isFinite(paintTs) && paintTs > 0
            ? formatDeltaDebugTs(paintTs)
            : "-";
        const gapPaintLabel =
          item && item.gapPaintMs != null && Number.isFinite(item.gapPaintMs)
            ? String(item.gapPaintMs) + "ms"
            : "-";
        return (
          "#" +
          String(index + 1).padStart(2, "0") +
          " srv=" +
          formatDeltaDebugTs(item.serverTs) +
          " cli_rx=" +
          formatDeltaDebugTs(rxTs) +
          " gap_rx=" +
          gapRxLabel +
          " cli_paint=" +
          cliPaintStr +
          " gap_paint=" +
          gapPaintLabel +
          " chars=" +
          String(item.chars || 0) +
          " part=" +
          String(item.partId || "-") +
          " msg=" +
          String(item.messageId || "-") +
          " delta=\"" +
          String(item.preview || "") +
          "\""
        );
      });
      const text = lines.join("\n");
      if (els.contextDeltaDebug) els.contextDeltaDebug.textContent = text;
      if (manageEl) manageEl.textContent = text;
    }

    return {
      writeDeltaDebugLogToStorage: writeDeltaDebugLogToStorage,
      readDeltaDebugLogFromStorage: readDeltaDebugLogFromStorage,
      restoreDeltaDebugLog: restoreDeltaDebugLog,
      renderDeltaDebugLog: renderDeltaDebugLog,
      recordDeltaDebugEvent: recordDeltaDebugEvent,
    };
  };
})(typeof globalThis !== "undefined" ? globalThis : window);
