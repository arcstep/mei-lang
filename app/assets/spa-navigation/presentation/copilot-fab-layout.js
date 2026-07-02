(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const DOCK_CLASS = "copilot-fab-dock";

  function floatingRoot() {
    return document.getElementById("access-chat-floating-root");
  }

  function layoutHost(root) {
    if (!(root instanceof HTMLElement)) return null;
    if (typeof boot.copilotFloatingOffsetParent === "function") {
      return boot.copilotFloatingOffsetParent(root);
    }
    return root.parentElement;
  }

  /** dock 仅包裹 FAB；工具栏 absolute 挂在 dock 外侧，不参与 root 尺寸。 */
  function ensureFabDock() {
    const root = floatingRoot();
    const fab = document.getElementById("access-chat-fab");
    if (!(root instanceof HTMLElement) || !(fab instanceof HTMLElement)) return null;
    let dock = root.querySelector(`.${DOCK_CLASS}`);
    if (!(dock instanceof HTMLElement)) {
      dock = document.createElement("div");
      dock.className = DOCK_CLASS;
      root.insertBefore(dock, fab);
      dock.appendChild(fab);
    } else if (fab.parentElement !== dock) {
      dock.appendChild(fab);
    }
    return dock;
  }

  /** 助手靠右 → end（工具栏在左）；靠左 → start（工具栏在右）。 */
  function detectToolbarSide(fab, hostRect) {
    const fabRect = fab.getBoundingClientRect();
    const fabCenterX = fabRect.left + fabRect.width / 2;
    const hostCenterX = hostRect.left + hostRect.width / 2;
    return fabCenterX >= hostCenterX ? "end" : "start";
  }

  function syncCopilotFabToolbarLayout() {
    const root = floatingRoot();
    const fab = document.getElementById("access-chat-fab");
    if (!(root instanceof HTMLElement) || !(fab instanceof HTMLElement)) return false;
    if (root.dataset.open === "true") return false;

    ensureFabDock();

    const letterboxLayout =
      root.classList.contains("mei-copilot-letterbox-fixed") &&
      typeof boot.resolveAccessFabLetterboxLayout === "function"
        ? boot.resolveAccessFabLetterboxLayout(root)
        : null;
    const host = layoutHost(root);
    const bounds =
      typeof boot.resolveViewportOverlayBounds === "function"
        ? boot.resolveViewportOverlayBounds(root)
        : null;
    const hostRect = letterboxLayout
      ? letterboxLayout.shellRect
      : host
        ? host.getBoundingClientRect()
        : bounds?.shellRect || {
            left: 0,
            top: 0,
            width: Number(window.innerWidth || 0),
            height: Number(window.innerHeight || 0),
          };

    root.dataset.copilotToolbarSide = detectToolbarSide(fab, hostRect);

    const toolbar = root.querySelector(".copilot-toolbar");
    if (toolbar instanceof HTMLElement) {
      const hostWidth = letterboxLayout
        ? letterboxLayout.width
        : host
          ? host.clientWidth || host.offsetWidth || 0
          : 0;
      if (hostWidth > 0) {
        toolbar.style.maxWidth = `${Math.min(720, Math.max(200, hostWidth - 96))}px`;
      }
    }
    return true;
  }

  function scheduleCopilotFabToolbarLayout() {
    if (boot._copilotFabLayoutRaf) {
      cancelAnimationFrame(boot._copilotFabLayoutRaf);
    }
    boot._copilotFabLayoutRaf = requestAnimationFrame(() => {
      boot._copilotFabLayoutRaf = 0;
      syncCopilotFabToolbarLayout();
    });
  }

  boot.copilotFabLayout = {
    ensureFabDock,
    syncCopilotFabToolbarLayout,
    scheduleCopilotFabToolbarLayout,
  };
})();
