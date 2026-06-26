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
