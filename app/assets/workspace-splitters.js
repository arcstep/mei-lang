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
  if (!root || !handles.length || window.matchMedia("(max-width: 1200px)").matches) return;
  const splitterPx = 8;
  const minMain = 320;
  const config = {
    left: {
      cssVar: "--workspace-left-aside",
      storageKey: "mei-lang.workspaceLeftAsidePx",
      fallback: 260,
      min: 220,
      axis: "x",
      target: root
    },
    right: {
      cssVar: "--workspace-right-aside",
      storageKey: "mei-lang.workspaceRightAsidePx",
      fallback: 320,
      min: 280,
      axis: "x",
      target: root
    }
  };
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
    const otherWidth = readPx(otherSide);
    const rect = root.getBoundingClientRect();
    const splittersTotalPx = splitterPx * 2;
    return Math.max(
      meta.min,
      rect.width - otherWidth - splittersTotalPx - minMain
    );
  }
  Object.keys(config).forEach((side) => {
    try {
      const meta = config[side];
      if (!meta.target) return;
      const saved = localStorage.getItem(meta.storageKey);
      let px = meta.fallback;
      if (saved) {
        const parsed = parseInt(saved, 10);
        if (!Number.isNaN(parsed) && parsed >= meta.min) {
          px = parsed;
        }
      }
      writePx(side, clamp(px, meta.min, maxPx(side)));
    } catch (_) {}
  });
  let dragging = false;
  let draggingSide = "";
  let activeHandle = null;
  let startCoord = 0;
  let startW = 0;
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
    applySize(coord);
    if (ev.cancelable) ev.preventDefault();
  }
  function onEnd() {
    if (!dragging) return;
    dragging = false;
    if (activeHandle) activeHandle.classList.remove("splitter-active");
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
    const handle = ev.currentTarget;
    const side = handle && handle.getAttribute ? handle.getAttribute("data-workspace-splitter") : "";
    if (!config[side]) return;
    const meta = config[side];
    if (!meta.target) return;
    dragging = true;
    draggingSide = side;
    activeHandle = handle;
    const point = ev.touches && ev.touches[0] ? ev.touches[0] : ev;
    startCoord = meta.axis === "y" ? point.clientY : point.clientX;
    startW = readPx(side);
    handle.classList.add("splitter-active");
    document.body.style.cursor = meta.axis === "y" ? "row-resize" : "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onEnd);
    window.addEventListener("touchmove", onMove, { passive: false });
    window.addEventListener("touchend", onEnd);
    window.addEventListener("touchcancel", onEnd);
    ev.preventDefault();
  }
  handles.forEach((handle) => {
    handle.addEventListener("mousedown", onStart);
    handle.addEventListener("touchstart", onStart, { passive: false });
  });
  const onResize = function () {
    Object.keys(config).forEach((side) => {
      const meta = config[side];
      if (!meta || !meta.target) return;
      writePx(side, clamp(readPx(side), meta.min, maxPx(side)));
    });
  };
  window.addEventListener("resize", onResize);
  boot.disposeWorkspaceSplitters = function () {
    onEnd();
    handles.forEach((handle) => {
      handle.removeEventListener("mousedown", onStart);
      handle.removeEventListener("touchstart", onStart, { passive: false });
    });
    window.removeEventListener("resize", onResize);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onEnd);
    window.removeEventListener("touchmove", onMove);
    window.removeEventListener("touchend", onEnd);
    window.removeEventListener("touchcancel", onEnd);
  };
})();
