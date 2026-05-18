/**
 * 状态栏、配置行、进度、模型选择、Markdown 与发送态。由 agent-panel 主文件装配 `CHR`。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallChrome = function (api) {
    var _markdownOptionsApplied = false;
  function renderInlineNote() {
    if (!api.els.config) return;
    const text = String(api.state.inlineNote || "").trim() || api.historyUnavailableReason();
    api.els.config.hidden = !text;
    api.els.config.textContent = text;
  }

  function renderProgressStrip() {
    if (!api.els.progressStrip || !api.els.progressLabel || !api.els.progressDetail || !api.els.progressItems) {
      return;
    }
    const progress = api.state.progress || {};
    const visible = !!progress.visible;
    api.els.progressStrip.hidden = !visible;
    if (!visible) {
      api.els.progressLabel.textContent = "";
      api.els.progressDetail.textContent = "";
      api.els.progressItems.innerHTML = "";
      return;
    }
    api.els.progressLabel.textContent = String(progress.label || "").trim();
    api.els.progressDetail.textContent = String(progress.detail || "").trim();
    api.els.progressItems.innerHTML = (Array.isArray(progress.items) ? progress.items : [])
      .map(function (item) {
        const label = api.$U.escapeHtml(String(item && item.label ? item.label : "").trim());
        const status = String(item && item.status ? item.status : "pending").trim();
        return '<span class="' + api.$U.progressChipClass(status) + '">' + label + "</span>";
      })
      .join("");
  }

  function renderHistoryButtons() {
    const unavailableReason = api.historyUnavailableReason();
    const undoEnabled =
      !unavailableReason && !api.state.loading && !api.state.sending && !api.state.aborting && api.canUndo();
    const redoEnabled =
      !unavailableReason && !api.state.loading && !api.state.sending && !api.state.aborting && api.canRedo();
    if (api.els.undo) {
      api.els.undo.disabled = !undoEnabled;
      api.els.undo.classList.toggle("is-active", undoEnabled);
      api.els.undo.title = unavailableReason || "撤回本轮代码修改";
    }
    if (api.els.redo) {
      api.els.redo.disabled = !redoEnabled;
      api.els.redo.classList.toggle("is-active", redoEnabled);
      api.els.redo.title = unavailableReason || "恢复最近撤回的代码修改";
    }
  }
  function renderRunButton(disabled) {
    if (!api.els.run) return;
    const isSending = api.state.sending;
    const isStopping = api.state.aborting;
    const canSubmit = api.canSubmitPrompt();
    const isPassive = !isSending && !canSubmit;
    api.els.run.disabled = isSending ? isStopping : (disabled || !canSubmit);
    api.els.run.textContent = isSending ? "■" : "➤";
    api.els.run.title = isSending
      ? (isStopping ? "停止中" : "停止发送")
      : canSubmit
        ? "发送"
        : "输入内容后可发送";
    api.els.run.setAttribute(
      "aria-label",
      isSending ? (isStopping ? "停止中" : "停止发送") : canSubmit ? "发送" : "等待输入",
    );
    api.els.run.classList.toggle("author-btn-danger", isSending);
    api.els.run.classList.toggle("author-btn-primary", !isSending && canSubmit);
    api.els.run.classList.toggle("author-btn-passive", isPassive);
  }

  function setInlineNote(message) {
    api.state.inlineNote = String(message || "").trim();
    renderInlineNote();
  }

  function setButtonState(disabled) {
    const controlsDisabled = disabled || api.state.sending || api.state.aborting;
    if (api.els.reconnect) api.els.reconnect.disabled = controlsDisabled;
    if (api.els.newSession) api.els.newSession.disabled = controlsDisabled;
    if (api.els.sessionSelect) api.els.sessionSelect.disabled = controlsDisabled;
    if (api.els.modeAsk) api.els.modeAsk.disabled = controlsDisabled;
    if (api.els.modeBuild) api.els.modeBuild.disabled = controlsDisabled;
    if (api.els.completionModelSelect) {
      api.els.completionModelSelect.disabled =
        controlsDisabled || api.els.completionModelSelect.hidden || !api.els.completionModelSelect.options.length;
    }
    if (api.els.contextRefresh) api.els.contextRefresh.disabled = controlsDisabled;
    renderRunButton(disabled);
    renderHistoryButtons();
  }

  function clearGenerationSettleTimer() {
    if (api.state.generationSettleTimer) {
      window.clearTimeout(api.state.generationSettleTimer);
    }
    api.state.generationSettleTimer = null;
  }

  function mergeDraftBackIntoInput() {
    const draft = String(api.state.pendingPromptDraft || "");
    if (!draft || !api.els.input) return;
    const current = String(api.els.input.value || "");
    api.els.input.value = current.trim() ? draft + "\n\n" + current : draft;
    api.autoResizeComposerInput();
    const cursor = draft.length;
    try {
      api.els.input.focus();
      api.els.input.setSelectionRange(cursor, cursor);
    } catch (_) {}
    api.state.pendingPromptDraft = "";
  }

  function finishSending(options) {
    const opts = options || {};
    clearGenerationSettleTimer();
    api.state.sending = false;
    api.state.aborting = false;
    api.state.sendAbortController = null;
    api.state.activeGenerationMessageId = "";
    if (opts.restoreDraft) {
      mergeDraftBackIntoInput();
    } else {
      api.state.pendingPromptDraft = "";
    }
    api.state.progress = {
      visible: false,
      label: "",
      detail: "",
      items: [],
    };
    setButtonState(false);
    renderProgressStrip();
  }

  function markGenerationActivity() {
    if (!api.state.sending) return;
    clearGenerationSettleTimer();
  }

  function clearDeltaDebugLog(options) {
    const opts = options || {};
    api.state.deltaDebugLog = [];
    if (opts.dropPersisted === true) {
      api.writeDeltaDebugLogToStorage(String(api.state.sessionId || ""), []);
    }
    api.renderDeltaDebugLog();
  }

  function activeGenerationFinished(rawMessages) {
    if (!api.state.sending) return false;
    const activeId = String(api.state.activeGenerationMessageId || "").trim();
    if (!activeId) return false;
    const message = (Array.isArray(rawMessages) ? rawMessages : []).find(function (item) {
      return String(item && item.message_id ? item.message_id : "") === activeId;
    });
    if (!message || String(message.role || "") !== "assistant") return false;
    return String(message.finish || "").trim().length > 0;
  }

  function renderStatus() {
    const runtime = api.state.runtime;
    const health = api.state.health;
    let label = "未配置";
    let dotClass = "author-server-dot author-server-dot-off";
    if (api.state.loading) {
      label = "刷新中";
    } else if (health && health.healthy && api.state.streamConnected) {
      label = "会话中";
      dotClass = "author-server-dot author-server-dot-on";
    } else if (health && health.healthy) {
      label = "已连接";
      dotClass = "author-server-dot author-server-dot-on";
    } else if (
      runtime &&
      String(runtime.connection_source || "").toLowerCase() === "native" &&
      runtime.running
    ) {
      label = health && health.healthy ? "已连接" : "内置助手未就绪";
      dotClass =
        health && health.healthy ? "author-server-dot author-server-dot-on" : "author-server-dot author-server-dot-off";
    } else if (runtime && runtime.connection_source === "managed" && runtime.running) {
      label = "启动中";
    } else if (runtime && runtime.connection_source === "external" && runtime.running) {
      label = "未连接";
    }
    if (api.els.serverStatus) {
      api.els.serverStatus.textContent = label;
    }
    if (api.els.serverDot) {
      api.els.serverDot.className = dotClass;
    }
    if (api.els.reconnect) {
      const shouldShowReconnect =
        !api.state.loading &&
        !!(runtime && runtime.running) &&
        !(health && health.healthy);
      api.els.reconnect.hidden = !shouldShowReconnect;
    }
  }

  function completionModelStorageKey() {
    try {
      var app = String(api.root.dataset.app || "default");
      return "mei.author.completionModel.v1." + app;
    } catch (_) {
      return "mei.author.completionModel.v1";
    }
  }

  function encodeCompletionOptionValue(providerId, modelId) {
    return String(providerId || "") + "\x1f" + String(modelId || "");
  }

  function decodeCompletionOptionValue(value) {
    var v = String(value || "");
    var i = v.indexOf("\x1f");
    if (i < 0) return null;
    var p = v.slice(0, i).trim();
    var m = v.slice(i + 1).trim();
    if (!p || !m) return null;
    return { provider_id: p, model_id: m };
  }

  function completionChoiceDisplayName(row) {
    if (!row) return "";
    var mid = String(row.model_id || "").trim();
    var lab = String(row.label || "").trim();
    if (lab && (lab.indexOf("·") >= 0 || lab.indexOf("\u00b7") >= 0)) {
      var sep = lab.indexOf("·") >= 0 ? "·" : "\u00b7";
      var parts = lab.split(sep);
      var last = String(parts[parts.length - 1] || "").trim();
      if (last) return last;
    }
    if (lab) return lab;
    return mid;
  }

  function setCompletionModelWrapVisible(show) {
    var wrap = api.els.completionModelWrap;
    if (!wrap) return;
    if (show) wrap.classList.remove("hidden");
    else wrap.classList.add("hidden");
  }

  function configureMarkdownOnce() {
    if (_markdownOptionsApplied) return;
    _markdownOptionsApplied = true;
    try {
      var mk = typeof marked !== "undefined" && marked && typeof marked.use === "function" ? marked : null;
      if (mk) {
        mk.use({
          async: false,
          breaks: true,
          gfm: true,
          renderer: {
            html: function (token) {
              var raw =
                token && token.raw != null
                  ? String(token.raw)
                  : token && token.text != null
                    ? String(token.text)
                    : "";
              return '<span class="author-chat-md-literal">' + api.$U.escapeHtml(raw) + "</span>";
            },
          },
        });
      } else if (typeof marked !== "undefined" && marked && typeof marked.setOptions === "function") {
        marked.setOptions({ async: false, breaks: true, gfm: true });
      }
    } catch (_) {}
  }

  function renderMarkdownToSafeHtml(src) {
    var raw = String(src || "");
    if (!raw.trim()) return "";
    configureMarkdownOnce();
    try {
      var mk = typeof marked !== "undefined" && marked && typeof marked.parse === "function" ? marked : null;
      var pur =
        typeof DOMPurify !== "undefined" && DOMPurify && typeof DOMPurify.sanitize === "function"
          ? DOMPurify
          : null;
      if (mk && pur) {
        var html = mk.parse(raw);
        return pur.sanitize(html, {
          ALLOWED_URI_REGEXP:
            /^(?:(?:https?|mailto):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i,
        });
      }
    } catch (_) {}
    return "<pre class=\"" + api.$U.CHAT_CLASS.body + "\">" + api.$U.escapeHtml(raw) + "</pre>";
  }

  function sizeCompletionModelSelectWidth() {
    var sel = api.els.completionModelSelect;
    var wrap = api.els.completionModelWrap;
    if (!sel || sel.hidden || !wrap || wrap.classList.contains("hidden") || !sel.options.length) {
      if (sel) sel.style.width = "";
      return;
    }
    var opt = sel.options[sel.selectedIndex];
    if (!opt) return;
    var text = String(opt.textContent || "");
    if (!api.state._completionModelMeasure) {
      var span = document.createElement("span");
      span.id = "author-completion-model-measure";
      span.setAttribute("aria-hidden", "true");
      span.style.cssText = "position:absolute;left:-9999px;top:0;white-space:nowrap;visibility:hidden;pointer-events:none;";
      document.body.appendChild(span);
      api.state._completionModelMeasure = span;
    }
    var measure = api.state._completionModelMeasure;
    var cs = window.getComputedStyle(sel);
    measure.style.font = cs.font;
    measure.style.fontSize = cs.fontSize;
    measure.style.fontFamily = cs.fontFamily;
    measure.style.fontWeight = cs.fontWeight;
    measure.style.letterSpacing = cs.letterSpacing;
    measure.textContent = text || "模型";
    var tw = measure.getBoundingClientRect().width;
    var pad = 22;
    var maxPx = 280;
    sel.style.width = Math.min(Math.max(40, tw + pad), maxPx) + "px";
  }

  function normalizedCompletionChoices(config) {
    if (!config || typeof config !== "object") return [];
    var raw = config.completion_model_choices;
    if (Array.isArray(raw) && raw.length) {
      return raw.map(function (row) {
        return {
          provider_id: String((row && row.provider_id) || "").trim(),
          model_id: String((row && row.model_id) || "").trim(),
          label: String((row && row.label) || "").trim(),
        };
      }).filter(function (row) {
        return row.provider_id && row.model_id;
      });
    }
    var pid = String(config.provider_id || "qwen").trim();
    var mid = String(config.completion_model || "").trim();
    if (!mid) return [];
    return [
      {
        provider_id: pid,
        model_id: mid,
        label: mid,
      },
    ];
  }

  function rememberSelectedCompletionModel(value) {
    try {
      localStorage.setItem(completionModelStorageKey(), String(value || ""));
    } catch (_) {}
  }

  function syncCompletionModelSelectFromConfig() {
    var sel = api.els.completionModelSelect;
    if (!sel) return;
    var config = api.state.config;
    var choices = normalizedCompletionChoices(config);
    var prevValue = String(sel.value || "");
    sel.innerHTML = "";
    for (var i = 0; i < choices.length; i++) {
      var row = choices[i];
      var op = document.createElement("option");
      op.value = encodeCompletionOptionValue(row.provider_id, row.model_id);
      op.textContent = completionChoiceDisplayName(row);
      sel.appendChild(op);
    }
    var saved = "";
    try {
      saved = String(localStorage.getItem(completionModelStorageKey()) || "").trim();
    } catch (_) {}
    var pick = saved || prevValue || "";
    var found = false;
    if (pick) {
      for (var j = 0; j < sel.options.length; j++) {
        if (sel.options[j].value === pick) {
          sel.selectedIndex = j;
          found = true;
          break;
        }
      }
    }
    if (!found && sel.options.length) {
      sel.selectedIndex = 0;
      rememberSelectedCompletionModel(sel.value);
    }
    var show = choices.length > 0;
    sel.hidden = !show;
    sel.disabled = !show;
    setCompletionModelWrapVisible(show);
    if (!show) {
      sel.innerHTML = "";
      sel.style.width = "";
    } else {
      sizeCompletionModelSelectWidth();
    }
  }

  function getSelectedCompletionModelRef() {
    var sel = api.els.completionModelSelect;
    if (!sel || sel.hidden || sel.disabled || !sel.options.length) return null;
    return decodeCompletionOptionValue(sel.value);
  }

  function syncModelLabelFromCompletionSelect() {
    var sel = api.els.completionModelSelect;
    if (sel && !sel.hidden && sel.selectedOptions && sel.selectedOptions[0]) {
      var ref = decodeCompletionOptionValue(sel.value);
      var t =
        ref && ref.model_id
          ? String(ref.model_id).trim()
          : String(sel.selectedOptions[0].textContent || "").trim();
      if (t) {
        api.state.modelLabel = t;
        if (api.els.modelLabel) api.els.modelLabel.textContent = api.state.modelLabel;
        renderStatusBarOpenCode();
      }
    }
    sizeCompletionModelSelectWidth();
  }
  function renderConfig() {
    const config = api.state.config;
    if (!config) {
      api.state.modelLabel = "模型";
      if (api.els.modelLabel) api.els.modelLabel.textContent = api.state.modelLabel;
      if (api.els.completionModelSelect) {
        api.els.completionModelSelect.innerHTML = "";
        api.els.completionModelSelect.hidden = true;
        api.els.completionModelSelect.disabled = true;
        api.els.completionModelSelect.style.width = "";
      }
      setCompletionModelWrapVisible(false);
      renderStatusBarOpenCode();
      return;
    }
    syncCompletionModelSelectFromConfig();
    if (
      api.els.completionModelSelect &&
      !api.els.completionModelSelect.hidden &&
      api.els.completionModelSelect.options.length
    ) {
      syncModelLabelFromCompletionSelect();
    } else {
      api.state.modelLabel =
        String(config.completion_model || config.provider_name || config.provider_id || "模型").trim() ||
        "模型";
      if (api.state.modelLabel && (api.state.modelLabel.indexOf("·") >= 0 || api.state.modelLabel.indexOf("\u00b7") >= 0)) {
        var sep2 = api.state.modelLabel.indexOf("·") >= 0 ? "·" : "\u00b7";
        var parts2 = api.state.modelLabel.split(sep2);
        var last = String(parts2[parts2.length - 1] || "").trim();
        if (last) api.state.modelLabel = last;
      }
      if (api.els.modelLabel) api.els.modelLabel.textContent = api.state.modelLabel;
    }
    renderStatusBarOpenCode();
  }

  function renderAgentMode() {
    const mode = api.normalizeAgentMode(api.state.agentMode);
    api.state.agentMode = mode;
    if (api.els.modeAsk) {
      const active = mode === "ask";
      api.els.modeAsk.classList.toggle("is-active", active);
      api.els.modeAsk.setAttribute("aria-pressed", active ? "true" : "false");
    }
    if (api.els.modeBuild) {
      const active = mode === "build";
      api.els.modeBuild.classList.toggle("is-active", active);
      api.els.modeBuild.setAttribute("aria-pressed", active ? "true" : "false");
    }
  }

  function rememberAgentMode() {
    try {
      localStorage.setItem(api.modeStorageKey(), api.normalizeAgentMode(api.state.agentMode));
    } catch (_) {}
  }

  function restoreAgentMode() {
    try {
      const saved = localStorage.getItem(api.modeStorageKey());
      if (saved) {
        api.state.agentMode = api.normalizeAgentMode(saved);
      }
    } catch (_) {}
    renderAgentMode();
  }

  function switchAgentMode(nextMode) {
    api.state.agentMode = api.normalizeAgentMode(nextMode);
    rememberAgentMode();
    renderAgentMode();
    api.state.contextPreviewFetchedAtMs = 0;
    api.state.contextPreviewScopeKey = "";
    setInlineNote(
      api.state.agentMode === "ask"
        ? "已切换到 Ask（访问侧问答，只读）"
        : "已切换到 Build（可生成并改写当前脚本）",
    );
  }

  function renderRuntime() {
    renderStatus();
    renderInlineNote();
    renderStatusBarOpenCode();
  }

  function renderSkillStatus() {
    const skill = api.state.skillStatus;
    if (!skill || !skill.source_present) {
      if (api.els.skillLine) {
        api.els.skillLine.textContent = "Skill: 未发现 MeiLang skill 源目录";
      }
      renderStatusBarOpenCode();
      return;
    }
    const summary = [];
    summary.push(skill.installed ? "Skill: 已安装" : "Skill: 仅源目录");
    if (skill.stale) summary.push("待同步");
    if (Number.isFinite(Number(skill.file_count))) {
      summary.push("文件 " + String(skill.file_count));
    }
    const updated = api.formatMsTimeForSkill(skill.install_updated_at_ms || skill.source_updated_at_ms);
    if (updated) {
      summary.push(updated);
    }
    if (skill.revision) {
      summary.push("rev " + String(skill.revision));
    }
    if (api.els.skillLine) {
      api.els.skillLine.textContent = summary.join(" · ");
    }
    renderStatusBarOpenCode();
  }

  function renderStatusBarSkill() {
    renderStatusBarOpenCode();
  }

  function renderStatusBarOpenCode() {
    if (!api.els.statusModelService) return;
    if (api.state.loading) {
      api.els.statusModelService.textContent = "模型服务 刷新中";
      api.els.statusModelService.title = "正在刷新模型服务状态";
      api.els.statusModelService.dataset.tone = "info";
      return;
    }
    const probe = api.state.modelProbe;
    if (!probe || typeof probe !== "object") {
      api.els.statusModelService.textContent = "模型服务 探测中";
      api.els.statusModelService.title = "正在探测当前模型服务连接状态";
      api.els.statusModelService.dataset.tone = "info";
      return;
    }
    const provider = String(probe && probe.provider_id ? probe.provider_id : "").trim() || "--";
    const model = String(probe && probe.model_id ? probe.model_id : "").trim() || "--";
    const latency = Number(probe && probe.latency_ms ? probe.latency_ms : 0);
    const latencyText = Number.isFinite(latency) && latency > 0 ? " · " + String(latency) + "ms" : "";
    if (probe && probe.reachable) {
      api.els.statusModelService.textContent = "模型服务 在线";
      api.els.statusModelService.title = "provider=" + provider + " · model=" + model + latencyText;
      api.els.statusModelService.dataset.tone = "good";
      return;
    }
    const nowMs = Date.now();
    const streak = Number(api.state.modelProbeFailureStreak || 0);
    const lastSuccessAt = Number(api.state.modelProbeLastSuccessAtMs || 0);
    const hasSuccess = Number.isFinite(lastSuccessAt) && lastSuccessAt > 0;
    const withinGrace = hasSuccess && nowMs - lastSuccessAt < api.MODEL_PROBE_RED_AFTER_MS;
    const transientFailure = hasSuccess
      ? streak < api.MODEL_PROBE_RED_AFTER_STREAK || withinGrace
      : streak < api.MODEL_PROBE_COLD_START_RED_AFTER_STREAK;
    const error = String(probe && probe.error ? probe.error : "").trim();
    const title = (error ? error + " · " : "") + "provider=" + provider + " · model=" + model + latencyText;
    if (transientFailure) {
      api.els.statusModelService.textContent = "模型服务 连接中";
      api.els.statusModelService.title = "正在尝试连接 · " + title;
      api.els.statusModelService.dataset.tone = "info";
      return;
    }
    api.els.statusModelService.textContent = "模型服务 异常";
    api.els.statusModelService.title = title;
    api.els.statusModelService.dataset.tone = "danger";
  }

    return {
      renderInlineNote: renderInlineNote,
      renderProgressStrip: renderProgressStrip,
      renderHistoryButtons: renderHistoryButtons,
      renderRunButton: renderRunButton,
      setInlineNote: setInlineNote,
      setButtonState: setButtonState,
      clearGenerationSettleTimer: clearGenerationSettleTimer,
      mergeDraftBackIntoInput: mergeDraftBackIntoInput,
      finishSending: finishSending,
      markGenerationActivity: markGenerationActivity,
      clearDeltaDebugLog: clearDeltaDebugLog,
      activeGenerationFinished: activeGenerationFinished,
      renderStatus: renderStatus,
      completionModelStorageKey: completionModelStorageKey,
      encodeCompletionOptionValue: encodeCompletionOptionValue,
      decodeCompletionOptionValue: decodeCompletionOptionValue,
      completionChoiceDisplayName: completionChoiceDisplayName,
      setCompletionModelWrapVisible: setCompletionModelWrapVisible,
      configureMarkdownOnce: configureMarkdownOnce,
      renderMarkdownToSafeHtml: renderMarkdownToSafeHtml,
      sizeCompletionModelSelectWidth: sizeCompletionModelSelectWidth,
      normalizedCompletionChoices: normalizedCompletionChoices,
      rememberSelectedCompletionModel: rememberSelectedCompletionModel,
      syncCompletionModelSelectFromConfig: syncCompletionModelSelectFromConfig,
      getSelectedCompletionModelRef: getSelectedCompletionModelRef,
      syncModelLabelFromCompletionSelect: syncModelLabelFromCompletionSelect,
      renderConfig: renderConfig,
      renderAgentMode: renderAgentMode,
      rememberAgentMode: rememberAgentMode,
      restoreAgentMode: restoreAgentMode,
      switchAgentMode: switchAgentMode,
      renderRuntime: renderRuntime,
      renderSkillStatus: renderSkillStatus,
      renderStatusBarSkill: renderStatusBarSkill,
      renderStatusBarOpenCode: renderStatusBarOpenCode,
    };
  };
})(window);
