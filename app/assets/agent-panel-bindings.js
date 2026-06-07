/**
 * 源码视图引用、composer、message meta、撤回状态、进度条推导、会话 diff 拉取。
 * 由 agent-panel 在 `RT` 就绪后调用 `__meiAgentPanelInstallBindings(api)`。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallBindings = function (api) {
    const els = api.els;
    const state = api.state;
    const $U = api.$U;
    const RT = api.RT;
    const COMPOSER_MIN_ROWS = api.COMPOSER_MIN_ROWS;
    const COMPOSER_MAX_ROWS = api.COMPOSER_MAX_ROWS;

    function composerDraftText() {
      return els.input && typeof els.input.value === "string" ? String(els.input.value) : "";
    }

    function refreshLinkedViewRefs() {
      els.sourceViewHost = document.getElementById("source-view-host");
      els.sourceViewSourcePanel = document.getElementById("source-view-source-panel");
      els.sourceViewSourceRaw = document.getElementById("source-view-source-raw");
      els.sourceViewDiffPanel = document.getElementById("source-view-diff-panel");
      els.accessFloatingRoot = document.getElementById("access-chat-floating-root");
      els.accessFab = document.getElementById("access-chat-fab");
      els.accessClose = document.getElementById("access-chat-close");
      els.accessPanel = document.getElementById("access-chat-overlay-panel");
      els.statusModelService = document.getElementById("mei-status-model-service");
    }

    function parsePx(value) {
      const n = Number.parseFloat(String(value || "0"));
      return Number.isFinite(n) ? n : 0;
    }

    function resolveComposerLineHeightPx(inputEl, style) {
      const explicit = parsePx(style && style.lineHeight ? style.lineHeight : "");
      if (explicit > 0) return explicit;
      const fontSize = parsePx(style && style.fontSize ? style.fontSize : "");
      return fontSize > 0 ? fontSize * 1.4 : 18;
    }

    function autoResizeComposerInput() {
      if (!els.input) return;
      const inputEl = els.input;
      const style = window.getComputedStyle(inputEl);
      const lineHeight = resolveComposerLineHeightPx(inputEl, style);
      const verticalPadding =
        parsePx(style.paddingTop) +
        parsePx(style.paddingBottom) +
        parsePx(style.borderTopWidth) +
        parsePx(style.borderBottomWidth);
      const minHeight = Math.round(lineHeight * COMPOSER_MIN_ROWS + verticalPadding);
      const maxHeight = Math.round(lineHeight * COMPOSER_MAX_ROWS + verticalPadding);
      inputEl.style.height = "auto";
      const scrollHeight = Math.max(inputEl.scrollHeight, minHeight);
      const nextHeight = Math.min(scrollHeight, maxHeight);
      inputEl.style.height = String(nextHeight) + "px";
      inputEl.style.overflowY = scrollHeight > maxHeight ? "auto" : "hidden";
    }

    function canSubmitPrompt() {
      return composerDraftText().trim().length > 0;
    }

    function normalizeFilePath(value) {
      return $U.normalizeFilePath(value);
    }

    function sourceTargetKey() {
      refreshLinkedViewRefs();
      const targetNode = els.sourceViewSourceRaw || els.sourceViewSourcePanel;
      if (targetNode && targetNode.dataset && targetNode.dataset.sourceTarget) {
        return normalizeFilePath(targetNode.dataset.sourceTarget);
      }
      return RT.currentTargetKey();
    }

    function sourceLanguage() {
      refreshLinkedViewRefs();
      const targetNode = els.sourceViewSourceRaw || els.sourceViewSourcePanel;
      if (targetNode && targetNode.dataset && targetNode.dataset.sourceLang) {
        return String(targetNode.dataset.sourceLang || "").trim().toLowerCase() || "plain";
      }
      return "plain";
    }

    function sourceRawText() {
      refreshLinkedViewRefs();
      return els.sourceViewSourceRaw ? String(els.sourceViewSourceRaw.textContent || "") : "";
    }

    function latestRoundAssistantMessageId() {
      const rounds = $U.conversationRounds(state.messages);
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

    /** diff 结果随当前管理页目标路径变化，缓存键需包含 path。 */
    function diffCacheKey(sessionId, messageId) {
      const base = messageKey(sessionId, messageId);
      const p = sourceTargetKey();
      return p ? base + "::diffPath::" + p : base;
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
        localStorage.setItem(RT.revertedStorageKey(), JSON.stringify(state.revertedMessageIds));
      } catch (_) {}
    }

    function restoreRevertedState() {
      state.revertedMessageIds = {};
      state.sessionHasRevertedChanges = {};
      try {
        const raw = localStorage.getItem(RT.revertedStorageKey());
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
            .map(function (item) {
              return String(item || "").trim();
            })
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
      const rounds = $U.conversationRounds(state.messages);
      for (let index = rounds.length - 1; index >= 0; index -= 1) {
        const round = rounds[index];
        const assistants = round && Array.isArray(round.assistants) ? round.assistants : [];
        const message = assistants.length ? assistants[assistants.length - 1] : null;
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
      const agent = RT.normalizeAgentMode(meta.agent || state.agentMode);
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

      let label = agent === "ask" ? "问答处理中" : "脚本生成中";
      if (runningTools.length > 0) {
        label = (agent === "ask" ? "问答处理中" : "脚本生成中") + " · 工具运行中";
      } else if (stepStarts > stepFinishes) {
        label = (agent === "ask" ? "问答处理中" : "脚本生成中") + " · 步骤处理中";
      } else if (parts.length > 0) {
        label = agent === "ask" ? "正在生成回答" : "正在生成结果";
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
          label: agent === "ask" ? "等待回答输出" : "等待执行输出",
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

    function isAuthoringWritebackRetired(status, detail) {
      return (
        Number(status) === 410 &&
        String(detail || "").indexOf("authoring_writeback_retired") >= 0
      );
    }

    async function fetchSessionDiff(messageId) {
      if (!state.sessionId || !RT.panelAuthoringEnabled()) return null;
      const params = new URLSearchParams();
      const mid = String(messageId || "").trim();
      if (mid) params.set("message_id", mid);
      const pathKey = sourceTargetKey();
      if (pathKey) params.set("path", pathKey);
      const qs = params.toString();
      const url =
        "/api/agent/session/" +
        encodeURIComponent(state.sessionId) +
        "/diff" +
        (qs ? "?" + qs : "");
      const response = await fetch(url);
      if (!response.ok) {
        let detail = "";
        try {
          detail = (await response.text()).trim();
        } catch (_) {}
        if (isAuthoringWritebackRetired(response.status, detail)) return null;
        throw new Error(detail || url + " -> " + response.status);
      }
      return response.json();
    }

    /** 与 `GET .../diff` 语义一致：占位快照或空 diff 不算「有改动」，避免误触发整页 reload。 */
    function sessionDiffHasMaterialChanges(diff) {
      if (!diff || typeof diff !== "object") return false;
      const topAdd = Number(diff.additions);
      const topDel = Number(diff.deletions);
      if ((Number.isFinite(topAdd) && topAdd > 0) || (Number.isFinite(topDel) && topDel > 0)) {
        return true;
      }
      const files = Array.isArray(diff.files) ? diff.files : [];
      return files.some(function (f) {
        if (!f || typeof f !== "object") return false;
        const a = Number(f.additions);
        const d = Number(f.deletions);
        if ((Number.isFinite(a) && a > 0) || (Number.isFinite(d) && d > 0)) return true;
        const after = String(f.after || "").trim();
        if (!after) return false;
        const low = after.toLowerCase();
        if (low.includes("no git worktree") || low.includes("native diff snapshot:")) return false;
        return after.split("\n").some(function (line) {
          const t = String(line || "");
          return (
            (t.startsWith("+") && !t.startsWith("+++")) ||
            (t.startsWith("-") && !t.startsWith("---"))
          );
        });
      });
    }

    return {
      composerDraftText: composerDraftText,
      refreshLinkedViewRefs: refreshLinkedViewRefs,
      autoResizeComposerInput: autoResizeComposerInput,
      canSubmitPrompt: canSubmitPrompt,
      normalizeFilePath: normalizeFilePath,
      sourceTargetKey: sourceTargetKey,
      sourceLanguage: sourceLanguage,
      sourceRawText: sourceRawText,
      latestRoundAssistantMessageId: latestRoundAssistantMessageId,
      latestDiffEligibleMessageId: latestDiffEligibleMessageId,
      diffCacheKey: diffCacheKey,
      setMessageMeta: setMessageMeta,
      getMessageMeta: getMessageMeta,
      setSessionRevertedFlag: setSessionRevertedFlag,
      hasSessionRevertedChanges: hasSessionRevertedChanges,
      persistRevertedState: persistRevertedState,
      restoreRevertedState: restoreRevertedState,
      revertedIdsForSession: revertedIdsForSession,
      setRevertedIdsForSession: setRevertedIdsForSession,
      isMessageReverted: isMessageReverted,
      latestUndoMessageId: latestUndoMessageId,
      canUndo: canUndo,
      canRedo: canRedo,
      deriveProgressFromMessages: deriveProgressFromMessages,
      fetchSessionDiff: fetchSessionDiff,
      sessionDiffHasMaterialChanges: sessionDiffHasMaterialChanges,
    };
  };
})(typeof globalThis !== "undefined" ? globalThis : window);
