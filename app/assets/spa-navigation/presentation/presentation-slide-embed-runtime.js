(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  let mountGeneration = 0;
  const activeCleanups = new Set();

  function parseAppIdFromPath() {
    const match = String(window.location.pathname || "").match(
      /^\/apps\/(?:app|access|access-only|access_only|copilot|speaker|run|presentation)\/([^/]+)/,
    );
    return match ? String(match[1] || "").trim() : "";
  }

  function parseSceneIdFromPath() {
    const match = String(window.location.pathname || "").match(/\/scene\/([^/?#]+)/);
    if (match) return String(match[1] || "").trim();
    const mei = window.__mei;
    return String(mei?.active_scene_id || mei?.activeSceneId || "home").trim() || "home";
  }

  function sceneTargetFile(sceneId) {
    const scene = String(sceneId || "home").trim() || "home";
    return `scene/${scene}.mei`;
  }

  function registerCleanup(cleanup) {
    if (typeof cleanup === "function") {
      activeCleanups.add(cleanup);
    }
  }

  function unmountAll() {
    mountGeneration += 1;
    activeCleanups.forEach((cleanup) => {
      try {
        cleanup();
      } catch (_) {
        /* ignore */
      }
    });
    activeCleanups.clear();
  }

  function resolveImageCandidates(appId, ref) {
    const encodedApp = encodeURIComponent(appId);
    const encodedRef = encodeURIComponent(ref);
    const base = `/workspace-app-assets/${encodedApp}/assets/`;
    return [
      `${base}${encodedRef}.svg`,
      `${base}${encodedRef}.png`,
      `${base}${encodedRef}.jpg`,
      `${base}${encodedRef}.webp`,
      `${base}presentation/${encodedRef}.svg`,
      `${base}presentation/${encodedRef}.png`,
    ];
  }

  function formatMetricValue(metric) {
    if (!metric || typeof metric !== "object") return "";
    const value = metric.value ?? metric.display_value ?? metric.displayValue ?? metric.result;
    if (value === null || value === undefined) return "";
    if (typeof value === "object") {
      if (value.value !== undefined) return String(value.value);
      return JSON.stringify(value);
    }
    return String(value);
  }

  function metricProps(appId, sceneId, metricId) {
    return {
      _mei: {
        app_id: appId,
        active_scene_id: sceneId,
        active_target_file: sceneTargetFile(sceneId),
        runtime_capabilities: {
          metric_query: {
            enabled: true,
            api: `/api/datasets/metrics/${encodeURIComponent(appId)}`,
            scene_qualified: true,
          },
        },
      },
      dataset: {
        __mei_runtime_ref: {
          metric_id: metricId,
        },
      },
    };
  }

  async function fetchMetricDisplay(appId, sceneId, metricId) {
    const runtime = window.__meiDatasetRuntime;
    if (runtime && typeof runtime.fetchPanelRuntimeMetrics === "function") {
      const data = await runtime.fetchPanelRuntimeMetrics(metricProps(appId, sceneId, metricId), {
        metricIds: [metricId],
      });
      const metrics = Array.isArray(data?.metrics)
        ? data.metrics
        : Array.isArray(data?.results)
          ? data.results
          : [];
      const found =
        typeof runtime.findRuntimeMetricInResults === "function"
          ? runtime.findRuntimeMetricInResults(metrics, { metric_id: metricId })
          : metrics.find((item) => String(item?.id || "").trim() === metricId);
      if (found) {
        return {
          label: String(found.label || found.title || metricId).trim() || metricId,
          value: formatMetricValue(found),
        };
      }
    }
    const response = await fetch(`/api/datasets/metrics/${encodeURIComponent(appId)}`, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        scene_id: sceneId,
        target: sceneTargetFile(sceneId),
        metric_ids: [metricId],
      }),
    });
    const data = await response.json().catch(() => ({}));
    if (!response.ok) {
      throw new Error(
        data?.error ||
          data?.diagnostics?.[0]?.message ||
          `metric query failed: ${response.status}`,
      );
    }
    const metrics = Array.isArray(data?.metrics)
      ? data.metrics
      : Array.isArray(data?.results)
        ? data.results
        : [];
    const found = metrics.find((item) => {
      const id = String(item?.id || "").trim();
      return id === metricId || id.endsWith(`::${metricId}`);
    });
    if (!found) {
      throw new Error(`metric \`${metricId}\` not found in response`);
    }
    return {
      label: String(found.label || found.title || metricId).trim() || metricId,
      value: formatMetricValue(found),
    };
  }

  function renderMetricNode(node, payload, kind) {
    const label = document.createElement("span");
    label.className = "mei-presentation-embed-metric-label";
    label.textContent = payload.label;
    const value = document.createElement("strong");
    value.className = "mei-presentation-embed-metric-value";
    value.textContent = payload.value || "—";
    node.innerHTML = "";
    node.classList.add("mei-presentation-embed--mounted");
    node.classList.toggle("mei-presentation-embed--chart", kind === "chart");
    node.append(label, value);
  }

  async function mountImage(node, ref, appId, generation) {
    if (!appId) {
      node.dataset.embedStatus = "missing-app-id";
      return;
    }
    const candidates = resolveImageCandidates(appId, ref);
    for (const src of candidates) {
      if (generation !== mountGeneration) return;
      const loaded = await new Promise((resolve) => {
        const img = new Image();
        img.onload = () => resolve(img);
        img.onerror = () => resolve(null);
        img.src = src;
      });
      if (generation !== mountGeneration) return;
      if (loaded) {
        loaded.className = "mei-presentation-embed-media";
        loaded.alt = ref;
        node.innerHTML = "";
        node.classList.add("mei-presentation-embed--mounted");
        node.appendChild(loaded);
        return;
      }
    }
    node.dataset.embedStatus = "image-not-found";
  }

  async function mountMetric(node, ref, appId, sceneId, kind, generation) {
    if (!appId) {
      node.dataset.embedStatus = "missing-app-id";
      return;
    }
    try {
      const payload = await fetchMetricDisplay(appId, sceneId, ref);
      if (generation !== mountGeneration) return;
      renderMetricNode(node, payload, kind);
    } catch (error) {
      if (generation !== mountGeneration) return;
      node.dataset.embedStatus = "metric-error";
      node.dataset.embedError = String(error?.message || error || "metric fetch failed");
    }
  }

  function mountViewpointEmbed(node, ref) {
    const selector = `[data-mei-viewpoint="${CSS.escape(ref)}"]`;
    const target = document.querySelector(selector);
    const panel =
      target?.closest?.("[data-mei-panel-id]") ||
      target?.closest?.(".mei-panel-root") ||
      target?.parentElement;
    if (panel instanceof HTMLElement) {
      const clone = panel.cloneNode(true);
      clone.classList.add("mei-presentation-embed-clone");
      clone.querySelectorAll?.("[id]").forEach((element) => {
        if (element instanceof HTMLElement && element.id) {
          element.id = `${element.id}__presentation_clone`;
        }
      });
      node.innerHTML = "";
      node.classList.add("mei-presentation-embed--mounted");
      node.appendChild(clone);
      return;
    }
    node.dataset.embedStatus = "viewpoint-not-mounted";
  }

  async function mountNode(node, context, generation) {
    if (!(node instanceof HTMLElement)) return;
    const kind = String(node.dataset.embedKind || "").trim();
    const ref = String(node.dataset.embedRef || "").trim();
    if (!kind || !ref) return;
    node.dataset.meiPresentationEmbedMount = "pending";
    delete node.dataset.embedStatus;
    delete node.dataset.embedError;
    if (kind === "image") {
      await mountImage(node, ref, context.appId, generation);
    } else if (kind === "metric" || kind === "chart") {
      await mountMetric(node, ref, context.appId, context.sceneId, kind, generation);
    } else if (kind === "embed") {
      mountViewpointEmbed(node, ref);
    }
    if (generation !== mountGeneration) return;
    node.dataset.meiPresentationEmbedMount = node.dataset.embedStatus ? "error" : "mounted";
  }

  async function mountSlideEmbeds(layer, step) {
    unmountAll();
    const generation = mountGeneration;
    const root =
      layer instanceof HTMLElement
        ? layer.querySelector(".mei-copilot-slide-inner") || layer
        : null;
    if (!root) return;
    const nodes = root.querySelectorAll("[data-embed-kind][data-embed-ref]");
    if (!nodes.length) return;
    const context = {
      appId: parseAppIdFromPath(),
      sceneId: parseSceneIdFromPath(),
      step,
    };
    registerCleanup(() => {
      nodes.forEach((node) => {
        if (!(node instanceof HTMLElement)) return;
        node.dataset.meiPresentationEmbedMount = "unmounted";
      });
    });
    for (const node of nodes) {
      await mountNode(node, context, generation);
    }
  }

  boot.presentationSlideEmbedRuntime = {
    mountSlideEmbeds,
    unmountAll,
  };
})();
