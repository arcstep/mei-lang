pub(super) const SPLITTER_SCRIPT: &str = r#"
(function initWorkspaceSplitters() {
  const root = document.getElementById("workspace-root");
  const handles = Array.from(document.querySelectorAll("[data-workspace-splitter]"));
  if (!root || !handles.length || window.matchMedia("(max-width: 1200px)").matches) return;
  const splitterPx = 8;
  const splittersTotalPx = splitterPx * handles.length;
  const minMain = 320;
  const config = {
    left: {
      cssVar: "--workspace-left-aside",
      storageKey: "mei-lang.workspaceLeftAsidePx",
      fallback: 260,
      min: 220
    },
    right: {
      cssVar: "--workspace-right-aside",
      storageKey: "mei-lang.workspaceRightAsidePx",
      fallback: 320,
      min: 280
    }
  };
  function clamp(n, lo, hi) {
    return Math.max(lo, Math.min(hi, n));
  }
  function readAsidePx(side) {
    const meta = config[side];
    if (!meta) return 0;
    const raw = getComputedStyle(root).getPropertyValue(meta.cssVar).trim();
    const m = raw.match(/^(\d+(?:\.\d+)?)px$/);
    if (m) return Math.round(parseFloat(m[1], 10));
    return meta.fallback;
  }
  function writeAsidePx(side, px) {
    const meta = config[side];
    if (!meta) return;
    root.style.setProperty(meta.cssVar, px + "px");
  }
  function maxAside(side) {
    const meta = config[side];
    const otherSide = side === "left" ? "right" : "left";
    const otherWidth = readAsidePx(otherSide);
    const rect = root.getBoundingClientRect();
    return Math.max(
      meta.min,
      rect.width - otherWidth - splittersTotalPx - minMain
    );
  }
  Object.keys(config).forEach((side) => {
    try {
      const meta = config[side];
      const saved = localStorage.getItem(meta.storageKey);
      if (saved) {
        const px = parseInt(saved, 10);
        if (!Number.isNaN(px) && px >= meta.min) {
          writeAsidePx(side, px);
        }
      }
    } catch (_) {}
  });
  let dragging = false;
  let draggingSide = "";
  let activeHandle = null;
  let startX = 0;
  let startW = 0;
  function applyWidth(clientX) {
    if (!draggingSide) return;
    const meta = config[draggingSide];
    const dx = clientX - startX;
    const rawNext = draggingSide === "left" ? startW + dx : startW - dx;
    const next = clamp(rawNext, meta.min, maxAside(draggingSide));
    writeAsidePx(draggingSide, next);
  }
  function onMove(ev) {
    if (!dragging) return;
    const x = ev.touches && ev.touches[0] ? ev.touches[0].clientX : ev.clientX;
    applyWidth(x);
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
        localStorage.setItem(meta.storageKey, String(readAsidePx(draggingSide)));
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
    dragging = true;
    draggingSide = side;
    activeHandle = handle;
    startX = ev.touches && ev.touches[0] ? ev.touches[0].clientX : ev.clientX;
    startW = readAsidePx(side);
    handle.classList.add("splitter-active");
    document.body.style.cursor = "col-resize";
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
})();
"#;
