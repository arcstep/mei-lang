  function replaceShellFromDoc(doc, url, replaceHistory) {
    const currentShell = document.querySelector(".shell");
    const nextShell = doc.querySelector(".shell");
    if (!currentShell || !nextShell) return false;
    syncElementAttributes(currentShell, nextShell, { preserve: [] });
    currentShell.replaceChildren(
      ...Array.from(nextShell.childNodes).map((node) => node.cloneNode(true)),
    );
    if (replaceHistory) {
      window.history.replaceState({}, "", url);
    } else {
      window.history.pushState({}, "", url);
    }
    return true;
  }

  async function syncMissingWorkspaceModulesOnly(doc, navigationId) {
    const bundleReady = await syncSceneBundleFromDoc(doc, navigationId);
    if (!bundleReady || navigationId !== currentNavigationId) return false;
    if (findSceneBundleSrcInDoc(doc)) return true;
    const scripts = collectBodyScripts(doc).filter((src) => {
      const path = normalizePath(src);
      return isWorkspaceComponentModulePath(path);
    });
    return syncPreviewWorkspaceScripts(scripts, navigationId);
  }

  async function syncPreviewWorkspaceScripts(scriptUrls, navigationId) {
    if (!Array.isArray(scriptUrls) || scriptUrls.length === 0) return true;
    for (const rawSrc of scriptUrls) {
      if (navigationId != null && navigationId !== currentNavigationId) return false;
      const src = String(rawSrc || "").trim();
      if (!src) continue;
      const path = normalizePath(src);
      if (!path) continue;
      if (isSceneBundlePath(path)) {
        const alreadyLoaded = document.querySelector(
          'script[data-mei-scene-bundle="true"][data-mei-persistent-script="' + path + '"]',
        );
        if (alreadyLoaded) continue;
        removeExistingSceneBundleScripts();
        await loadScript(src, {
          module: true,
          persistentKey: path,
          sceneBundle: true,
          softFail: true,
        });
        wakeRuntimeAfterSceneBundleLoaded();
        continue;
      }
      if (!isWorkspaceComponentModulePath(path)) continue;
      if (
        document.querySelector('script[data-mei-persistent-script="' + path + '"]')
      ) {
        continue;
      }
      await loadScript(src, { module: true, persistentKey: path, softFail: true });
    }
    return true;
  }

  boot.syncPreviewWorkspaceScripts = syncPreviewWorkspaceScripts;

  async function ensureHostBundlesFromDoc(doc, navigationId, currentUrl, nextUrl) {
    for (const src of collectBodyScripts(doc)) {
      if (navigationId !== currentNavigationId) return false;
      const path = normalizePath(src);
      if (path !== "/app-bundles/manage.js" && path !== "/app-bundles/access.js") {
        continue;
      }
      const alreadyLoaded =
        document.querySelector('script[data-mei-persistent-script="' + path + '"]') ||
        document.querySelector('script[data-mei-reload-script="' + path + '"]');
      if (alreadyLoaded) {
        if (currentUrl && nextUrl && shouldReloadHostBundle(path, currentUrl, nextUrl)) {
          await loadScript(path + "?spa=" + Date.now(), {
            reloadKey: path,
            softFail: true,
          });
        }
        continue;
      }
      await loadScript(src, { persistentKey: path, softFail: true });
    }
    return true;
  }

  async function syncScriptsFromDocument(doc, navigationId, options) {
    const opts = options || {};
    const currentUrl = opts.currentUrl;
    const nextUrl = opts.nextUrl;
    const bundleReady = await syncSceneBundleFromDoc(doc, navigationId);
    if (!bundleReady || navigationId !== currentNavigationId) return false;
    const docUsesSceneBundle = Boolean(findSceneBundleSrcInDoc(doc));
    const scripts = collectBodyScripts(doc);
    for (const src of scripts) {
      if (navigationId !== currentNavigationId) return false;
      const path = normalizePath(src);
      if (!path) continue;
      if (path === SPA_NAV_SCRIPT) continue;
      if (
        path === "/app-bundles/manage.js" ||
        path === "/app-bundles/access.js"
      ) {
        if (
          currentUrl &&
          nextUrl &&
          !shouldReloadHostBundle(path, currentUrl, nextUrl)
        ) {
          await loadScript(src, { persistentKey: path });
          continue;
        }
      }
      if (
        opts.preserveManageWorkspace &&
        path === "/app-bundles/manage.js"
      ) {
        continue;
      }
      if (
        opts.preserveAgentPanel &&
        path.startsWith("/app-assets/") &&
        path.includes("agent-panel")
      ) {
        continue;
      }
      if (opts.preserveStatusBar && path === "/app-assets/statusbar.js") {
        continue;
      }
      if (
        opts.preserveWorkspaceSplitters &&
        path === "/app-assets/workspace-splitters.js"
      ) {
        continue;
      }
      if (
        opts.preserveSourceTreeControls &&
        path === "/app-assets/source-tree-controls.js"
      ) {
        continue;
      }
      if (isSceneBundlePath(path)) {
        continue;
      }
      if (docUsesSceneBundle && isWorkspaceComponentModulePath(path)) {
        continue;
      }
      if (isWorkspaceComponentModulePath(path)) {
        await loadScript(src, { module: true, persistentKey: path, softFail: true });
        continue;
      }
      if (path.startsWith("/app-assets/")) {
        if (RELOAD_APP_SCRIPTS.has(path)) {
          const withBuster = path + "?spa=" + Date.now();
          await loadScript(withBuster, { reloadKey: path, softFail: true });
          continue;
        }
        await loadScript(src, { persistentKey: path, softFail: true });
        continue;
      }
      if (path.startsWith("/app-bundles/")) {
        if (RELOAD_BUNDLE_SCRIPTS.has(path)) {
          const withBuster = path + "?spa=" + Date.now();
          await loadScript(withBuster, { reloadKey: path, softFail: true });
          continue;
        }
        await loadScript(src, { persistentKey: path, softFail: true });
        continue;
      }
    }
    return true;
  }

  function cloneNodeOrNull(node) {
    return node ? node.cloneNode(true) : null;
  }

  function extractManagePanelContext(root) {
    if (!root) return null;
    return {
      app: String(root.dataset.app || ""),
      scene: String(root.dataset.scene || ""),
      file: String(root.dataset.file || root.dataset.target || ""),
      sceneTarget: String(root.dataset.sceneTarget || ""),
      mode: String(root.dataset.mode || ""),
      sourceViews: String(root.dataset.sourceViews || ""),
      viewTab: String(root.dataset.viewTab || ""),
    };
  }

  function dispatchManageContextChange(detail) {
    if (!detail) return;
    document.dispatchEvent(
      new CustomEvent("mei:manage-context-change", {
        detail,
      }),
    );
  }

  function normalizeNavHref(rawHref) {
    try {
      const url = new URL(rawHref, window.location.href);
      url.searchParams.delete("tab");
      return url.pathname + "?" + url.searchParams.toString();
    } catch (_) {
      return String(rawHref || "");
    }
  }

  function syncBuildReachabilityTreeState(currentSidebar, nextSidebar) {
    const currentRoot = currentSidebar.querySelector(".build-reachability-tree");
    const nextRoot = nextSidebar.querySelector(".build-reachability-tree");
    if (!currentRoot || !nextRoot) return false;

    const nextByNode = new Map();
    nextRoot.querySelectorAll("a[data-build-node]").forEach((link) => {
      const key = String(link.getAttribute("data-build-node") || "").trim();
      if (key) nextByNode.set(key, link);
    });
    currentRoot.querySelectorAll("a[data-build-node]").forEach((link) => {
      const key = String(link.getAttribute("data-build-node") || "").trim();
      const next = nextByNode.get(key);
      if (!next) return;
      link.className = next.className;
      link.setAttribute("href", next.getAttribute("href") || "");
      if (next.hasAttribute("title")) {
        link.setAttribute("title", next.getAttribute("title") || "");
      } else {
        link.removeAttribute("title");
      }
      Array.from(link.attributes)
        .filter((attr) => attr.name.startsWith("data-"))
        .forEach((attr) => link.removeAttribute(attr.name));
      Array.from(next.attributes)
        .filter((attr) => attr.name.startsWith("data-"))
        .forEach((attr) => link.setAttribute(attr.name, attr.value));
      link.innerHTML = next.innerHTML;
    });

    const nextDetailsByBranch = new Map();
    nextRoot.querySelectorAll("details.build-tree-details[data-build-tree-branch]").forEach((details) => {
      const id = String(details.getAttribute("data-build-tree-branch") || "").trim();
      if (id) nextDetailsByBranch.set(id, details.open);
    });
    currentRoot
      .querySelectorAll("details.build-tree-details[data-build-tree-branch]")
      .forEach((details) => {
        const id = String(details.getAttribute("data-build-tree-branch") || "").trim();
        if (!id || !nextDetailsByBranch.has(id)) return;
        const wasOpen = details.open;
        const serverWantsOpen = nextDetailsByBranch.get(id);
        details.open = serverWantsOpen || wasOpen;
      });
    return true;
  }

  function syncSidebarLinkState(currentSidebar, nextSidebar) {
    if (!currentSidebar || !nextSidebar) return;
    if (syncBuildReachabilityTreeState(currentSidebar, nextSidebar)) return;
    currentSidebar.className = nextSidebar.className;
    const currentLinks = Array.from(currentSidebar.querySelectorAll("a.tree-link"));
    const nextLinks = Array.from(nextSidebar.querySelectorAll("a.tree-link"));
    const nextByKey = new Map();
    nextLinks.forEach((link) => {
      nextByKey.set(normalizeNavHref(link.getAttribute("href") || ""), link);
    });
    currentLinks.forEach((link) => {
      const key = normalizeNavHref(link.getAttribute("href") || "");
      const next = nextByKey.get(key);
      if (!next) return;
      link.className = next.className;
      link.setAttribute("href", next.getAttribute("href") || "");
      if (next.hasAttribute("title")) {
        link.setAttribute("title", next.getAttribute("title") || "");
      } else {
        link.removeAttribute("title");
      }
      Array.from(link.attributes)
        .filter((attr) => attr.name.startsWith("data-"))
        .forEach((attr) => link.removeAttribute(attr.name));
      Array.from(next.attributes)
        .filter((attr) => attr.name.startsWith("data-"))
        .forEach((attr) => link.setAttribute(attr.name, attr.value));
      link.innerHTML = next.innerHTML;
    });
    const currentDetails = Array.from(
      currentSidebar.querySelectorAll(".tree-li-branch > details"),
    );
    const nextDetails = Array.from(
      nextSidebar.querySelectorAll(".tree-li-branch > details"),
    );
    currentDetails.forEach((detail, index) => {
      if (index >= nextDetails.length) return;
      const wasOpen = detail.open;
      const serverWantsOpen = nextDetails[index].open;
      // 服务端仅按「选中路径」展开祖先；合并保留用户已展开的其它分支，避免换文件整树收起。
      detail.open = serverWantsOpen || wasOpen;
    });
  }

  function syncStatusbarContent(currentStatusbar, nextStatusbar) {
    if (!currentStatusbar || !nextStatusbar) return;
    currentStatusbar.className = nextStatusbar.className;
    const currentLayout = currentStatusbar.querySelector(".statusbar-layout");
    const nextLayout = nextStatusbar.querySelector(".statusbar-layout");
    if (!currentLayout || !nextLayout) return;
    currentLayout.className = nextLayout.className;
    const currentTracks = Array.from(currentLayout.children);
    const nextTracks = Array.from(nextLayout.children);
    currentTracks.forEach((track, index) => {
      if (index >= nextTracks.length) return;
      track.className = nextTracks[index].className;
      track.replaceChildren(
        ...Array.from(nextTracks[index].childNodes).map((node) => node.cloneNode(true)),
      );
    });
  }

  function syncElementAttributes(currentEl, nextEl, options) {
    if (!currentEl || !nextEl) return;
    const opts = options || {};
    const preserve = new Set(opts.preserve || []);
    Array.from(currentEl.attributes).forEach((attr) => {
      if (preserve.has(attr.name)) return;
      currentEl.removeAttribute(attr.name);
    });
    Array.from(nextEl.attributes).forEach((attr) => {
      if (preserve.has(attr.name) && currentEl.hasAttribute(attr.name)) return;
      currentEl.setAttribute(attr.name, attr.value);
    });
  }

  /** 同一 manage 路径下换 file/scene/tab 只换工作区，避免整页重载 manage bundle。 */
  function shouldPreserveManageWorkspace(currentUrl, nextUrl) {
    return (
      currentUrl.pathname === nextUrl.pathname &&
      (currentUrl.pathname.startsWith("/apps/manage/") ||
        currentUrl.pathname.startsWith("/apps/build/"))
    );
  }

  function syncSceneDrilldownContextFromDoc(doc) {
    const currentCtx = document.getElementById("mei-scene-drilldown-context");
    const nextCtx = doc.getElementById("mei-scene-drilldown-context");
    if (!currentCtx || !nextCtx) return;
    currentCtx.textContent = nextCtx.textContent || "";
    try {
      delete window.__meiSceneDrilldownContext;
    } catch (_) {}
  }

  function syncHostRuntimeCapabilitiesFromDoc(doc) {
    const currentNode = document.getElementById("mei-host-runtime-capabilities");
    const nextNode = doc.getElementById("mei-host-runtime-capabilities");
    if (!currentNode || !nextNode) return;
    currentNode.textContent = nextNode.textContent || "";
    try {
      delete window.__meiHostRuntimeCapabilities;
    } catch (_) {}
  }

  function syncBodyThemeFromDoc(doc) {
    const nextBody = doc.body;
    if (!nextBody) return;
    if (nextBody.className) {
      document.body.className = nextBody.className;
    }
    const nextStyle = nextBody.getAttribute("style");
    if (nextStyle != null) {
      document.body.setAttribute("style", nextStyle);
    }
    const view = nextBody.getAttribute("data-mei-view");
    if (view) {
      document.body.setAttribute("data-mei-view", view);
    }
  }

  /** 同路径 SPA 只替换 #workspace-root 时，顶栏仍在壳外，需从下一页文档同步 href（访问 / 演示 / 应用切换）。 */
  function syncManageTopbarFromDoc(doc) {
    try {
      const currentHeader = document.querySelector("header.topbar-shell");
      const nextHeader = doc.querySelector("header.topbar-shell");
      if (!currentHeader || !nextHeader) return;

      const currentGroup = currentHeader.querySelector("sl-button-group.mode-tab-group");
      const nextGroup = nextHeader.querySelector("sl-button-group.mode-tab-group");
      if (currentGroup && nextGroup) {
        const curBtns = currentGroup.querySelectorAll("sl-button[href]");
        const nextBtns = nextGroup.querySelectorAll("sl-button[href]");
        const n = Math.min(curBtns.length, nextBtns.length);
        for (let i = 0; i < n; i++) {
          const nh = nextBtns[i].getAttribute("href");
          if (nh) curBtns[i].setAttribute("href", nh);
        }
      }

      const curLaunch = currentHeader.querySelector("sl-button.topbar-launch-btn");
      const nextLaunch = nextHeader.querySelector("sl-button.topbar-launch-btn");
      if (curLaunch && nextLaunch) {
        const nh = nextLaunch.getAttribute("href");
        if (nh) curLaunch.setAttribute("href", nh);
      }

      const curTabs = currentHeader.querySelectorAll("nav.app-tabs a[href]");
      const nextTabs = nextHeader.querySelectorAll("nav.app-tabs a[href]");
      const m = Math.min(curTabs.length, nextTabs.length);
      for (let j = 0; j < m; j++) {
        const h = nextTabs[j].getAttribute("href");
        if (h) curTabs[j].setAttribute("href", h);
      }

      const curBread = currentHeader.querySelector(".app-current-path");
      const nextBread = nextHeader.querySelector(".app-current-path");
      if (curBread && nextBread) {
        curBread.replaceWith(nextBread.cloneNode(true));
      }
    } catch (err) {
      console.warn("[spa-navigation] sync topbar skipped", err);
    }
  }

  function swapManageWorkspace(doc, url, replaceHistory) {
    const currentShell = document.querySelector(".shell");
    const nextShell = doc.querySelector(".shell");
    const currentWorkspace = document.getElementById("workspace-root");
    const nextWorkspace = doc.getElementById("workspace-root");
    const currentLeftSidebar =
      currentWorkspace && currentWorkspace.querySelector("aside.sidebar.left");
    const nextLeftSidebar =
      nextWorkspace && nextWorkspace.querySelector("aside.sidebar.left");
    const currentMain = currentWorkspace && currentWorkspace.querySelector("main.main");
    const nextMain = nextWorkspace && nextWorkspace.querySelector("main.main");
    const currentRightSidebar =
      currentWorkspace && currentWorkspace.querySelector("aside.sidebar.right");
    const nextRightSidebar =
      nextWorkspace && nextWorkspace.querySelector("aside.sidebar.right");
    const currentStatusbar = document.querySelector(".statusbar");
    const nextStatusbar = doc.querySelector(".statusbar");
    const nextPanelRoot =
      nextRightSidebar && nextRightSidebar.querySelector("#meilang-author-panel");
    const nextPanelContext = extractManagePanelContext(nextPanelRoot);

    if (
      !currentShell ||
      !nextShell ||
      !currentWorkspace ||
      !currentLeftSidebar ||
      !nextLeftSidebar ||
      !currentMain ||
      !nextMain ||
      !currentRightSidebar ||
      !nextRightSidebar
    ) {
      return false;
    }

    currentShell.className = nextShell.className;
    syncElementAttributes(currentShell, nextShell, { preserve: [] });
    syncElementAttributes(currentWorkspace, nextWorkspace, { preserve: ["id"] });
    syncSidebarLinkState(currentLeftSidebar, nextLeftSidebar);
    currentRightSidebar.className = nextRightSidebar.className;
    const preparedMain = cloneNodeOrNull(nextMain);
    if (!preparedMain) return false;
    preparedMain.classList.add("spa-fragment-enter");
    currentMain.replaceWith(preparedMain);
    if (currentStatusbar && nextStatusbar) {
      syncStatusbarContent(currentStatusbar, nextStatusbar);
    }

    syncManageTopbarFromDoc(doc);
    syncBodyThemeFromDoc(doc);
    syncSceneDrilldownContextFromDoc(doc);
    syncHostRuntimeCapabilitiesFromDoc(doc);

    if (replaceHistory) {
      window.history.replaceState({}, "", url);
    } else {
      window.history.pushState({}, "", url);
    }
    return true;
  }

