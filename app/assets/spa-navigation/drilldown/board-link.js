  function normalizeSceneParams(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
    const normalized = {};
    Object.entries(raw).forEach(([key, value]) => {
      const id = String(key || "").trim();
      if (!id) return;
      normalized[id] = value;
    });
    return normalized;
  }

  function resolveBoardLinkFields(popup, runtimeSceneNavMap = null) {
    if (!popup || typeof popup !== "object") return null;
    const boardLink = isBoardLinkConfig(popup);
    const panelPopup = isPanelPopupConfig(popup);
    if (!boardLink && !panelPopup) return null;
    const legacyTemplate = panelPopup && !boardLink ? normalizePanelTemplateId(popup?.template) : "";
    const sceneRef =
      popup?.scene && typeof popup.scene === "object" && !Array.isArray(popup.scene) ? popup.scene : {};
    const params = normalizeSceneParams(popup?.params);
    const sceneFile = normalizeDrilldownScenePath(
      nonEmptyString(
        popup?.scene_file,
        popup?.sceneFile,
        sceneRef?.scene_file,
        sceneRef?.sceneFile,
        boardLink ? "" : BOARD_TEMPLATE_SCENE_FILES[legacyTemplate],
      ),
    );
    const localNav = normalizeSceneLocalNav(
      popup?.local_nav ||
        popup?.localNav ||
        sceneRef?.local_nav ||
        sceneRef?.localNav ||
        resolveSceneLocalNav(sceneFile, runtimeSceneNavMap),
    );
    const sceneId = nonEmptyString(
      popup?.scene_id,
      popup?.sceneId,
      sceneRef?.scene_id,
      sceneRef?.sceneId,
      localNav?.sceneId,
      sceneFile ? DRILLDOWN_SCENE_BY_FILE[sceneFile] : "",
    );
    const entry = normalizeTabId(
      nonEmptyString(
        popup?.entry,
        popup?.entry_tab,
        popup?.entryTab,
        sceneRef?.entry,
        sceneRef?.entry_tab,
        sceneRef?.entryTab,
        popup?.focus,
        params?.entry,
        params?.entry_tab,
        params?.entryTab,
        params?.focus,
        localNav?.defaultEntry,
      ),
    );
    return {
      boardLink: boardLink || Boolean(sceneFile),
      panelPopup,
      legacyTemplate,
      sceneRef,
      sceneFile,
      sceneId,
      projection: normalizeProjection(popup?.projection),
      entry,
      localNav,
      params,
    };
  }

  function normalizePanelTemplateId(template) {
    const raw = String(template || "").trim();
    if (!raw || raw === "metric_default") return "metric_board_default";
    return raw;
  }

  function normalizeDrilldownScenePath(raw) {
    let path = String(raw || "")
      .trim()
      .replace(/\\/g, "/");
    while (path.startsWith("../")) {
      path = path.slice(3);
    }
    return path.replace(/^\.?\/*/, "");
  }

