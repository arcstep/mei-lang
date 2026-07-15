/**
 * 源码 / CodeMirror / merge diff / 管理页 diff 角标：由 agent-panel 主文件装配。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallSourceView = function (api) {
    function refreshLinkedViewRefs() {
      api.refreshLinkedViewRefs();
    }

    function destroySourceEditor() {
      if (api.state.sourceViewResizeObserver) {
        try {
          api.state.sourceViewResizeObserver.disconnect();
        } catch (_) {}
        api.state.sourceViewResizeObserver = null;
      }
      api.state.sourceCodeMirror = null;
      api.state.sourceEditorContainer = null;
      if (api.els.sourceViewSourcePanel) {
        api.els.sourceViewSourcePanel.innerHTML = "";
      }
    }

    function destroySourceDiffView() {
      refreshLinkedViewRefs();
      if (api.state.sourceDiffResizeObserver) {
        try {
          api.state.sourceDiffResizeObserver.disconnect();
        } catch (_) {}
        api.state.sourceDiffResizeObserver = null;
      }
      api.state.sourceDiffMergeView = null;
      if (api.els.sourceViewDiffPanel) {
        api.els.sourceViewDiffPanel.innerHTML = "";
      }
    }

    function refreshSourceEditors() {
      const views = [
        api.state.sourceCodeMirror,
        api.state.sourceDiffMergeView && typeof api.state.sourceDiffMergeView.editor === "function"
          ? api.state.sourceDiffMergeView.editor()
          : null,
        api.state.sourceDiffMergeView &&
        typeof api.state.sourceDiffMergeView.leftOriginal === "function"
          ? api.state.sourceDiffMergeView.leftOriginal()
          : null,
        api.state.sourceDiffMergeView &&
        typeof api.state.sourceDiffMergeView.rightOriginal === "function"
          ? api.state.sourceDiffMergeView.rightOriginal()
          : null,
      ].filter(Boolean);
      views.forEach(function (view) {
        if (view && typeof view.refresh === "function") {
          view.refresh();
        }
      });
    }

    function refreshSourceDiffView() {
      refreshSourceEditors();
    }

    function scheduleSourceDiffRefresh() {
      if (!api.state.sourceDiffMergeView || typeof global.requestAnimationFrame !== "function") {
        refreshSourceDiffView();
        return;
      }
      global.requestAnimationFrame(function () {
        refreshSourceDiffView();
        global.requestAnimationFrame(function () {
          refreshSourceDiffView();
        });
      });
    }

    function bindSourceDiffResizeRefresh() {
      refreshLinkedViewRefs();
      if (!api.els.sourceViewDiffPanel || typeof ResizeObserver !== "function") {
        return;
      }
      if (api.state.sourceDiffResizeObserver) {
        try {
          api.state.sourceDiffResizeObserver.disconnect();
        } catch (_) {}
      }
      api.state.sourceDiffResizeObserver = new ResizeObserver(function () {
        scheduleSourceDiffRefresh();
      });
      api.state.sourceDiffResizeObserver.observe(api.els.sourceViewDiffPanel);
    }

    function bindSourceViewResizeRefresh() {
      refreshLinkedViewRefs();
      if (!api.els.sourceViewHost || typeof ResizeObserver !== "function") {
        return;
      }
      if (api.state.sourceViewResizeObserver) {
        try {
          api.state.sourceViewResizeObserver.disconnect();
        } catch (_) {}
      }
      api.state.sourceViewResizeObserver = new ResizeObserver(function () {
        scheduleSourceDiffRefresh();
      });
      api.state.sourceViewResizeObserver.observe(api.els.sourceViewHost);
      if (api.els.sourceViewSourcePanel) {
        api.state.sourceViewResizeObserver.observe(api.els.sourceViewSourcePanel);
      }
    }

    function ensureSourceEditor() {
      refreshLinkedViewRefs();
      if (!api.els.sourceViewSourcePanel || !global.CodeMirror) {
        return;
      }
      if (
        api.state.sourceCodeMirror &&
        api.state.sourceEditorContainer === api.els.sourceViewSourcePanel
      ) {
        refreshSourceEditors();
        return;
      }
      initSourceEditor();
    }

    function codeMirrorModeOption() {
      const lang = api.sourceLanguage();
      const target = api.sourceTargetKey();
      const ext = (target.split(".").pop() || "").toLowerCase();
      if (lang === "mei" || ext === "mei" || ext === "star") return "mei";
      if (lang === "json" || ext === "json" || ext === "jsonc") {
        return { name: "javascript", json: true };
      }
      if (lang === "typescript" || ext === "ts" || ext === "tsx") {
        return { name: "javascript", typescript: true };
      }
      if (lang === "javascript" || ext === "js" || ext === "jsx" || ext === "mjs" || ext === "cjs") {
        return "javascript";
      }
      if (lang === "css" || ext === "css" || ext === "scss" || ext === "less") return "css";
      if (lang === "python" || ext === "py" || ext === "pyi") return "python";
      if (lang === "xml" || ext === "xml" || ext === "svg") {
        return { name: "xml", htmlMode: false };
      }
      if (lang === "html" || ext === "html" || ext === "htm") {
        return { name: "xml", htmlMode: true };
      }
      return null;
    }

    function initSourceEditor() {
      refreshLinkedViewRefs();
      if (!api.els.sourceViewSourcePanel || !global.CodeMirror) {
        return;
      }
      destroySourceEditor();
      api.state.sourceCodeMirror = global.CodeMirror(api.els.sourceViewSourcePanel, {
        value: api.sourceRawText(),
        lineNumbers: true,
        readOnly: true,
        mode: codeMirrorModeOption(),
        theme: "default",
        lineWrapping: false,
        scrollbarStyle: "native",
      });
      api.state.sourceEditorContainer = api.els.sourceViewSourcePanel;
      bindSourceViewResizeRefresh();
      scheduleSourceDiffRefresh();
    }

    function renderSourceViewMode(mode) {
      refreshLinkedViewRefs();
      const nextMode = mode === "diff" ? "diff" : "source";
      api.state.sourceViewMode = nextMode;
      if (api.els.sourceViewSourcePanel) {
        api.els.sourceViewSourcePanel.hidden = nextMode !== "source";
      }
      if (api.els.sourceViewDiffPanel) {
        api.els.sourceViewDiffPanel.hidden = nextMode !== "diff";
      }
      if (api.els.sourceViewDiffBtn) {
        const active = nextMode === "diff";
        api.els.sourceViewDiffBtn.classList.toggle("is-active", active);
        api.els.sourceViewDiffBtn.setAttribute("aria-pressed", active ? "true" : "false");
      }
      if (nextMode === "source") {
        ensureSourceEditor();
      }
      scheduleSourceDiffRefresh();
    }

    function pickDiffFileForTarget(diff) {
      const files = Array.isArray(diff && diff.files) ? diff.files : [];
      if (!files.length) return null;
      const target = api.sourceTargetKey();
      const exact = files.find(function (file) {
        return api.normalizeFilePath(file && file.file) === target;
      });
      if (exact) return exact;
      const targetName = target.split("/").pop() || target;
      const fuzzy = files.find(function (file) {
        const filePath = api.normalizeFilePath(file && file.file);
        return filePath === targetName || filePath.endsWith("/" + targetName);
      });
      return fuzzy || files[0];
    }

    function renderSourceDiff(fileDiff, messageId) {
      refreshLinkedViewRefs();
      if (!api.els.sourceViewDiffPanel) return false;
      if (!global.CodeMirror || typeof global.CodeMirror.MergeView !== "function") {
        api.setInlineNote("差异视图不可用：CodeMirror 未加载。");
        return false;
      }
      if (typeof global.diff_match_patch !== "function") {
        api.setInlineNote("差异视图不可用：diff 引擎未加载。");
        return false;
      }
      const beforeText = String(fileDiff && fileDiff.before ? fileDiff.before : "");
      const afterText = String(fileDiff && fileDiff.after ? fileDiff.after : "");
      destroySourceDiffView();
      renderSourceViewMode("diff");
      api.state.sourceDiffMergeView = global.CodeMirror.MergeView(api.els.sourceViewDiffPanel, {
        value: afterText,
        orig: beforeText,
        lineNumbers: true,
        readOnly: true,
        mode: "mei",
        theme: "default",
        highlightDifferences: true,
        connect: "align",
        collapseIdentical: false,
        revertButtons: false,
      });
      api.state.sourceDiffMessageId = String(messageId || "");
      bindSourceDiffResizeRefresh();
      scheduleSourceDiffRefresh();
      return true;
    }

    function leaveDiffView() {
      refreshLinkedViewRefs();
      api.state.sourceDiffMessageId = "";
      destroySourceDiffView();
      const keepDiffMode = api.currentManageTab() === "diff";
      renderSourceViewMode(keepDiffMode ? "diff" : "source");
      if (keepDiffMode && api.els.sourceViewDiffPanel) {
        api.els.sourceViewDiffPanel.innerHTML =
          '<div class="grid place-content-center gap-2 rounded-xl border border-dashed border-slate-600/55 bg-slate-950/35 p-6 text-center text-xs text-slate-400">暂无可显示差异</div>';
      }
    }

    function applyManageTabMode(tab) {
      api.renderDeltaDebugLog();
      refreshLinkedViewRefs();
      const next = String(tab || "").trim().toLowerCase();
      if (next === "source") {
        ensureSourceEditor();
        leaveDiffView();
        return;
      }
      if (next !== "diff") return;
      renderSourceViewMode("diff");
      if (!api.state.latestDiffMessageId) {
        if (api.els.sourceViewDiffPanel) {
          api.els.sourceViewDiffPanel.innerHTML =
            '<div class="grid place-content-center gap-2 rounded-xl border border-dashed border-slate-600/55 bg-slate-950/35 p-6 text-center text-xs text-slate-400">暂无可查看差异</div>';
        }
        return;
      }
      inspectDiffForMessage(api.state.latestDiffMessageId).catch(function (error) {
        api.setInlineNote("读取差异失败：" + String(error.message || error));
      });
    }

    async function inspectDiffForMessage(messageId) {
      const sid = String(api.state.sessionId || "").trim();
      const mid = String(messageId || "").trim();
      if (!sid || !mid) return false;
      if (mid !== String(api.state.latestDiffMessageId || "")) {
        api.setInlineNote("仅支持查看最后一轮 Build 的差异。");
        return false;
      }
      const cacheKey = api.diffCacheKey(sid, mid);
      const diff =
        api.state.messageDiffCache[cacheKey] || (await api.fetchSessionDiff(mid));
      api.state.messageDiffCache[cacheKey] = diff;
      const hasFiles = api.sessionDiffHasMaterialChanges(diff);
      api.setMessageMeta(mid, { hasDiff: hasFiles });
      if (!hasFiles) {
        api.setInlineNote("暂无可显示的文件差异。");
        leaveDiffView();
        setDiffTabBadge(0, 0);
        return false;
      }
      const fileDiff = pickDiffFileForTarget(diff);
      if (!fileDiff) {
        api.setInlineNote("当前目标文件没有可显示差异。");
        leaveDiffView();
        setDiffTabBadge(0, 0);
        return false;
      }
      const st = diffLineStatsFromSummary(diff);
      setDiffTabBadge(st.additions, st.deletions);
      return renderSourceDiff(fileDiff, mid);
    }

    function ensureManageDiffTabBadge() {
      const tab = document.getElementById("manage-tab-diff");
      if (!tab) return null;
      let badge = document.getElementById("manage-tab-diff-badge");
      if (!badge) {
        badge = document.createElement("span");
        badge.id = "manage-tab-diff-badge";
        badge.className = "manage-view-tab-badge";
        badge.hidden = true;
        tab.appendChild(badge);
      }
      return badge;
    }

    function setDiffTabBadge(additions, deletions) {
      const a = Math.max(0, Number(additions) || 0);
      const d = Math.max(0, Number(deletions) || 0);
      const total = a + d;
      const badge = ensureManageDiffTabBadge();
      if (!badge) return;
      if (!total) {
        badge.textContent = "";
        badge.hidden = true;
        badge.removeAttribute("title");
        return;
      }
      badge.textContent = String(total);
      badge.hidden = false;
      badge.title = "相对上一轮 Build：新增 +" + String(a) + " 行，删除 -" + String(d) + " 行";
    }

    function diffLineStatsFromSummary(diff) {
      if (!diff || typeof diff !== "object") return { additions: 0, deletions: 0 };
      let a = Number(diff.additions);
      let d = Number(diff.deletions);
      if (Number.isFinite(a) && Number.isFinite(d) && (a > 0 || d > 0)) {
        return { additions: Math.max(0, a), deletions: Math.max(0, d) };
      }
      let hitA = 0;
      let hitD = 0;
      const files = Array.isArray(diff.files) ? diff.files : [];
      files.forEach(function (f) {
        if (!f || typeof f !== "object") return;
        const fa = Number(f.additions);
        const fd = Number(f.deletions);
        if (Number.isFinite(fa) && Number.isFinite(fd) && (fa > 0 || fd > 0)) {
          hitA += Math.max(0, fa);
          hitD += Math.max(0, fd);
          return;
        }
        const after = String(f.after || "");
        after.split("\n").forEach(function (line) {
          const t = String(line || "");
          if (t.startsWith("+") && !t.startsWith("+++")) hitA += 1;
          else if (t.startsWith("-") && !t.startsWith("---")) hitD += 1;
        });
      });
      return { additions: hitA, deletions: hitD };
    }

    async function refreshDiffTabBadge() {
      if (
        !api.state.sessionId ||
        !api.state.health ||
        !api.state.health.healthy ||
        api.historyUnavailableReason()
      ) {
        setDiffTabBadge(0, 0);
        return;
      }
      const mid = String(api.state.latestDiffMessageId || "").trim();
      if (!mid) {
        setDiffTabBadge(0, 0);
        return;
      }
      try {
        const cacheKey = api.diffCacheKey(api.state.sessionId, mid);
        const diff =
          api.state.messageDiffCache[cacheKey] || (await api.fetchSessionDiff(mid));
        if (diff && typeof diff === "object") {
          api.state.messageDiffCache[cacheKey] = diff;
        }
        if (!api.sessionDiffHasMaterialChanges(diff)) {
          setDiffTabBadge(0, 0);
          return;
        }
        const stats = diffLineStatsFromSummary(diff);
        setDiffTabBadge(stats.additions, stats.deletions);
      } catch (_) {
        setDiffTabBadge(0, 0);
      }
    }

    function syncSourceDiffEntry() {
      api.state.latestRoundAssistantId = api.latestRoundAssistantMessageId();
      api.state.latestDiffMessageId = api.latestDiffEligibleMessageId();
      if (api.els.sourceViewDiffBtn) {
        const enabled = !!api.state.latestDiffMessageId && !api.historyUnavailableReason();
        api.els.sourceViewDiffBtn.disabled = !enabled;
        api.els.sourceViewDiffBtn.title = enabled
          ? "查看最后一轮 Build 差异（行数见管理页「修改」角标）"
          : api.historyUnavailableReason() || "暂无可查看差异";
      }
      const diffTab = document.getElementById("manage-tab-diff");
      if (diffTab) {
        const enabled = !!api.state.latestDiffMessageId && !api.historyUnavailableReason();
        diffTab.hidden = !enabled;
      }
      if (
        api.state.sourceViewMode === "diff" &&
        api.state.sourceDiffMessageId &&
        api.state.sourceDiffMessageId !== api.state.latestDiffMessageId
      ) {
        leaveDiffView();
      } else if (!api.state.latestDiffMessageId && api.state.sourceViewMode === "diff") {
        leaveDiffView();
      }
      void refreshDiffTabBadge();
    }

    return {
      destroySourceEditor: destroySourceEditor,
      destroySourceDiffView: destroySourceDiffView,
      refreshSourceEditors: refreshSourceEditors,
      refreshSourceDiffView: refreshSourceDiffView,
      scheduleSourceDiffRefresh: scheduleSourceDiffRefresh,
      bindSourceDiffResizeRefresh: bindSourceDiffResizeRefresh,
      bindSourceViewResizeRefresh: bindSourceViewResizeRefresh,
      ensureSourceEditor: ensureSourceEditor,
      codeMirrorModeOption: codeMirrorModeOption,
      initSourceEditor: initSourceEditor,
      renderSourceViewMode: renderSourceViewMode,
      pickDiffFileForTarget: pickDiffFileForTarget,
      renderSourceDiff: renderSourceDiff,
      leaveDiffView: leaveDiffView,
      applyManageTabMode: applyManageTabMode,
      inspectDiffForMessage: inspectDiffForMessage,
      ensureManageDiffTabBadge: ensureManageDiffTabBadge,
      setDiffTabBadge: setDiffTabBadge,
      diffLineStatsFromSummary: diffLineStatsFromSummary,
      refreshDiffTabBadge: refreshDiffTabBadge,
      syncSourceDiffEntry: syncSourceDiffEntry,
    };
  };
})(window);
