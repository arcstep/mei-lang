(function initSourceTreeControls() {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (typeof boot.disposeSourceTreeControls === "function") {
    try {
      boot.disposeSourceTreeControls();
    } catch (_) {}
    boot.disposeSourceTreeControls = null;
  }
  const sidebar = document.querySelector(".sidebar.left");
  if (!sidebar) return;
  let pendingTreeNavFrame = 0;
  let pendingTreeNavTimer = 0;

  const showTreeNavigationPending = (label) => {
    const main = document.querySelector("#workspace-root main.main");
    if (!main) return;
    main.setAttribute("aria-busy", "true");
    let overlay = main.querySelector('[data-mei-tree-nav-loading="true"]');
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.setAttribute("data-mei-tree-nav-loading", "true");
      overlay.style.cssText = [
        "position:absolute",
        "inset:0",
        "z-index:60",
        "display:grid",
        "place-items:center",
        "padding:24px",
        "background:linear-gradient(180deg, rgba(8,15,30,.40), rgba(8,15,30,.72))",
        "backdrop-filter:blur(2px)",
        "pointer-events:none",
      ].join(";");
      const card = document.createElement("div");
      card.style.cssText = [
        "display:grid",
        "gap:8px",
        "min-width:220px",
        "padding:16px 18px",
        "border-radius:14px",
        "border:1px solid rgba(96,165,250,.35)",
        "background:rgba(15,23,42,.90)",
        "box-shadow:0 12px 40px rgba(2,6,23,.28)",
        "color:#e2e8f0",
        "text-align:center",
      ].join(";");
      const title = document.createElement("strong");
      title.textContent = "正在打开资源";
      title.style.cssText = "font-size:14px;font-weight:700;color:#f8fafc;";
      const detail = document.createElement("span");
      detail.setAttribute("data-mei-tree-nav-target", "true");
      detail.style.cssText =
        "font-size:12px;line-height:1.5;color:#93c5fd;font-family:ui-monospace,SFMono-Regular,monospace;";
      const hint = document.createElement("span");
      hint.textContent = "左侧资源栏跳转中，请稍候...";
      hint.style.cssText = "font-size:11px;line-height:1.5;color:#94a3b8;";
      card.appendChild(title);
      card.appendChild(detail);
      card.appendChild(hint);
      overlay.appendChild(card);
      if (getComputedStyle(main).position === "static") {
        main.style.position = "relative";
      }
      main.appendChild(overlay);
    }
    const detail = overlay.querySelector('[data-mei-tree-nav-target="true"]');
    if (detail) {
      detail.textContent = label || "目标文件";
    }
  };

  const clearPendingTreeNavigation = () => {
    if (pendingTreeNavFrame) {
      cancelAnimationFrame(pendingTreeNavFrame);
      pendingTreeNavFrame = 0;
    }
    if (pendingTreeNavTimer) {
      clearTimeout(pendingTreeNavTimer);
      pendingTreeNavTimer = 0;
    }
  };

  const setBranchOpenRecursively = (rootDetails, nextOpen) => {
    rootDetails.open = nextOpen;
    const descendants = rootDetails.querySelectorAll(".tree-li-branch > details");
    descendants.forEach((child) => {
      child.open = nextOpen;
    });
  };

  const onDblClick = (event) => {
    if (!(event.target instanceof Element)) return;
    const summary = event.target.closest(".tree-folder-summary");
    if (!summary) return;
    const details = summary.closest("details");
    if (!details) return;
    const nextOpen = !details.open;
    setBranchOpenRecursively(details, nextOpen);
  };

  const onClick = (event) => {
    if (event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    if (!(event.target instanceof Element)) return;
    const link = event.target.closest("a.tree-link[href]");
    if (!link || !sidebar.contains(link)) return;
    const targetHref = link.href;
    if (!targetHref || targetHref === window.location.href) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    showTreeNavigationPending(link.textContent?.trim() || "");
    clearPendingTreeNavigation();
    pendingTreeNavFrame = requestAnimationFrame(() => {
      pendingTreeNavFrame = 0;
      pendingTreeNavTimer = window.setTimeout(() => {
        pendingTreeNavTimer = 0;
        window.location.assign(targetHref);
      }, 0);
    });
  };

  sidebar.addEventListener("dblclick", onDblClick);
  sidebar.addEventListener("click", onClick, true);

  boot.disposeSourceTreeControls = function () {
    clearPendingTreeNavigation();
    sidebar.removeEventListener("dblclick", onDblClick);
    sidebar.removeEventListener("click", onClick, true);
    if (boot.disposeSourceTreeControls) {
      boot.disposeSourceTreeControls = null;
    }
  };
})();
