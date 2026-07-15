  function resolvePopupDebugHost() {
    try {
      if (window.parent && window.parent !== window) {
        const host = window.parent.document.getElementById("mei-runtime-query-errors");
        if (host) return host;
      }
    } catch (_) {
      /* ignore */
    }
    return document.getElementById("mei-runtime-query-errors");
  }

  function recordPopupDebugIssue({
    level = "error",
    message = "",
    phase = "",
    detail = {},
    config = {},
    datasetId = "",
    metricId = "",
    root = null,
    stack = "",
  } = {}) {
    const payload = {
      phase: String(phase || "").trim(),
      message: String(message || "").trim(),
      sceneId: nonEmptyString(
        config?.runtimeSceneId,
        config?.boardSceneId,
        config?.hostSceneId,
        config?.sceneId,
        detail?.board_scene_id,
        detail?.scene_id,
      ),
      target: nonEmptyString(
        config?.runtimeSceneFile,
        config?.boardSceneFile,
        config?.hostSceneFile,
        config?.runtimeRef?.scenePath,
        detail?.board_scene_file,
        detail?.scene_path,
      ),
      panelId: nonEmptyString(
        config?.panelId,
        detail?.page_panel_id,
        detail?.panel_id,
        detail?.popup?.panel_id,
      ),
      datasetId: String(datasetId || "").trim(),
      metricId: String(metricId || "").trim(),
      template: nonEmptyString(config?.panelTemplate, config?.popup?.template),
    };
    const traceId =
      level !== "warn" && typeof boot.reportClientError === "function"
        ? boot.reportClientError({
            kind: "drilldown_error",
            message: payload.message || "二级看板运行失败",
            sceneId: payload.sceneId,
            component: "mei-drilldown",
            panelId: payload.panelId,
            phase: payload.phase,
            target: payload.target,
            stack,
          })
        : "";
    payload.traceId = String(traceId || "").trim();
    if (root instanceof HTMLElement && payload.traceId) {
      root.dataset.meiClientErrorTraceId = payload.traceId;
    }
    const logger = level === "warn" ? console.warn : console.error;
    logger("[mei][popup-panel]", payload);
    const host = resolvePopupDebugHost();
    if (!(host instanceof HTMLElement)) return payload.traceId;
    const tone =
      level === "warn"
        ? "rgba(250, 204, 21, .24);border:1px solid rgba(250, 204, 21, .45);color:#fde68a;"
        : "rgba(127, 29, 29, .18);border:1px solid rgba(248, 113, 113, .4);color:#fecaca;";
    const context = [
      payload.phase ? `phase=${payload.phase}` : "",
      payload.sceneId ? `scene=${payload.sceneId}` : "",
      payload.target ? `file=${payload.target}` : "",
      payload.datasetId ? `dataset=${payload.datasetId}` : "",
      payload.metricId ? `metric=${payload.metricId}` : "",
      payload.template ? `template=${payload.template}` : "",
      payload.traceId ? `trace=${payload.traceId}` : "",
    ]
      .filter(Boolean)
      .join(" · ");
    host.insertAdjacentHTML(
      "afterbegin",
      `<div style="display:block;margin:6px 0;padding:8px;border-radius:8px;${tone}font-size:11px;line-height:1.45;">` +
        `<strong>scene_projection</strong>${context ? ` · ${context}` : ""}<br/>` +
        `<code style="display:block;margin-top:4px;white-space:pre-wrap;word-break:break-word;color:inherit;">${String(
          payload.message || "unknown popup error"
        )
          .replaceAll("&", "&amp;")
          .replaceAll("<", "&lt;")
          .replaceAll(">", "&gt;")}</code></div>`
    );
    return payload.traceId;
  }

