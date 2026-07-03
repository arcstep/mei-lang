(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const CONTEXT_PLANE_ID = "mei-viewport-context-plane";
  const VIEWPORT_ROOT_SELECTOR = '[data-mei-frame-viewport="true"]';

  function resolveViewportFrameRoot() {
    const node = document.querySelector(VIEWPORT_ROOT_SELECTOR);
    return node instanceof HTMLElement ? node : null;
  }

  function readViewportFrameScale() {
    const root = resolveViewportFrameRoot();
    const scale = Number(root?.dataset?.meiFrameScale || 1);
    return Number.isFinite(scale) && scale > 0 ? scale : 1;
  }

  function resolveViewportStageShell() {
    if (typeof boot.resolveViewportStageHost === "function") {
      const host = boot.resolveViewportStageHost();
      if (host instanceof HTMLElement && host !== document.body) {
        return host;
      }
    }
    const viewport = resolveViewportFrameRoot();
    const shell = viewport?.querySelector(".preview-stage-shell");
    return shell instanceof HTMLElement ? shell : null;
  }

  function resolveViewportStageSurface() {
    const shell = resolveViewportStageShell();
    const stage = shell?.querySelector(".preview-stage.preview-surface");
    return stage instanceof HTMLElement ? stage : null;
  }

  function clientPointToStageLocal(stage, clientX, clientY) {
    if (!(stage instanceof HTMLElement)) {
      return { left: clientX, top: clientY };
    }
    const rect = stage.getBoundingClientRect();
    const designW = stage.offsetWidth || 1920;
    const designH = stage.offsetHeight || 1080;
    const scaleX = rect.width > 0 ? designW / rect.width : 1;
    const scaleY = rect.height > 0 ? designH / rect.height : 1;
    return {
      left: (clientX - rect.left) * scaleX,
      top: (clientY - rect.top) * scaleY,
    };
  }

  function viewportOverlayActive() {
    return Boolean(resolveViewportStageShell());
  }

  function windowBounds(padding = 10) {
    const width = Number(window.innerWidth || 0);
    const height = Number(window.innerHeight || 0);
    return {
      mode: "window",
      scale: 1,
      width,
      height,
      padding,
      shell: null,
      shellRect: null,
      stage: null,
      clientToLocal(clientLeft, clientTop) {
        return { left: clientLeft, top: clientTop };
      },
      clampClientRect(left, top, width, height) {
        const maxW = Math.max(0, this.width - padding * 2);
        const maxH = Math.max(0, this.height - padding * 2);
        const w = Math.min(width, maxW);
        const h = Math.min(height, maxH);
        let nextLeft = Math.min(
          Math.max(padding, left),
          Math.max(padding, this.width - w - padding),
        );
        let nextTop = Math.min(
          Math.max(padding, top),
          Math.max(padding, this.height - h - padding),
        );
        return { left: nextLeft, top: nextTop, width: w, height: h };
      },
      clampLocalRect(left, top, width, height) {
        return this.clampClientRect(left, top, width, height);
      },
      maxWidth(defaultMax) {
        return Math.min(defaultMax, Math.max(0, this.width - padding * 2));
      },
      maxHeight(defaultMax) {
        return Math.min(defaultMax, Math.max(0, this.height - padding * 2));
      },
    };
  }

  function resolveViewportOverlayBounds(anchorEl, padding = 10) {
    const shell = resolveViewportStageShell();
    if (!shell) {
      return windowBounds(padding);
    }
    const shellRect = shell.getBoundingClientRect();
    const stage = resolveViewportStageSurface();
    const scale = readViewportFrameScale();
    if (stage) {
      const designW = stage.offsetWidth || 1920;
      const designH = stage.offsetHeight || 1080;
      return {
        mode: "stage-design",
        scale,
        width: designW,
        height: designH,
        designW,
        designH,
        padding,
        shell,
        shellRect,
        stage,
        clientToLocal(clientLeft, clientTop) {
          return clientPointToStageLocal(stage, clientLeft, clientTop);
        },
        clampClientRect(left, top, width, height) {
          const minLeft = shellRect.left + padding;
          const minTop = shellRect.top + padding;
          const maxLeft = shellRect.right - padding;
          const maxTop = shellRect.bottom - padding;
          const maxW = Math.max(0, maxLeft - minLeft);
          const maxH = Math.max(0, maxTop - minTop);
          const w = Math.min(width, maxW);
          const h = Math.min(height, maxH);
          let nextLeft = Math.min(Math.max(minLeft, left), Math.max(minLeft, maxLeft - w));
          let nextTop = Math.min(Math.max(minTop, top), Math.max(minTop, maxTop - h));
          return { left: nextLeft, top: nextTop, width: w, height: h };
        },
        clampLocalRect(left, top, width, height) {
          const maxW = Math.max(0, designW - padding * 2);
          const maxH = Math.max(0, designH - padding * 2);
          const w = Math.min(width, maxW);
          const h = Math.min(height, maxH);
          let nextLeft = Math.min(
            Math.max(padding, left),
            Math.max(padding, designW - w - padding),
          );
          let nextTop = Math.min(
            Math.max(padding, top),
            Math.max(padding, designH - h - padding),
          );
          return { left: nextLeft, top: nextTop, width: w, height: h };
        },
        maxWidth(defaultMax) {
          return Math.min(defaultMax, Math.max(0, designW - padding * 2));
        },
        maxHeight(defaultMax) {
          return Math.min(defaultMax, Math.max(0, designH - padding * 2));
        },
      };
    }
    const width = Math.max(0, shell.clientWidth || shellRect.width || 0);
    const height = Math.max(0, shell.clientHeight || shellRect.height || 0);
    return {
      mode: "stage-shell",
      scale,
      width,
      height,
      padding,
      shell,
      shellRect,
      stage: null,
      clientToLocal(clientLeft, clientTop) {
        return {
          left: clientLeft - shellRect.left,
          top: clientTop - shellRect.top,
        };
      },
      clampClientRect(left, top, width, height) {
        const minLeft = shellRect.left + padding;
        const minTop = shellRect.top + padding;
        const maxLeft = shellRect.right - padding;
        const maxTop = shellRect.bottom - padding;
        const maxW = Math.max(0, maxLeft - minLeft);
        const maxH = Math.max(0, maxTop - minTop);
        const w = Math.min(width, maxW);
        const h = Math.min(height, maxH);
        let nextLeft = Math.min(Math.max(minLeft, left), Math.max(minLeft, maxLeft - w));
        let nextTop = Math.min(Math.max(minTop, top), Math.max(minTop, maxTop - h));
        return { left: nextLeft, top: nextTop, width: w, height: h };
      },
      clampLocalRect(left, top, width, height) {
        const maxW = Math.max(0, this.width - padding * 2);
        const maxH = Math.max(0, this.height - padding * 2);
        const w = Math.min(width, maxW);
        const h = Math.min(height, maxH);
        let nextLeft = Math.min(
          Math.max(padding, left),
          Math.max(padding, this.width - w - padding),
        );
        let nextTop = Math.min(
          Math.max(padding, top),
          Math.max(padding, this.height - h - padding),
        );
        return { left: nextLeft, top: nextTop, width: w, height: h };
      },
      maxWidth(defaultMax) {
        return Math.min(defaultMax, Math.max(0, width - padding * 2));
      },
      maxHeight(defaultMax) {
        return Math.min(defaultMax, Math.max(0, height - padding * 2));
      },
    };
  }

  function resolveOverlayMountRoot(anchorEl) {
    const shell = resolveViewportStageShell();
    if (!shell) {
      return document.body;
    }
    const stage = resolveViewportStageSurface();
    if (anchorEl instanceof Element) {
      if (anchorEl.closest("#mei-layer2-workspace")) {
        return anchorEl.closest("#mei-layer2-workspace");
      }
      if (stage && anchorEl.closest(".preview-stage.preview-surface")) {
        return stage;
      }
    }
    return stage || shell;
  }

  function ensureViewportContextPlane(root) {
    if (!(root instanceof HTMLElement) || root === document.body) {
      return null;
    }
    let plane = root.querySelector(`:scope > #${CONTEXT_PLANE_ID}`);
    if (!plane) {
      plane = document.createElement("div");
      plane.id = CONTEXT_PLANE_ID;
      plane.className = "mei-viewport-context-plane";
      root.appendChild(plane);
    }
    return plane;
  }

  function resolveRuntimeOverlayZIndex(token, anchorEl) {
    const inLayer2 =
      anchorEl instanceof Element && Boolean(anchorEl.closest("#mei-layer2-workspace"));
    const table = {
      map_tools: 1210,
      tooltip: inLayer2 ? 2300 : 1300,
      text_popover: 2350,
      spa_loading: 5050,
    };
    return table[String(token || "").trim()] ?? 1300;
  }

  function mountViewportFloatingNode(node, anchorEl) {
    if (!(node instanceof HTMLElement)) {
      return null;
    }
    const root = resolveOverlayMountRoot(anchorEl);
    const plane = ensureViewportContextPlane(root);
    const parent = plane || root;
    if (node.parentElement !== parent) {
      parent.appendChild(node);
    }
    node.classList.toggle("mei-viewport-floating-in-stage", Boolean(plane));
    return parent;
  }

  function copilotFloatingBoundsSizePatched() {
    const shell = resolveViewportStageShell();
    if (shell) {
      return {
        width: Math.max(0, shell.clientWidth || shell.offsetWidth || 0),
        height: Math.max(0, shell.clientHeight || shell.offsetHeight || 0),
      };
    }
  }

  function mountRuntimeOverlay(node, options = {}) {
    if (!(node instanceof HTMLElement)) {
      return null;
    }
    const role = String(options.role || "tooltip").trim();
    node.setAttribute("data-mei-overlay-role", role);
    node.style.removeProperty("z-index");
    const anchor = options.anchor;
    if (role === "map_tools" && typeof boot.mountCockpitFloatingControl === "function") {
      return boot.mountCockpitFloatingControl(node, anchor);
    }
    if (role === "spa_loading") {
      if (node.parentElement !== document.body) {
        document.body.appendChild(node);
      }
      return document.body;
    }
    return mountViewportFloatingNode(node, anchor);
  }

  boot.readViewportFrameScale = readViewportFrameScale;
  boot.viewportOverlayActive = viewportOverlayActive;
  boot.resolveViewportOverlayBounds = resolveViewportOverlayBounds;
  boot.resolveOverlayMountRoot = resolveOverlayMountRoot;
  boot.ensureViewportContextPlane = ensureViewportContextPlane;
  boot.mountViewportFloatingNode = mountViewportFloatingNode;
  boot.mountRuntimeOverlay = mountRuntimeOverlay;
  boot.resolveRuntimeOverlayZIndex = resolveRuntimeOverlayZIndex;
  boot.clientPointToStageLocal = clientPointToStageLocal;
  boot._viewportOverlayBoundsPatched = copilotFloatingBoundsSizePatched;
})();
