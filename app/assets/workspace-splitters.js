(function initWorkspaceSplitters() {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (typeof boot.disposeWorkspaceSplitters === "function") {
    try {
      boot.disposeWorkspaceSplitters();
    } catch (_) {}
    boot.disposeWorkspaceSplitters = null;
  }
  const root = document.getElementById("workspace-root");
  const handles = Array.from(document.querySelectorAll("[data-workspace-splitter]"));
  const toggleButtons = Array.from(document.querySelectorAll("[data-workspace-toggle]"));
  const splitterPx = 8;
  const activateDragDeltaPx = 3;
  const minMain = 320;
  const config = {
    left: {
      cssVar: "--workspace-left-aside",
      storageKey: "mei-lang.workspaceLeftAsidePx",
      collapsedKey: "mei-lang.workspaceLeftAsideCollapsed",
      fallback: 260,
      min: 220,
      axis: "x",
      target: root
    },
    right: {
      cssVar: "--workspace-right-aside",
      storageKey: "mei-lang.workspaceRightAsidePx",
      collapsedKey: "mei-lang.workspaceRightAsideCollapsed",
      fallback: 320,
      min: 280,
      axis: "x",
      target: root
    }
  };
  const activeHandles = handles.filter(function (handle) {
    const side = handle && handle.getAttribute ? handle.getAttribute("data-workspace-splitter") : "";
    return !!config[side];
  });
  const activeSides = Array.from(
    new Set(
      activeHandles
        .map(function (handle) {
          return handle.getAttribute("data-workspace-splitter") || "";
        })
        .concat(
          toggleButtons
            .map(function (button) {
              return button.getAttribute("data-workspace-toggle") || "";
            })
            .filter(function (side) {
              return !!config[side];
            })
        )
    )
  );
  if (!root || !activeHandles.length || !activeSides.length) return;
  const collapsed = { left: false, right: false };
  function toggleButton(side) {
    return toggleButtons.find(function (button) {
      return button.getAttribute("data-workspace-toggle") === side;
    }) || null;
  }
  function syncCollapsedUi() {
    root.dataset.leftCollapsed = collapsed.left ? "true" : "false";
    root.dataset.rightCollapsed = collapsed.right ? "true" : "false";
    ["left", "right"].forEach(function (side) {
      const button = toggleButton(side);
      if (!button) return;
      const isCollapsed = !!collapsed[side];
      button.dataset.collapsed = isCollapsed ? "true" : "false";
      const noun = side === "left" ? "左侧资源栏" : "右侧助手栏";
      const action = isCollapsed ? "展开" : "折叠";
      button.setAttribute("aria-label", action + noun);
      button.setAttribute("title", action + noun);
    });
  }
  function clamp(n, lo, hi) {
    return Math.max(lo, Math.min(hi, n));
  }
  function readPx(side) {
    const meta = config[side];
    if (!meta || !meta.target) return 0;
    const raw = getComputedStyle(meta.target).getPropertyValue(meta.cssVar).trim();
    const m = raw.match(/^(\d+(?:\.\d+)?)px$/);
    if (m) return Math.round(parseFloat(m[1], 10));
    return meta.fallback;
  }
  function writePx(side, px) {
    const meta = config[side];
    if (!meta || !meta.target) return;
    meta.target.style.setProperty(meta.cssVar, px + "px");
  }
  function maxPx(side) {
    const meta = config[side];
    if (!meta || !meta.target) return meta ? meta.fallback : 0;
    const otherSide = side === "left" ? "right" : "left";
    const otherWidth = activeSides.includes(otherSide) ? readPx(otherSide) : 0;
    const rect = root.getBoundingClientRect();
    const splittersTotalPx = activeHandles.length * splitterPx;
    return Math.max(
      meta.min,
      rect.width - otherWidth - splittersTotalPx - minMain
    );
  }
  activeSides.forEach((side) => {
    try {
      const meta = config[side];
      if (!meta.target) return;
      collapsed[side] = localStorage.getItem(meta.collapsedKey) === "1";
      const saved = localStorage.getItem(meta.storageKey);
      let px = meta.fallback;
      if (saved) {
        const parsed = parseInt(saved, 10);
        if (!Number.isNaN(parsed) && parsed >= meta.min) {
          px = parsed;
        }
      }
      if (collapsed[side]) {
        writePx(side, 0);
      } else {
        writePx(side, clamp(px, meta.min, maxPx(side)));
      }
    } catch (_) {}
  });
  syncCollapsedUi();
  let dragging = false;
  let draggingSide = "";
  let activeHandle = null;
  let startCoord = 0;
  let startW = 0;
  let dragActivated = false;
  function applySize(clientCoord) {
    if (!draggingSide) return;
    const meta = config[draggingSide];
    const delta = clientCoord - startCoord;
    const rawNext =
      draggingSide === "left" || draggingSide === "preview"
        ? startW + delta
        : startW - delta;
    const next = clamp(rawNext, meta.min, maxPx(draggingSide));
    writePx(draggingSide, next);
  }
  function onMove(ev) {
    if (!dragging) return;
    const meta = config[draggingSide];
    const point = ev.touches && ev.touches[0] ? ev.touches[0] : ev;
    const coord = meta && meta.axis === "y" ? point.clientY : point.clientX;
    if (!dragActivated && Math.abs(coord - startCoord) >= activateDragDeltaPx) {
      dragActivated = true;
      if (activeHandle) activeHandle.classList.add("splitter-active");
    }
    applySize(coord);
    if (ev.cancelable) ev.preventDefault();
  }
  function onEnd() {
    if (!dragging) return;
    dragging = false;
    if (activeHandle) activeHandle.classList.remove("splitter-active");
    dragActivated = false;
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onEnd);
    window.removeEventListener("touchmove", onMove);
    window.removeEventListener("touchend", onEnd);
    window.removeEventListener("touchcancel", onEnd);
    try {
      const meta = config[draggingSide];
      if (meta) {
        localStorage.setItem(meta.storageKey, String(readPx(draggingSide)));
      }
    } catch (_) {}
    activeHandle = null;
    draggingSide = "";
  }
  function onStart(ev) {
    if (ev.type === "mousedown" && ev.button !== 0) return;
    if (ev.target instanceof Element && ev.target.closest("[data-workspace-toggle]")) return;
    const handle = ev.currentTarget;
    const side = handle && handle.getAttribute ? handle.getAttribute("data-workspace-splitter") : "";
    if (!config[side]) return;
    if (collapsed[side]) return;
    const meta = config[side];
    if (!meta.target) return;
    dragging = true;
    draggingSide = side;
    activeHandle = handle;
    const point = ev.touches && ev.touches[0] ? ev.touches[0] : ev;
    startCoord = meta.axis === "y" ? point.clientY : point.clientX;
    startW = readPx(side);
    dragActivated = false;
    handle.classList.remove("splitter-active");
    document.body.style.cursor = meta.axis === "y" ? "row-resize" : "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onEnd);
    window.addEventListener("touchmove", onMove, { passive: false });
    window.addEventListener("touchend", onEnd);
    window.addEventListener("touchcancel", onEnd);
    ev.preventDefault();
  }
  function toggleSide(side) {
    const meta = config[side];
    if (!meta || !meta.target) return;
    if (collapsed[side]) {
      collapsed[side] = false;
      try {
        localStorage.removeItem(meta.collapsedKey);
      } catch (_) {}
      const saved = localStorage.getItem(meta.storageKey);
      let px = meta.fallback;
      if (saved) {
        const parsed = parseInt(saved, 10);
        if (!Number.isNaN(parsed) && parsed >= meta.min) {
          px = parsed;
        }
      }
      writePx(side, clamp(px, meta.min, maxPx(side)));
    } else {
      const current = readPx(side);
      if (current >= meta.min) {
        try {
          localStorage.setItem(meta.storageKey, String(current));
        } catch (_) {}
      }
      collapsed[side] = true;
      writePx(side, 0);
      try {
        localStorage.setItem(meta.collapsedKey, "1");
      } catch (_) {}
    }
    syncCollapsedUi();
  }
  function onToggleClick(ev) {
    ev.preventDefault();
    ev.stopPropagation();
    const button = ev.currentTarget;
    const side = button && button.getAttribute ? button.getAttribute("data-workspace-toggle") || "" : "";
    if (!side) return;
    toggleSide(side);
  }
  activeHandles.forEach((handle) => {
    handle.addEventListener("mousedown", onStart);
    handle.addEventListener("touchstart", onStart, { passive: false });
  });
  toggleButtons.forEach((button) => {
    button.addEventListener("click", onToggleClick);
  });
  const onResize = function () {
    activeSides.forEach((side) => {
      const meta = config[side];
      if (!meta || !meta.target) return;
      if (collapsed[side]) {
        writePx(side, 0);
      } else {
        writePx(side, clamp(readPx(side), meta.min, maxPx(side)));
      }
    });
    syncCollapsedUi();
  };
  window.addEventListener("resize", onResize);
  boot.disposeWorkspaceSplitters = function () {
    onEnd();
    activeHandles.forEach((handle) => {
      handle.removeEventListener("mousedown", onStart);
      handle.removeEventListener("touchstart", onStart, { passive: false });
    });
    toggleButtons.forEach((button) => {
      button.removeEventListener("click", onToggleClick);
    });
    window.removeEventListener("resize", onResize);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onEnd);
    window.removeEventListener("touchmove", onMove);
    window.removeEventListener("touchend", onEnd);
    window.removeEventListener("touchcancel", onEnd);
  };
})();
