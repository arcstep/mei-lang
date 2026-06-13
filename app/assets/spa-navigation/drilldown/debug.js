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
  } = {}) {
    const payload = {
      phase: String(phase || "").trim(),
      message: String(message || "").trim(),
      sceneId: nonEmptyString(config?.sceneId, detail?.scene_id),
      target: nonEmptyString(config?.runtimeRef?.scenePath, detail?.scene_path),
      datasetId: String(datasetId || "").trim(),
      metricId: String(metricId || "").trim(),
      template: nonEmptyString(config?.panelTemplate, config?.popup?.template),
    };
    const logger = level === "warn" ? console.warn : console.error;
    logger("[mei][popup-panel]", payload);
    const host = resolvePopupDebugHost();
    if (!(host instanceof HTMLElement)) return;
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
  }

