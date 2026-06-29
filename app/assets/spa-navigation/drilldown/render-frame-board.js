  async function renderFrameBoardSceneContent(root, detail, config) {
    applyDrilldownOverlayMeta(root, config);
    setDrilldownOverlayStatus(root, "loading");
    const appId = resolvePreviewAppId();
    const sceneId = nonEmptyString(config?.boardSceneId, config?.sceneId);
    const host = root.querySelector('[data-drilldown-table-host="true"]');
    if (!appId || !sceneId || !(host instanceof HTMLElement)) {
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
    const url = `/apps/app/${appId}/scene/${encodeURIComponent(sceneId)}`;
    try {
      const response = await fetch(url, {
        credentials: "same-origin",
        headers: { "x-mei-spa-nav": "1" },
      });
      if (!response.ok) {
        throw new Error(`scene fetch failed: ${response.status}`);
      }
      const html = await response.text();
      const doc = new DOMParser().parseFromString(html, "text/html");
      const surface = doc.querySelector(
        "[data-mei-frame-viewport] .preview-surface, .preview-surface.preview-stage",
      );
      if (!(surface instanceof HTMLElement)) {
        throw new Error("board scene preview surface missing");
      }
      host.replaceChildren();
      const mount = document.createElement("div");
      mount.className = "access-drilldown-frame-board-mount";
      mount.style.cssText =
        "width:100%;height:100%;min-height:320px;overflow:auto;box-sizing:border-box;";
      mount.appendChild(surface.cloneNode(true));
      host.appendChild(mount);
      setDrilldownOverlayStatus(root, "ready");
      dispatchPreviewUpdated("drilldown", { resetRuntimeQueryCache: false });
      if (typeof boot.scheduleFrameViewportRelayout === "function") {
        boot.scheduleFrameViewportRelayout();
      }
      return true;
    } catch (error) {
      recordPopupDebugIssue({
        level: "error",
        message: String(error?.message || error || "frame board scene mount failed"),
        phase: "frame_board_scene_mount",
        detail,
        config,
      });
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
  }
