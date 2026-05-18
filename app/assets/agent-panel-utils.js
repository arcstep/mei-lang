/**
 * 作者面板共用纯逻辑：在拼接 bundle 中先于 `agent-panel.js` 执行。
 * 挂载 `window.MeiAgentPanelUtils`（escapeHtml / fetchJson / CHAT_CLASS / 路径归一化 / delta 调试行归一化 / 对话轮次归并 / 角色与进度 chip 样式）。
 */
(function () {
  if (window.MeiAgentPanelUtils) return;

  const CHAT_CLASS = {
    messageBase:
      "author-chat-message group grid gap-1 bg-transparent px-0 py-0.5 pl-2 border-l-2 border-l-transparent",
    messageUser: "author-chat-user border-l-blue-400/65",
    messageAssistant: "author-chat-assistant border-l-emerald-400/55",
    messageAssistantReverted: "author-chat-assistant-reverted border-l-slate-400/65",
    messageSystem: "author-chat-system border-l-amber-300/55",
    roleBase: "author-chat-role text-[10px] font-bold tracking-[0.02em] opacity-90",
    roleUser: "text-blue-300",
    roleAssistant: "text-emerald-300",
    roleAssistantReverted: "text-slate-400",
    roleSystem: "text-amber-300",
    head: "author-chat-head flex items-center justify-between gap-2",
    meta:
      "author-chat-meta inline-flex items-center gap-1.5 opacity-0 pointer-events-none transition-opacity group-hover:opacity-100 group-hover:pointer-events-auto",
    time: "author-chat-time whitespace-nowrap text-[10px] text-slate-400",
    copyButton:
      "author-chat-copy-btn agent-copy-btn rounded-full border border-blue-400/30 bg-slate-950/40 px-2 py-0.5 text-[10px] font-bold text-blue-300 transition-colors hover:border-blue-300/70 hover:bg-blue-600/20",
    inlineActions: "author-chat-inline-actions flex flex-wrap gap-2",
    actionButton:
      "author-chat-action-btn agent-action-btn rounded-full border border-blue-300/45 bg-blue-900/30 px-2.5 py-1.5 text-[11px] font-bold text-slate-200 transition-colors hover:border-blue-200/80 hover:bg-blue-600/40",
    round: "author-chat-round grid gap-2",
    empty:
      "author-chat-empty rounded-xl border border-dashed border-slate-600/55 px-4 py-4 text-center text-xs leading-6 text-slate-400",
    block: "author-chat-block grid gap-1 border-none bg-transparent p-0",
    blockDetails: "author-chat-block-details grid gap-1.5",
    blockSummary:
      "author-chat-block-label list-none cursor-pointer text-[11px] font-bold tracking-[0.01em]",
    blockLabel: "author-chat-block-label text-[11px] font-bold tracking-[0.01em]",
    body: "author-chat-body m-0 whitespace-pre-wrap break-words font-mono text-xs leading-6 text-slate-200",
    bodyMarkdown: "author-chat-body author-chat-md text-xs leading-relaxed text-slate-200",
    progressChip:
      "author-progress-chip inline-flex items-center gap-1.5 rounded-full border border-slate-600/60 bg-slate-950/45 px-2 py-0.5 text-[10px] font-bold text-slate-300",
    progressChipRunning: "border-teal-400/50 bg-teal-700/20 text-teal-100",
    progressChipDone: "border-blue-400/50 bg-blue-800/25 text-blue-100",
    progressChipError: "border-red-400/55 bg-red-900/30 text-red-100",
    progressChipPending: "border-amber-400/45 bg-amber-900/25 text-amber-100",
  };

  window.MeiAgentPanelUtils = {
    CHAT_CLASS,
    escapeHtml(value) {
      return String(value)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;");
    },
    async fetchJson(url, init) {
      const response = await fetch(url, init);
      if (!response.ok) {
        let detail = "";
        try {
          detail = (await response.text()).trim();
        } catch (_) {}
        throw new Error(detail || url + " -> " + response.status);
      }
      return response.json();
    },
    chatMessageRoleClass(roleRaw, reverted) {
      if (roleRaw === "user") return CHAT_CLASS.messageUser;
      if (roleRaw === "assistant") {
        return reverted ? CHAT_CLASS.messageAssistantReverted : CHAT_CLASS.messageAssistant;
      }
      return CHAT_CLASS.messageSystem;
    },
    chatRoleTextClass(roleRaw, reverted) {
      if (roleRaw === "user") return CHAT_CLASS.roleUser;
      if (roleRaw === "assistant") {
        return reverted ? CHAT_CLASS.roleAssistantReverted : CHAT_CLASS.roleAssistant;
      }
      return CHAT_CLASS.roleSystem;
    },
    chatBlockLabelToneClass(type) {
      const kind = String(type || "text").toLowerCase();
      if (kind === "reasoning") return "text-amber-200";
      if (kind === "tool") return "text-teal-200";
      if (kind === "patch") return "text-orange-200";
      if (kind === "debug") return "text-violet-200";
      if (kind === "diff") return "text-amber-300";
      if (kind === "code") return "text-blue-200";
      return "text-blue-300";
    },
    progressChipClass(status) {
      const kind = String(status || "pending").toLowerCase();
      if (kind === "running") return CHAT_CLASS.progressChip + " " + CHAT_CLASS.progressChipRunning;
      if (kind === "done") return CHAT_CLASS.progressChip + " " + CHAT_CLASS.progressChipDone;
      if (kind === "error") return CHAT_CLASS.progressChip + " " + CHAT_CLASS.progressChipError;
      return CHAT_CLASS.progressChip + " " + CHAT_CLASS.progressChipPending;
    },
    /** 与作者面板路由/路径归一化一致：trim、反斜杠转正斜杠、去 `./` 前缀。 */
    normalizeFilePath(value) {
      return String(value || "")
        .trim()
        .replace(/\\/g, "/")
        .replace(/^\.\/+/, "");
    },
    /** 将扁平 messages 列表按 user/assistant/system 归并为对话轮次（纯函数）。 */
    conversationRounds(messages) {
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
    },
    normalizeDeltaDebugRows(rows) {
      const src = Array.isArray(rows) ? rows : [];
      return src
        .map(function (item) {
          if (!item || typeof item !== "object") return null;
          const serverTs = Number(item.serverTs || 0);
          const clientRxTs =
            Number(item.clientRxTs || 0) || Number(item.clientTs || 0);
          const gapRxMs =
            item.gapRxMs != null && Number.isFinite(Number(item.gapRxMs))
              ? Number(item.gapRxMs)
              : item.gapMs != null && Number.isFinite(Number(item.gapMs))
                ? Number(item.gapMs)
                : null;
          const paintTs =
            item.paintTs != null && Number.isFinite(Number(item.paintTs))
              ? Number(item.paintTs)
              : null;
          const gapPaintMs =
            item.gapPaintMs != null && Number.isFinite(Number(item.gapPaintMs))
              ? Number(item.gapPaintMs)
              : null;
          return {
            serverTs: Number.isFinite(serverTs) ? serverTs : 0,
            clientRxTs: Number.isFinite(clientRxTs) ? clientRxTs : 0,
            paintTs: paintTs,
            partId: String(item.partId || ""),
            messageId: String(item.messageId || ""),
            chars: Number(item.chars || 0),
            preview: String(item.preview || ""),
            gapRxMs: gapRxMs,
            gapPaintMs: gapPaintMs,
          };
        })
        .filter(Boolean)
        .slice(0, 120);
    },
  };
})();
