  function queueUpdateViewport(root) {
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

  /** page-flow / fit-width：仅按宽度适配宿主，纵向可滚动，避免「贴右长条」。 */
  function computeManageWidthFitScale(root, hostWidth, contentWidth, fluidHeight) {
    if (hostWidth <= 0 || contentWidth <= 0) return 1;
    const editMode = String(root.dataset.editScaleMode || "")
      .trim()
      .toLowerCase();
    const widthOnly =
      fluidHeight ||
      editMode === "fit-width" ||
      editMode === "fit_width" ||
      editMode === "width";
    if (!widthOnly) {
      return null;
    }
    const fit = hostWidth / contentWidth;
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
    if (!viewportToolbarEnabled(root)) return;
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
    if (!viewportToolbarEnabled(root)) {
      root?.querySelector(":scope > .preview-viewport-toolbar")?.remove();
      return null;
    }
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
    if (!viewportToolbarEnabled(root)) return;
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

  function unlockStageLayoutForDebug(stage, designHeight, fluidHeight) {
    stage.style.overflow = "visible";
    stage.style.height = "auto";
    stage.style.minHeight = fluidHeight ? "0" : `${round(designHeight)}px`;
    stage.style.maxHeight = "none";
    if (fluidHeight) {
      relaxPageFlowStageGrid(stage);
    } else if (stage.dataset.meiDebugLayoutUnlocked !== "true") {
      const rows = getComputedStyle(stage).gridTemplateRows;
      if (rows && rows !== "none") {
        const parts = rows.split(/\s+/).filter((row) => row && row !== "none");
        stage.style.gridTemplateRows = parts
          .map((row, index) => {
            if (/^\d+(\.\d+)?px$/.test(row) && parseFloat(row) < 48) {
              return "minmax(240px, auto)";
            }
            if (index === 0) return row;
            if (/minmax|fr/i.test(row)) return "auto";
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

  function measureStageContentExtentByRect(stage, contentMaxWidth) {
    const cap = contentMaxWidth > 0 ? contentMaxWidth : 0;
    const stageRect = stage.getBoundingClientRect();
    if (cap > 0 && stageRect.width > 0 && stageRect.width <= cap + 2) {
      return { width: cap, height: Math.max(1, stageRect.height) };
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
        const rect = node.getBoundingClientRect();
        if (rect.width < 1 || rect.height < 1) return;
        hasContent = true;
        const right = rect.right - stageRect.left;
        const bottom = rect.bottom - stageRect.top;
        if (right > maxRight) maxRight = right;
        if (bottom > maxBottom) maxBottom = bottom;
      });
    let width = hasContent ? Math.max(1, maxRight, stageRect.width) : stageRect.width;
    let height = hasContent ? Math.max(1, maxBottom, stageRect.height) : stageRect.height;
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

  function closestPanelId(node) {
    const panel = node?.closest?.(".preview-card[data-mei-panel-id]");
    const id = String(panel?.getAttribute?.("data-mei-panel-id") || "").trim();
    return id || null;
  }

  function closestPanelLabel(node) {
    const panel = node?.closest?.(".preview-card[data-mei-panel-id]");
    return panel ? nodeAuditLabel(panel) : nodeAuditLabel(node);
  }

  function parsePropsPayload(node) {
    const raw = String(node?.getAttribute?.("data-props") || "").trim();
    if (!raw) return {};
    try {
      return JSON.parse(raw);
    } catch (_) {
      return {};
    }
  }

  function metricRoleNodes(card) {
    return Array.from(card.querySelectorAll("[data-props]"))
      .map((node) => ({ node, props: parsePropsPayload(node) }))
      .filter((entry) => String(entry.props?.metric_role || "").trim());
  }

  function widthBucket(width) {
    return Math.round(Number(width || 0) / 16) * 16;
  }

  function buildRuntimeEvalReport(diagnostics) {
    const items = Array.isArray(diagnostics) ? diagnostics : [];
    const severityWeight = {
      error: 100,
      warning: 40,
      info: 10,
    };
    const metrics = {
      total: items.length,
      errors: 0,
      warnings: 0,
      infos: 0,
      countsByCode: {},
    };
    const panelScores = new Map();
    let score = 0;
    items.forEach((diag) => {
      const severity = String(diag?.severity || "info").trim().toLowerCase();
      if (severity === "error") metrics.errors += 1;
      else if (severity === "warning") metrics.warnings += 1;
      else metrics.infos += 1;
      const weight = severityWeight[severity] || 0;
      score += weight;
      const code = String(diag?.code || "layout_eval_runtime_unknown").trim();
      metrics.countsByCode[code] = (metrics.countsByCode[code] || 0) + 1;
      const panelId = String(diag?.panelId || "").trim();
      if (panelId) {
        panelScores.set(panelId, (panelScores.get(panelId) || 0) + weight);
      }
    });
    const worstPanels = Array.from(panelScores.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([panelId, panelScore]) => ({
        panelId,
        score: panelScore,
      }));
    return {
      score,
      blocking: metrics.errors > 0,
      worstPanels,
      metrics,
    };
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
        hits.push({
          panelId: closestPanelId(node),
          label: nodeAuditLabel(node),
        });
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
        if (!clipsY && !clipsX) return;
        const nodeRect = node.getBoundingClientRect();
        let xOverflow = false;
        let yOverflow = false;
        node.querySelectorAll(":scope > *, :scope > * *").forEach((child) => {
          if (xOverflow && yOverflow) return;
          if (!(child instanceof Element)) return;
          const rect = child.getBoundingClientRect();
          if (rect.width < 1 || rect.height < 1) return;
          if (clipsX && (rect.left < nodeRect.left - 1 || rect.right > nodeRect.right + 1)) {
            xOverflow = true;
          }
          if (clipsY && (rect.top < nodeRect.top - 1 || rect.bottom > nodeRect.bottom + 1)) {
            yOverflow = true;
          }
        });
        if ((clipsY && yOverflow) || (clipsX && xOverflow)) {
          hits.push({
            panelId: closestPanelId(node),
            label: nodeAuditLabel(node),
          });
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
        hits.push({
          panelId: String(panel.getAttribute("data-mei-panel-id") || "").trim() || null,
          label: nodeAuditLabel(panel),
          gap,
        });
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
        hits.push({
          panelId: String(panel.getAttribute("data-mei-panel-id") || "").trim() || null,
          label: nodeAuditLabel(panel),
          overflowPx: maxBottom - bodyRect.bottom,
        });
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

  function rectOverlapX(a, b) {
    return Math.min(a.right, b.right) - Math.max(a.left, b.left);
  }

  function rectOverlapY(a, b) {
    return Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
  }

  function detectCardOverlap(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".panel-body-cell").forEach((body) => {
      if (hits.length >= limit) return;
      const groupPanelId = closestPanelId(body);
      const groupLabel = closestPanelLabel(body);
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
            hits.push({
              panelId: groupPanelId,
              label: groupLabel,
              pair: [nodeAuditLabel(cards[i]), nodeAuditLabel(cards[j])],
            });
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
      const groupPanelId = closestPanelId(body);
      const groupLabel = closestPanelLabel(body);
      const cards = Array.from(
        body.querySelectorAll(':scope > .preview-card[data-mei-panel-id]'),
      );
      if (cards.length < 2) return;
      const isMetricGroup = cards.every((card) => metricRoleNodes(card).length > 0);
      const maxHorizontalGap = isMetricGroup ? 32 : 12;
      const maxVerticalGap = isMetricGroup ? 24 : 16;
      const maxPadding = isMetricGroup ? 64 : 24;
      const bodyRect = body.getBoundingClientRect();
      const horizontal = cards
        .map((node) => node.getBoundingClientRect())
        .filter((rect) => rect.width > 1 && rect.height > 1)
        .sort((a, b) => a.left - b.left);
      if (horizontal.length >= 2) {
        const pair = horizontal.find((rect, idx) => {
          if (idx === 0) return false;
          return rectOverlapY(horizontal[idx - 1], rect) > 8;
        });
        const pairIdx = pair ? horizontal.indexOf(pair) : -1;
        const prev = pairIdx > 0 ? horizontal[pairIdx - 1] : null;
        const gap = prev && pair ? pair.left - prev.right : null;
        if (gap != null && (gap < 4 || gap > maxHorizontalGap)) {
          hits.push({
            panelId: groupPanelId,
            label: groupLabel,
            axis: "horizontal",
            gap,
          });
        }
      }
      const vertical = cards
        .map((node) => node.getBoundingClientRect())
        .filter((rect) => rect.width > 1 && rect.height > 1)
        .sort((a, b) => a.top - b.top);
      if (vertical.length >= 2) {
        const pair = vertical.find((rect, idx) => {
          if (idx === 0) return false;
          return rectOverlapX(vertical[idx - 1], rect) > 8;
        });
        const pairIdx = pair ? vertical.indexOf(pair) : -1;
        const prev = pairIdx > 0 ? vertical[pairIdx - 1] : null;
        const gap = prev && pair ? pair.top - prev.bottom : null;
        if (gap != null && (gap < 4 || gap > maxVerticalGap)) {
          hits.push({
            panelId: groupPanelId,
            label: groupLabel,
            axis: "vertical",
            gap,
          });
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
      if (maxPad > maxPadding) {
        hits.push({
          panelId: groupPanelId,
          label: groupLabel,
          axis: "padding",
          gap: maxPad,
        });
      }
      const horizontalPadDelta = Math.abs(leftPad - rightPad);
      if (horizontalPadDelta > 12) {
        hits.push({
          panelId: groupPanelId,
          label: groupLabel,
          axis: "padding_symmetry",
          gap: horizontalPadDelta,
          leftPad,
          rightPad,
        });
      }
    });
    return hits;
  }

  function detectMetricVerticalBandDrift(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".preview-card[data-mei-panel-id]").forEach((card) => {
      if (hits.length >= limit) return;
      const rect = card.getBoundingClientRect();
      if (rect.width < 8 || rect.height < 24) return;
      const roles = metricRoleNodes(card);
      const labelNode = roles.find((entry) => entry.props.metric_role === "label");
      const valueNode = roles.find((entry) => entry.props.metric_role === "value");
      if (!labelNode || !valueNode) return;
      const labelRect = labelNode.node.getBoundingClientRect();
      const valueRect = valueNode.node.getBoundingClientRect();
      const midY = rect.top + rect.height * 0.5;
      const labelCenterY = (labelRect.top + labelRect.bottom) / 2;
      const valueBottomY = valueRect.bottom;
      const panelId = closestPanelId(card);
      const label = closestPanelLabel(card);
      if (labelCenterY > midY + 6) {
        hits.push({
          panelId,
          label,
          role: "label",
          spread: labelCenterY - midY,
          detail: "label_below_midline",
        });
        return;
      }
      if (valueBottomY < midY - 4) {
        hits.push({
          panelId,
          label,
          role: "value",
          spread: midY - valueBottomY,
          detail: "value_above_midline",
        });
      }
    });
    return hits;
  }

  function detectMetricAlignmentDrift(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".panel-body-cell").forEach((body) => {
      if (hits.length >= limit) return;
      const groupPanelId = closestPanelId(body);
      const groupLabel = closestPanelLabel(body);
      const cards = Array.from(
        body.querySelectorAll(':scope > .preview-card[data-mei-panel-id]'),
      )
        .map((card) => {
          const rect = card.getBoundingClientRect();
          const roles = metricRoleNodes(card);
          const slotRects = {
            label: roles
              .filter((entry) => entry.props.metric_role === "label")
              .map((entry) => entry.node.getBoundingClientRect().left - rect.left),
            value: roles
              .filter((entry) => entry.props.metric_role === "value")
              .map((entry) => entry.node.getBoundingClientRect().right - rect.left),
            unit: roles
              .filter((entry) => entry.props.metric_role === "unit")
              .map((entry) => entry.node.getBoundingClientRect().right - rect.left),
          };
          return { rect, slotRects };
        })
        .filter((entry) => entry.rect.width > 1 && entry.rect.height > 1);
      if (cards.length < 2) return;
      const buckets = new Map();
      cards.forEach((card) => {
        const bucket = widthBucket(card.rect.width);
        if (!buckets.has(bucket)) buckets.set(bucket, []);
        buckets.get(bucket).push(card);
      });
      buckets.forEach((bucketCards) => {
        if (bucketCards.length < 2 || hits.length >= limit) return;
        const labelLefts = bucketCards.flatMap((item) => item.slotRects.label);
        const valueRights = bucketCards.flatMap((item) => item.slotRects.value);
        const unitRights = bucketCards.flatMap((item) => item.slotRects.unit);
        const spreads = [];
        if (labelLefts.length >= 2) {
          spreads.push({ role: "label", spread: Math.max(...labelLefts) - Math.min(...labelLefts) });
        }
        if (valueRights.length >= 2) {
          spreads.push({ role: "value", spread: Math.max(...valueRights) - Math.min(...valueRights) });
        }
        if (unitRights.length >= 2) {
          spreads.push({ role: "unit", spread: Math.max(...unitRights) - Math.min(...unitRights) });
        }
        const worst = spreads.sort((a, b) => b.spread - a.spread)[0];
        if (worst && worst.spread > 8) {
          hits.push({
            panelId: groupPanelId,
            label: groupLabel,
            role: worst.role,
            spread: worst.spread,
          });
        }
      });
    });
    return hits;
  }

  const LAYOUT_AUDIT_EVENT = "mei:layout-audit";

  function layoutAuditStorageKey(sourcePath) {
    const pathname = String(window.location.pathname || "").trim();
    const source = String(sourcePath || "main.mei").trim() || "main.mei";
    return `mei:layout-audit:${pathname}:${source}`;
  }

  function persistLayoutAudit(payload) {
    try {
      const key = layoutAuditStorageKey(payload?.sourcePath);
      sessionStorage.setItem(key, JSON.stringify(payload));
    } catch (_) {}
  }

  function dispatchLayoutAudit(targetDocument, payload) {
    if (!targetDocument) return;
    targetDocument.dispatchEvent(new CustomEvent(LAYOUT_AUDIT_EVENT, { detail: payload }));
  }

  function sceneIdFromLocation() {
    try {
      const url = new URL(window.location.href);
      const scene = String(url.searchParams.get("scene") || "").trim();
      if (scene) return scene;
      const match = String(url.pathname || "").match(/\/scene\/([^/?#]+)/i);
      if (match && match[1]) {
        return decodeURIComponent(match[1]);
      }
    } catch (_) {}
    return "";
  }

  function collectPanelMeta(stage) {
    const map = new Map();
    if (!stage || !stage.querySelectorAll) return map;
    stage.querySelectorAll("[data-mei-panel-id]").forEach((panel) => {
      const panelId = String(panel.getAttribute("data-mei-panel-id") || "").trim();
      if (!panelId || map.has(panelId)) return;
      const label =
        String(panel.querySelector?.("[data-mei-panel-head]")?.getAttribute?.("aria-label") || "").trim() ||
        panelId;
      map.set(panelId, {
        panelId,
        panelLabel: label,
        componentLabel: label,
      });
    });
    return map;
  }

  function enrichLayoutDiagnostics(diagnostics, panelMeta, sceneId, targetFile) {
    const list = Array.isArray(diagnostics) ? diagnostics : [];
    return list.map((diag) => {
      const panelId = String(diag?.panelId || "").trim();
      const panel = panelId ? panelMeta.get(panelId) : null;
      const panelLabel = String(diag?.panelLabel || diag?.label || panel?.panelLabel || "").trim();
      const componentLabel = String(diag?.component_label || panelLabel || panel?.componentLabel || "").trim();
      return {
        ...diag,
        panelId: panelId || undefined,
        panelLabel: panelLabel || undefined,
        component_label: componentLabel || undefined,
        scene_id: sceneId || undefined,
        target_file: targetFile || undefined,
        source_path: String(diag?.source_path || targetFile || "").trim() || undefined,
      };
    });
  }

  function publishLayoutAudit(root, stage, diagnostics, report = {}) {
    const targetFile = String(
      root?.dataset?.targetFile || root?.dataset?.sourcePath || root?.dataset?.activeTarget || "main.mei"
    ).trim();
    const sceneId = String(root?.dataset?.sceneId || "").trim() || sceneIdFromLocation();
    const panelMeta = collectPanelMeta(stage);
    const normalizedDiagnostics = enrichLayoutDiagnostics(diagnostics, panelMeta, sceneId, targetFile);
    const normalizedWorstPanels = (Array.isArray(report?.worstPanels) ? report.worstPanels : []).map((entry) => {
      const panelId = String(entry?.panelId || "").trim();
      const panel = panelId ? panelMeta.get(panelId) : null;
      return {
        ...entry,
        panelId: panelId || entry?.panelId,
        panelLabel: String(entry?.panelLabel || panel?.panelLabel || "").trim() || undefined,
      };
    });
    const payload = {
      sourcePath: targetFile || "main.mei",
      targetFile: targetFile || "main.mei",
      sceneId: sceneId || undefined,
      diagnostics: normalizedDiagnostics,
      score: Number(report?.score || 0),
      blocking: report?.blocking === true,
      worstPanels: normalizedWorstPanels,
      metrics: report?.metrics && typeof report.metrics === "object" ? report.metrics : {},
    };
    window.__meiLastLayoutEval = payload;
    persistLayoutAudit(payload);
    dispatchLayoutAudit(document, payload);
    if (window.parent && window.parent !== window) {
      try {
        window.parent.__meiLastLayoutEval = payload;
      } catch (_) {}
      try {
        dispatchLayoutAudit(window.parent.document, payload);
      } catch (_) {}
      try {
        window.parent.postMessage({ type: LAYOUT_AUDIT_EVENT, detail: payload }, window.location.origin);
      } catch (_) {}
    }
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
        severity: "error",
        code: "layout_eval_canvas_overflow_x",
        message: `横向内容超出设计宽度：实测 ${Math.round(extentWidth)}px / 设计 ${Math.round(designWidth)}px`,
      });
    }
    if (contentHeight > designHeight + 1) {
      diagnostics.push({
        severity: "warning",
        code: "layout_eval_canvas_overflow_y",
        message: `纵向内容超出设计高度：实测 ${Math.round(contentHeight)}px / 设计 ${Math.round(designHeight)}px`,
      });
    }
    const clipped = detectClippedNodes(stage);
    if (clipped.length) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_clipped_content",
        panelId: clipped[0]?.panelId || null,
        message: `检测到父容器裁切风险：${clipped.map((hit) => hit.label).join("、")}`,
      });
    }
    const degenerate = detectDegenerateBoxes(stage);
    if (degenerate.length) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_degenerate_box",
        panelId: degenerate[0]?.panelId || null,
        message: `检测到零尺寸/退化盒：${degenerate.map((hit) => hit.label).join("、")}`,
      });
    }
    const spacing = detectHeadMetricSpacing(stage);
    if (spacing.length) {
      diagnostics.push({
        severity: "info",
        code: "layout_eval_head_body_spacing_loose",
        panelId: spacing[0]?.panelId || null,
        message: `检测到标题与指标区起始距离偏大：${spacing
          .map((hit) => `${hit.label}(+${Math.round(hit.gap)}px)`)
          .join("、")}`,
      });
    }
    const bottomClip = detectBottomClipRisk(stage);
    if (bottomClip.length) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_panel_bottom_clip_risk",
        panelId: bottomClip[0]?.panelId || null,
        message: `检测到 panel 底部裁切风险：${bottomClip
          .map((hit) => `${hit.label}(+${Math.round(hit.overflowPx)}px)`)
          .join("、")}`,
      });
    }
    const overlap = detectCardOverlap(stage);
    if (overlap.length) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_card_overlap",
        panelId: overlap[0]?.panelId || null,
        message: `检测到卡片重叠：${overlap
          .map((hit) => `${hit.label}(${hit.pair.join(" x ")})`)
          .join("、")}`,
      });
    }
    const gapBudget = detectCardGapBudget(stage);
    if (gapBudget.length) {
      diagnostics.push({
        severity: gapBudget.some(
          (hit) => hit.axis === "padding" || hit.axis === "padding_symmetry",
        )
          ? "warning"
          : "info",
        code: "layout_eval_card_gap_budget_runtime",
        panelId: gapBudget[0]?.panelId || null,
        message: `检测到卡组 gap/留白偏离预算：${gapBudget
          .map((hit) => {
            if (hit.axis === "padding") return `${hit.label}(四周留白≈${Math.round(hit.gap)}px)`;
            if (hit.axis === "padding_symmetry") {
              return `${hit.label}(左右留白≈${Math.round(hit.leftPad || 0)}px/${Math.round(hit.rightPad || 0)}px)`;
            }
            const axis = hit.axis === "vertical" ? "纵向" : "横向";
            return `${hit.label}(${axis} gap≈${Math.round(hit.gap)}px)`;
          })
          .join("、")}`,
      });
    }
    const alignment = detectMetricAlignmentDrift(stage);
    if (alignment.length) {
      diagnostics.push({
        severity: "warning",
        code: "layout_eval_metric_alignment_drift",
        panelId: alignment[0]?.panelId || null,
        message: `检测到指标参考线漂移：${alignment
          .map((hit) => `${hit.label}(${hit.role} 偏移≈${Math.round(hit.spread)}px)`)
          .join("、")}`,
      });
    }
    const verticalBands = detectMetricVerticalBandDrift(stage);
    if (verticalBands.length) {
      diagnostics.push({
        severity: "warning",
        code: "layout_eval_metric_vertical_band_drift",
        panelId: verticalBands[0]?.panelId || null,
        message: `检测到指标垂直分区落点偏离：${verticalBands
          .map((hit) => {
            const hint =
              hit.detail === "label_below_midline"
                ? "标题区落入下半区"
                : "数值区落在上半区";
            return `${hit.label}(${hint}，≈${Math.round(hit.spread)}px)`;
          })
          .join("、")}`,
      });
    }
    publishLayoutAudit(root, stage, diagnostics, buildRuntimeEvalReport(diagnostics));
  }
