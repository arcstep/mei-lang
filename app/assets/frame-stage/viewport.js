    if (viewportUpdateQueued.get(root)) return;
    viewportUpdateQueued.set(root, true);
    requestAnimationFrame(() => {
      viewportUpdateQueued.delete(root);
      updateViewport(root);
    });
  }

  function invalidateManageLayout(root) {
    delete root.dataset.meiLayoutKey;
    root.dataset.meiRelayoutPass = "0";
    const timers = manageRelayoutTimers.get(root);
    if (timers?.t500) clearTimeout(timers.t500);
    manageRelayoutTimers.delete(root);
  }

  function applyFluidWidthLayout(shell, stage, contentMaxWidth, hostWidth) {
    const stageWidth = Math.min(contentMaxWidth, hostWidth);
    shell.style.width = `${round(stageWidth)}px`;
    shell.style.height = "auto";
    shell.style.maxHeight = "none";
    shell.style.overflow = "visible";
    stage.style.width = `${round(stageWidth)}px`;
    stage.style.height = "auto";
    stage.style.maxHeight = "none";
    stage.style.transform = "none";
    removeDesignBounds(shell);
  }

  function removeViewportChrome(root, shell) {
    root?.querySelector(".preview-viewport-chrome")?.remove();
    shell?.querySelector(".preview-viewport-chrome")?.remove();
  }

  function debugLayoutContainer(shell) {
    return shell?.querySelector(".preview-stage-scale-wrap") || shell;
  }

  function ensureStageScaleWrap(shell, stage) {
    let wrap = shell.querySelector(".preview-stage-scale-wrap");
    if (!wrap) {
      wrap = document.createElement("div");
      wrap.className = "preview-stage-scale-wrap";
      if (stage.parentElement === shell) {
        shell.insertBefore(wrap, stage);
        wrap.appendChild(stage);
      } else {
        shell.appendChild(wrap);
        if (stage.parentElement !== wrap) {
          wrap.appendChild(stage);
        }
      }
    }
    return wrap;
  }

  function ensureStageScaleInner(wrap) {
    let inner = wrap.querySelector(":scope > .preview-stage-scale-inner");
    if (!inner) {
      inner = document.createElement("div");
      inner.className = "preview-stage-scale-inner";
      [...wrap.childNodes].forEach((node) => inner.appendChild(node));
      wrap.appendChild(inner);
    }
    return inner;
  }

  const MANAGE_ZOOM_STORAGE_KEY = "mei-manage-preview-zoom";
  const MANAGE_ZOOM_MIN = 0.1;
  const MANAGE_ZOOM_MAX = 2;

  function initManagePreviewZoom(root) {
    if (!root.dataset.previewZoom) {
      root.dataset.previewZoom =
        localStorage.getItem(MANAGE_ZOOM_STORAGE_KEY) || "fit";
    }
  }

  function computeManageFitScale(
    hostWidth,
    hostHeight,
    contentWidth,
    contentHeight,
  ) {
    if (hostWidth <= 0 || hostHeight <= 0 || contentWidth <= 0 || contentHeight <= 0) {
      return 1;
    }
    const sx = hostWidth / contentWidth;
    const sy = hostHeight / contentHeight;
    const fit = Math.min(sx, sy);
    return fit > 0 ? fit : 1;
  }

  function resolveManagePreviewZoom(root, fitScale) {
    initManagePreviewZoom(root);
    const raw = String(root.dataset.previewZoom || "fit").trim().toLowerCase();
    if (raw === "fit" || raw === "auto") return fitScale;
    const value = Number(raw);
    if (Number.isFinite(value) && value > 0) {
      return Math.min(MANAGE_ZOOM_MAX, Math.max(MANAGE_ZOOM_MIN, value));
    }
    return fitScale;
  }

  function setManagePreviewZoom(root, value) {
    root.dataset.previewZoom = value;
    try {
      localStorage.setItem(MANAGE_ZOOM_STORAGE_KEY, value);
    } catch (_) {}
    invalidateManageLayout(root);
    updateManageZoomToolbar(root);
    updateViewport(root);
  }

  function stepManagePreviewZoom(root, delta) {
    initManagePreviewZoom(root);
    const fitScale = Number(root.dataset.meiFitScale || 1);
    const current =
      root.dataset.previewZoom === "fit"
        ? fitScale
        : resolveManagePreviewZoom(root, fitScale);
    const next = Math.min(
      MANAGE_ZOOM_MAX,
      Math.max(MANAGE_ZOOM_MIN, round(current + delta)),
    );
    setManagePreviewZoom(root, String(next));
  }

  function ensureManageZoomToolbar(root) {
    let toolbar = root.querySelector(":scope > .preview-viewport-toolbar");
    const shell = root.querySelector(".preview-stage-shell");
    if (!toolbar) {
      toolbar = document.createElement("div");
      toolbar.className = "preview-viewport-toolbar";
      const bar = document.createElement("div");
      bar.className = "preview-viewport-zoom-bar";
      bar.dataset.previewZoomBar = "true";
      const title = document.createElement("span");
      title.className = "preview-viewport-zoom-title";
      title.textContent = "视窗";
      bar.appendChild(title);
      const presets = [
        ["fit", "自适应"],
        ["1", "100%"],
        ["0.75", "75%"],
        ["0.5", "50%"],
        ["minus", "−"],
        ["plus", "+"],
      ];
      presets.forEach(([value, label]) => {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "preview-viewport-zoom-btn";
        btn.dataset.previewZoom = value;
        btn.textContent = label;
        if (value === "minus") btn.title = "缩小";
        if (value === "plus") btn.title = "放大";
        bar.appendChild(btn);
      });
      const readout = document.createElement("span");
      readout.className = "preview-viewport-zoom-readout";
      readout.dataset.previewZoomReadout = "true";
      readout.textContent = "—";
      bar.appendChild(readout);
      toolbar.appendChild(bar);
      const chrome = document.createElement("div");
      chrome.className = "preview-viewport-chrome";
      chrome.setAttribute("aria-hidden", "true");
      toolbar.appendChild(chrome);
      root.insertBefore(toolbar, shell || root.firstChild);
    }
    const orphanChrome = root.querySelector(":scope > .preview-viewport-chrome");
    const toolbarChrome = toolbar.querySelector(".preview-viewport-chrome");
    if (orphanChrome && toolbarChrome && orphanChrome !== toolbarChrome) {
      toolbarChrome.replaceWith(orphanChrome);
    } else if (orphanChrome && !toolbarChrome) {
      toolbar.appendChild(orphanChrome);
    }
    return toolbar;
  }

  function updateManageZoomToolbar(root) {
    ensureManageZoomToolbar(root);
    const bar = root.querySelector("[data-preview-zoom-bar]");
    if (!bar) return;
    const mode = String(root.dataset.previewZoom || "fit");
    const applied = Number(root.dataset.meiAppliedZoom || 1);
    bar.querySelectorAll("[data-preview-zoom]").forEach((btn) => {
      const zoom = String(btn.dataset.previewZoom || "");
      const isStep = zoom === "minus" || zoom === "plus";
      const isActive =
        !isStep &&
        (zoom === mode ||
          (mode !== "fit" &&
            zoom !== "minus" &&
            zoom !== "plus" &&
            Math.abs(Number(zoom) - Number(mode)) < 0.001));
      btn.classList.toggle("is-active", isActive);
    });
    const readout = bar.querySelector("[data-preview-zoom-readout]");
    if (readout) {
      readout.textContent = `${Math.round(applied * 100)}%`;
    }
  }

  function unlockStageLayoutForDebug(stage, designHeight) {
    stage.style.overflow = "visible";
    stage.style.height = "auto";
    stage.style.minHeight = `${round(designHeight)}px`;
    stage.style.maxHeight = "none";
    if (stage.dataset.meiDebugLayoutUnlocked !== "true") {
      const rows = getComputedStyle(stage).gridTemplateRows;
      if (rows && rows !== "none") {
        const parts = rows.split(/\s+/).filter((row) => row && row !== "none");
        stage.style.gridTemplateRows = parts
          .map((row, index) => {
            if (index === 0) return row;
            if (/minmax|fr/i.test(row)) return "auto";
            if (Number.isFinite(Number.parseFloat(row))) return "auto";
            return row;
          })
          .join(" ");
      }
      stage.dataset.meiDebugLayoutUnlocked = "true";
    }
    stage.querySelectorAll(".preview-card, .preview-panel-body").forEach((node) => {
      node.style.overflow = "visible";
      node.style.maxHeight = "none";
    });
  }

  function resolveCanvasWidth(root, designWidth) {
    const cap = Number(root.dataset.contentMaxWidth || 0);
    const declared = Number(designWidth || root.dataset.designWidth || 0);
    if (cap > 0 && declared > 0) return Math.min(cap, declared);
    return declared;
  }

  function clearDebugOverlayNodes(container) {
    container
      ?.querySelectorAll(
        ".preview-design-bounds, .preview-design-overflow-veil, .preview-design-overflow-veil-x",
      )
      .forEach((node) => node.remove());
  }

  /** 在布局坐标（未缩放）下测量；勿用缩放后的 getBoundingClientRect（边栏变宽→zoom 变大→假横向溢出）。 */
  function measureStageContentExtent(stage, contentMaxWidth) {
    const cap = contentMaxWidth > 0 ? contentMaxWidth : 0;
    if (cap > 0 && stage.offsetWidth > 0 && stage.offsetWidth <= cap + 2) {
      const height = Math.max(stage.offsetHeight, stage.scrollHeight);
      return { width: cap, height: Math.max(1, height) };
    }
    let maxRight = 0;
    let maxBottom = 0;
    let hasContent = false;
    stage
      .querySelectorAll(".preview-card, .panel-head-cell, .panel-body-cell, .panel-heading, .preview-panel-body")
      .forEach((node) => {
        if (
          node.closest(
            ".preview-design-bounds, .preview-design-overflow-veil, .preview-design-overflow-veil-x",
          )
        ) {
          return;
        }
        const right = node.offsetLeft + node.offsetWidth;
        const bottom = node.offsetTop + node.offsetHeight;
        if (node.offsetWidth < 1 || node.offsetHeight < 1) return;
        hasContent = true;
        if (right > maxRight) maxRight = right;
        if (bottom > maxBottom) maxBottom = bottom;
      });
    let width = hasContent ? Math.max(1, maxRight, stage.offsetWidth) : stage.offsetWidth;
    let height = hasContent ? Math.max(1, maxBottom, stage.offsetHeight) : stage.offsetHeight;
    if (cap > 0 && width <= cap + 2) {
      width = cap;
    }
    return { width, height };
  }

  function measureStageContentSize(
    stage,
    canvasWidth,
    designHeight,
    fluidHeight,
    contentMaxWidth,
  ) {
    const extent = measureStageContentExtent(stage, contentMaxWidth);
    return {
      width: Math.max(canvasWidth, extent.width),
      extentWidth: extent.width,
      height: Math.max(fluidHeight ? extent.height : designHeight, extent.height),
      extentHeight: extent.height,
    };
  }

  function nodeAuditLabel(node) {
    if (!node) return "unknown";
    const id = node.id ? `#${node.id}` : "";
    if (id) return id;
    const area =
      node.getAttribute("data-area") || node.getAttribute("area") || node.dataset?.area;
    if (area) return `${node.tagName.toLowerCase()}[area=${area}]`;
    const klass = String(node.className || "")
      .split(/\s+/)
      .map((item) => item.trim())
      .filter(Boolean)[0];
    if (klass) return `${node.tagName.toLowerCase()}.${klass}`;
    return node.tagName.toLowerCase();
  }

  function detectDegenerateBoxes(stage, limit = 8) {
    const hits = [];
    stage
      .querySelectorAll(
        ".preview-card, .preview-panel-body, .panel-body-cell, .panel-head-cell, .panel-heading",
      )
      .forEach((node) => {
        if (hits.length >= limit) return;
        if (node.offsetWidth > 0 && node.offsetHeight > 0) return;
        hits.push(nodeAuditLabel(node));
      });
    return hits;
  }

  function detectClippedNodes(stage, limit = 8) {
    const hits = [];
    stage
      .querySelectorAll(
        ".preview-card, .preview-panel-body, .panel-body-cell, .panel-head-cell, .panel-heading",
      )
      .forEach((node) => {
        if (hits.length >= limit) return;
        const style = getComputedStyle(node);
        const clipsY = /hidden|clip/.test(style.overflowY || style.overflow || "");
        const clipsX = /hidden|clip/.test(style.overflowX || style.overflow || "");
        const yOverflow = node.scrollHeight - node.clientHeight > 1;
        const xOverflow = node.scrollWidth - node.clientWidth > 1;
        if ((clipsY && yOverflow) || (clipsX && xOverflow)) {
          hits.push(nodeAuditLabel(node));
        }
      });
    return hits;
  }

  function detectHeadMetricSpacing(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".preview-card").forEach((panel) => {
      if (hits.length >= limit) return;
      const head = panel.querySelector(":scope > .panel-head-cell");
      const body = panel.querySelector(":scope > .panel-body-cell");
      if (!head || !body) return;
      const first = body.querySelector(
        ":scope > .preview-card, :scope > .component-card, :scope > .mei-text, :scope > [data-metric-role]",
      );
      if (!first) return;
      const gap = first.getBoundingClientRect().top - body.getBoundingClientRect().top;
      if (gap > 28) {
        hits.push(`${nodeAuditLabel(panel)}(+${Math.round(gap)}px)`);
      }
    });
    return hits;
  }

  function detectBottomClipRisk(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".preview-card").forEach((panel) => {
      if (hits.length >= limit) return;
      const body = panel.querySelector(":scope > .panel-body-cell");
      if (!body) return;
      const panelStyle = getComputedStyle(panel);
      const bodyStyle = getComputedStyle(body);
      const panelClips = /hidden|clip/.test(panelStyle.overflowY || panelStyle.overflow || "");
      const bodyClips = /hidden|clip/.test(bodyStyle.overflowY || bodyStyle.overflow || "");
      if (!panelClips && !bodyClips) return;
      const bodyRect = body.getBoundingClientRect();
      let maxBottom = bodyRect.top;
      body
        .querySelectorAll(":scope > .preview-card, :scope > .component-card, :scope > *")
        .forEach((node) => {
          const rect = node.getBoundingClientRect();
          if (rect.height < 1) return;
          if (rect.bottom > maxBottom) maxBottom = rect.bottom;
        });
      if (maxBottom > bodyRect.bottom + 1) {
        hits.push(
          `${nodeAuditLabel(panel)}(+${Math.round(maxBottom - bodyRect.bottom)}px)`,
        );
      }
    });
    return hits;
  }

  function rectsOverlap(a, b) {
    return (
      a.left < b.right - 1 &&
      a.right > b.left + 1 &&
      a.top < b.bottom - 1 &&
      a.bottom > b.top + 1
    );
  }

  function detectCardOverlap(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".panel-body-cell").forEach((body) => {
      if (hits.length >= limit) return;
      const cards = Array.from(
        body.querySelectorAll(':scope > .preview-card[data-mei-panel-id]'),
      );
      if (cards.length < 2) return;
      for (let i = 0; i < cards.length; i += 1) {
        if (hits.length >= limit) break;
        const a = cards[i].getBoundingClientRect();
        if (a.width < 1 || a.height < 1) continue;
        for (let j = i + 1; j < cards.length; j += 1) {
          if (hits.length >= limit) break;
          const b = cards[j].getBoundingClientRect();
          if (b.width < 1 || b.height < 1) continue;
          if (rectsOverlap(a, b)) {
            hits.push(
              `${nodeAuditLabel(cards[i])} x ${nodeAuditLabel(cards[j])}`,
            );
          }
        }
      }
    });
    return hits;
  }

  function detectCardGapBudget(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".panel-body-cell").forEach((body) => {
      if (hits.length >= limit) return;
      const cards = Array.from(
        body.querySelectorAll(':scope > .preview-card[data-mei-panel-id]'),
      );
      if (cards.length < 2) return;
      const bodyRect = body.getBoundingClientRect();
      const horizontal = cards
        .map((node) => node.getBoundingClientRect())
        .filter((rect) => rect.width > 1 && rect.height > 1)
        .sort((a, b) => a.left - b.left);
      if (horizontal.length >= 2) {
        const gap = horizontal[1].left - horizontal[0].right;
        if (gap < 4 || gap > 12) {
          hits.push(`${nodeAuditLabel(body)}(横向 gap≈${Math.round(gap)}px)`);
        }
      }
      const vertical = cards
        .map((node) => node.getBoundingClientRect())
        .filter((rect) => rect.width > 1 && rect.height > 1)
        .sort((a, b) => a.top - b.top);
      if (vertical.length >= 2) {
        const gap = vertical[1].top - vertical[0].bottom;
        if (gap < 4 || gap > 16) {
          hits.push(`${nodeAuditLabel(body)}(纵向 gap≈${Math.round(gap)}px)`);
        }
      }
      const leftPad = Math.max(0, horizontal[0]?.left - bodyRect.left || 0);
      const rightPad = Math.max(
        0,
        bodyRect.right - (horizontal[horizontal.length - 1]?.right || bodyRect.right),
      );
      const topPad = Math.max(0, vertical[0]?.top - bodyRect.top || 0);
      const bottomPad = Math.max(
        0,
        bodyRect.bottom - (vertical[vertical.length - 1]?.bottom || bodyRect.bottom),
      );
      const maxPad = Math.max(leftPad, rightPad, topPad, bottomPad);
      if (maxPad > 24) {
        hits.push(`${nodeAuditLabel(body)}(四周留白≈${Math.round(maxPad)}px)`);
      }
    });
    return hits;
  }

  function publishLayoutAudit(root, diagnostics) {
    const payload = {
      sourcePath: String(root?.dataset?.sourcePath || root?.dataset?.activeTarget || "main.mei"),
      diagnostics: Array.isArray(diagnostics) ? diagnostics : [],
    };
    document.dispatchEvent(new CustomEvent("mei:layout-audit", { detail: payload }));
  }

  function runLayoutAudit(
    root,
    stage,
    designWidth,
    designHeight,
    contentWidth,
    contentHeight,
    extentWidth,
  ) {
    if (!root || !stage) return;
    const diagnostics = [];
    if (extentWidth > designWidth + 1) {
      diagnostics.push({
        severity: "warning",
        code: "layout_audit_canvas_overflow_x",
        message: `横向内容超出设计宽度：实测 ${Math.round(extentWidth)}px / 设计 ${Math.round(designWidth)}px`,
      });
    }
    if (contentHeight > designHeight + 1) {
      diagnostics.push({
        severity: "warning",
        code: "layout_audit_canvas_overflow_y",
        message: `纵向内容超出设计高度：实测 ${Math.round(contentHeight)}px / 设计 ${Math.round(designHeight)}px`,
      });
    }
    const clipped = detectClippedNodes(stage);
    if (clipped.length) {
      diagnostics.push({
        severity: "warning",
        code: "layout_audit_clipped_content",
        message: `检测到父容器裁切风险：${clipped.join("、")}`,
      });
    }
    const degenerate = detectDegenerateBoxes(stage);
    if (degenerate.length) {
      diagnostics.push({
        severity: "warning",
        code: "layout_audit_degenerate_box",
        message: `检测到零尺寸/退化盒：${degenerate.join("、")}`,
      });
    }
    const spacing = detectHeadMetricSpacing(stage);
    if (spacing.length) {
      diagnostics.push({
        severity: "info",
        code: "layout_audit_head_body_spacing_loose",
        message: `检测到标题与指标区起始距离偏大：${spacing.join("、")}`,
      });
    }
    const bottomClip = detectBottomClipRisk(stage);
    if (bottomClip.length) {
      diagnostics.push({
        severity: "warning",
        code: "layout_audit_panel_bottom_clip_risk",
        message: `检测到 panel 底部裁切风险：${bottomClip.join("、")}`,
      });
    }
    const overlap = detectCardOverlap(stage);
    if (overlap.length) {
      diagnostics.push({
        severity: "warning",
        code: "layout_audit_card_overlap",
        message: `检测到卡片重叠：${overlap.join("、")}`,
      });
    }
