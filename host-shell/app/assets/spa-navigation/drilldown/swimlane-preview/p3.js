    appendSwimlaneSubtitle(panel, row, mapping);
    appendSwimlaneContext(panel, row, mapping);

    const lanesRoot = document.createElement("div");
    lanesRoot.className = "access-drilldown-swimlane-lanes";
    cloneArray(mapping.lanes).forEach((lane) => {
      const laneEl = document.createElement("div");
      laneEl.className = "access-drilldown-swimlane-lane";
      const laneLabel = document.createElement("div");
      laneLabel.className = "access-drilldown-swimlane-lane-label";
      laneLabel.textContent = String(lane?.label || lane?.id || "流程").trim();
      laneEl.appendChild(laneLabel);
      const track = document.createElement("div");
      track.className = "access-drilldown-swimlane-track";
      const steps = cloneArray(lane?.steps);
      const stepStates = resolveSequentialStepStates(steps, row);
      stepStates.forEach((stepState, index) => {
        if (index > 0) {
          const connector = document.createElement("span");
          connector.className = "access-drilldown-swimlane-connector";
          connector.setAttribute("aria-hidden", "true");
          track.appendChild(connector);
        }
        track.appendChild(renderSwimlaneNode(stepState));
      });
      laneEl.appendChild(track);
      lanesRoot.appendChild(laneEl);
    });
    panel.appendChild(lanesRoot);
    host.appendChild(panel);
  }

  function normalizeUploadRelPath(relPath) {
    let path = String(relPath || "").trim().replace(/\\/g, "/");
    if (!path) return "";
    path = path.replace(/^\/+/, "");
    if (path.startsWith("upload/")) {
      path = path.slice("upload/".length);
    }
    return path;
  }

  function resolveUploadDownloadUrl(appId, relPath, options = {}) {
    const path = normalizeUploadRelPath(relPath);
    if (!path) return "";
    const app = String(appId || resolvePreviewAppId() || "").trim();
    if (!app) return "";
    const params = new URLSearchParams();
    params.set("path", path);
    if (options.inline) {
      params.set("inline", "true");
    }
    if (options.matchBasename) {
      params.set("match_basename", "true");
      const basename = String(options.basename || "").trim();
      if (basename) {
        params.set("basename", basename);
      }
    }
    return `/api/upload/download/${encodeURIComponent(app)}?${params.toString()}`;
  }

  function isDocumentPreviewPending(row, mapping) {
    const statusField = String(mapping?.status_field || mapping?.statusField || "附件状态").trim();
    const status = resolveCaseDetailFieldValue(row, { field: statusField });
    const pendingValues = cloneArray(mapping?.pending_status_values || mapping?.pendingStatusValues)
      .map((entry) => String(entry || "").trim())
      .filter(Boolean);
    if (pendingValues.length && pendingValues.includes(status)) return true;
    const docField = String(
      mapping?.document_path_field || mapping?.documentPathField || "附件相对路径",
    ).trim();
    return !resolveCaseDetailFieldValue(row, { field: docField });
  }

  function appendDocumentPreviewPlaceholder(panel, text, { hint = false } = {}) {
    const placeholder = document.createElement("div");
    placeholder.className = "access-drilldown-document-preview-empty";
    if (hint) {
      placeholder.classList.add("access-drilldown-document-preview-empty--hint");
    }
    placeholder.textContent = text;
    panel.appendChild(placeholder);
    return placeholder;
  }

  function createDocumentPreviewPanelShell({ title = "制度文件预览", idle = false } = {}) {
    const panel = document.createElement("div");
    panel.className = "access-drilldown-document-preview-panel";
    if (idle) {
      panel.classList.add("access-drilldown-document-preview-panel--idle");
    }
    const titleEl = document.createElement("div");
    titleEl.className = "access-drilldown-document-preview-title";
    titleEl.textContent = title;
    panel.appendChild(titleEl);
    return panel;
  }

  function renderDocumentPreviewPanel(host, row, config) {
    if (!(host instanceof HTMLElement)) return;
    host.replaceChildren();
    const mapping = resolveListPreviewMapping(config);
    if (!row || typeof row !== "object") {
      if (!mapping) {
        const empty = document.createElement("div");
        empty.className = "access-drilldown-list-preview-empty";
        empty.textContent = "点击清单中的条目查看详情";
        host.appendChild(empty);
        return;
      }
      const previewOnly = Boolean(mapping?.preview_only || mapping?.previewOnly);
      const panel = createDocumentPreviewPanelShell({
        idle: true,
        title: previewOnly ? "健全机制文档" : "制度文件预览",
      });
      appendDocumentPreviewPlaceholder(
        panel,
        previewOnly
          ? "正在加载制度文件…"
          : "点击左侧清单中的机制名称，在此预览 PDF 制度文件",
        { hint: true },
      );
      host.appendChild(panel);
      return;
    }
    if (!mapping) {
      renderListPreviewItemPanel(host, row, config);
      return;
    }
    const titleText = resolveCaseDetailFieldValue(row, {
      field: mapping?.title_field || mapping?.titleField || "机制名称",
      fallback_fields: mapping?.title_fallback_fields || mapping?.titleFallbackFields,
    });
    const panel = createDocumentPreviewPanelShell({ title: titleText });

    if (isDocumentPreviewPending(row, mapping)) {
      appendDocumentPreviewPlaceholder(panel, "制度文件待上传");
      host.appendChild(panel);
      return;
    }

    const docPath = resolveCaseDetailFieldValue(row, {
      field: mapping?.document_path_field || mapping?.documentPathField || "附件相对路径",
    });
    const src = resolveUploadDownloadUrl(
      mapping?.upload_app_id || mapping?.uploadAppId,
      docPath,
      { inline: true },
    );
    if (!src) {
      appendDocumentPreviewPlaceholder(panel, "暂无可预览的 PDF");
      host.appendChild(panel);
      return;
    }

    const frame = document.createElement("div");
    frame.className = "access-drilldown-document-preview-frame";
    frame.style.flex = "1 1 auto";
    frame.style.minHeight = "0";
    frame.style.height = "100%";
    const iframe = document.createElement("iframe");
    iframe.className = "access-drilldown-document-preview-iframe";
    iframe.style.width = "100%";
    iframe.style.height = "100%";
    iframe.style.border = "0";
    iframe.src = src;
    iframe.title = titleText || "PDF 预览";
    frame.appendChild(iframe);
    panel.appendChild(frame);
    host.appendChild(panel);
  }

  function resolveVideoPreviewSectionTitle(mapping, fallback = "视频预览") {
    const text = String(
      mapping?.video_section_title || mapping?.videoSectionTitle || fallback,
    ).trim();
    return text || fallback;
  }

  function resolveSummaryImageSectionTitle(mapping, fallback = "预警列表") {
    const text = String(
      mapping?.summary_image_section_title || mapping?.summaryImageSectionTitle || fallback,
    ).trim();
    return text || fallback;
  }

  function resolveSummaryImagePreviewUrl(row, mapping) {
    if (!row || typeof row !== "object" || !mapping || typeof mapping !== "object") return "";
    const dir = String(
      mapping?.summary_image_dir || mapping?.summaryImageDir || "预警摘要图片",
    ).trim();
    const idField = String(
      mapping?.summary_image_id_field || mapping?.summaryImageIdField || "视频编号",
    ).trim();
    const videoId = resolveCaseDetailFieldValue(row, { field: idField });
    if (!dir || !videoId) return "";
    return resolveUploadDownloadUrl(
      mapping?.upload_app_id || mapping?.uploadAppId,
      dir,
      { inline: true, matchBasename: true, basename: videoId },
    );
  }

  function resolveVideoPreviewPath(row, mapping) {
    if (!row || typeof row !== "object" || !mapping || typeof mapping !== "object") return "";
    const pathField = String(mapping?.video_path_field || mapping?.videoPathField || "视频路径").trim();
    let path = resolveCaseDetailFieldValue(row, { field: pathField });
    if (path) return path;
    const idField = String(mapping?.video_id_field || mapping?.videoIdField || "视频编号").trim();
    const videoId = resolveCaseDetailFieldValue(row, { field: idField });
    if (!videoId) return "";
    const prefix = String(mapping?.video_path_prefix || mapping?.videoPathPrefix || "videos/").trim();
    const suffix = String(mapping?.video_path_suffix || mapping?.videoPathSuffix || ".mp4").trim();
    return `${prefix}${videoId}${suffix}`;
  }


  function createVideoSubtitleCockpitShell({
    title = "视频预览",
    summaryTitle = "预警列表",
    idle = false,
  } = {}) {
    const panel = document.createElement("div");
    panel.className = "access-drilldown-video-cockpit-panel";
    if (idle) {
      panel.classList.add("access-drilldown-video-cockpit-panel--idle");
    }
    const videoSection = document.createElement("section");
    videoSection.className = "access-drilldown-video-cockpit-video";
    const videoTitle = document.createElement("div");
    videoTitle.className = "access-drilldown-video-cockpit-section-title";
    videoTitle.textContent = title;
    videoSection.appendChild(videoTitle);
    const videoFrame = document.createElement("div");
    videoFrame.className = "access-drilldown-video-cockpit-video-frame";
    videoSection.appendChild(videoFrame);
    const subtitleSection = document.createElement("section");
    subtitleSection.className = "access-drilldown-video-cockpit-subtitle access-drilldown-video-cockpit-summary-image";
    const subtitleTitle = document.createElement("div");
    subtitleTitle.className = "access-drilldown-video-cockpit-section-title";
    subtitleTitle.textContent = summaryTitle;
    subtitleSection.appendChild(subtitleTitle);
    const subtitleBody = document.createElement("div");
    subtitleBody.className = "access-drilldown-video-cockpit-subtitle-body access-drilldown-video-cockpit-summary-image-body";
    subtitleSection.appendChild(subtitleBody);
    panel.appendChild(videoSection);
    panel.appendChild(subtitleSection);
    return { panel, videoFrame, subtitleBody, videoTitle, subtitleTitle };
  }

  function appendVideoCockpitPlaceholder(frame, text, { hint = false } = {}) {
    const placeholder = document.createElement("div");
    placeholder.className = "access-drilldown-video-cockpit-empty";
    if (hint) {
      placeholder.classList.add("access-drilldown-video-cockpit-empty--hint");
    }
    placeholder.textContent = text;
    frame.appendChild(placeholder);
    return placeholder;
  }

  const SUMMARY_IMAGE_ZOOM_MIN = 1;
  const SUMMARY_IMAGE_ZOOM_MAX = 4;
  const SUMMARY_IMAGE_ZOOM_STEP = 1.25;

  function clampSummaryImageZoom(value) {
    const n = Number(value);
    if (!Number.isFinite(n)) return SUMMARY_IMAGE_ZOOM_MIN;
    return Math.min(SUMMARY_IMAGE_ZOOM_MAX, Math.max(SUMMARY_IMAGE_ZOOM_MIN, n));
  }

  function mountSummaryImagePanZoomControls(viewport, stage, tools) {
    let zoom = SUMMARY_IMAGE_ZOOM_MIN;
    let panX = 0;
    let panY = 0;
    let panMode = false;
    let dragState = null;
    const panButton = tools.querySelector('[data-summary-image-action="pan"]');

    const applyTransform = () => {
      stage.style.transform = `translate(${panX}px, ${panY}px) scale(${zoom})`;
    };

    const syncPanButton = () => {
      if (!(panButton instanceof HTMLButtonElement)) return;
      panButton.classList.toggle("is-active", panMode);
      panButton.setAttribute("aria-pressed", panMode ? "true" : "false");
    };

    const resetView = () => {
      zoom = SUMMARY_IMAGE_ZOOM_MIN;
      panX = 0;
      panY = 0;
      panMode = false;
      viewport.classList.remove("is-pan-active", "is-dragging");
      syncPanButton();
      applyTransform();
    };

    const setZoom = (nextZoom) => {
      zoom = clampSummaryImageZoom(nextZoom);
      applyTransform();
    };

    const canDrag = () => panMode || zoom > SUMMARY_IMAGE_ZOOM_MIN + 0.001;

    tools.addEventListener("click", (event) => {
      const button = event.target?.closest?.("[data-summary-image-action]");
      if (!(button instanceof HTMLButtonElement)) return;
      event.preventDefault();
      event.stopPropagation();
      const action = button.getAttribute("data-summary-image-action");
      if (action === "zoom-in") {
        setZoom(zoom * SUMMARY_IMAGE_ZOOM_STEP);
        return;
      }
      if (action === "zoom-out") {
        setZoom(zoom / SUMMARY_IMAGE_ZOOM_STEP);
        if (zoom <= SUMMARY_IMAGE_ZOOM_MIN + 0.001) {
          panX = 0;
          panY = 0;
          applyTransform();
        }
        return;
      }
      if (action === "pan") {
        panMode = !panMode;
        viewport.classList.toggle("is-pan-active", panMode);
        syncPanButton();
        return;
      }
      if (action === "reset") {
        resetView();
      }
    });

    viewport.addEventListener("pointerdown", (event) => {
      if (!canDrag()) return;
      if (event.button !== 0) return;
      if (event.target?.closest?.(".access-drilldown-video-cockpit-summary-image-tools")) return;
      dragState = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        originPanX: panX,
        originPanY: panY,
      };
      viewport.classList.add("is-dragging");
      if (typeof viewport.setPointerCapture === "function") {
        viewport.setPointerCapture(event.pointerId);
      }
      event.preventDefault();
    });

    const finishDrag = (event) => {
      if (!dragState) return;
      if (event.pointerId !== dragState.pointerId) return;
      dragState = null;
      viewport.classList.remove("is-dragging");
      if (typeof viewport.releasePointerCapture === "function") {
        try {
          viewport.releasePointerCapture(event.pointerId);
        } catch {
          // ignore stale capture
        }
      }
    };

    viewport.addEventListener("pointermove", (event) => {
      if (!dragState || event.pointerId !== dragState.pointerId) return;
      panX = dragState.originPanX + (event.clientX - dragState.startX);
      panY = dragState.originPanY + (event.clientY - dragState.startY);
      applyTransform();
      event.preventDefault();
    });
    viewport.addEventListener("pointerup", finishDrag);
    viewport.addEventListener("pointercancel", finishDrag);
    viewport.addEventListener("lostpointercapture", () => {
      dragState = null;
      viewport.classList.remove("is-dragging");
    });

    applyTransform();
    return { resetView };
  }

  function createSummaryImageViewport(image) {
    const viewport = document.createElement("div");
    viewport.className = "access-drilldown-video-cockpit-summary-image-viewport";

    const stage = document.createElement("div");
    stage.className = "access-drilldown-video-cockpit-summary-image-stage";
    stage.appendChild(image);

    const tools = document.createElement("div");
    tools.className = "access-drilldown-video-cockpit-summary-image-tools";
    tools.setAttribute("role", "group");
    tools.setAttribute("aria-label", "摘要图片缩放");
    tools.innerHTML = `
      <button type="button" data-summary-image-action="zoom-in" title="放大" aria-label="放大">+</button>
      <button type="button" data-summary-image-action="zoom-out" title="缩小" aria-label="缩小">−</button>
      <button type="button" data-summary-image-action="pan" title="拖拽平移" aria-label="拖拽平移" aria-pressed="false">✋</button>
      <button type="button" data-summary-image-action="reset" title="复原视图" aria-label="复原视图">◎</button>
    `;

    viewport.appendChild(stage);
    viewport.appendChild(tools);
    mountSummaryImagePanZoomControls(viewport, stage, tools);
    return viewport;
  }

  function renderSummaryImagePreview(subtitleBody, row, mapping) {
    if (!(subtitleBody instanceof HTMLElement)) return;
    subtitleBody.replaceChildren();
    const src = resolveSummaryImagePreviewUrl(row, mapping);
    if (!src) {
      subtitleBody.textContent = "暂无预警摘要图片";
      return;
    }
    const image = document.createElement("img");
    image.className = "access-drilldown-video-cockpit-summary-image-img";
    image.alt = "预警摘要图片";
    image.loading = "lazy";
    image.decoding = "async";
    image.draggable = false;
    image.src = src;
    image.addEventListener("error", () => {
      subtitleBody.replaceChildren();
      appendVideoCockpitPlaceholder(subtitleBody, "未找到匹配的预警摘要图片，请确认已上传");
    });
    subtitleBody.appendChild(createSummaryImageViewport(image));
  }

  function renderVideoSubtitleCockpitPanel(host, row, config) {
    if (!(host instanceof HTMLElement)) return;
    host.replaceChildren();
    const mapping = resolveListPreviewMapping(config);
    if (!mapping) {
      const empty = document.createElement("div");
      empty.className = "access-drilldown-list-preview-empty";
      empty.textContent = "点击清单中的条目查看详情";
      host.appendChild(empty);
      return;
    }
    const videoSectionTitle = resolveVideoPreviewSectionTitle(mapping);
    const summarySectionTitle = resolveSummaryImageSectionTitle(mapping);
    if (!row || typeof row !== "object") {
      const { panel, videoFrame, subtitleBody } = createVideoSubtitleCockpitShell({
        title: videoSectionTitle,
        summaryTitle: summarySectionTitle,
        idle: true,
      });
      appendVideoCockpitPlaceholder(
        videoFrame,
        "请选择预警记录或上传视频",
        { hint: true },
      );
      subtitleBody.textContent = "请选择预警记录查看摘要图片";
      host.appendChild(panel);
      return;
    }
    const titleText = resolveCaseDetailFieldValue(row, {
      field: mapping?.title_field || mapping?.titleField || "视频编号",
      fallback_fields: mapping?.title_fallback_fields || mapping?.titleFallbackFields,
    });
    const { panel, videoFrame, subtitleBody, videoTitle, subtitleTitle } = createVideoSubtitleCockpitShell({
      title: videoSectionTitle,
      summaryTitle: summarySectionTitle,
    });
    if (videoTitle instanceof HTMLElement) {
      videoTitle.textContent = titleText
        ? `${videoSectionTitle} · ${titleText}`
        : videoSectionTitle;
    }
    if (subtitleTitle instanceof HTMLElement) {
      subtitleTitle.textContent = summarySectionTitle;
    }
    const relPath = resolveVideoPreviewPath(row, mapping);
    const src = resolveUploadDownloadUrl(
      mapping?.upload_app_id || mapping?.uploadAppId,
      relPath,
      { inline: true },
    );
    if (!src) {
      appendVideoCockpitPlaceholder(videoFrame, "暂无可预览的视频");
    } else {
      const video = document.createElement("video");
      video.className = "access-drilldown-video-cockpit-player";
      video.controls = true;
      video.preload = "metadata";
      video.playsInline = true;
      video.src = src;
      video.title = titleText || "执法视频预览";
      video.addEventListener("error", () => {
        videoFrame.replaceChildren();
        appendVideoCockpitPlaceholder(videoFrame, "未找到可播放的视频文件，请确认视频路径已上传");
      });
      videoFrame.appendChild(video);
    }
    renderSummaryImagePreview(subtitleBody, row, mapping);
    host.appendChild(panel);
  }
