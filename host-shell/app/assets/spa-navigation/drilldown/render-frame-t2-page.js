  async function composeBoardSceneSurface(appId, sceneId) {
    const ctx = {
      app_id: appId,
      appId,
      scene_id: sceneId,
      sceneId,
      surface: "app",
      mode: "app",
    };
    const result = await boot.viewRevisionClient?.negotiateWithLocalMiss?.(ctx, { silent: true });
    const layers = result?.assemble?.layers;
    if (!layers) {
      throw new Error("board scene layer assembly failed");
    }
    const temp = document.createElement("div");
    temp.setAttribute("data-mei-compose-placeholder", "1");
    const composeAxes =
      typeof boot.viewRevisionClient?.buildComposeRequest === "function"
        ? boot.viewRevisionClient.buildComposeRequest(ctx)
        : { review_projection: "live_full", data_mode: "eval", route_mode: "app" };
    const composed = boot.viewCompositor?.composeFromLayers?.(temp, layers, composeAxes);
    if (!composed) {
      throw new Error("board scene compose failed");
    }
    return (
      temp.querySelector("[data-mei-frame-viewport]") ||
      temp.querySelector(".preview-surface.preview-stage") ||
      temp.querySelector(".preview-surface")
    );
  }

  async function renderFrameBoardSceneContent(root, detail, config) {
    applyDrilldownOverlayMeta(root, config);
    setDrilldownOverlayStatus(root, "loading");
    const appId = resolvePreviewAppId();
    const sceneId = nonEmptyString(config?.boardSceneId, config?.sceneId);
    const host = root.querySelector('[data-drilldown-table-host="true"]');
    if (!appId || !sceneId || !(host instanceof HTMLElement)) {
      setDrilldownOverlayStatus(root, "error", {
        message: "frame 看板缺少 app、scene 或挂载节点",
        phase: "frame_board_setup",
        detail,
        config,
      });
      return false;
    }
    const url = `/apps/${encodeURIComponent(appId)}/${encodeURIComponent(sceneId)}`;
    try {
      if (!boot.viewRevisionClient?.negotiateWithLocalMiss || !boot.viewCompositor?.composeFromLayers) {
        throw new Error("board scene compose unavailable");
      }
      const surface = await composeBoardSceneSurface(appId, sceneId);
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
        root,
        stack: error?.stack || "",
      });
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
  }
