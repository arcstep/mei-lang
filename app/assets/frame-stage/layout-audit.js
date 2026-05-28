  const manageRelayoutTimers = new WeakMap();

  /** 异步内容（字体/图表）稳定前最多补测一次，避免每帧触发 ResizeObserver 振荡。 */
  function scheduleManageViewportRelayout(root, contentHeight) {
    const pass = Number(root.dataset.meiRelayoutPass || 0);
    if (pass >= 1) return;
    const prev = manageRelayoutTimers.get(root) || {};
    if (prev.t500) return;
    const prevH = Number(root.dataset.meiScheduledContentHeight || 0);
    if (pass > 0 && Math.abs(prevH - contentHeight) < 2) return;
    root.dataset.meiScheduledContentHeight = String(Math.round(contentHeight));
    prev.t500 = setTimeout(() => {
      const timers = manageRelayoutTimers.get(root) || {};
      timers.t500 = null;
      root.dataset.meiRelayoutPass = "1";
      delete root.dataset.meiLayoutKey;
      updateViewport(root);
    }, 500);
    manageRelayoutTimers.set(root, prev);
  }

  /** 拆掉 manage 调试缩放壳，避免 inner 设计宽与 stage 宿主宽不一致导致整块偏右。 */
  function flattenStageScaleWrap(shell, stage) {
    const wrap = shell?.querySelector(":scope > .preview-stage-scale-wrap");
    if (!wrap) return;
    removeDesignBounds(shell);
    const inner = wrap.querySelector(":scope > .preview-stage-scale-inner");
    const moveOut = (node) => {
      if (!node || node === wrap) return;
      shell.insertBefore(node, wrap);
    };
    if (inner) {
      inner.style.transform = "none";
      inner.style.width = "";
      inner.style.height = "";
      [...inner.childNodes].forEach(moveOut);
    } else {
      [...wrap.childNodes].forEach(moveOut);
    }
    wrap.remove();
    if (stage && stage.parentElement !== shell) {
      shell.appendChild(stage);
    }
  }

  /** page-flow：舞台与 panel 行高随内容，取消 1fr 撑满与 slot 居中留白。 */
  function relaxPageFlowStageGrid(stage) {
    const panels = stage?.querySelectorAll(":scope > .preview-card") || [];
    if (panels.length > 0) {
      stage.style.gridTemplateRows = `repeat(${panels.length}, auto)`;
    }
    panels.forEach((card) => {
      card.style.gridTemplate = "none";
      card.style.gridTemplateAreas = '"body"';
      card.style.gridTemplateColumns = "minmax(0, 1fr)";
      card.style.gridTemplateRows = "auto";
      card.style.alignSelf = "start";
      card.style.height = "auto";
      card.querySelectorAll(".panel-body-cell, .preview-panel-body").forEach((body) => {
        body.style.height = "auto";
        body.style.minHeight = "0";
      });
      card.querySelectorAll(".component-card").forEach((slot) => {
        slot.style.display = "flex";
        slot.style.flexDirection = "column";
        slot.style.alignItems = "stretch";
        slot.style.height = "auto";
        slot.style.minHeight = "0";
        slot.style.width = "100%";
        slot.style.justifyContent = "flex-start";
      });
    });
  }

  /**
   * 访问态页面流（fluid_height）：定宽不超过宿主，左上对齐，纵向随内容延伸。
   */
  function applyFluidPageFlowLayout(root, shell, stage, hostWidth, designWidth) {
    flattenStageScaleWrap(shell, stage);
    removeDesignBounds(shell);
    const canvasWidth = Math.max(1, Math.min(designWidth, Math.max(1, hostWidth)));
    root.style.display = "block";
    root.style.justifyItems = "";
    root.style.alignItems = "";
    root.style.alignContent = "";
    shell.style.display = "block";
    shell.style.alignItems = "";
    shell.style.justifyContent = "";
    shell.style.justifySelf = "";
    shell.style.alignSelf = "";
    shell.style.width = "100%";
    shell.style.height = "auto";
    shell.style.maxWidth = "100%";
    shell.style.maxHeight = "none";
    shell.style.margin = "0";
    shell.style.marginInline = "0";
    shell.style.overflow = "visible";
    shell.style.position = "relative";
    stage.style.width = "100%";
    stage.style.maxWidth = `${round(canvasWidth)}px`;
    stage.style.height = "auto";
    stage.style.minHeight = "0";
    stage.style.maxHeight = "none";
    stage.style.transform = "none";
    stage.style.zoom = "";
    stage.style.transformOrigin = "top left";
    relaxPageFlowStageGrid(stage);
    root.dataset.meiFrameScale = "1";
    root.dataset.meiAppliedZoom = "1";
    root.dataset.meiManageRelayout = "";
    delete root.dataset.meiLayoutKey;
    root.scrollLeft = 0;
  }

  /**
   * 访问 / 运行态：contain（默认）或 cover；contain 等比铺满宿主且不裁切，信纸区居中。
   */
  function applyCanvasScaleLayout(
    root,
    shell,
    stage,
    scaleMode,
    hostWidth,
    hostHeight,
    designWidth,
    designHeight,
  ) {
    removeDesignBounds(shell);
    const mode = String(scaleMode || "contain").trim().toLowerCase();
    const scale = computeScale(mode, hostWidth, hostHeight, designWidth, designHeight);
    const scaleText = round(scale);
    const shellWidth = round(designWidth * scale);
    const shellHeight = round(designHeight * scale);

    shell.style.display = "flex";
    shell.style.alignItems = "center";
    shell.style.justifyContent = "center";
    shell.style.width = `${shellWidth}px`;
    shell.style.height = `${shellHeight}px`;
    shell.style.maxWidth = "none";
    shell.style.maxHeight = "none";
    shell.style.margin = "0";
    shell.style.flex = "none";
    shell.style.position = "relative";
    shell.style.overflow = "hidden";
    shell.style.boxSizing = "border-box";

    stage.style.width = `${round(designWidth)}px`;
    stage.style.height = `${round(designHeight)}px`;
    stage.style.minHeight = "";
    stage.style.flexShrink = "0";
    stage.style.transformOrigin = "center center";
    stage.style.transform =
      Math.abs(scaleText - 1) > 0.001 ? `scale(${scaleText})` : "none";
    stage.style.zoom = "";

    root.dataset.meiFrameScale = String(scaleText);
    root.dataset.meiManageRelayout = "";
  }

  function syncChromeNoneViewportBox(root) {
    if (!isChromeNoneAccess()) return;
    root.style.width = "100vw";
    root.style.height = "100vh";
    root.style.maxWidth = "100vw";
    root.style.maxHeight = "100vh";
    if (window.visualViewport) {
      root.style.width = `${window.visualViewport.width}px`;
      root.style.height = `${window.visualViewport.height}px`;
    }
  }

  function managePreviewTabIsActive() {
    try {
      const url = new URL(window.location.href);
      const tab = String(url.searchParams.get("tab") || "preview").trim().toLowerCase();
      return !tab || tab === "preview";
    } catch (_) {
      return true;
    }
  }

  function updateViewport(root) {
    if (!root || root.hidden || root.closest?.("[hidden]")) return;
    const auditOnly = root.dataset.meiLayoutAuditRoot === "true";
    if (auditOnly && String(root.dataset.routeMode || "").trim().toLowerCase() === "manage") {
      if (!managePreviewTabIsActive()) return;
    }
    const designWidthDeclared = Number(root.dataset.designWidth || 0);
    const designWidth = resolveCanvasWidth(root, designWidthDeclared);
    const designHeight = Number(root.dataset.designHeight || 0);
    const contentMaxWidth = Number(root.dataset.contentMaxWidth || 0);
    const contentHeightAttr = Number(root.dataset.contentHeight || 0);
    const fluidHeight = root.dataset.contentFluidHeight === "true";
    const contentHeight = contentHeightAttr || designHeight;
    const scaleMode = String(root.dataset.scaleMode || "contain").trim().toLowerCase();
    const editScaleMode = String(root.dataset.editScaleMode || "contain").trim().toLowerCase();
    const overflowMode = String(root.dataset.overflowMode || "clip").trim().toLowerCase();
    const safe = readSafeInsets(root, overflowMode);
    const shell = root.querySelector(".preview-stage-shell");
    const stage = root.querySelector(".preview-stage");
    if (auditOnly) {
      const auditStage =
        (root.matches?.(".preview-surface, .preview-stage") ? root : null) ||
        root.querySelector(".preview-surface, .preview-stage");
      if (!auditStage) return;
      const extent = measureStageContentExtentByRect(auditStage, contentMaxWidth);
      const auditWidth =
        designWidth ||
        auditStage.offsetWidth ||
        auditStage.getBoundingClientRect().width ||
        extent.width;
      const auditHeight =
        designHeight ||
        auditStage.offsetHeight ||
        auditStage.getBoundingClientRect().height ||
        extent.height;
      runLayoutAudit(
        root,
        auditStage,
        Math.max(1, auditWidth),
        Math.max(1, auditHeight),
        extent.width,
        extent.height,
        extent.width,
      );
      return;
    }
    if (!shell || !stage || !designWidth) return;

    syncChromeNoneViewportBox(root);
    const { hostWidth, hostHeight } = isManagePreviewRoute(root)
      ? resolveManageHostSize(root, safe)
      : resolveHostSize(root, safe);

    if (isManagePreviewRoute(root)) {
      if (viewportLayoutApplying.get(root)) return;
      viewportLayoutApplying.set(root, true);
      try {
        if (fluidHeight) {
          applyFluidPageFlowLayout(root, shell, stage, hostWidth, designWidth);
          unlockStageLayoutForDebug(stage, designHeight || contentHeight, true);
          initManagePreviewZoom(root);
          ensureManageZoomToolbar(root);
          updateManageZoomToolbar(root);
          const canvasWidth = Math.max(
            1,
            Math.min(designWidth, Math.max(1, hostWidth)),
          );
          const extent = measureStageContentExtent(
            stage,
            contentMaxWidth > 0 ? contentMaxWidth : 0,
          );
          const aspectRatio = String(root.dataset.aspectRatio || "").trim();
          ensureViewportChrome(
            root,
            canvasWidth,
            designHeight || contentHeight,
            aspectRatio,
            extent.height,
            true,
          );
          runLayoutAudit(
            root,
            stage,
            canvasWidth,
            designHeight || contentHeight || extent.height,
            extent.width,
            extent.height,
            extent.width,
          );
        } else {
          applyDebugPreviewLayout(
            root,
            shell,
            stage,
            hostWidth,
            hostHeight,
            designWidthDeclared,
            designHeight || contentHeight,
            fluidHeight,
            contentMaxWidth > 0 ? contentMaxWidth : 0,
          );
        }
      } finally {
        requestAnimationFrame(() => viewportLayoutApplying.delete(root));
      }
      return;
    }

    if (fluidHeight) {
      applyFluidPageFlowLayout(root, shell, stage, hostWidth, designWidth);
      const extent = measureStageContentExtent(stage, designWidth);
      runLayoutAudit(
        root,
        stage,
        designWidth,
        designHeight || contentHeight || extent.height,
        extent.width,
        extent.height,
        extent.width,
      );
      return;
    }

    if (contentMaxWidth > 0) {
      applyFluidWidthLayout(shell, stage, contentMaxWidth, hostWidth);
      const extent = measureStageContentExtent(stage, contentMaxWidth);
      runLayoutAudit(
        root,
        stage,
        designWidth,
        designHeight || contentHeight,
        extent.width,
        extent.height,
        extent.width,
      );
      return;
    }

    if (!designHeight && !contentHeight) return;
    applyCanvasScaleLayout(
      root,
      shell,
      stage,
      scaleMode,
      hostWidth,
      hostHeight,
      designWidth,
      designHeight || contentHeight,
    );
    const extent = measureStageContentExtent(stage, contentMaxWidth);
    runLayoutAudit(
      root,
      stage,
      designWidth,
      designHeight || contentHeight,
      extent.width,
      extent.height,
      extent.width,
    );
  }
