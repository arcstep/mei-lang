          return MSG.refreshAll();
        })
        .catch(function (error) {
          CHR.setInlineNote("重连失败：" + String(error.message || error));
        });
    });
  }

  if (els.newSession) {
    els.newSession.addEventListener("click", function () {
      MSG.createSession().catch(function (error) {
        CHR.setInlineNote("创建会话失败：" + String(error.message || error));
      });
    });
  }

  if (els.sessionSelect) {
    const onSessionSelectChange = function () {
      state.sessionId = String(els.sessionSelect.value || "");
      restoreDeltaDebugLog(state.sessionId);
      state.sessionTargetKey = RT.currentSessionBindingFingerprint();
      MSG.resetPendingPermissionState();
      MSG.rememberSession();
      MSG.refreshMessages().catch(function (error) {
        CHR.setInlineNote("读取会话失败：" + String(error.message || error));
      });
      SES.connectEvents(true);
    };
    els.sessionSelect.addEventListener("sl-change", onSessionSelectChange);
    els.sessionSelect.addEventListener("change", onSessionSelectChange);
  }

  if (els.run) {
    els.run.addEventListener("click", function () {
      MSG.sendPrompt().catch(function (error) {
        CHR.setInlineNote("发送失败：" + String(error.message || error));
      });
    });
  }

  if (els.contextRefresh) {
    els.contextRefresh.addEventListener("click", function () {
      CTX.refreshContextPreview(true).catch(function (error) {
        CHR.setInlineNote("刷新上下文预览失败：" + String(error.message || error));
      });
    });
  }

  const resourceVisibilitySelect = document.getElementById("author-resource-visibility-select");
  if (resourceVisibilitySelect) {
    resourceVisibilitySelect.addEventListener("sl-change", function () {
      state.contextPreviewFetchedAtMs = 0;
      state.contextPreviewScopeKey = "";
      CTX.refreshContextPreview(true).catch(function (error) {
        CHR.setInlineNote("刷新上下文预览失败：" + String(error.message || error));
      });
    });
  }

  if (els.input) {
    els.input.addEventListener("input", function () {
      autoResizeComposerInput();
      CHR.renderRunButton(state.loading);
    });
    els.input.addEventListener("keydown", function (event) {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        MSG.sendPrompt().catch(function (error) {
          CHR.setInlineNote("发送失败：" + String(error.message || error));
        });
      }
    });
    autoResizeComposerInput();
  }

  const onComposerInputWindowResize = function () {
    autoResizeComposerInput();
    CHR.sizeCompletionModelSelectWidth();
    if (
      AF.isAccessFloatingMode() &&
      els.accessFloatingRoot &&
      els.accessFloatingRoot.dataset.positioned === "true"
    ) {
      const bootApi = window.__meiLangBoot || {};
      const offsetParent =
        typeof bootApi.copilotFloatingOffsetParent === "function"
          ? bootApi.copilotFloatingOffsetParent(els.accessFloatingRoot)
          : null;
      const hostRect = offsetParent
        ? offsetParent.getBoundingClientRect()
        : { left: 0, top: 0 };
      const rect = els.accessFloatingRoot.getBoundingClientRect();
      const pos = AF.applyAccessFloatingPosition(
        rect.left - hostRect.left,
        rect.top - hostRect.top,
      );
      if (pos) AF.rememberAccessFloatingPosition(pos.left, pos.top);
    }
    if (typeof boot.reclampAccessFloatingInViewport === "function") {
      boot.reclampAccessFloatingInViewport();
    }
    const layout = boot.copilotFabLayout;
    if (layout && typeof layout.scheduleCopilotFabToolbarLayout === "function") {
      layout.scheduleCopilotFabToolbarLayout();
    }
  };
  window.addEventListener("resize", onComposerInputWindowResize);

  if (els.modeAsk) {
    els.modeAsk.addEventListener("click", function () {
      CHR.switchAgentMode("ask");
    });
  }

  if (els.modeBuild) {
    els.modeBuild.addEventListener("click", function () {
      CHR.switchAgentMode("build");
    });
  }

  if (els.completionModelSelect) {
    els.completionModelSelect.addEventListener("change", function () {
      CHR.rememberSelectedCompletionModel(els.completionModelSelect.value);
      CHR.syncModelLabelFromCompletionSelect();
      CHR.sizeCompletionModelSelectWidth();
      CTX.refreshModelProbe(true).catch(function () {});
    });
  }

  function copilotPresentationFabContext() {
    const ctx = boot.copilotFabContext;
    if (ctx && typeof ctx.copilotFabContextActive === "function") {
      return ctx.copilotFabContextActive();
    }
    if (/^\/apps\/(copilot|speaker)\//.test(String(window.location.pathname || ""))) {
      return true;
    }
    if (
      document.getElementById("copilot-shell") ||
      document.getElementById("mei-presentation-manifest")
    ) {
      return true;
    }
    const eng = boot.presentationStepEngine;
    return !!(eng && typeof eng.hasManifest === "function" && eng.hasManifest());
  }

  if (els.accessFab) {
    els.accessFab.addEventListener("pointerdown", AF.beginAccessFloatingDrag);
  }

  if (els.accessClose) {
    els.accessClose.addEventListener("click", function () {
      AF.toggleAccessFloatingPanel(false);
    });
  }

  const onAccessFloatingEscape = function (event) {
    if (!AF.isAccessFloatingMode()) return;
    if (event && event.key === "Escape" && state.accessFloatingOpen) {
      AF.toggleAccessFloatingPanel(false);
    }
  };
  document.addEventListener("keydown", onAccessFloatingEscape);
  document.addEventListener("pointermove", AF.continueAccessFloatingDrag);
  document.addEventListener("pointerup", AF.endAccessFloatingDrag);
  document.addEventListener("pointercancel", AF.endAccessFloatingDrag);

  if (els.sourceViewDiffBtn) {
    els.sourceViewDiffBtn.addEventListener("click", function () {
      if (RT.currentManageTab() !== "diff") {
        RT.setManageTab("diff");
        return;
      }
      if (!state.latestDiffMessageId) {
        CHR.setInlineNote("最后一轮 Build 生成改动后才可查看差异。");
        return;
      }
      SRC.inspectDiffForMessage(state.latestDiffMessageId).catch(function (error) {
        CHR.setInlineNote("读取差异失败：" + String(error.message || error));
      });
    });
  }

  if (els.undo) {
    els.undo.addEventListener("click", function () {
      const messageId = latestUndoMessageId();
      if (!messageId) return;
      MSG.applyRevertForMessage(messageId).catch(function (error) {
        CHR.setInlineNote("撤回失败：" + String(error.message || error));
      });
    });
  }

  if (els.redo) {
    els.redo.addEventListener("click", function () {
      if (!canRedo()) return;
      MSG.applyUnrevertForSession().catch(function (error) {
        CHR.setInlineNote("恢复失败：" + String(error.message || error));
      });
    });
  }

  const onManageTabChange = function (event) {
    const nextTab =
      event && event.detail && typeof event.detail.tab === "string"
        ? event.detail.tab
        : RT.currentManageTab();
    SRC.applyManageTabMode(nextTab);
  };
  document.addEventListener("mei:manage-tab-change", onManageTabChange);

  const onManageSourceBundleReady = function () {
    if (!SRC || typeof SRC.ensureSourceEditor !== "function") return;
    const nextTab = RT.currentManageTab();
    SRC.ensureSourceEditor();
    if (typeof SRC.applyManageTabMode === "function") {
      SRC.applyManageTabMode(nextTab);
    }
  };
  document.addEventListener("mei:manage-source-bundle-ready", onManageSourceBundleReady);

  const onManageContextChange = function (event) {
    const detail = event && event.detail && typeof event.detail === "object"
      ? event.detail
      : {};
    if (detail && typeof detail.app === "string") {
      root.dataset.app = detail.app;
    }
    if (detail && typeof detail.scene === "string") {
      root.dataset.scene = detail.scene;
    }
    const nextFile =
      detail && typeof detail.file === "string"
        ? detail.file
        : detail && typeof detail.target === "string"
          ? detail.target
          : "";
    if (nextFile) {
      root.dataset.file = nextFile;
    }
    if (detail && typeof detail.sceneTarget === "string") {
      root.dataset.sceneTarget = detail.sceneTarget;
    }
    if (detail && typeof detail.entryTarget === "string") {
      root.dataset.sceneTarget = detail.entryTarget;
    }
    if (detail && typeof detail.mode === "string") {
      root.dataset.mode = detail.mode;
    }
    if (detail && typeof detail.sourceViews === "string") {
      root.dataset.sourceViews = detail.sourceViews;
    }
    if (detail && typeof detail.viewTab === "string") {
      root.dataset.viewTab = detail.viewTab;
    }
    state.contextPreview = null;
    state.contextPreviewBackoffUntilMs = 0;
    state.contextPreviewScopeKey = "";
    state.contextPreviewFetchedAtMs = 0;
    state.modelProbe = null;
    state.modelProbeFetchedAtMs = 0;
    state._meiAutoSessionOnce = false;
    CTX.renderContextPreview();
    SRC.destroySourceDiffView();
    SRC.destroySourceEditor();
    refreshLinkedViewRefs();
    AF.restoreAccessFloatingPanel();
    SRC.ensureSourceEditor();
    SRC.applyManageTabMode(RT.currentManageTab());
    root.classList.add("is-soft-refresh");
    restoreRevertedState();
    CHR.restoreAgentMode();
    MSG.restoreSession();
    restoreDeltaDebugLog(state.sessionId);
    if ($U.areAgentRequestsBlocked()) {
      renderDeltaDebugLog();
      window.setTimeout(function () {
        root.classList.remove("is-soft-refresh");
      }, 80);
      return;
    }
    MSG.refreshAll().catch(function (error) {
      CHR.setInlineNote("刷新作者助手面板失败：" + String(error.message || error));
    }).finally(function () {
      renderDeltaDebugLog();
      window.setTimeout(function () {
        root.classList.remove("is-soft-refresh");
      }, 80);
    });
  };
  document.addEventListener("mei:manage-context-change", onManageContextChange);

  const onBrowserQueryStateChange = function () {
    if ($U.areAgentRequestsBlocked()) return;
    state.contextPreviewScopeKey = "";
    state.contextPreviewFetchedAtMs = 0;
    state.contextPreviewBackoffUntilMs = 0;
    CTX.refreshContextPreview(true).catch(function () {});
  };
  document.addEventListener("mei:query-state-change", onBrowserQueryStateChange);

  restoreRevertedState();
  CHR.restoreAgentMode();
  AF.restoreAccessFloatingPanel();
  MSG.restoreSession();
  restoreDeltaDebugLog(state.sessionId);
  window.requestAnimationFrame(function () {
    if (typeof boot.syncAccessFloatingViewportMount === "function") {
      boot.syncAccessFloatingViewportMount();
    }
  });
  const initialTab = RT.currentManageTab();
  SRC.initSourceEditor();
  SRC.renderSourceViewMode(initialTab === "diff" ? "diff" : "source");
  CHR.renderProgressStrip();
  CTX.renderContextPreview();
  SRC.syncSourceDiffEntry();
  function startAgentTransport() {
    MSG.refreshAll()
      .then(function (ok) {
        if ($U.areAgentRequestsBlocked() || ok === false) {
          return;
        }
        SES.startPolling();
        if (initialTab !== "diff") return;
        if (!state.latestDiffMessageId) {
          return;
        }
        SRC.inspectDiffForMessage(state.latestDiffMessageId).catch(function (error) {
          CHR.setInlineNote("读取差异失败：" + String(error.message || error));
        });
      })
      .catch(function () {})
      .finally(function () {
        renderDeltaDebugLog();
      });
  }
  if (!$U.areAgentRequestsBlocked()) {
    $U.resolveAgentAuthGate()
      .then(function (gate) {
        if (!gate.allowed) {
          $U.blockAgentRequests(gate.reason);
          CHR.setInlineNote($U.agentRequestsBlockMessage(gate.reason));
          return;
        }
        startAgentTransport();
      })
      .catch(function () {
        $U.blockAgentRequests("session_check_error");
        CHR.setInlineNote($U.agentRequestsBlockMessage("session_check_error"));
      });
  }

  const beforeUnloadHandler = function () {
    SES.closeEventStream();
  };
  window.addEventListener("beforeunload", beforeUnloadHandler);
  boot.disposeAgentPanel = function () {
    SES.dispose();
    document.removeEventListener("mei:manage-tab-change", onManageTabChange);
    document.removeEventListener("mei:manage-source-bundle-ready", onManageSourceBundleReady);
    document.removeEventListener("mei:manage-context-change", onManageContextChange);
    document.removeEventListener("mei:query-state-change", onBrowserQueryStateChange);
    document.removeEventListener("keydown", onAccessFloatingEscape);
    document.removeEventListener("pointermove", AF.continueAccessFloatingDrag);
    document.removeEventListener("pointerup", AF.endAccessFloatingDrag);
    document.removeEventListener("pointercancel", AF.endAccessFloatingDrag);
    window.removeEventListener("beforeunload", beforeUnloadHandler);
    window.removeEventListener("resize", onComposerInputWindowResize);
    if (els.accessFab) {
      els.accessFab.removeEventListener("pointerdown", AF.beginAccessFloatingDrag);
    }
    if (state._completionModelMeasure && state._completionModelMeasure.parentNode) {
      try {
        state._completionModelMeasure.parentNode.removeChild(state._completionModelMeasure);
      } catch (_) {}
    }
    state._completionModelMeasure = null;
  };
})();
