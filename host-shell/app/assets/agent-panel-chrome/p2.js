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
