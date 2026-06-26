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
