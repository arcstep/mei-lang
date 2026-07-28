/**
 * Preview materializer: structure tree DOM + eval mounts + runtime.plans injection.
 */
(function initPreviewMaterializer(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function extractLayerDocument(layerValue) {
    if (!layerValue) return null;
    if (typeof layerValue === "string") {
      try {
        return JSON.parse(layerValue);
      } catch (_) {
        return null;
      }
    }
    if (Array.isArray(layerValue.nodes) || layerValue.schema_version) {
      return layerValue;
    }
    if (layerValue.document) return layerValue.document;
    return layerValue;
  }

  function escapeHtmlAttr(value) {
    return String(value || "")
      .replace(/&/g, "&amp;")
      .replace(/"/g, "&quot;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function isViewportMetaContentNode(node) {
    const role = String(node?.ui_role || "").trim().toLowerCase();
    if (role !== "content") return false;
    const scope = String(node?.preview_scope || "").trim().toLowerCase();
    // T1 地图操作 chrome（center-rail / map-stage overlay）须保持可见。
    if (scope.includes("/map_viewport/") || scope.includes("/map_stage_overlay/")) return false;
    const label = String(node?.label || "").trim().toLowerCase();
    if (label.startsWith("viewport:")) return true;
    return scope.endsWith("/map-viewport") || scope.endsWith("/world_viewport");
  }

  function isViewportNode(_node) {
    // Compose 只保留 scene 级 wrapStructureTreeInSceneViewport；不为 map-viewport 等面板再套 preview-stage。
    return false;
  }

  const DEFAULT_SCENE_VIEWPORT = Object.freeze({
    design_width: 1920,
    design_height: 1080,
    scale_mode: "contain",
    overflow_mode: "clip",
    aspect_ratio: "16:9",
    route_mode: "app",
  });

  function resolveSceneViewportMeta(structureDoc) {
    const doc = extractLayerDocument(structureDoc);
    const meta = doc?.frame_viewport || {};
    const designWidth = Number(meta.design_width ?? meta.designWidth) || DEFAULT_SCENE_VIEWPORT.design_width;
    const designHeight =
      Number(meta.design_height ?? meta.designHeight) || DEFAULT_SCENE_VIEWPORT.design_height;
    return {
      ...meta,
      design_width: designWidth,
      design_height: designHeight,
      scale_mode: meta.scale_mode || meta.scaleMode || DEFAULT_SCENE_VIEWPORT.scale_mode,
      overflow_mode:
        meta.overflow_mode || meta.overflowMode || meta.overflow || DEFAULT_SCENE_VIEWPORT.overflow_mode,
      aspect_ratio: meta.aspect_ratio || meta.aspectRatio || DEFAULT_SCENE_VIEWPORT.aspect_ratio,
      route_mode: meta.route_mode || meta.routeMode || DEFAULT_SCENE_VIEWPORT.route_mode,
      scene_id: meta.scene_id || doc?.scene_id || null,
    };
  }

  function isDocumentComposeSurface(root, structureDoc) {
    if (!(root instanceof HTMLElement)) return false;
    if (root.classList.contains("mei-compose-document-host")) return true;
    const surface = String(
      root.getAttribute("data-mei-compose-root") ||
        root.getAttribute("data-route-mode") ||
        "",
    )
      .trim()
      .toLowerCase();
    if (surface === "admin" || surface === "config" || surface === "upload") {
      return true;
    }
    const routeMode = String(resolveSceneViewportMeta(structureDoc).route_mode || "")
      .trim()
      .toLowerCase();
    return routeMode === "page" || routeMode === "report" || routeMode === "document";
  }

  function applyFrameViewportMeta(el, meta, docLevel) {
    const vp = resolveSceneViewportMeta({ frame_viewport: meta || docLevel });
    if (!(el instanceof HTMLElement)) return;
    el.setAttribute("data-mei-frame-viewport", "true");
    el.classList.add("preview-viewport", "preview-surface");
    el.setAttribute("data-design-width", String(vp.design_width));
    el.setAttribute("data-design-height", String(vp.design_height));
    el.setAttribute("data-scale-mode", String(vp.scale_mode));
    el.setAttribute("data-overflow-mode", String(vp.overflow_mode));
    if (vp.aspect_ratio) el.setAttribute("data-aspect-ratio", String(vp.aspect_ratio));
    if (vp.target_file) el.setAttribute("data-target-file", String(vp.target_file));
    if (vp.scene_id) el.setAttribute("data-scene-id", String(vp.scene_id));
    el.setAttribute("data-route-mode", String(vp.route_mode || DEFAULT_SCENE_VIEWPORT.route_mode));
    el.classList.add("preview-viewport-access-clip");
    el.classList.remove("preview-viewport-edit-debug");
  }

  function ensureComposeFrameHost(root) {
    if (!(root instanceof HTMLElement)) return;
    root.classList.add("frame-stage-enabled");
    if (root.id === "mei-compose-root" || root.classList.contains("preview-pane-scroll")) {
      root.classList.add("overflow-hidden", "mei-compose-frame-host");
      root.classList.remove("overflow-auto");
    }
  }

  function pinSceneRootToDesignViewport(tree, vpMeta) {
    if (!(tree instanceof HTMLElement)) return;
    const dw = Number(vpMeta?.design_width) || DEFAULT_SCENE_VIEWPORT.design_width;
    const dh = Number(vpMeta?.design_height) || DEFAULT_SCENE_VIEWPORT.design_height;
    const widthText = `${dw}px`;
    const heightText = `${dh}px`;
    tree.style.width = widthText;
    tree.style.height = heightText;
    tree.style.minWidth = "0";
    tree.style.minHeight = "0";
    tree.style.boxSizing = "border-box";
    tree.style.position = "relative";
    tree.dataset.meiSceneDesignWidth = String(dw);
    tree.dataset.meiSceneDesignHeight = String(dh);
    const viewport = tree.closest("[data-mei-compose-scene-viewport]");
    if (viewport instanceof HTMLElement) {
      viewport.style.setProperty("--mei-scene-design-width", widthText);
      viewport.style.setProperty("--mei-scene-design-height", heightText);
    }
  }

  let currentTagLookup = new Map();

  function buildComponentTagLookup(layers) {
    const map = new Map();
    const ingest = (assets) => {
      if (!Array.isArray(assets)) return;
      assets.forEach((asset) => {
        const key = String(asset?.key || "").trim();
        const tag = String(asset?.tag || "").trim();
        if (key && tag) map.set(key, tag);
      });
    };
    ingest(global.__mei?.component_assets);
    ingest(extractLayerDocument(layers?.["runtime.plans"])?.component_assets);
    return map;
  }

  function resolveComponentTag(useKey) {
    const key = String(useKey || "").trim();
    if (!key) return "";
    return currentTagLookup.get(key) || "";
  }

  function isMetricTemplateKind(kind) {
    const normalized = String(kind || "").trim().toLowerCase();
    return (
      normalized === "metric-card" ||
      normalized === "stack" ||
      normalized === "stack_desc" ||
      normalized === "row" ||
      normalized.endsWith("_stack") ||
      normalized === "icon_left" ||
      normalized === "solid_row"
    );
  }

  function metricMountScopeHints(metricId) {
    const id = String(metricId || "").trim();
    if (!id) return [];
    const hints = new Set([id]);
    if (id.endsWith("_count")) {
      const base = id.slice(0, -"_count".length);
      // Avoid ultra-short bases like `case` matching unrelated scopes (`cases`).
      if (base && base.length >= 4) {
        hints.add(base);
        hints.add(`${base}_card`);
      }
    }
    return [...hints];
  }

  function findMetricCardHostInScope(scopeEl) {
    if (!(scopeEl instanceof HTMLElement)) return null;
    return (
      scopeEl.querySelector(".component-host.metric-card") ||
      scopeEl.querySelector('[data-mei-metric-card="true"] .component-host') ||
      null
    );
  }

  function scopeLookupCandidates(scopeKey) {
    const scope = String(scopeKey || "").trim();
    if (!scope) return [];
    const candidates = [scope];
    if (scope.startsWith("t1/")) {
      candidates.push(scope.slice(3));
    } else if (!scope.startsWith("scene:")) {
      candidates.push(`t1/${scope}`);
    }
    return candidates;
  }

  // Gold-case section_layout uses area `title`; `title_zone` / `head` are legacy.
  // Projected title blocks are often `.../title/title` — treat as head text, not the slot.
  const SECTION_HEAD_SLOT_SELECTOR =
    '[data-preview-scope$="/title_zone"], [data-preview-scope$="/title_zone/mei.text"], ' +
    '[data-preview-scope$="/head"], [data-preview-scope$="/head/mei.text"], ' +
    '[data-preview-scope$="/title"]:not([data-preview-scope$="/title/title"]), ' +
    '[data-preview-scope$="/title/mei.text"]';

  function isSectionHeadScope(scopeKey) {
    const scope = String(scopeKey || "").trim().toLowerCase();
    return (
      scope.endsWith("/head") ||
      scope.endsWith("/head/mei.text") ||
      scope.endsWith("/title_zone") ||
      scope.endsWith("/title_zone/mei.text") ||
      scope.endsWith("/title") ||
      scope.endsWith("/title/mei.text") ||
      scope.endsWith("/title/title")
    );
  }

  function isSectionHeadMeiTextScope(scopeKey) {
    const scope = String(scopeKey || "").trim().toLowerCase();
    return (
      scope.endsWith("/head/mei.text") ||
      scope.endsWith("/title_zone/mei.text") ||
      scope.endsWith("/title/mei.text") ||
      scope.endsWith("/title/title")
    );
  }

  function isIgnoredSectionHeadLabel(label) {
    const normalized = String(label || "").trim().toLowerCase();
    return (
      !normalized ||
      normalized === "head" ||
      normalized === "title_zone" ||
      normalized === "title"
    );
  }

  function resolveEvalSlotLabel(entry) {
    const label = String(entry?.label || "").trim();
    if (isIgnoredSectionHeadLabel(label)) return "";
    return label;
  }

  function shouldBindHeadComponentMounts(componentMounts) {
    const mounts = componentMounts || [];
    if (!mounts.length) return false;
    // Rail-level head slots may incorrectly aggregate dozens of mei.text mounts.
    if (mounts.length > 4) return false;
    return true;
  }

  function isStackMetricEvalEntry(entry, container) {
    const kind = String(entry?.content_kind || "").trim().toLowerCase();
    const stackKinds = new Set(["stack", "stack_desc", "row"]);
    if (!stackKinds.has(kind)) return false;
    if (resolveMetricCardSection(container)) return true;
    const useKeys = Array.isArray(entry?.use_keys) ? entry.use_keys : [];
    return useKeys.some((key) => isMetricTemplateKind(key));
  }

  function inferSceneMountForScope(scopeKey, sceneMountByMetric) {
    const scope = String(scopeKey || "").trim().toLowerCase();
    if (!scope || !sceneMountByMetric?.size) return null;
    let best = null;
    let bestScore = 0;
    for (const [metricId, mount] of sceneMountByMetric) {
      for (const hint of metricMountScopeHints(metricId)) {
        const token = String(hint || "").trim().toLowerCase();
        if (!token || token.length < 4) continue;
        const segmentHit =
          scope === token ||
          scope.endsWith(`/${token}`) ||
          scope.includes(`/${token}/`) ||
          scope.endsWith(`/${token}_card`) ||
          scope.includes(`/${token}_card/`);
        if (!segmentHit) continue;
        const score = token.length;
        if (score > bestScore) {
          best = mount;
          bestScore = score;
        }
      }
    }
    return best;
  }

  function buildSyntheticStackMetricMounts(labelText, contentKind, sceneMount) {
    const template = String(contentKind || "stack").trim().toLowerCase() === "stack_desc"
      ? "stack_desc"
      : "stack";
    const mounts = [
      {
        use_key: "metric-card",
        mount_role: "shell",
        props: {
          chrome: "bare",
          __mei_metric_template: template,
          __mei_metric_title_ratio: "2",
          __mei_metric_content_ratio: "3",
          height: "100%",
          width: "100%",
          overflow: "hidden",
        },
      },
      {
        use_key: "mei.text",
        props: {
          metric_role: "label",
          content: String(labelText || "指标").trim() || "指标",
          align: "center",
          metric_v_align: "center",
        },
      },
    ];
    if (sceneMount?.metric_id && sceneMount?.owner_resource_id) {
      const metricContent = {
        __ref: "metric",
        id: sceneMount.metric_id,
        from_dataset: sceneMount.owner_resource_id,
        metric_id: sceneMount.metric_id,
      };
      mounts.push({
        use_key: "mei.text",
        props: {
          metric_role: "value",
          content: metricContent,
          align: "center",
          metric_v_align: "center",
        },
      });
    } else {
      mounts.push({
        use_key: "mei.text",
        props: {
          metric_role: "value",
          content: "--",
          align: "center",
          metric_v_align: "center",
        },
      });
    }
    if (template !== "stack_desc") {
      mounts.push({
        use_key: "mei.text",
        props: {
          metric_role: "unit",
          content: "",
          align: "center",
          metric_v_align: "center",
        },
      });
    }
    return mounts;
  }

  function applyHeadSlotLabel(container, labelText) {
    const label = String(labelText || "").trim();
    if (isIgnoredSectionHeadLabel(label)) return false;
    const headScopeSelector = SECTION_HEAD_SLOT_SELECTOR;
    const isHead = container.matches?.(headScopeSelector);
    const headSlot = isHead ? container : container.closest(headScopeSelector) || container;
    if (!(headSlot instanceof HTMLElement)) return false;
    headSlot.setAttribute("data-mei-eval-label", label);
    return true;
  }

  function scheduleDeferredComposeMap(host) {
    if (!(host instanceof HTMLElement)) return;
    const mount = () => {
      if (host.getAttribute("data-mei-map-mounted") === "1") return;
      host.setAttribute("data-mei-map-mounted", "1");
      const tag = resolveComponentTag("map.maplibre");
      if (!tag) return;
      const existing = host.querySelector('[data-mei-use-key="map.maplibre"]');
      if (!(existing instanceof HTMLElement)) {
        host.innerHTML = `<${tag} data-mei-use-key="map.maplibre"></${tag}>`;
      }
      refreshComposeMaps(host.closest(".mei-structure-tree") || host);
    };
    if (typeof global.requestIdleCallback === "function") {
      global.requestIdleCallback(() => mount(), { timeout: 1500 });
    } else {
      global.setTimeout(mount, 80);
    }
  }

  function findHostForMount(root, mount, scopeKey, useKeys, index, container) {
    const metricId = String(mount?.metric_id || "").trim();
    if (metricId) {
      const searchRoot = container instanceof HTMLElement ? container : root;
      for (const hint of metricMountScopeHints(metricId)) {
        const token = String(hint || "").trim();
        if (!token || token.length < 4) continue;
        // Prefer path-segment matches; substring `case` must not hit `cases`.
        const scopeSelector = [
          `[data-preview-scope$="/${CSS.escape(token)}"]`,
          `[data-preview-scope$="/${CSS.escape(token)}_card"]`,
          `[data-preview-scope*="/${CSS.escape(token)}/"]`,
          `[data-mei-panel-id$="/${CSS.escape(token)}"]`,
          `[data-mei-panel-id$="${CSS.escape(token)}"]`,
        ].join(", ");
        const scopeEl = searchRoot.querySelector(scopeSelector);
        if (!(scopeEl instanceof HTMLElement)) {
          continue;
        }
        // Only bind scene metric mounts onto metric-card hosts — never clobber
        // data-table / chart component-hosts in a loosely matched section.
        const host = findMetricCardHostInScope(scopeEl);
        if (host instanceof HTMLElement) {
          return host;
        }
      }
    }
    const useKey = String(useKeys[index] || useKeys[0] || mount?.use_key || "").trim();
    if (useKey && scopeKey !== "scene:default") {
      const scopeRoot =
        scopeKey === "scene:default" || !container ? root : container;
      // Prefer the host under this exact preview scope — never the first
      // matching use_key elsewhere in a parent layout (e.g. chart.column in
      // inspection vs penalty, or a chart wrongly nested under a metric card).
      const scopedBlock =
        scopeRoot.querySelector(
          `[data-preview-scope="${CSS.escape(scopeKey)}"] [data-mei-use-key="${CSS.escape(useKey)}"]`,
        ) ||
        (scopeRoot.getAttribute?.("data-preview-scope") === scopeKey
          ? scopeRoot.querySelector(`[data-mei-use-key="${CSS.escape(useKey)}"]`)
          : null);
      const block =
        scopedBlock ||
        scopeRoot.querySelector(
          `[data-preview-scope$="/${CSS.escape(useKey)}"] [data-mei-use-key="${CSS.escape(useKey)}"], [data-preview-scope$="/${CSS.escape(useKey)}"][data-mei-use-key="${CSS.escape(useKey)}"]`,
        );
      const host = block?.querySelector?.(".component-host") || block;
      if (host instanceof HTMLElement) {
        return host;
      }
    }
    return null;
  }

  function metricSlotVerticalHostClass(props) {
    const raw = String(props?.metric_v_align || "").trim().toLowerCase();
    if (raw === "start" || raw === "top") return "component-card--slot-v-start";
    if (raw === "end" || raw === "bottom" || raw === "baseline") {
      return "component-card--slot-v-end";
    }
    return "component-card--slot-v-center";
  }

  function ratioFrTrack(raw, fallback) {
    const parsed = Number.parseFloat(String(raw ?? "").trim());
    const value = Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
    return Number.isInteger(value) ? `${value}fr` : `${value}fr`;
  }

  /** 裸 `Nfr`/`1fr` ≡ minmax(auto, Nfr)，Fill-down 内容会撑破轨道；统一收成 minmax(0, …)。 */
  function hardenFrTracks(value) {
    const raw = String(value || "").trim();
    if (!raw) return raw;
    return raw
      .split(/\s+/)
      .filter(Boolean)
      .map((track) => {
        if (/^minmax\(/i.test(track)) return track;
        if (/^[\d.]+fr$/i.test(track)) return `minmax(0, ${track})`;
        return track;
      })
      .join(" ");
  }

  function hardenNodeGridRows(node) {
    if (!(node instanceof HTMLElement)) return;
    const current = String(node.style.gridTemplateRows || "").trim();
    if (!current) return;
    const next = hardenFrTracks(current);
    if (next && next !== current) {
      node.style.gridTemplateRows = next;
    }
  }

  function parseHostProps(node) {
    if (!(node instanceof HTMLElement)) return {};
    const raw = String(node.getAttribute("data-props") || "").trim();
    if (!raw) return {};
    try {
      return JSON.parse(raw);
    } catch (_) {
      return {};
    }
  }

  function filterMountsForScope(scopeKey, mounts) {
    if (!isSectionHeadScope(scopeKey)) {
      return mounts || [];
    }
    return (mounts || []).filter((mount) => {
      const props = mount?.props && typeof mount.props === "object" ? mount.props : {};
      if (String(props.metric_role || "").trim()) return false;
      const content = props.content;
      if (typeof content === "string" && /拖动平移|滚轮缩放/.test(content)) return false;
      return true;
    });
  }

  function isDuplicateMetricCardLeafScope(scopeKey) {
    const scope = String(scopeKey || "").trim().toLowerCase();
    return /\/(label|value|unit)\/mei\.text$/.test(scope);
  }

  function filterComponentMountsForScope(scopeKey, mounts) {
    const filtered = filterMountsForScope(scopeKey, mounts);
    if (isSectionHeadScope(scopeKey)) {
      return filtered;
    }
    return (filtered || []).filter((mount) => {
      const useKey = String(mount?.use_key || "").trim();
      if (useKey !== "mei.text") return true;
      const props = mount?.props && typeof mount.props === "object" ? mount.props : {};
      if (String(props.metric_role || props.metricRole || "").trim()) return true;
      // Authored plain-text leaves carry string content (metric cards use object
      // content + metric_role). Keep them under classic `…/area/mei.text` scopes
      // and duplicate-segment leaves like `…/chart/chart`.
      if (isDuplicateMetricCardLeafScope(scopeKey)) return false;
      return typeof props.content === "string" && props.content.trim().length > 0;
    });
  }

  function suppressDecomposedMetricCardDuplicateSlots(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll('[data-preview-scope$="_card_content"]:not([data-mei-metric-card])')
      .forEach((slot) => {
        if (!(slot instanceof HTMLElement)) return;
        const scope = String(slot.getAttribute("data-preview-scope") || "");
        if (scope.includes("/content/")) return;
        const cardId = scope.split("/").pop() || "";
        if (!cardId.endsWith("_card_content")) return;
        const metricCard = root.querySelector(
          `[data-mei-metric-card][data-preview-scope*="/content/${cardId}"]`,
        );
        if (!(metricCard instanceof HTMLElement)) return;
        slot.style.display = "none";
        slot.setAttribute("aria-hidden", "true");
        slot.style.pointerEvents = "none";
        slot.style.overflow = "hidden";
      });
  }

  function suppressDuplicateMetricCardLeafSlots(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll('[data-preview-scope*="card_content/"]').forEach((el) => {
      if (!(el instanceof HTMLElement)) return;
      const scope = String(el.getAttribute("data-preview-scope") || "").toLowerCase();
      if (!/\/card_content\/(label|value|unit)(\/|$)/.test(scope)) return;
      if (el.closest('[data-mei-metric-card="true"]')) return;
      el.style.display = "none";
      el.setAttribute("aria-hidden", "true");
      el.style.pointerEvents = "none";
      el.style.overflow = "hidden";
    });
  }

  function resolveMetricCardBodyCell(section) {
    if (!(section instanceof HTMLElement)) return null;
    let bodyCell = section.querySelector(":scope > .panel-body-cell");
    if (!(bodyCell instanceof HTMLElement)) {
      const host = section.querySelector(":scope > .component-host");
      bodyCell = document.createElement("div");
      bodyCell.className = "panel-body-cell";
      bodyCell.setAttribute("data-mei-panel-body", "true");
      if (host instanceof HTMLElement) {
        section.replaceChild(bodyCell, host);
        bodyCell.appendChild(host);
      } else {
        const nextHost = document.createElement("div");
        nextHost.className = "component-host metric-card";
        bodyCell.appendChild(nextHost);
        section.appendChild(bodyCell);
      }
    }
    return bodyCell;
  }

  function applyMetricStackGridLayout(target) {
    if (!(target instanceof HTMLElement)) return;
    const template = String(
      target.getAttribute("data-mei-metric-template") ||
        target.closest("[data-mei-metric-card]")?.getAttribute("data-mei-metric-template") ||
        "stack",
    ).trim();
    const card = target.closest("[data-mei-metric-card]") || target;
    const style = target.style;
    style.display = "grid";
    style.boxSizing = "border-box";
    style.minHeight = "0";
    style.minWidth = "0";
    style.width = "100%";
    style.height = "100%";
    // icon_left / strip_icon_left: shell (or card) reserves left padding for a
    // background icon. auto+center collapses the text track → "待.." truncation
    // and a huge gap on the summary strip. Match zhifa: fill width, start-align.
    const padHost =
      [card, card.parentElement].find((el) => {
        if (!(el instanceof HTMLElement)) return false;
        const padLeft = Number.parseFloat(global.getComputedStyle?.(el)?.paddingLeft || "0") || 0;
        return padLeft >= 48;
      }) || card;
    const padLeft =
      Number.parseFloat(global.getComputedStyle?.(padHost)?.paddingLeft || "0") || 0;
    const iconReserved = padLeft >= 48;
    const inlineAlign = String(
      card.getAttribute("data-mei-metric-inline-align") ||
        target.getAttribute("data-mei-metric-inline-align") ||
        "compact",
    )
      .trim()
      .toLowerCase();
    const compactInline = inlineAlign !== "between" && inlineAlign !== "spread";
    // 「20 项」：value 与 unit 同基线、约一字空格；between/spread 略松。
    const valueUnitColGap = compactInline ? "0 0.3em" : "0 4px";
    const PROGRESS_STACK_ROWS = "12px 20px 34px 10px 18px 10px";
    const PROGRESS_STACK_AREAS =
      '". ." "label label" "value unit" ". ." "desc desc" ". ."';
    if (template === "row") {
      // 横排看板默认（作者声明 ui.row_accent_* 即可）：
      // 标签列用 max-content（不可再被压回 4em）；数值拿走剩余宽 + 单位保底宽左跟。
      // minmax(4em, max-content) 在窄槽仍会缩到 4em，导致「执法记…」截断。
      const centerValuePack =
        inlineAlign === "center" || inlineAlign === "middle" || inlineAlign === "center_value";
      if (centerValuePack) {
        // 查实率等长条：label 左置；value+unit 作为整体居中于内容区。
        style.gridTemplateColumns = "1fr auto auto 1fr";
        style.gridTemplateRows = "1fr";
        style.gridTemplateAreas = '"label value unit ."';
        style.alignItems = "center";
        style.justifyItems = "stretch";
        style.justifyContent = "stretch";
        style.gap = "0 0.3em";
        card.setAttribute("data-mei-metric-value-unit-tight", "true");
        card.setAttribute("data-mei-metric-inline-align", "center");
        applyRowMetricCenterAlign(target);
        return;
      }
      style.gridTemplateColumns = "max-content minmax(0, 1fr) minmax(1.25em, auto)";
      style.gridTemplateRows = "1fr";
      style.gridTemplateAreas = '"label value unit"';
      style.alignItems = "center";
      style.justifyItems = "stretch";
      style.justifyContent = iconReserved ? "start" : "stretch";
      style.gap = compactInline ? "0 0.25em" : "2px 4px";
      card.setAttribute("data-mei-metric-value-unit-tight", compactInline ? "true" : "false");
      applyRowMetricBoardAlign(target);
      return;
    }
    if (template !== "stack" && template !== "stack_desc") return;
    const titleRatio =
      card.getAttribute("data-mei-metric-title-ratio") || "2";
    const contentRatio =
      card.getAttribute("data-mei-metric-content-ratio") || "3";
    const cardProps = parseHostProps(card);
    const descMode = String(
      card.getAttribute("data-mei-metric-desc-mode") ||
        target.getAttribute("data-mei-metric-desc-mode") ||
        cardProps.__mei_metric_desc_mode ||
        cardProps.metric_desc_mode ||
        "",
    )
      .trim()
      .toLowerCase();
    const hasProgressDesc = !!card.querySelector(
      "mei-cockpit-metric-progress, [data-mei-use-key='cockpit.metric-progress']",
    );
    const isProgressCard = descMode === "progress" || hasProgressDesc;
    // Progress cards author fixed px tracks + spacer rows (e.g. clean SVG 104px shell).
    // Collapsing them into 2fr/3fr/auto clips label/value under overflow:hidden.
    const authoredRows = String(
      target.dataset.manifestGridRows || target.style.gridTemplateRows || "",
    ).trim();
    const authoredCols = String(
      target.dataset.manifestGridColumns || target.style.gridTemplateColumns || "",
    ).trim();
    const authoredTrackCount = authoredRows
      ? authoredRows.split(/\s+/).filter(Boolean).length
      : 0;
    const preserveAuthoredProgress =
      isProgressCard || (authoredTrackCount >= 5 && /\dpx/i.test(authoredRows));
    if (isProgressCard) {
      card.setAttribute("data-mei-metric-desc-mode", "progress");
    }
    style.gridTemplateColumns = authoredCols || "auto auto";
    if (isProgressCard) {
      // Always use canonical progress areas so desc cannot auto-place between label/value.
      style.gridTemplateRows =
        authoredTrackCount >= 5 && /\dpx/i.test(authoredRows)
          ? authoredRows
          : PROGRESS_STACK_ROWS;
      style.gridTemplateAreas = PROGRESS_STACK_AREAS;
    } else if (preserveAuthoredProgress) {
      style.gridTemplateRows = authoredRows || PROGRESS_STACK_ROWS;
      style.gridTemplateAreas = PROGRESS_STACK_AREAS;
    } else if (template === "stack_desc") {
      // Keep desc on its own row so the badge does not overlap value/unit.
      style.gridTemplateRows = `${ratioFrTrack(titleRatio, 1)} ${ratioFrTrack(contentRatio, 1)} auto`;
      style.gridTemplateAreas = '"label label" "value unit" "desc desc"';
    } else {
      style.gridTemplateRows = `${ratioFrTrack(titleRatio, 1)} ${ratioFrTrack(contentRatio, 1)}`;
      style.gridTemplateAreas = '"label label" "value unit"';
    }
    style.alignItems = "stretch";
    style.justifyItems = iconReserved ? "start" : "center";
    style.justifyContent = iconReserved ? "start" : "center";
    // Progress keeps row gap 0; column gap still needs the one-space value|unit feel.
    style.gap = isProgressCard || preserveAuthoredProgress ? "0 0.3em" : valueUnitColGap;
    card.setAttribute("data-mei-metric-value-unit-tight", "true");
  }

  function pinMetricProgressToDescArea(bodyCell) {
    if (!(bodyCell instanceof HTMLElement)) return;
    const progress =
      bodyCell.querySelector("mei-cockpit-metric-progress") ||
      bodyCell.querySelector("[data-mei-use-key='cockpit.metric-progress']");
    if (!(progress instanceof HTMLElement)) return;
    const hostCard =
      progress.closest(".component-card") ||
      (progress.parentElement instanceof HTMLElement ? progress.parentElement : null);
    const target = hostCard instanceof HTMLElement ? hostCard : progress;
    target.style.gridArea = "desc";
    target.style.minWidth = "0";
    target.style.minHeight = "0";
    target.setAttribute("data-metric-role", "desc");
  }

  function patchMetricTextAlign(node, align) {
    if (!(node instanceof HTMLElement)) return;
    const props = parseHostProps(node);
    if (String(props.align || "").trim().toLowerCase() === align) return;
    const next = { ...props, align };
    node.setAttribute("data-props", JSON.stringify(next));
    if (typeof node._bind === "function") {
      try {
        node._bind();
      } catch (_) {}
    }
  }

  /** Row 横排看板：label 两端对齐 / value 小数点对齐 / unit 左跟. */
  function applyRowMetricBoardAlign(bodyCell) {
    if (!(bodyCell instanceof HTMLElement)) return;
    bodyCell.querySelectorAll(":scope > .component-card").forEach((slot) => {
      if (!(slot instanceof HTMLElement)) return;
      const text = slot.querySelector("mei-text, MEI-TEXT");
      const role = String(parseHostProps(text).metric_role || "").trim();
      if (role === "label") {
        slot.style.justifySelf = "stretch";
        patchMetricTextAlign(text, "justify");
      } else if (role === "value") {
        slot.style.justifySelf = "stretch";
        patchMetricTextAlign(text, "decimal");
      } else if (role === "unit") {
        slot.style.justifySelf = "start";
        patchMetricTextAlign(text, "left");
      }
    });
  }

  /** 长条查实率等：value+unit 紧挨并居于内容区水平中线；label 仍在左侧。 */
  function applyRowMetricCenterAlign(bodyCell) {
    if (!(bodyCell instanceof HTMLElement)) return;
    bodyCell.querySelectorAll(":scope > .component-card").forEach((slot) => {
      if (!(slot instanceof HTMLElement)) return;
      const text = slot.querySelector("mei-text, MEI-TEXT");
      const role = String(parseHostProps(text).metric_role || "").trim();
      if (role === "label") {
        slot.style.justifySelf = "start";
        patchMetricTextAlign(text, "left");
      } else if (role === "value") {
        slot.style.justifySelf = "end";
        patchMetricTextAlign(text, "left");
      } else if (role === "unit") {
        slot.style.justifySelf = "start";
        patchMetricTextAlign(text, "left");
      }
    });
  }

  function wrapMetricRoleNode(node, role) {
    if (!(node instanceof HTMLElement)) return node;
    const parentCard = node.closest(".component-card");
    if (parentCard instanceof HTMLElement) return node;
    const props = parseHostProps(node);
    const slotClass = metricSlotVerticalHostClass(props);
    const card = document.createElement("section");
    card.className = `component-card ${slotClass}`.trim();
    card.style.gridArea = role;
    card.style.minWidth = "0";
    card.style.minHeight = "0";
    // stack：value 靠 unit；row 由 applyRowMetricBoardAlign 设为 stretch + 右齐。
    if (role === "value") card.style.justifySelf = "end";
    if (role === "unit") card.style.justifySelf = "start";
    const host = document.createElement("div");
    host.className = "component-host";
    node.replaceWith(card);
    host.appendChild(node);
    card.appendChild(host);
    return node;
  }

  function normalizeMetricCardSection(section) {
    if (!(section instanceof HTMLElement)) return;
    if (section.getAttribute("data-mei-metric-card") !== "true") return;
    section.classList.add("preview-card-bare");
    const bodyCell = resolveMetricCardBodyCell(section);
    if (!(bodyCell instanceof HTMLElement)) return;
    const host = bodyCell.querySelector(".component-host");
    const isMetricRole = (role) =>
      role === "label" || role === "value" || role === "unit" || role === "desc";
    const roleNodes = host
      ? Array.from(host.querySelectorAll("mei-text, MEI-TEXT")).filter((node) => {
          const role = String(parseHostProps(node).metric_role || "").trim();
          return isMetricRole(role);
        })
      : Array.from(bodyCell.querySelectorAll("mei-text, MEI-TEXT")).filter((node) => {
          const role = String(parseHostProps(node).metric_role || "").trim();
          return isMetricRole(role);
        });
    if (!roleNodes.length && bodyCell.querySelector(":scope > .component-card")) {
      pinMetricProgressToDescArea(bodyCell);
      applyMetricStackGridLayout(bodyCell);
      return;
    }
    if (!roleNodes.length) {
      return;
    }
    roleNodes.forEach((node) => {
      const role = String(parseHostProps(node).metric_role || "").trim();
      wrapMetricRoleNode(node, role);
    });
    if (host instanceof HTMLElement) {
      Array.from(host.querySelectorAll(":scope > .component-card")).forEach((card) => {
        bodyCell.appendChild(card);
      });
      if (!host.childElementCount) {
        host.remove();
      }
    }
    pinMetricProgressToDescArea(bodyCell);
    applyMetricStackGridLayout(bodyCell);
    section.setAttribute("data-mei-metric-card-normalized", "1");
  }

  function escapeHtmlText(value) {
    return String(value || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function buildSectionHeadMarkup(titleText) {
    const title = escapeHtmlText(String(titleText || "").trim() || "板块标题");
    return (
      `<div class="panel-head-cell panel-heading panel-heading-plain panel-heading-compact" data-mei-panel-head="true">` +
      `<div class="panel-head-slot"><div class="panel-heading-copy"><h3>${title}</h3></div></div></div>`
    );
  }

  /** Prefer live theme gradient so ops.sceneThemes updates title bars without rewarming baked literals. */
  function livePanelTitleBarCellStyle(cellStyle) {
    let style = String(cellStyle || "").trim();
    style = style
      .replace(/background-image\s*:[^;]*;?/gi, "")
      .replace(/background-size\s*:[^;]*;?/gi, "")
      .replace(/background-position\s*:[^;]*;?/gi, "")
      .replace(/background-repeat\s*:[^;]*;?/gi, "")
      .replace(/;;+/g, ";")
      .replace(/^;|;$/g, "");
    const liveBg =
      "background-image:var(--mei-gradient-panel-title-bar);" +
      "background-size:100% 100%;background-position:center;background-repeat:no-repeat;";
    return style ? `${style};${liveBg}` : liveBg;
  }

  function applyHeadChromeFromSlot(headEl, headChrome) {
    if (!(headEl instanceof HTMLElement) || !headChrome || typeof headChrome !== "object") {
      return false;
    }
    const title = String(headChrome.title || "").trim() || "板块标题";
    const classes = Array.isArray(headChrome.heading_classes)
      ? headChrome.heading_classes.filter(Boolean).join(" ")
      : "panel-heading panel-heading-plain panel-heading-compact";
    const cellStyle = livePanelTitleBarCellStyle(headChrome.cell_style);
    const caret = headChrome.caret && typeof headChrome.caret === "object" ? headChrome.caret : {};
    const caretEnabled = caret.enabled === true;
    const caretStyle = String(caret.style || "").trim();
    const typo =
      headChrome.heading_typography && typeof headChrome.heading_typography === "object"
        ? headChrome.heading_typography
        : {};
    let h3Style = "";
    if (typo.font_size) h3Style += `font-size:${typo.font_size};`;
    if (typo.color) h3Style += `color:${typo.color};`;
    if (typo.font_family) h3Style += `font-family:${typo.font_family};`;
    if (typo.font_weight != null && String(typo.font_weight).trim()) {
      h3Style += `font-weight:${typo.font_weight};`;
    }
    if (typo.letter_spacing) h3Style += `letter-spacing:${typo.letter_spacing};`;
    const cellAlignsCenter =
      /justify-content\s*:\s*center/i.test(cellStyle) ||
      String(headChrome.align || "").trim().toLowerCase() === "center";
    if (cellAlignsCenter) {
      h3Style += "text-align:center;width:100%;";
    }
    const caretAttrs = caretEnabled
      ? ` data-mei-head-carets="true" data-mei-head-carets-mode="${escapeHtmlAttr(String(caret.mode || "slot"))}"`
      : "";
    const combinedStyle = [cellStyle, caretEnabled ? caretStyle : ""].filter(Boolean).join("");
    headEl.className = "mei-compose-slot preview-card preview-card-bare mei-compose-section-head";
    headEl.innerHTML =
      `<div class="panel-head-cell ${escapeHtmlAttr(classes)}" data-mei-panel-head="true"${caretAttrs}` +
      (combinedStyle ? ` style="${escapeHtmlAttr(combinedStyle)}"` : "") +
      `><div class="panel-head-slot"><div class="panel-heading-copy"${
      cellAlignsCenter ? ' style="width:100%;display:flex;justify-content:center;"' : ""
    }><h3` +
      (h3Style ? ` style="${escapeHtmlAttr(h3Style)}"` : "") +
      `>${escapeHtmlText(title)}</h3></div></div></div>`;
    headEl.setAttribute("data-mei-section-head-chrome", "1");
    headEl.removeAttribute("data-mei-section-head-normalized");
    return true;
  }

  function applyPanelShellFromSlot(container, panelShell) {
    if (!(container instanceof HTMLElement) || !panelShell?.props) return false;
    let target = container;
    if (!target.hasAttribute("data-preview-scope")) {
      const scoped = container.closest("[data-preview-scope]");
      if (scoped instanceof HTMLElement) target = scoped;
    }
    if (!target.classList.contains("mei-compose-content-group")) {
      if (
        !target.classList.contains("preview-card") &&
        !target.classList.contains("mei-compose-block") &&
        !target.hasAttribute("data-mei-metric-card")
      ) {
        const content = target.closest('[data-mei-ui-role="content"], [data-mei-ui-role="slot"]');
        if (content instanceof HTMLElement) target = content;
      }
    }
    // Nested areas inside a compound-metric host must stay transparent — only the
    // compound content node owns the shared slot-frame background.
    const compoundHost = target.closest?.('[data-mei-content-kind="compound-metric"]');
    if (
      compoundHost instanceof HTMLElement &&
      compoundHost !== target &&
      target.getAttribute("data-mei-content-kind") !== "compound-metric"
    ) {
      return false;
    }
    const shellProps = panelShell.props;
    const chromeBare = String(shellProps.chrome || "").trim() === "bare";
    if (chromeBare) {
      target.classList.add("preview-card-bare");
    }
    applyContainerVisualStyle(target, shellProps);
    target.setAttribute("data-mei-panel-shell-applied", "1");
    return true;
  }

  function clearNestedCompoundSlotFrames(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll('[data-mei-content-kind="compound-metric"]').forEach((host) => {
      if (!(host instanceof HTMLElement)) return;
      host
        .querySelectorAll(
          ':scope [data-mei-ui-role="slot"], :scope [data-mei-metric-card], :scope .preview-card',
        )
        .forEach((el) => {
          if (!(el instanceof HTMLElement) || el === host) return;
          if (el.getAttribute("data-mei-content-kind") === "compound-metric") return;
          el.style.backgroundImage = "none";
          el.style.backgroundColor = "transparent";
          el.style.background = "transparent";
          el.style.boxShadow = "none";
          el.removeAttribute("data-mei-slot-frame-bg");
          el.removeAttribute("data-mei-panel-shell-applied");
        });
    });
  }

  function normalizeSectionHeadSlot(headSlot) {
    if (!(headSlot instanceof HTMLElement)) return;
    if (headSlot.getAttribute("data-mei-section-head-chrome") === "1") return;
    const scope = String(headSlot.getAttribute("data-preview-scope") || "");
    if (!isSectionHeadScope(scope)) return;

    let titleText = String(headSlot.getAttribute("data-mei-eval-label") || "").trim();
    if (!titleText) {
      titleText = String(headSlot.getAttribute("data-mei-structure-label") || "").trim();
    }
    if (!titleText) {
      const childLabel = headSlot.querySelector("[data-mei-structure-label]");
      if (childLabel instanceof HTMLElement) {
        titleText = String(childLabel.getAttribute("data-mei-structure-label") || "").trim();
      }
    }
    if (!titleText) {
      const section = headSlot.closest('[data-mei-ui-role="section"]');
      if (section instanceof HTMLElement) {
        titleText = String(section.getAttribute("data-mei-structure-label") || "").trim();
      }
    }
    headSlot.querySelectorAll("mei-text, MEI-TEXT").forEach((node) => {
      const props = parseHostProps(node);
      if (String(props.metric_role || "").trim()) {
        node.remove();
        return;
      }
      const content = props.content;
      if (!titleText && typeof content === "string" && content.trim()) {
        titleText = content.trim();
      }
    });
    const existingH3 = headSlot.querySelector("h3");
    if (!titleText && existingH3 instanceof HTMLElement) {
      const current = String(existingH3.textContent || "").trim();
      if (current && !isIgnoredSectionHeadLabel(current)) {
        return;
      }
    }
    if (!titleText) {
      const warningPanel = headSlot.closest('[data-preview-scope$="/warning"]');
      const warningScope = String(warningPanel?.getAttribute("data-preview-scope") || "");
      if (warningScope.includes("warning")) {
        titleText = "监督预警";
      }
    }
    if (!titleText) {
      const label = String(headSlot.getAttribute("data-mei-panel-id") || scope)
        .split("/")
        .filter(Boolean)
        .pop();
      titleText = !isIgnoredSectionHeadLabel(label) ? label.replace(/_/g, " ") : "板块标题";
    }

    headSlot.className = "mei-compose-slot preview-card preview-card-bare mei-compose-section-head";
    headSlot.innerHTML = buildSectionHeadMarkup(titleText);
    headSlot.setAttribute("data-mei-section-head-normalized", "1");
  }

  function normalizeAllSectionHeadSlots(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(SECTION_HEAD_SLOT_SELECTOR)
      .forEach((headSlot) => {
        if (headSlot instanceof HTMLElement && headSlot.getAttribute("data-mei-section-head-chrome") === "1") {
          return;
        }
        normalizeSectionHeadSlot(headSlot);
      });
  }

  function hideLayoutDebugRegions(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll('[data-preview-scope*="layout_debug"]')
      .forEach((el) => {
        if (!(el instanceof HTMLElement)) return;
        el.style.display = "none";
        el.setAttribute("aria-hidden", "true");
        el.style.pointerEvents = "none";
      });
  }

  function createMetricCardSection(scope, uiRole, nodeLabel) {
    const section = document.createElement("section");
    section.className = "preview-card preview-card-bare mei-compose-block";
    section.setAttribute("data-mei-metric-card", "true");
    if (scope) section.setAttribute("data-preview-scope", scope);
    section.setAttribute("data-mei-ui-role", uiRole || "content");
    section.setAttribute("data-mei-use-key", "metric-card");
    if (nodeLabel) section.setAttribute("data-mei-structure-label", String(nodeLabel));
    const bodyCell = document.createElement("div");
    bodyCell.className = "panel-body-cell";
    bodyCell.setAttribute("data-mei-panel-body", "true");
    const host = document.createElement("div");
    host.className = "component-host metric-card";
    bodyCell.appendChild(host);
    section.appendChild(bodyCell);
    return section;
  }

  function createBlockSection(useKey, scope, uiRole) {
    const section = document.createElement("section");
    const key = String(useKey || "").trim();
    const bareChrome =
      key === "cockpit.header-brand" ||
      key.startsWith("chart.") ||
      key === "mei.chart" ||
      key.startsWith("mei-chart") ||
      // semantic_scene compose 会给每个 component 包一层 .preview-card（默认 14px 圆角）；
      // 驾驶舱内嵌可视化与 chart.* 一样走 bare，避免园区圆环/罚金等出现圆角底。
      key === "cockpit.donut-trio" ||
      key === "cockpit.park-amount-list" ||
      key === "cockpit.scroll-list" ||
      key === "cockpit.data-table" ||
      key === "cockpit.metric-progress" ||
      // thunder 时间标尺 / 雷暴列表 / 右栏统计等：禁止默认 preview-card 圆角底
      key.startsWith("thunder.") ||
      key === "map.maplibre";
    section.className = bareChrome
      ? "preview-card preview-card-bare mei-compose-block"
      : "preview-card mei-compose-block";
    if (!key) return section;
    if (scope) section.setAttribute("data-preview-scope", scope);
    section.setAttribute("data-mei-use-key", key);
    section.setAttribute("data-mei-ui-role", uiRole || "content");
    const host = document.createElement("div");
    host.className = "component-host";
    if (key === "cockpit.world-stage") {
      host.classList.add("mei-compose-deferred-world-stage");
      host.setAttribute("data-mei-defer-mount", "true");
      section.appendChild(host);
      return section;
    }
    if (key === "map.maplibre") {
      host.classList.add("mei-compose-deferred-map");
      host.setAttribute("data-mei-defer-mount", "true");
      section.appendChild(host);
      scheduleDeferredComposeMap(host);
      return section;
    }
    const tag = resolveComponentTag(key);
    if (tag) {
      host.innerHTML = `<${tag} data-mei-use-key="${escapeHtmlAttr(key)}"></${tag}>`;
    }
    section.appendChild(host);
    return section;
  }

  function stampStructureIdentity(el, node) {
    if (!(el instanceof HTMLElement) || !node) return el;
    const nodeId = String(node.node_id || "").trim();
    const scope = String(node.preview_scope || "").trim();
    const role = String(node.ui_role || "").trim().toLowerCase();
    if (nodeId) {
      el.setAttribute("data-build-node", nodeId);
      el.setAttribute("data-mei-node-id", nodeId);
    }
    if (scope && !el.getAttribute("data-preview-scope")) {
      el.setAttribute("data-preview-scope", scope);
    }
    if (role && !el.getAttribute("data-mei-ui-role")) {
      el.setAttribute("data-mei-ui-role", role);
    }
    if (node.panel_id && !el.getAttribute("data-mei-panel-id")) {
      el.setAttribute("data-mei-panel-id", String(node.panel_id));
    }
    const planeCode = String(node.plane || "").trim();
    if (planeCode) {
      if (!el.getAttribute("data-mei-plane")) el.setAttribute("data-mei-plane", planeCode);
      if (!el.getAttribute("data-mei-tier")) el.setAttribute("data-mei-tier", planeCode);
    }
    return el;
  }

  function createNodeElement(node, structureDoc) {
    const role = String(node.ui_role || "").toLowerCase();
    const scope = String(node.preview_scope || "").trim();

    if (isViewportNode(node)) {
      const section = document.createElement("section");
      section.className = "preview-viewport preview-surface mei-compose-viewport";
      if (scope) section.setAttribute("data-preview-scope", scope);
      section.setAttribute("data-mei-ui-role", String(node.ui_role || "region"));
      applyFrameViewportMeta(section, node.frame_viewport, structureDoc?.frame_viewport);
      const stageShell = document.createElement("div");
      stageShell.className = "preview-stage-shell";
      const stage = document.createElement("section");
      stage.className = "preview-surface preview-stage";
      stageShell.appendChild(stage);
      section.appendChild(stageShell);
      section.__meiStageTarget = stage;
      return stampStructureIdentity(section, node);
    }

    if (role === "content") {
      if (isViewportMetaContentNode(node)) {
        const placeholder = document.createElement("div");
        placeholder.className = "mei-compose-viewport-meta";
        placeholder.hidden = true;
        if (scope) placeholder.setAttribute("data-preview-scope", scope);
        return stampStructureIdentity(placeholder, node);
      }
      const contentKind = String(node.content_kind || "").trim().toLowerCase();
      if (contentKind === "compound-metric") {
        const compoundHost = document.createElement("section");
        compoundHost.className = "mei-compose-node mei-compose-content";
        if (scope) compoundHost.setAttribute("data-preview-scope", scope);
        compoundHost.setAttribute("data-mei-ui-role", "content");
        compoundHost.setAttribute("data-mei-content-kind", "compound-metric");
        if (node.label) {
          compoundHost.setAttribute("data-mei-structure-label", String(node.label));
        }
        return stampStructureIdentity(compoundHost, node);
      }
      // Container content (e.g. status-flow grid host) must keep children + layout;
      // do not collapse into leaf metric-card / content-group mounts.
      const hasStructureChildren =
        Array.isArray(node.children) && node.children.length > 0;
      if (!hasStructureChildren) {
        const keys = Array.isArray(node.use_keys) && node.use_keys.length
          ? node.use_keys
          : node.content_kind
            ? [node.content_kind]
            : [];
        if (keys.length === 1) {
          const key = keys[0];
          const scopeLower = scope.toLowerCase();
          if (
            isMetricTemplateKind(key) &&
            !scopeLower.includes("/map_viewport/") &&
            !scopeLower.endsWith("/map-viewport")
          ) {
            if (scopeLower.includes("/hint/") || scopeLower.includes("stage-aperture-hint")) {
              return stampStructureIdentity(
                createBlockSection("mei.text", scope, node.ui_role),
                node,
              );
            }
            return stampStructureIdentity(
              createMetricCardSection(scope, node.ui_role, node.label),
              node,
            );
          }
          if (!isMetricTemplateKind(key)) {
            return stampStructureIdentity(
              createBlockSection(key, scope, node.ui_role),
              node,
            );
          }
        }
        if (keys.length > 1) {
          const wrap = document.createElement("div");
          wrap.className = "mei-compose-content-group";
          if (scope) wrap.setAttribute("data-preview-scope", scope);
          if (node.label) wrap.setAttribute("data-mei-structure-label", String(node.label));
          keys.forEach((key) => wrap.appendChild(createBlockSection(key, scope, node.ui_role)));
          return stampStructureIdentity(wrap, node);
        }
      }
    }

    const tag =
      role === "slot" || role === "section" || role === "region" || role === "slide"
        ? "section"
        : "div";
    const el = document.createElement(tag);
    el.className = `mei-compose-node mei-compose-${role || "node"}`;
    if (scope) el.setAttribute("data-preview-scope", scope);
    if (node.panel_id) {
      el.setAttribute("data-mei-panel-id", String(node.panel_id));
    } else if (
      (role === "slot" || role === "section" || role === "region" || role === "slide") &&
      scope
    ) {
      el.setAttribute("data-mei-panel-id", scope);
    }
    el.setAttribute("data-mei-ui-role", String(node.ui_role || ""));
    if (node.label) {
      el.setAttribute("data-mei-structure-label", String(node.label));
    }
    // Deck controller resolves slides by panel name / leaf id.
    if (role === "slide") {
      const leaf =
        String(node.panel_name || node.panel_id || scope || "")
          .split("/")
          .filter(Boolean)
          .pop() || "";
      if (leaf) el.setAttribute("data-mei-panel-name", leaf);
    }
    const planeCode = String(node.plane || "").trim();
    if (planeCode) {
      el.setAttribute("data-mei-plane", planeCode);
      el.setAttribute("data-mei-tier", planeCode);
    }
    // Plane 旧 structure 常无 preview_scope；用 plane code 补上，便于 layout_budget 对齐。
    if (role === "plane" && !el.getAttribute("data-preview-scope")) {
      const fallback = planeCode || String(node.label || "").trim();
      if (fallback) el.setAttribute("data-preview-scope", fallback.toLowerCase());
    }
    return stampStructureIdentity(el, node);
  }

  function mountTargetForParent(parentEl) {
    return parentEl;
  }

  function buildStructureTree(root, structureDoc, options) {
    if (!(root instanceof HTMLElement)) return false;
    const startedAt = typeof performance !== "undefined" ? performance.now() : Date.now();
    boot.renderPipelineMark?.("compose_structure:begin");
    const doc = extractLayerDocument(structureDoc);
    const allNodes = Array.isArray(doc?.nodes) ? doc.nodes : [];
    if (!allNodes.length) {
      boot.renderPipelineMark?.("compose_structure:end", { nodes: 0, durationMs: 0 });
      return false;
    }

    const projection = options?.review_projection || options?.reviewProjection || "";
    let nodes = allNodes;
    if (projection && boot.viewCompositor?.nodesForProjection) {
      const visible = boot.viewCompositor.nodesForProjection(doc, projection);
      const allowed = new Set(visible.map((node) => node.node_id));
      const byId = new Map(allNodes.map((node) => [node.node_id, node]));
      for (const node of visible) {
        let parentId = String(node.parent_id || "").trim();
        while (parentId) {
          if (allowed.has(parentId)) break;
          allowed.add(parentId);
          parentId = String(byId.get(parentId)?.parent_id || "").trim();
        }
      }
      nodes = allNodes.filter((node) => allowed.has(node.node_id));
    }

    const nodeById = new Map();
    nodes.forEach((node) => nodeById.set(node.node_id, node));
    const childrenByParent = new Map();
    nodes.forEach((node) => {
      const parentId = String(node.parent_id || "").trim();
      if (!parentId) return;
      const children = childrenByParent.get(parentId) || [];
      children.push(node.node_id);
      childrenByParent.set(parentId, children);
    });

    const resolveRoots =
      boot.structureTreeMaterializer?.resolveRoots ||
      ((allNodes, sceneRoots) => {
        if (Array.isArray(sceneRoots) && sceneRoots.length) {
          const roots = sceneRoots
            .map((id) => nodeById.get(id))
            .filter(Boolean);
          if (roots.length) return roots;
        }
        return allNodes.filter((node) => !String(node.parent_id || "").trim());
      });

    const container = document.createElement("div");
    container.className = "mei-structure-tree";

    function mountSubtree(node, parentEl) {
      if (!node || !(parentEl instanceof HTMLElement)) return;
      const created = createNodeElement(node, doc);
      const target = mountTargetForParent(parentEl);
      if (created instanceof HTMLElement) {
        target.appendChild(created);
        const childIds = Array.isArray(node.children) && node.children.length
          ? node.children
          : childrenByParent.get(node.node_id) || [];
        childIds.forEach((childId) => {
          const child = nodeById.get(childId);
          if (child) mountSubtree(child, created);
        });
        return;
      }
      if (created instanceof DocumentFragment) {
        target.appendChild(created);
      }
    }

    const roots = resolveRoots(nodes, doc.scene_roots);
    if (roots.length) {
      roots.forEach((node) => mountSubtree(node, container));
    } else {
      nodes.forEach((node) => {
        const created = createNodeElement(node, doc);
        if (created instanceof HTMLElement) {
          container.appendChild(created);
        }
      });
    }

    root.querySelectorAll(".mei-structure-tree").forEach((el) => el.remove());
    root.appendChild(container);
    boot.renderPipelineMark?.("compose_structure:end", {
      nodes: nodes.length,
      durationMs: Math.round(
        (typeof performance !== "undefined" ? performance.now() : Date.now()) - startedAt,
      ),
    });
    return container.childNodes.length > 0;
  }

  function collectEvalDocs(layers) {
    const docs = [];
    Object.entries(layers || {}).forEach(([name, value]) => {
      if (!name.startsWith("eval.slot_group.")) return;
      const doc = extractLayerDocument(value);
      if (doc?.slots) docs.push(doc);
    });
    return docs;
  }

  function propsFromMount(mount) {
    if (!mount || typeof mount !== "object") return {};
    return {
      metric_id: mount.metric_id,
      slot_key: mount.slot_key,
      owner_resource_id: mount.owner_resource_id,
      payload_ref: mount.payload_ref,
      data_mode: mount.data_mode,
      state: mount.state,
      client_eligible: mount.client_eligible,
    };
  }

  function propsWithPreviewScope(props, scopeKey) {
    const scope = String(scopeKey || "").trim();
    if (!scope) return props || {};
    const next = { ...(props || {}) };
    next._mei = {
      ...(next._mei && typeof next._mei === "object" && !Array.isArray(next._mei)
        ? next._mei
        : {}),
      preview_scope: scope,
    };
    return next;
  }

  function placeholderMountProps(mount, scopeKey) {
    const rawProps = mount?.props && typeof mount.props === "object" ? mount.props : {};
    const role = String(rawProps.metric_role || rawProps.metricRole || "").trim();
    const placeholder = boot.devEvalPlaceholderProps?.(mount) || {
      content: "--",
      text: "--",
      value: "--",
      "data-mei-dev-eval-placeholder": "1",
    };
    return propsWithPreviewScope(
      {
        ...(role ? { metric_role: role } : {}),
        ...placeholder,
      },
      scopeKey,
    );
  }

  function isScalarMetricLeafMount(mount) {
    const useKey = String(mount?.use_key || "").trim();
    if (useKey === "mei.text") return true;
    if (useKey === "metric-card") {
      const role = String(mount?.props?.metric_role || mount?.props?.metricRole || "").trim();
      return Boolean(role);
    }
    return false;
  }

  function enrichComponentMetricRefs(value, scopeKey, parentHint = null) {
    if (!value || typeof value !== "object") return value;
    if (Array.isArray(value)) {
      return value.map((entry) => enrichComponentMetricRefs(entry, scopeKey, parentHint));
    }
    const refKind = String(value.__ref || "").trim().toLowerCase();
    if (refKind === "metric" || refKind === "metric_ref") {
      const metricId = String(
        value.id ||
          value.metric_id ||
          value.__args?.arg0 ||
          value.__args?.[0] ||
          "",
      ).trim();
      // For table row drilldown, prefer popup.params.rowset_dataset_id when present
      // on the parent props object (passed as parentHint).
      const preferredRowset = String(
        parentHint?.popup?.params?.rowset_dataset_id ||
          parentHint?.popup?.params?.rowsetDatasetId ||
          parentHint?.row_drilldown_popup?.params?.rowset_dataset_id ||
          "",
      ).trim();
      const datasetId = String(
        preferredRowset ||
          value.from_dataset ||
          value.dataset_id ||
          value.__args?.from_dataset ||
          "",
      ).trim();
      if (metricId && datasetId) {
        const runtimeRef = {
          kind: "metric",
          metric_id: metricId,
          dataset_id: datasetId,
          scene_id: String(
            parentHint?.popup?.scene_id ||
              global.__mei?.bootstrap_seed?.scope ||
              global.__mei?.bootstrap_scope ||
              "home",
          ),
          scene_path: String(parentHint?.popup?.scene_file || "").trim() || undefined,
        };
        return { ...value, __mei_runtime_ref: runtimeRef };
      }
    }
    const next = {};
    for (const [key, entry] of Object.entries(value)) {
      // Pass the object itself as parentHint so nested drilldownMetric can see popup.
      next[key] = enrichComponentMetricRefs(entry, scopeKey, value);
    }
    return next;
  }

  function frozenMountProps(mount, scopeKey) {
    const rawProps = mount?.props && typeof mount.props === "object" ? mount.props : {};
    const enriched = propsWithPreviewScope(
      enrichComponentMetricRefs(enrichComposeComponentProps(rawProps), scopeKey),
      scopeKey,
    );
    const viewport = document.querySelector("[data-mei-frame-viewport]");
    const shell = document.querySelector(
      ".shell[data-app-path], .shell[data-compile-epoch], .shell[data-compile-target]",
    );
    const shellAppId = String(
      shell?.getAttribute("data-app-path") ||
        shell?.getAttribute("data-app") ||
        global.__mei?.bootstrap_app_id ||
        global.__mei?.app_id ||
        global.__meiRuntimeAppId ||
        "",
    ).trim();
    enriched._mei = {
      ...(typeof enriched._mei === "object" && !Array.isArray(enriched._mei) ? enriched._mei : {}),
      ...(shellAppId ? { app_id: shellAppId } : {}),
      runtime_capabilities: readHostRuntimeCapabilitiesForCompose(),
      active_scene_id: String(
        global.__mei?.bootstrap_seed?.scope || global.__mei?.bootstrap_scope || "home",
      ),
      active_target_file:
        viewport?.getAttribute("data-target-file") ||
        shell?.getAttribute("data-compile-target") ||
        "src/scene/home.mei",
      entry_target:
        viewport?.getAttribute("data-target-file") ||
        shell?.getAttribute("data-compile-target") ||
        "src/scene/home.mei",
      compile_epoch: shell?.getAttribute("data-compile-epoch") || undefined,
    };
    enriched["data-mei-dev-eval-placeholder"] = "1";
    return enriched;
  }

  function mountPropsForEval(mount, scopeKey, sceneMount, allowMetric) {
    const rawProps =
      mount?.props && typeof mount.props === "object" && Object.keys(mount.props).length
        ? mount.props
        : enrichComposeComponentProps(propsFromMount(mount));
    const propsMount = { ...(mount || {}), props: rawProps };
    if (allowMetric) {
      // cockpit.data-table keeps metric refs on `dataset` / nested props — not
      // only `content`. enrichRuntimeMetricRef alone never injects those refs.
      const withComponentRefs = enrichComponentMetricRefs(
        enrichComposeComponentProps(rawProps),
        scopeKey,
      );
      return propsWithPreviewScope(
        enrichRuntimeMetricRef(withComponentRefs, sceneMount || mount),
        scopeKey,
      );
    }
    // Authored plain-text leaves already carry string `content`; do not replace with `--`.
    const authoredText =
      typeof rawProps.content === "string" && rawProps.content.trim().length > 0
        ? rawProps.content
        : typeof rawProps.text === "string" && rawProps.text.trim().length > 0
          ? rawProps.text
          : "";
    if (authoredText && String(mount?.use_key || "").trim() === "mei.text") {
      const normalized = {
        ...rawProps,
        content:
          typeof rawProps.content === "string"
            ? rawProps.content
                .replace(/\\n/g, "\n")
                .replace(/\\t/g, "\t")
                .replace(/\\r/g, "\r")
            : rawProps.content,
        text:
          typeof rawProps.text === "string"
            ? rawProps.text
                .replace(/\\n/g, "\n")
                .replace(/\\t/g, "\t")
                .replace(/\\r/g, "\r")
            : rawProps.text,
      };
      return frozenMountProps({ ...(mount || {}), props: normalized }, scopeKey);
    }
    // Static metric cards bake `{label,value,unit}` into content; keep them even when
    // allowMetric is false (dev_eval static / scoped), instead of placeholder `--`.
    const authoredMetric =
      rawProps.content &&
      typeof rawProps.content === "object" &&
      !Array.isArray(rawProps.content) &&
      !rawProps.content.__mei_runtime_ref &&
      !rawProps.content.shape &&
      (rawProps.content.label != null ||
        rawProps.content.value != null ||
        rawProps.content.unit != null ||
        rawProps.content.desc != null);
    if (authoredMetric && String(mount?.use_key || "").trim() === "mei.text") {
      return frozenMountProps(propsMount, scopeKey);
    }
    if (isScalarMetricLeafMount(propsMount)) {
      return placeholderMountProps(propsMount, scopeKey);
    }
    return frozenMountProps(propsMount, scopeKey);
  }


  function isUnresolvedMeiRef(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    return "__var" in value || "__member" in value || "__call" in value;
  }

  function resolveComposePropRef(value) {
    if (!isUnresolvedMeiRef(value)) return value;
    if (value.__call === "shared_ref") {
      const fallback = value.__args?.arg1 ?? value.__args?.[1];
      if (fallback != null && !isUnresolvedMeiRef(fallback)) {
        return fallback;
      }
    }
    const member = String(value.__member || "").trim();
    if (member === "FOCUS_INSET" || member === "focus_inset") {
      return null;
    }
    return value;
  }

  function enrichComposeComponentProps(props) {
    if (!props || typeof props !== "object") return props;
    if (Array.isArray(props)) {
      return props.map((entry) => enrichComposeComponentProps(entry));
    }
    const next = {};
    for (const [key, value] of Object.entries(props)) {
      if (value && typeof value === "object") {
        if (isUnresolvedMeiRef(value)) {
          next[key] = resolveComposePropRef(value);
        } else {
          next[key] = enrichComposeComponentProps(value);
        }
      } else {
        next[key] = value;
      }
    }
    if (next.mapViewport && typeof next.mapViewport === "object") {
      const viewport = enrichComposeComponentProps({ ...next.mapViewport });
      next.mapViewport = viewport;
      next.mapLayoutMode = next.mapLayoutMode || viewport.mode || "cockpitBleed";
    }
    if (next.mapSpec && typeof next.mapSpec === "object") {
      next.mapSpec = enrichComposeComponentProps({ ...next.mapSpec });
    }
    return next;
  }

  function refreshComposeMaps(root) {
    const scope = root instanceof HTMLElement ? root : document;
    const run = () => {
      scope.querySelectorAll("mei-map-maplibre").forEach((map) => {
        if (!(map instanceof HTMLElement)) return;
        const wrap = map.shadowRoot?.querySelector(".wrap");
        if (wrap instanceof HTMLElement && map.clientHeight > 0) {
          wrap.style.height = "100%";
          wrap.style.minHeight = "0";
        }
        try {
          map.scheduleRefresh?.({ forceRender: true });
          map.map?.resize?.();
        } catch (_) {}
      });
    };
    requestAnimationFrame(() => {
      run();
      requestAnimationFrame(run);
    });
  }

  function readHostRuntimeCapabilitiesForCompose() {
    try {
      const cached = global.__meiHostRuntimeCapabilities;
      if (cached && typeof cached === "object" && !Array.isArray(cached)) {
        return cached;
      }
      const el = document.getElementById("mei-host-runtime-capabilities");
      if (!(el instanceof HTMLElement)) return {};
      const parsed = JSON.parse(el.textContent || "{}");
      return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
    } catch (_) {
      return {};
    }
  }

  function enrichRuntimeMetricRef(props, sceneMount) {
    const next = { ...(props || {}) };
    if (next.__mei_runtime_ref) return next;
    const content =
      next.content && typeof next.content === "object" && !Array.isArray(next.content)
        ? next.content
        : null;
    const metricId = String(
      content?.id || content?.metric_id || sceneMount?.metric_id || next.metric_id || "",
    ).trim();
    const datasetId = String(
      content?.from_dataset ||
        sceneMount?.owner_resource_id ||
        next.owner_resource_id ||
        "",
    ).trim();
    if (!metricId || !datasetId) return next;
    const runtimeRef = {
      kind: "metric",
      metric_id: metricId,
      dataset_id: datasetId,
      slot_key: sceneMount?.slot_key || next.slot_key,
      payload_ref: sceneMount?.payload_ref || next.payload_ref,
      scene_id: String(
        sceneMount?.scene_id ||
          global.__mei?.bootstrap_seed?.scope ||
          global.__mei?.bootstrap_scope ||
          "home",
      ),
    };
    next.__mei_runtime_ref = runtimeRef;
    if (content) {
      next.content = { ...content, __mei_runtime_ref: runtimeRef };
    }
    const viewport = document.querySelector("[data-mei-frame-viewport]");
    const shell = document.querySelector(
      ".shell[data-app-path], .shell[data-compile-epoch], .shell[data-compile-target]",
    );
    const shellAppId = String(
      shell?.getAttribute("data-app-path") ||
        shell?.getAttribute("data-app") ||
        global.__mei?.bootstrap_app_id ||
        global.__mei?.app_id ||
        global.__meiRuntimeAppId ||
        "",
    ).trim();
    next._mei = {
      ...(typeof next._mei === "object" && !Array.isArray(next._mei) ? next._mei : {}),
      ...(shellAppId ? { app_id: shellAppId } : {}),
      runtime_capabilities: readHostRuntimeCapabilitiesForCompose(),
      active_scene_id: runtimeRef.scene_id,
      active_target_file:
        viewport?.getAttribute("data-target-file") ||
        shell?.getAttribute("data-compile-target") ||
        "src/scene/home.mei",
      entry_target:
        viewport?.getAttribute("data-target-file") ||
        shell?.getAttribute("data-compile-target") ||
        "src/scene/home.mei",
      compile_epoch: shell?.getAttribute("data-compile-epoch") || undefined,
    };
    return next;
  }

  function sceneMountsByMetricId(evalDocs) {
    const map = new Map();
    for (const doc of evalDocs || []) {
      const entry = doc.slots?.["scene:default"];
      for (const mount of entry?.mounts || []) {
        const id = String(mount?.metric_id || "").trim();
        if (id) map.set(id, mount);
      }
    }
    return map;
  }

  function buildComposeDomIndex(root) {
    const byScope = new Map();
    const byPanel = new Map();
    if (!(root instanceof HTMLElement)) return { byScope, byPanel };
    if (root.hasAttribute("data-preview-scope")) {
      byScope.set(String(root.getAttribute("data-preview-scope") || ""), root);
    }
    root.querySelectorAll("[data-preview-scope], [data-mei-panel-id]").forEach((node) => {
      if (!(node instanceof HTMLElement)) return;
      const scope = String(node.getAttribute("data-preview-scope") || "").trim();
      const panel = String(node.getAttribute("data-mei-panel-id") || "").trim();
      if (scope && !byScope.has(scope)) byScope.set(scope, node);
      if (panel && !byPanel.has(panel)) byPanel.set(panel, node);
    });
    return { byScope, byPanel };
  }

  function findScopeContainer(root, scopeKey, index) {
    const scope = String(scopeKey || "").trim();
    if (!scope || scope === "scene:default") {
      return root;
    }
    for (const candidate of scopeLookupCandidates(scope)) {
      const indexed = index?.byScope?.get(candidate) || index?.byPanel?.get(candidate);
      if (indexed instanceof HTMLElement) return indexed;
      const el =
        root.querySelector(`[data-preview-scope="${CSS.escape(candidate)}"]`) ||
        root.querySelector(`[data-mei-panel-id="${CSS.escape(candidate)}"]`);
      if (el instanceof HTMLElement) {
        return el;
      }
    }
    return null;
  }

  function resolveEvalSlotContainer(root, scopeKey, index) {
    let container = findScopeContainer(root, scopeKey, index);
    if (container instanceof HTMLElement) {
      return container;
    }
    const scope = String(scopeKey || "").trim();
    if (isSectionHeadMeiTextScope(scope)) {
      return findScopeContainer(root, scope.replace(/\/mei\.text$/, ""), index);
    }
    return null;
  }

  function promoteSectionHeadMeiTextNodes(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(
        '[data-preview-scope*="/title_zone/mei.text"], [data-preview-scope*="/head/mei.text"], [data-preview-scope$="/title/mei.text"], [data-preview-scope$="/title/title"]',
      )
      .forEach((node) => {
        if (!(node instanceof HTMLElement)) return;
        const scope = String(node.getAttribute("data-preview-scope") || "").trim();
        if (!isSectionHeadMeiTextScope(scope)) return;
        const title = String(
          node.getAttribute("data-mei-eval-label") ||
            node.getAttribute("data-mei-structure-label") ||
            "",
        ).trim();
        if (isIgnoredSectionHeadLabel(title)) return;
        const headSection = document.createElement("section");
        headSection.className =
          "mei-compose-slot preview-card preview-card-bare mei-compose-section-head";
        headSection.setAttribute("data-preview-scope", scope.replace(/\/mei\.text$/, ""));
        headSection.setAttribute("data-mei-eval-label", title);
        headSection.innerHTML = buildSectionHeadMarkup(title);
        headSection.setAttribute("data-mei-section-head-normalized", "1");
        node.replaceWith(headSection);
      });
  }

  function applyRailHeadTitlesFromEval(root, evalDocs) {
    if (!(root instanceof HTMLElement)) return;
    for (const doc of evalDocs || []) {
      for (const [scopeKey, entry] of Object.entries(doc.slots || {})) {
        if (!isSectionHeadMeiTextScope(scopeKey)) continue;
        if (entry?.head_chrome && typeof entry.head_chrome === "object") continue;
        const label = resolveEvalSlotLabel(entry);
        if (!label) continue;
        const headScope = String(scopeKey).replace(/\/mei\.text$/, "");
        const headEl = findScopeContainer(root, headScope);
        if (!(headEl instanceof HTMLElement)) continue;
        if (headEl.getAttribute("data-mei-section-head-chrome") === "1") continue;
        applyHeadSlotLabel(headEl, label);
        normalizeSectionHeadSlot(headEl);
      }
    }
  }

  function runtimeAssetVersion() {
    const accessScript = [...document.scripts].find((script) =>
      String(script.src || "").includes("/app-bundles/access.js"),
    );
    if (!accessScript?.src) return "";
    try {
      return new URL(accessScript.src, document.baseURI).searchParams.get("v") || "";
    } catch {
      return "";
    }
  }

  function versionWorkspaceBackgroundImage(raw) {
    const image = String(raw || "").trim();
    const version = runtimeAssetVersion();
    if (!image || !version || !image.includes("/workspace-app-assets/")) return image;
    return image.replace(
      /url\((["']?)(\/workspace-app-assets\/[^"')]+)\1\)/g,
      (match, quote, assetPath) => {
        if (/[?&]v=/.test(assetPath)) return match;
        const separator = assetPath.includes("?") ? "&" : "?";
        return `url(${quote}${assetPath}${separator}v=${encodeURIComponent(version)}${quote})`;
      },
    );
  }

  function normalizeBackgroundImageValue(raw) {
    const image = String(raw || "").trim();
    if (!image) return "";
    if (
      image.startsWith("linear-gradient") ||
      image.startsWith("radial-gradient") ||
      image.startsWith("repeating-linear-gradient") ||
      image.startsWith("repeating-radial-gradient")
    ) {
      return image;
    }
    if (image.startsWith("url(")) {
      return versionWorkspaceBackgroundImage(image);
    }
    return versionWorkspaceBackgroundImage(`url("${image.replace(/"/g, '\\"')}")`);
  }

  function normalizeBackgroundLayerList(raw) {
    if (Array.isArray(raw)) {
      return raw
        .map((item) => String(item || "").trim())
        .filter(Boolean);
    }
    const text = String(raw || "").trim();
    return text ? [text] : [];
  }

  function applyBackgroundInlineStyle(style, background) {
    if (!style || background == null) return;
    if (typeof background === "string") {
      const value = String(background).trim();
      if (!value) return;
      // Multi-layer shorthand (e.g. corner L-decor + fill color) must use `background`.
      if (value.includes(",") && /linear-gradient|radial-gradient|url\(/i.test(value)) {
        style.background = versionWorkspaceBackgroundImage(value);
        return;
      }
      if (
        value.startsWith("linear-gradient") ||
        value.startsWith("radial-gradient") ||
        value.startsWith("repeating-linear-gradient") ||
        value.startsWith("repeating-radial-gradient")
      ) {
        style.backgroundImage = value;
      } else if (value.startsWith("url(")) {
        style.backgroundImage = normalizeBackgroundImageValue(value);
      } else {
        style.background = value;
      }
      return;
    }
    if (typeof background !== "object") return;
    const images = normalizeBackgroundLayerList(background.image).map((image) =>
      normalizeBackgroundImageValue(image),
    );
    const color = String(background.color || "").trim();
    if (images.length > 0) {
      // Keep fill color under icon / slot-fill layers (status-flow shells).
      // Without an explicit color, clear `.preview-card` default wash so SVG
      // skins (e.g. metric-bg-clean) remain visible.
      style.backgroundColor = color || "transparent";
      style.backgroundImage = images.join(", ");
      const sizes = normalizeBackgroundLayerList(background.size);
      if (sizes.length) style.backgroundSize = sizes.join(", ");
      const positions = normalizeBackgroundLayerList(background.position);
      if (positions.length) style.backgroundPosition = positions.join(", ");
      const repeats = normalizeBackgroundLayerList(background.repeat);
      if (repeats.length) style.backgroundRepeat = repeats.join(", ");
      return;
    }
    if (color) style.background = color;
  }

  function applyContainerVisualStyle(el, props) {
    if (!(el instanceof HTMLElement) || !props || typeof props !== "object") return;
    const style = el.style;
    applyBackgroundInlineStyle(style, props.background);
    if (
      props.__mei_layout_fill === true ||
      String(props.__mei_layout_fill || "").trim() === "true" ||
      String(props.__mei_layout_fill || "").trim() === "1"
    ) {
      // Fill-down is a compiled layout contract, not an app/scope heuristic.
      // Preserve it in DOM so the common layout layer can enforce the complete
      // slot -> content stretch chain.
      el.setAttribute("data-mei-layout-fill", "true");
    }
    const stringKeys = [
      ["padding", "padding"],
      ["margin", "margin"],
      ["border", "border"],
      ["radius", "border-radius"],
      ["box_shadow", "box-shadow"],
      ["overflow", "overflow"],
      ["min_height", "min-height"],
      ["height", "height"],
      ["width", "width"],
      ["position", "position"],
      ["top", "top"],
      ["left", "left"],
      ["right", "right"],
      ["bottom", "bottom"],
      ["max_width", "max-width"],
      ["min_width", "min-width"],
      ["box_sizing", "box-sizing"],
      ["justify_self", "justify-self"],
      ["align_self", "align-self"],
    ];
    for (const [propKey, cssName] of stringKeys) {
      const raw = props[propKey];
      if (typeof raw === "string" && raw.trim()) {
        style.setProperty(cssName, raw.trim());
      }
    }
    const zIndex = props.z_index ?? props["z-index"];
    if (typeof zIndex === "number" && Number.isFinite(zIndex)) {
      style.zIndex = String(zIndex);
    } else if (typeof zIndex === "string" && zIndex.trim()) {
      style.zIndex = zIndex.trim();
    }
    const pointerEvents = props.pointer_events ?? props["pointer-events"];
    if (typeof pointerEvents === "string" && pointerEvents.trim()) {
      const pe = pointerEvents.trim();
      style.pointerEvents = pe;
      // Author-declared hit target; normalizeT1 must not overwrite with scope heuristics.
      if (el instanceof HTMLElement) {
        el.setAttribute("data-mei-pointer-events", pe);
      }
    }
    if (props.__mei_metric_template != null) {
      el.setAttribute("data-mei-metric-template", String(props.__mei_metric_template));
    }
    if (props.__mei_metric_density != null) {
      el.setAttribute("data-mei-metric-density", String(props.__mei_metric_density));
    }
    if (props.__mei_metric_inline_align != null) {
      el.setAttribute(
        "data-mei-metric-inline-align",
        String(props.__mei_metric_inline_align),
      );
    }
    const descModeRaw = props.__mei_metric_desc_mode ?? props.metric_desc_mode;
    if (descModeRaw != null && String(descModeRaw).trim()) {
      el.setAttribute("data-mei-metric-desc-mode", String(descModeRaw).trim());
    }
    if (
      props.__mei_slot_frame_bg === true ||
      String(props.__mei_slot_frame_bg || "").trim() === "true" ||
      String(props.__mei_slot_frame_bg || "").trim() === "1"
    ) {
      el.setAttribute("data-mei-slot-frame-bg", "true");
      // Slot chrome owns the whole allocated slot by definition. Treat this
      // semantic marker as fill even for legacy macros that predate
      // `__mei_layout_fill`.
      el.setAttribute("data-mei-layout-fill", "true");
      const forceStretch =
        props.__mei_slot_bg_stretch === true ||
        String(props.__mei_slot_bg_stretch || "").trim() === "true" ||
        String(props.__mei_slot_bg_stretch || "").trim() === "1";
      if (forceStretch || slotFrameBackgroundNeedsStretch(props.background)) {
        el.setAttribute("data-mei-slot-bg-stretch", "true");
      } else {
        el.removeAttribute("data-mei-slot-bg-stretch");
      }
    }
  }

  /** SVG metric-bg skins stretch to the card; layered corner/icon stacks do not. */
  function slotFrameBackgroundNeedsStretch(background) {
    if (background == null) return false;
    if (typeof background === "string") {
      return /metric-bg-|url\(/i.test(background) && !/,/.test(background);
    }
    if (typeof background !== "object") return false;
    const images = normalizeBackgroundLayerList(background.image);
    if (images.length !== 1) return false;
    const sizes = normalizeBackgroundLayerList(background.size);
    if (sizes.length && sizes.some((size) => size !== "100% 100%")) return false;
    return /metric-bg-|url\(/i.test(images[0]);
  }

  function resolveMetricCardSection(container) {
    if (!(container instanceof HTMLElement)) return null;
    if (container.hasAttribute("data-mei-metric-card")) return container;
    const nested = container.querySelector("[data-mei-metric-card]");
    return nested instanceof HTMLElement ? nested : container.closest("[data-mei-metric-card]");
  }

  function ensureMetricCardComponentHost(container) {
    const section = resolveMetricCardSection(container);
    if (!(section instanceof HTMLElement)) return null;
    const bodyCell = resolveMetricCardBodyCell(section);
    if (!(bodyCell instanceof HTMLElement)) return null;
    let host = bodyCell.querySelector(":scope > .component-host");
    if (!(host instanceof HTMLElement)) {
      host = bodyCell.querySelector(".component-host");
    }
    if (!(host instanceof HTMLElement)) {
      host = document.createElement("div");
      host.className = "component-host metric-card";
      bodyCell.appendChild(host);
    }
    return host;
  }

  function propagateMetricCardPopupFromMounts(container, mounts) {
    const valueMount = (mounts || []).find((mount) => {
      const props = mount?.props && typeof mount.props === "object" ? mount.props : {};
      return (
        String(mount?.use_key || "").trim() === "mei.text" &&
        String(props.metric_role || props.metricRole || "").trim() === "value" &&
        props.popup &&
        typeof props.popup === "object"
      );
    });
    const shellMount = (mounts || []).find(
      (mount) =>
        String(mount?.use_key || "").trim() === "metric-card" &&
        String(mount?.mount_role || "").trim() === "shell",
    );
    const popup = valueMount?.props?.popup || shellMount?.props?.popup;
    if (!popup || typeof popup !== "object") return;
    const section = resolveMetricCardSection(container);
    if (!(section instanceof HTMLElement)) return;
    section.querySelectorAll('[data-metric-role="value"], mei-text[data-metric-role="value"]').forEach(
      (node) => {
        if (!(node instanceof HTMLElement)) return;
        const props = parseHostProps(node);
        if (props.popup) return;
        applyPropsToHost(node, { ...props, popup });
      },
    );
  }

  function applyMetricCardShellFromMounts(container, mounts) {
    const shellMount = (mounts || []).find(
      (mount) =>
        String(mount?.use_key || "").trim() === "metric-card" &&
        String(mount?.mount_role || "").trim() === "shell",
    );
    if (!shellMount?.props) return;
    const section = resolveMetricCardSection(container);
    if (section instanceof HTMLElement) {
      const shellProps = { ...shellMount.props };
      // Nested metric cards inside a compound host share the compound frame.
      const insideCompound =
        section.closest?.('[data-mei-content-kind="compound-metric"]') &&
        section.getAttribute("data-mei-content-kind") !== "compound-metric";
      if (insideCompound) {
        shellProps.background = "transparent";
        delete shellProps.__mei_slot_frame_bg;
      } else if (isTransparentBackgroundProp(shellProps.background)) {
        // Inner metric() shells are authored transparent; the outer
        // slot_metric_shell owns the visible frame via panel_shell. Do not
        // paint transparent over that frame.
        delete shellProps.background;
        delete shellProps.__mei_slot_frame_bg;
      }
      if (String(shellProps.chrome || "").trim() === "bare") {
        section.classList.add("preview-card-bare");
      }
      if (shellProps.__mei_metric_template != null) {
        section.setAttribute(
          "data-mei-metric-template",
          String(shellProps.__mei_metric_template),
        );
      }
      if (shellProps.__mei_metric_title_ratio != null) {
        section.setAttribute(
          "data-mei-metric-title-ratio",
          String(shellProps.__mei_metric_title_ratio),
        );
      }
      if (shellProps.__mei_metric_content_ratio != null) {
        section.setAttribute(
          "data-mei-metric-content-ratio",
          String(shellProps.__mei_metric_content_ratio),
        );
      }
      applyContainerVisualStyle(section, shellProps);
    }
    propagateMetricCardPopupFromMounts(container, mounts);
  }

  function isTransparentBackgroundProp(background) {
    if (background == null) return true;
    if (typeof background === "string") {
      return background.trim().toLowerCase() === "transparent" || !background.trim();
    }
    if (typeof background === "object") {
      const color = String(background.color || "").trim().toLowerCase();
      const image = background.image;
      const hasImage = Array.isArray(image)
        ? image.some((item) => String(item || "").trim())
        : Boolean(String(image || "").trim());
      return !hasImage && (!color || color === "transparent");
    }
    return false;
  }

  function applyPropsToHost(host, props) {
    if (!(host instanceof HTMLElement)) return;
    const serialized = JSON.stringify(props || {});
    const isMetricCardHost =
      host.classList.contains("metric-card") ||
      (host.classList.contains("component-host") && host.closest("[data-mei-metric-card]")) ||
      (host.hasAttribute("data-mei-metric-card") &&
        host.querySelector(".component-host.metric-card, .component-host"));
    let target = host;
    if (host.hasAttribute("data-mei-metric-card")) {
      const cardHost = host.querySelector(".component-host.metric-card, .component-host");
      if (cardHost instanceof HTMLElement) {
        target = cardHost;
      }
    } else if (!isMetricCardHost) {
      const hostIsComponent =
        host.hasAttribute("data-mei-use-key") || host.tagName.toLowerCase().includes("-");
      target = hostIsComponent
        ? host
        : host.querySelector("[data-mei-use-key]") || host.firstElementChild || host;
    }
    if (!(target instanceof HTMLElement)) return;
    const attributeMatches = target.getAttribute("data-props") === serialized;
    let instanceMatches = false;
    if ("props" in target) {
      try {
        instanceMatches = JSON.stringify(target.props || {}) === serialized;
      } catch (_) {
        instanceMatches = false;
      }
    }
    if (
      attributeMatches &&
      (instanceMatches ||
        (typeof target._bind !== "function" && typeof target.render !== "function"))
    ) {
      return;
    }
    if (!attributeMatches) {
      target.setAttribute("data-props", serialized);
    }
    if (typeof target._bind === "function") {
      try {
        target._bind();
      } catch (_) {}
    } else if (typeof target.render === "function") {
      try {
        // Some lightweight Web Components parse props only during connect and
        // expose render() without observing data-props. Keep their instance
        // state in sync when eval layers settle after the structure mount.
        target.props = props || {};
        target.render(props || {});
      } catch (_) {}
    }
  }

  function ensureComponentHostChildren(host, mounts, sceneMountByMetric, scopeKey, allowEval = true) {
    if (!(host instanceof HTMLElement)) return 0;
    let applied = 0;
    for (const mount of mounts || []) {
      const useKey = String(mount?.use_key || "").trim();
      if (!useKey) continue;
      if (
        useKey === "metric-card" &&
        String(mount?.mount_role || "").trim() === "shell"
      ) {
        continue;
      }
      const rawProps = mount?.props && typeof mount.props === "object" ? mount.props : {};
      const metricId = String(rawProps?.content?.id || rawProps?.content?.metric_id || "").trim();
      const sceneMount = metricId ? sceneMountByMetric?.get(metricId) : null;
      const allowMetric =
        allowEval &&
        (!metricId ||
          typeof boot.devEvalAllowsMetric !== "function" ||
          boot.devEvalAllowsMetric(metricId, scopeKey));
      const props = mountPropsForEval(mount, scopeKey, sceneMount, allowMetric);
      const metricRole = String(props.metric_role || props.metricRole || "").trim();
      const tag = resolveComponentTag(useKey);
      const instanceId = String(mount?.block_id || "").trim();
    // Authored plain-text leaves (`…/area/mei.text`) carry string content and
    // must not be dropped — metric_role is only required inside metric cards.
    if (
      !metricRole &&
      useKey === "mei.text" &&
      host.closest?.('[data-mei-metric-card="true"]')
    ) {
      continue;
    }
      let selector = `[data-mei-use-key="${CSS.escape(useKey)}"]`;
      if (metricRole) {
        selector += `[data-metric-role="${CSS.escape(metricRole)}"]`;
      }
      if (instanceId) {
        selector += `[data-mei-instance-id="${CSS.escape(instanceId)}"]`;
      }
      let target = host.querySelector(selector);
      if (!(target instanceof HTMLElement) && instanceId) {
        let unclaimedSelector = `[data-mei-use-key="${CSS.escape(useKey)}"]:not([data-mei-instance-id])`;
        if (metricRole) {
          unclaimedSelector += `[data-metric-role="${CSS.escape(metricRole)}"]`;
        }
        const unclaimed = host.querySelector(unclaimedSelector);
        if (unclaimed instanceof HTMLElement) {
          unclaimed.setAttribute("data-mei-instance-id", instanceId);
          target = unclaimed;
        }
      }
      if (!(target instanceof HTMLElement) && tag) {
        target = document.createElement(tag);
        target.setAttribute("data-mei-use-key", useKey);
        if (instanceId) {
          target.setAttribute("data-mei-instance-id", instanceId);
        }
        if (metricRole) {
          target.setAttribute("data-metric-role", metricRole);
        }
        target.setAttribute("data-props", JSON.stringify(props || {}));
        host.appendChild(target);
        if (typeof target._bind === "function") {
          try {
            target._bind();
          } catch (_) {}
        }
        applied += 1;
        continue;
      }
      if (target instanceof HTMLElement) {
        applyPropsToHost(target, props);
        applied += 1;
      }
    }
    return applied;
  }

  function findComponentHostForScope(container, scopeKey, useKeys) {
    if (!(container instanceof HTMLElement)) return null;
    const scope = String(scopeKey || "").trim();
    if (scope) {
      const scoped =
        container.matches?.(`[data-preview-scope="${CSS.escape(scope)}"]`)
          ? container
          : container.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`);
      if (scoped instanceof HTMLElement) {
        // Prefer a direct component-host under this scope, not nested metric cards.
        const direct =
          scoped.querySelector(":scope > .component-host, :scope > .panel-body-cell > .component-host") ||
          (scoped.classList?.contains("component-host") ? scoped : null);
        if (direct instanceof HTMLElement) return direct;
        const byUseKey = String((useKeys && useKeys[0]) || "").trim();
        if (byUseKey) {
          const keyed = scoped.querySelector(
            `:scope [data-mei-use-key="${CSS.escape(byUseKey)}"]`,
          );
          if (keyed instanceof HTMLElement) {
            return keyed.closest(".component-host") || keyed.parentElement || keyed;
          }
        }
      }
    }
    const useKey = String((useKeys && useKeys[0]) || "").trim();
    if (useKey) {
      const byKey =
        container.querySelector(`[data-mei-use-key="${CSS.escape(useKey)}"] .component-host`) ||
        container.querySelector(`[data-mei-use-key="${CSS.escape(useKey)}"]`);
      if (byKey instanceof HTMLElement) {
        return byKey.classList?.contains("component-host")
          ? byKey
          : byKey.querySelector(".component-host") || byKey;
      }
    }
    return container.querySelector(":scope > .component-host") || null;
  }

  function cleanupComposeStructureArtifacts(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll("[data-mei-compose-scene-viewport]").forEach((el) => el.remove());
    root.querySelectorAll(".mei-structure-tree").forEach((el) => el.remove());
  }

  function tierZIndexForPlane(planeCode) {
    const tier = String(planeCode || "").trim().toLowerCase();
    const entries = global.__mei?.layer_plan?.tiers?.[tier];
    if (Array.isArray(entries) && entries.length) {
      const maxZ = entries.reduce((acc, entry) => {
        const z = Number(entry?.zIndex ?? entry?.z_index ?? 0);
        return Number.isFinite(z) ? Math.max(acc, z) : acc;
      }, 0);
      if (maxZ > 0) return maxZ;
    }
    if (tier === "t0") return 1;
    if (tier === "t1") return 1000;
    if (tier === "t2") return 2000;
    return 0;
  }

  function normalizeStagePanelId(value) {
    return String(value || "")
      .trim()
      .toLowerCase()
      .replaceAll("-", "_");
  }

  function stagePlanEntries() {
    const tiers = global.__mei?.layer_plan?.tiers;
    if (!tiers || typeof tiers !== "object") return [];
    return Object.values(tiers)
      .flatMap((entries) => (Array.isArray(entries) ? entries : []))
      .filter((entry) => {
        const stageKind = String(entry?.stageKind || entry?.stage_kind || "").trim();
        const panelId = String(entry?.panelId || entry?.panel_id || "").trim();
        return Boolean(stageKind && panelId);
      })
      .sort((left, right) => {
        const leftId = normalizeStagePanelId(left?.panelId || left?.panel_id);
        const rightId = normalizeStagePanelId(right?.panelId || right?.panel_id);
        return rightId.length - leftId.length;
      });
  }

  function applyStagePlanMetadata(root) {
    if (!(root instanceof HTMLElement)) return;
    const entries = stagePlanEntries();
    if (!entries.length) return;
    root.querySelectorAll("[data-preview-scope]").forEach((el) => {
      if (!(el instanceof HTMLElement)) return;
      const scope = normalizeStagePanelId(el.getAttribute("data-preview-scope"));
      const panelId = normalizeStagePanelId(el.getAttribute("data-mei-panel-id"));
      const label = normalizeStagePanelId(el.getAttribute("data-mei-structure-label"));
      const candidates = [scope, panelId, label].filter(Boolean);
      const entry = entries.find((item) => {
        const id = normalizeStagePanelId(item?.panelId || item?.panel_id);
        return id && candidates.some((candidate) => candidate.includes(id));
      });
      if (!entry) return;
      const stageKind = String(entry.stageKind || entry.stage_kind || "").trim();
      const viewFamily = String(entry.viewFamily || entry.view_family || "").trim();
      if (stageKind) el.setAttribute("data-mei-stage-kind", stageKind);
      if (viewFamily) el.setAttribute("data-mei-view-family", viewFamily);
      el.setAttribute(
        "data-mei-stage-panel-id",
        String(entry.panelId || entry.panel_id || "").trim(),
      );
    });
  }

  function isLayoutUnit(el) {
    if (!(el instanceof HTMLElement)) return false;
    const role = String(el.getAttribute("data-mei-ui-role") || "").trim().toLowerCase();
    return (
      role === "region" ||
      role === "section" ||
      el.classList.contains("mei-compose-region") ||
      el.classList.contains("mei-compose-section")
    );
  }

  function layoutUnitsFor(container) {
    if (!(container instanceof HTMLElement)) return [];
    return [...container.children].filter((child) => isLayoutUnit(child));
  }

  function resolveRegionGridArea(scope) {
    const normalized = String(scope || "").trim().toLowerCase();
    if (!normalized) return "";
    const leaf = normalized.split("/").pop() || normalized;
    if (
      normalized.includes("home_header") ||
      normalized.includes("header_region") ||
      normalized.includes("region-header") ||
      leaf === "header" ||
      normalized.endsWith("/header") ||
      normalized === "header"
    ) {
      return "header";
    }
    if (normalized.includes("left_rail")) return "left_rail";
    if (
      normalized.includes("center_rail") ||
      normalized.includes("center_top") ||
      normalized.includes("realtime_center")
    ) {
      return "center_rail";
    }
    if (normalized.includes("right_rail")) return "right_rail";
    if (normalized.includes("map_stage")) return "map_stage";
    return "";
  }

  function isLayoutDebugScope(scope) {
    return String(scope || "").trim().toLowerCase().includes("layout_debug");
  }

  function isOverlayRegionScope(scope) {
    const normalized = String(scope || "").trim().toLowerCase();
    return (
      normalized.includes("stage_aperture") ||
      normalized.includes("viewport_frame") ||
      normalized.includes("viewport_canvas") ||
      normalized.includes("world_viewport")
    );
  }

  /**
   * Legacy scope→grid heuristic. Normal path must use compiled/SSR grid
   * templates; this only runs when the contract is missing (compat/diagnose).
   */
  function inferT1PlaneGrid(regionScopes) {
    const scopes = regionScopes.map((scope) => String(scope || "").toLowerCase());
    const hasLeft = scopes.some((scope) => scope.includes("left_rail"));
    const hasCenter = scopes.some(
      (scope) =>
        scope.includes("center_rail") ||
        scope.includes("center_top") ||
        scope.includes("realtime_center"),
    );
    const hasRight = scopes.some((scope) => scope.includes("right_rail"));
    const hasMapStage = scopes.some((scope) => scope.includes("map_stage"));
    if (hasLeft && hasCenter && hasRight) {
      return {
        rows: "72px 1fr",
        columns: "2fr 3fr 2fr",
        areas: '"header header header" "left_rail center_rail right_rail"',
        overlayArea: "center_rail",
      };
    }
    if (hasMapStage && hasRight) {
      return {
        rows: "72px 1fr",
        columns: "1fr 520px",
        areas: '"header header" "map_stage right_rail"',
        overlayArea: "map_stage",
      };
    }
    return null;
  }

  function hasAuthoredGridContract(el) {
    if (!(el instanceof HTMLElement)) return false;
    const rows = String(el.style.gridTemplateRows || "").trim();
    const cols = String(el.style.gridTemplateColumns || "").trim();
    const areas = String(el.style.gridTemplateAreas || "").trim();
    const template = String(el.style.gridTemplate || "").trim();
    return Boolean(rows || cols || areas || template);
  }

  /** 仅当 layout_budget 已声明 plane 网格时才告警；当前产物无 plane 条目时 inference 是正常路径。 */
  function expectsCompiledT1Grid(planeEl) {
    if (!(planeEl instanceof HTMLElement)) return false;
    const manifest = global.__mei?.layout_budget_manifest?.entries;
    if (!manifest || typeof manifest !== "object") return false;
    const keys = [
      String(planeEl.getAttribute("data-preview-scope") || "").trim(),
      String(
        planeEl.getAttribute("data-mei-plane") || planeEl.getAttribute("data-mei-tier") || "",
      )
        .trim()
        .toLowerCase(),
    ].filter(Boolean);
    for (const key of keys) {
      const entry = manifest[key];
      if (!entry || typeof entry !== "object") continue;
      if (
        entry.grid_template_rows ||
        entry.gridTemplateRows ||
        entry.grid_template_columns ||
        entry.gridTemplateColumns ||
        entry.grid_template_areas ||
        entry.gridTemplateAreas
      ) {
        return true;
      }
    }
    return false;
  }

  function warnMissingCompiledT1Grid(scopes) {
    if (typeof console === "undefined" || !console.warn) return;
    console.warn(
      "[mei-layout] missing compiled T1 grid; falling back to scope inference",
      scopes,
    );
  }

  function applyT1GridLayout(container, units, grid) {
    if (!(container instanceof HTMLElement) || !units.length || !grid) return;
    container.style.display = "grid";
    container.style.width = "100%";
    container.style.height = "100%";
    container.style.minHeight = "0";
    // Prefer compiled/SSR contract; never overwrite authored tracks with scope inference.
    if (!String(container.style.gridTemplateRows || "").trim()) {
      container.style.gridTemplateRows = grid.rows;
    }
    if (!String(container.style.gridTemplateColumns || "").trim()) {
      container.style.gridTemplateColumns = grid.columns;
    }
    if (!String(container.style.gridTemplateAreas || "").trim()) {
      container.style.gridTemplateAreas = grid.areas;
    }

    units.forEach((unit) => {
      const scope = unit.getAttribute("data-preview-scope") || "";
      if (isOverlayRegionScope(scope)) {
        if (!String(unit.style.gridArea || "").trim()) {
          unit.style.gridArea = grid.overlayArea;
        }
        unit.style.position = "relative";
        unit.style.pointerEvents = "none";
        unit.style.minHeight = "0";
        unit.style.minWidth = "0";
        return;
      }
      const area = resolveRegionGridArea(scope);
      if (area && !String(unit.style.gridArea || "").trim()) {
        unit.style.gridArea = area;
      }
      unit.style.minHeight = "0";
      unit.style.minWidth = "0";
    });
  }

  function resolveT1LayoutTargets(planeEl) {
    const layoutRegions = layoutUnitsFor(planeEl).filter((region) => {
      const scope = region.getAttribute("data-preview-scope") || "";
      return !isLayoutDebugScope(scope);
    });
    if (!layoutRegions.length) return null;

    // Normal path: compiled plane/region already carries grid tracks.
    if (hasAuthoredGridContract(planeEl)) {
      return { container: planeEl, units: layoutRegions, grid: null, authored: true };
    }
    if (layoutRegions.length === 1 && hasAuthoredGridContract(layoutRegions[0])) {
      const region = layoutRegions[0];
      const nested = layoutUnitsFor(region).filter((unit) => {
        const scope = unit.getAttribute("data-preview-scope") || "";
        return !isLayoutDebugScope(scope);
      });
      return {
        container: region,
        units: nested.length ? nested : layoutRegions,
        grid: null,
        authored: true,
      };
    }

    if (layoutRegions.length === 1) {
      const region = layoutRegions[0];
      const nested = layoutUnitsFor(region).filter((unit) => {
        const scope = unit.getAttribute("data-preview-scope") || "";
        return !isLayoutDebugScope(scope);
      });
      const nestedScopes = nested.map((unit) => unit.getAttribute("data-preview-scope") || "");
      const nestedGrid = inferT1PlaneGrid(nestedScopes);
      if (nestedGrid && nested.length > 1) {
        if (expectsCompiledT1Grid(planeEl)) {
          warnMissingCompiledT1Grid(nestedScopes);
        }
        return { container: region, units: nested, grid: nestedGrid, authored: false };
      }
    }

    const regionScopes = layoutRegions.map(
      (region) => region.getAttribute("data-preview-scope") || "",
    );
    const multiRegionGrid = inferT1PlaneGrid(regionScopes);
    if (multiRegionGrid && layoutRegions.length > 1) {
      if (expectsCompiledT1Grid(planeEl)) {
        warnMissingCompiledT1Grid(regionScopes);
      }
      return { container: planeEl, units: layoutRegions, grid: multiRegionGrid, authored: false };
    }

    if (layoutRegions.length === 1) {
      const region = layoutRegions[0];
      const sections = layoutUnitsFor(region).filter((unit) => {
        const scope = unit.getAttribute("data-preview-scope") || "";
        return !isLayoutDebugScope(scope);
      });
      const sectionScopes = sections.map(
        (section) => section.getAttribute("data-preview-scope") || "",
      );
      const sectionGrid = inferT1PlaneGrid(sectionScopes);
      if (sectionGrid && sections.length > 1) {
        if (expectsCompiledT1Grid(planeEl)) {
          warnMissingCompiledT1Grid(sectionScopes);
        }
        return { container: region, units: sections, grid: sectionGrid, authored: false };
      }
    }

    if (multiRegionGrid) {
      return { container: planeEl, units: layoutRegions, grid: multiRegionGrid, authored: false };
    }
    return null;
  }

  function applyPlaneRegionLayout(planeEl) {
    if (!(planeEl instanceof HTMLElement)) return;
    const plane = String(
      planeEl.getAttribute("data-mei-plane") || planeEl.getAttribute("data-mei-tier") || "",
    ).trim();
    const z = tierZIndexForPlane(plane);
    if (z > 0) {
      planeEl.style.zIndex = String(z);
      planeEl.style.position = "absolute";
      planeEl.style.inset = "0";
    }

    const regions = layoutUnitsFor(planeEl);

    if (plane.toUpperCase() === "T0") {
      planeEl.style.display = "grid";
      planeEl.style.width = "100%";
      planeEl.style.height = "100%";
      planeEl.style.minHeight = "0";
      if (!hasAuthoredGridContract(planeEl)) {
        planeEl.style.gridTemplate = '"stage" 1fr / 1fr';
      }
      regions.forEach((region) => {
        if (!String(region.style.gridArea || "").trim()) {
          region.style.gridArea = "stage";
        }
        region.style.width = "100%";
        region.style.height = "100%";
        region.style.minHeight = "0";
        region.style.minWidth = "0";
      });
      return;
    }

    if (plane.toUpperCase() === "T2") {
      planeEl.style.display = "grid";
      planeEl.style.width = "100%";
      planeEl.style.height = "100%";
      planeEl.style.minHeight = "0";
      if (!hasAuthoredGridContract(planeEl)) {
        planeEl.style.gridTemplate = '"main" 1fr / 1fr';
      }
      regions.forEach((region) => {
        if (!String(region.style.gridArea || "").trim()) {
          region.style.gridArea = "main";
        }
        region.style.width = "100%";
        region.style.height = "100%";
        region.style.minHeight = "0";
        region.style.minWidth = "0";
      });
      return;
    }

    if (plane.toUpperCase() !== "T1") return;

    const layout = resolveT1LayoutTargets(planeEl);
    if (!layout) return;
    planeEl.style.width = "100%";
    planeEl.style.height = "100%";
    planeEl.style.minHeight = "0";
    if (layout.container !== planeEl) {
      layout.container.style.width = "100%";
      layout.container.style.height = "100%";
      layout.container.style.minHeight = "0";
      layout.container.style.boxSizing = "border-box";
    }
    regions.forEach((region) => {
      region.style.width = "100%";
      region.style.height = "100%";
      region.style.minHeight = "0";
      region.style.minWidth = "0";
    });
    if (layout.grid) {
      applyT1GridLayout(layout.container, layout.units, layout.grid);
    } else if (layout.authored && layout.container instanceof HTMLElement) {
      // Authored tracks already on the plane (layout_budget); still must place
      // regions into named areas. home_header → header via resolveRegionGridArea.
      layout.container.style.display = "grid";
      layout.units.forEach((unit) => {
        const scope = unit.getAttribute("data-preview-scope") || "";
        if (isOverlayRegionScope(scope)) {
          if (!String(unit.style.gridArea || "").trim()) {
            unit.style.gridArea = "center_rail";
          }
        } else {
          const area = resolveRegionGridArea(scope);
          if (area && !String(unit.style.gridArea || "").trim()) {
            unit.style.gridArea = area;
          }
        }
        unit.style.minHeight = "0";
        unit.style.minWidth = "0";
      });
    }
  }

  function wrapStructureTreeInSceneViewport(root, tree, structureDoc) {
    const vpMeta = resolveSceneViewportMeta(structureDoc);
    if (!(tree instanceof HTMLElement) || !(root instanceof HTMLElement)) return false;

    if (isDocumentComposeSurface(root, structureDoc)) {
      const viewport = root.querySelector(":scope > [data-mei-compose-scene-viewport]");
      if (tree.parentElement !== root) root.appendChild(tree);
      if (viewport instanceof HTMLElement) viewport.remove();
      root.classList.remove("frame-stage-enabled", "mei-compose-frame-host", "overflow-hidden");
      root.classList.add("overflow-auto", "mei-compose-document-host");
      tree.style.width = "100%";
      tree.style.height = "auto";
      tree.style.minWidth = "0";
      tree.style.minHeight = "100%";
      tree.style.position = "relative";
      return true;
    }

    ensureComposeFrameHost(root);

    let viewport = root.querySelector(":scope > [data-mei-compose-scene-viewport]");
    if (!(viewport instanceof HTMLElement)) {
      viewport = document.createElement("section");
      viewport.className = "preview-viewport mei-compose-scene-viewport";
      viewport.setAttribute("data-mei-compose-scene-viewport", "1");
      root.appendChild(viewport);
    }

    applyFrameViewportMeta(viewport, vpMeta, vpMeta);

    let stageShell = viewport.querySelector(":scope > .preview-stage-shell");
    if (!(stageShell instanceof HTMLElement)) {
      stageShell = document.createElement("div");
      stageShell.className = "preview-stage-shell";
      viewport.appendChild(stageShell);
    }

    let stage = stageShell.querySelector(":scope > .preview-stage");
    if (!(stage instanceof HTMLElement)) {
      stage = document.createElement("section");
      stage.className = "preview-surface preview-stage";
      stage.style.position = "relative";
      stage.style.boxSizing = "border-box";
      stageShell.appendChild(stage);
    }

    if (tree.parentElement !== stage) {
      stage.appendChild(tree);
    }

    pinSceneRootToDesignViewport(tree, vpMeta);
    return true;
  }

  function normalizeDocumentComposeLayout(root) {
    if (!(root instanceof HTMLElement) || !isDocumentComposeSurface(root, null)) return;
    root.classList.remove("frame-stage-enabled", "mei-compose-frame-host", "overflow-hidden");
    root.classList.add("overflow-auto", "mei-compose-document-host");
    const tree = root.querySelector(":scope > .mei-structure-tree");
    if (!(tree instanceof HTMLElement)) return;
    tree.style.width = "100%";
    tree.style.height = "auto";
    tree.style.minHeight = "100%";
    tree.style.position = "relative";
    tree
      .querySelectorAll(
        '[data-mei-ui-role="scene"], [data-mei-ui-role="plane"], [data-mei-ui-role="region"], [data-mei-ui-role="section"], [data-mei-ui-role="slot"], [data-mei-ui-role="content"]',
      )
      .forEach((node) => {
        if (!(node instanceof HTMLElement)) return;
        node.style.position = "relative";
        node.style.inset = "auto";
        node.style.width = "100%";
        node.style.height = "auto";
        node.style.minWidth = "0";
        node.style.minHeight = "0";
        node.style.pointerEvents = "auto";
        if (node.getAttribute("data-mei-ui-role") === "plane") {
          node.style.zIndex = "auto";
        }
      });
  }

  function scopeEndsWith(scope, suffix) {
    const normalized = String(scope || "").trim().toLowerCase();
    const tail = String(suffix || "").trim().toLowerCase();
    return normalized === tail || normalized.endsWith(`/${tail}`);
  }

  function normalizeT0BasemapPlane(planeEl) {
    if (!(planeEl instanceof HTMLElement)) return;
    const plane = String(planeEl.getAttribute("data-mei-plane") || "").trim().toUpperCase();
    if (plane !== "T0") return;
    planeEl.querySelectorAll(".mei-compose-region, .mei-compose-section, .mei-compose-slot").forEach(
      (el) => {
        if (!(el instanceof HTMLElement)) return;
        el.style.width = "100%";
        el.style.height = "100%";
        el.style.minHeight = "0";
        el.style.minWidth = "0";
        el.style.position = "relative";
        // 底图链必须可点；勿沿用 T1 map_stage 的 pointer-events:none。
        el.style.pointerEvents = "auto";
      },
    );
    const mapHost =
      planeEl.querySelector("mei-map-maplibre") ||
      planeEl.querySelector("map-maplibre") ||
      planeEl.querySelector('[data-mei-use-key="map.maplibre"]');
    if (!(mapHost instanceof HTMLElement)) return;
    let el = mapHost;
    while (el && el !== planeEl) {
      if (el instanceof HTMLElement) {
        el.style.width = "100%";
        el.style.height = "100%";
        el.style.minHeight = "0";
        el.style.minWidth = "0";
        el.style.pointerEvents = "auto";
        if (el.classList.contains("component-host") || el.classList.contains("preview-card")) {
          el.style.position = "absolute";
          el.style.inset = "0";
        }
      }
      el = el.parentElement;
    }
    mapHost.style.pointerEvents = "auto";
  }

  function normalizeMapStageSection(section) {
    if (!(section instanceof HTMLElement)) return;
    const scope = String(section.getAttribute("data-preview-scope") || "");
    if (!scopeEndsWith(scope, "map_stage")) return;
    section.querySelectorAll(".mei-compose-viewport-meta[hidden]").forEach((el) => el.remove());
    const bodySlot = section.querySelector('[data-preview-scope$="/map_stage/body"]');
    if (bodySlot instanceof HTMLElement && section.querySelector('[data-preview-scope*="stage-aperture"]')) {
      bodySlot.remove();
    }
  }

  function normalizeMapOperationViewportSection(section) {
    if (!(section instanceof HTMLElement)) return;
    const scope = String(section.getAttribute("data-preview-scope") || "").toLowerCase();
    // T0 底图 map_stage 不是观察窗叠层，不要写成 pointer-events:none。
    if (scope === "t0" || scope.startsWith("t0/")) return;
    const isCenterRailViewport = scope.endsWith("/map_viewport");
    const isMapStageOverlay =
      scope.includes("map_stage_overlay") || scope.endsWith("/map_stage");
    if (!isCenterRailViewport && !isMapStageOverlay) return;

    section.querySelectorAll(".mei-compose-viewport-meta[hidden]").forEach((meta) => {
      if (!(meta instanceof HTMLElement)) return;
      const parent = meta.parentElement;
      if (!(parent instanceof HTMLElement)) return;
      while (meta.firstChild) {
        parent.insertBefore(meta.firstChild, meta);
      }
      meta.remove();
    });

    const viewportContent =
      section.querySelector('[data-preview-scope$="/map-viewport"]') ||
      section.querySelector('[data-preview-scope$="/map_viewport/map-viewport"]');
    if (viewportContent instanceof HTMLElement) {
      viewportContent
        .querySelectorAll(":scope > .panel-body-cell, :scope > .preview-card.mei-compose-block > .panel-body-cell")
        .forEach((el) => el.remove());
    }
    const layoutHost =
      viewportContent instanceof HTMLElement ? viewportContent : section;
    layoutHost.style.display = "grid";
    layoutHost.style.gridTemplateRows = "1fr auto";
    layoutHost.style.gridTemplateColumns = "1fr";
    layoutHost.style.width = "100%";
    layoutHost.style.height = "100%";
    layoutHost.style.minHeight = "0";
    layoutHost.style.gap = "8px";
    layoutHost.style.pointerEvents = "none";
    section.style.pointerEvents = "none";

    section.querySelectorAll('[data-preview-scope*="map-interaction-surface"]').forEach((el) => {
      if (!(el instanceof HTMLElement)) return;
      el.setAttribute("data-mei-panel-name", "map-interaction-surface");
      el.style.display = "grid";
      el.style.gridTemplateRows = "auto 1fr";
      el.style.gridTemplateColumns = "1fr auto";
      el.style.gridTemplateAreas = '"_ tools" "frame frame"';
      el.style.gap = "8px";
      el.style.width = "100%";
      el.style.height = "100%";
      el.style.minHeight = "0";
      el.style.pointerEvents = "none";
    });
    section
      .querySelectorAll(
        '[data-preview-scope*="stage-aperture-frame"], [data-preview-scope*="/frame"]',
      )
      .forEach((el) => {
        if (!(el instanceof HTMLElement)) return;
        if (!String(el.getAttribute("data-preview-scope") || "").includes("frame")) return;
        el.setAttribute("data-mei-panel-name", "stage-aperture-frame");
        el.style.width = "100%";
        el.style.height = "100%";
        el.style.minHeight = "0";
        el.style.boxSizing = "border-box";
        el.style.pointerEvents = "none";
        // 不回填调试黄虚线；作者需要框线时在 viewport-chrome / props 显式声明。
      });
    section.querySelectorAll('[data-preview-scope*="map-tools-slot"]').forEach((el) => {
      if (el instanceof HTMLElement) {
        el.setAttribute("data-mei-panel-name", "map-tools-slot");
        el.style.pointerEvents = "auto";
      }
    });
    section.querySelectorAll('[data-preview-scope*="aperture"]').forEach((el) => {
      if (!(el instanceof HTMLElement)) return;
      el.style.minHeight = "0";
      el.style.height = "100%";
      el.style.width = "100%";
    });
    // 观察窗底部操作提示已移除；若遗留 hint 槽则隐藏，勿再注入兜底文案。
    const hintSlot = section.querySelector(
      '[data-preview-scope$="/hint"], [data-mei-panel-name="stage-aperture-hint"]',
    );
    if (hintSlot instanceof HTMLElement) {
      hintSlot.style.display = "none";
      hintSlot.setAttribute("aria-hidden", "true");
      hintSlot.querySelectorAll(".mei-map-viewport-hint").forEach((node) => node.remove());
    }
  }

  /** region 滚动口意图：显式 overflow，或 layout_budget，或含非零 px 下界的 minmax 行轨。 */
  function resolveRailOverflowIntent(rail) {
    if (!(rail instanceof HTMLElement)) return "hidden";
    const styleOv = String(rail.style.overflow || "").trim().toLowerCase();
    if (styleOv === "auto" || styleOv === "scroll") return styleOv;
    const scope = String(rail.getAttribute("data-preview-scope") || "").trim();
    const budgetOv = String(
      global.__mei?.layout_budget_manifest?.entries?.[scope]?.overflow || "",
    )
      .trim()
      .toLowerCase();
    if (budgetOv === "auto" || budgetOv === "scroll") return budgetOv;
    const rows = String(
      rail.style.gridTemplateRows || rail.dataset.manifestGridRows || "",
    ).trim();
    // minmax(220px, 1fr) 等：min 总和可超过可视高，必须 auto 才能出现滚动条。
    if (/minmax\(\s*\d+(?:\.\d+)?px/i.test(rows)) return "auto";
    return "hidden";
  }

  function applyRailRegionSectionLayouts(root) {
    if (!(root instanceof HTMLElement)) return;
    const tree = root.querySelector(".mei-structure-tree");
    if (!(tree instanceof HTMLElement)) return;
    const railScopes = ["t1/left_rail", "t1/center_rail", "t1/right_rail"];
    railScopes.forEach((scope) => {
      const rail = tree.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`);
      if (!(rail instanceof HTMLElement)) return;
      const sections = layoutUnitsFor(rail).filter((unit) => {
        const role = String(unit.getAttribute("data-mei-ui-role") || "").toLowerCase();
        return role === "section";
      });
      if (sections.length < 2) return;
      // 仅在作者/SSR 未声明行轨时回退均分；勿覆盖已有 Nfr（含 2.52fr 等）。
      const authoredRows = String(rail.style.gridTemplateRows || "").trim();
      if (!authoredRows) {
        rail.style.display = "grid";
        rail.style.gridTemplateRows = `repeat(${sections.length}, minmax(0, 1fr))`;
      }
      rail.style.minHeight = "0";
      rail.style.height = "100%";
      // 整栏滚动口：style / layout_budget.overflow / px-floor minmax 行轨（内容预算高于可视窗）。
      // region.mei 的 props.overflow 不会进 structure.full；勿无 hidden 盖掉可滚意图。
      rail.style.overflow = resolveRailOverflowIntent(rail);
      if (!rail.style.rowGap && !rail.style.gap) {
        // Cockpit StageLayoutProfile region→section default (omit-inject parity).
        rail.style.rowGap = "1px";
      }
      sections.forEach((section) => {
        section.style.minHeight = "0";
        section.style.minWidth = "0";
        section.style.overflow = "hidden";
        hardenNodeGridRows(section);
        section
          .querySelectorAll(
            ':scope > [data-preview-scope$="/content_zone"], :scope > [data-preview-scope$="/body"], :scope > .mei-compose-slot, :scope > .preview-card',
          )
          .forEach((el) => {
            if (!(el instanceof HTMLElement)) return;
            el.style.minHeight = "0";
            el.style.maxHeight = "100%";
            el.style.overflow = "hidden";
            hardenNodeGridRows(el);
            const host = el.querySelector(":scope > .component-host, :scope > .panel-body-cell > .component-host");
            if (host instanceof HTMLElement) {
              host.style.minHeight = "0";
              host.style.maxHeight = "100%";
              host.style.height = "100%";
              host.style.overflow = "hidden";
            }
          });
      });
      hardenNodeGridRows(rail);
    });
  }

  function normalizeSectionContentZonePlacement(root) {
    if (!(root instanceof HTMLElement)) return;
    const tree = root.querySelector(".mei-structure-tree") || root;
    tree.querySelectorAll('[data-mei-ui-role="section"]').forEach((section) => {
      if (!(section instanceof HTMLElement)) return;
      const areas = String(
        section.style.gridTemplateAreas || getComputedStyle(section).gridTemplateAreas || "",
      );
      const bodyArea = /\bbody\b/.test(areas)
        ? "body"
        : /\bcontent_zone\b/.test(areas)
          ? "content_zone"
          : "";
      if (!bodyArea) return;
      const titleArea = /\btitle\b/.test(areas)
        ? "title"
        : /\btitle_zone\b/.test(areas)
          ? "title_zone"
          : "";
      [...section.children].forEach((child) => {
        if (!(child instanceof HTMLElement)) return;
        const scope = String(child.getAttribute("data-preview-scope") || "");
        const isTitle =
          scope.endsWith("/title_zone") ||
          scope.endsWith("/title") ||
          scope.endsWith("/head") ||
          child.classList.contains("mei-compose-section-head");
        if (isTitle) {
          if (titleArea) child.style.gridArea = titleArea;
          return;
        }
        // 单内容子节点若落在 auto，会挤进 54px 标题轨；强制挂到 content_zone/body。
        child.style.gridArea = bodyArea;
        child.style.minHeight = "0";
        child.style.height = "100%";
        child.style.alignSelf = "stretch";
        child.style.width = "100%";
      });
      // 保留 title/content 双轨：即使标题节点暂缺，也让内容挂在 content_zone/body，
      // 避免 auto 落入 54px 标题轨导致整栏挤压。
    });
  }

  function applyComposeThemeLayout(root) {
    if (!(root instanceof HTMLElement)) return false;
    const patches = global.__mei?.theme_layout;
    if (patches && typeof patches === "object" && boot.viewCompositor?.applyThemeAndOverlay) {
      boot.viewCompositor.applyThemeAndOverlay(root, null, { patches });
    }
    applyRailRegionSectionLayouts(root);
    if (global.MeiProjectionDepth?.applyLayoutBudgetManifest) {
      global.MeiProjectionDepth.applyLayoutBudgetManifest(root.ownerDocument || document);
    }
    // layout budget 可能再次写入裸 `1fr`；预算后再 harden 一次。
    applyRailRegionSectionLayouts(root);
    normalizeMetricCompoundSections(root);
    normalizeSectionContentZonePlacement(root);
    clipChartSlotsToHost(root);
    normalizeScreenHeaderBrandBlocks(root);
    return true;
  }

  function normalizeComposeCockpitLayouts(root) {
    if (!(root instanceof HTMLElement)) return;
    const tree = root.querySelector(".mei-structure-tree");
    if (!(tree instanceof HTMLElement)) return;
    tree.querySelectorAll('[data-mei-ui-role="plane"], .mei-compose-plane').forEach((plane) => {
      normalizeT0BasemapPlane(plane);
    });
    tree.querySelectorAll('[data-preview-scope$="/map_stage"], [data-preview-scope$="map_stage"]').forEach(
      (section) => normalizeMapStageSection(section),
    );
    tree.querySelectorAll(
      '[data-preview-scope$="/map_viewport"], [data-preview-scope*="map_stage_overlay"], [data-preview-scope$="/map_stage"]',
    ).forEach((section) => normalizeMapOperationViewportSection(section));
    applyRailRegionSectionLayouts(root);
    normalizeSectionContentZonePlacement(root);
    normalizeT1InteractivePointerEvents(tree);
    normalizeMapViewportPointerEvents(tree);
    // 指针规则之后再钉一次 T0，避免 map_stage 叠层逻辑误伤底图。
    tree.querySelectorAll('[data-mei-ui-role="plane"], .mei-compose-plane').forEach((plane) => {
      normalizeT0BasemapPlane(plane);
    });
    hideLayoutDebugRegions(tree);
  }

  function applyComposeStructureLayout(root, structureDoc) {
    const tree = root.querySelector(".mei-structure-tree");
    if (!(tree instanceof HTMLElement)) return false;

    tree.classList.add("mei-compose-scene-root");
    tree.style.display = "";
    tree.removeAttribute("aria-hidden");
    tree.removeAttribute("hidden");

    wrapStructureTreeInSceneViewport(root, tree, structureDoc);

    // 先打 layout_budget（含 plane/region 编译网格），再 resolve T1；否则会误报 missing compiled T1 grid。
    if (global.MeiProjectionDepth?.applyLayoutBudgetManifest) {
      global.MeiProjectionDepth.applyLayoutBudgetManifest(root.ownerDocument || document);
    }

    tree.querySelectorAll('[data-mei-ui-role="plane"], .mei-compose-plane').forEach((plane) => {
      if (!(plane instanceof HTMLElement)) return;
      applyPlaneRegionLayout(plane);
    });

    normalizeComposeCockpitLayouts(root);

    if (global.MeiProjectionDepth?.applyLayoutBudgetManifest) {
      global.MeiProjectionDepth.applyLayoutBudgetManifest(root.ownerDocument || document);
    }
    // budget 写入行轨后再钉一次 overflow（px-floor minmax → auto）。
    applyRailRegionSectionLayouts(root);
    normalizeMetricCompoundSections(root);
    clipChartSlotsToHost(root);
    normalizeScreenHeaderBrandBlocks(root);
    return true;
  }

  function notifyPreviewComposed(root) {
    if (!(root instanceof HTMLElement)) return;
    root.removeAttribute("data-mei-compose-placeholder");
    root.removeAttribute("aria-busy");
    try {
      if (typeof global.__meiSyncRuntimeQueryAppContext === "function") {
        global.__meiSyncRuntimeQueryAppContext({ clearCaches: false });
      } else if (
        typeof global.__meiDatasetRuntime?.syncRuntimeQueryAppContextFromPage === "function"
      ) {
        global.__meiDatasetRuntime.syncRuntimeQueryAppContextFromPage({ clearCaches: false });
      }
    } catch (_) {}
    const prefetchRoot =
      root.closest("#mei-compose-root, .preview-pane-scroll") || root;
    if (typeof global.__meiDatasetRuntime?.prefetchVisiblePanelMetrics === "function") {
      try {
        global.__meiDatasetRuntime.prefetchVisiblePanelMetrics(prefetchRoot);
      } catch (_) {}
    } else {
      global.dispatchEvent(new CustomEvent("meilang:prefetch-panel-metrics"));
    }
    if (typeof global.dispatchPreviewUpdated === "function") {
      global.dispatchPreviewUpdated("page", {
        resetRuntimeQueryCache: false,
        source: "compose_layers",
      });
    } else {
      global.dispatchEvent(
        new CustomEvent("meilang:preview-updated", {
          detail: { scope: "page", resetRuntimeQueryCache: false, source: "compose_layers" },
        }),
      );
    }
    if (typeof boot.scheduleFrameViewportRelayout === "function") {
      boot.scheduleFrameViewportRelayout();
      requestAnimationFrame(() => {
        boot.scheduleFrameViewportRelayout();
        refreshComposeMaps(root);
      });
    } else {
      refreshComposeMaps(root);
    }
    // After slides DOM exists, re-bind Stage Surface (early boot may have defaulted to cockpit).
    try {
      if (typeof boot.stageSurface?.syncFromLocation === "function") {
        boot.stageSurface.syncFromLocation();
      }
    } catch (_) {}
  }

  function normalizeScreenHeaderBrandBlocks(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(
        '[data-preview-scope$="/screen_header_brand"], [data-mei-use-key="cockpit.header-brand"]',
      )
      .forEach((node) => {
        const block =
          node instanceof HTMLElement && node.classList.contains("mei-compose-block")
            ? node
            : node.closest?.(".mei-compose-block");
        if (!(block instanceof HTMLElement)) return;
        block.classList.add("preview-card-bare");
        block.style.border = "none";
        block.style.padding = "0";
        block.style.gap = "0";
        block.style.borderRadius = "0";
        block.style.overflow = "hidden";
        block.style.background = "transparent";
      });
  }

  function resolveManifestGapPx(node, fallback = "2px") {
    if (!(node instanceof HTMLElement)) return fallback;
    const fromManifest = String(node.dataset.manifestGap || "").trim();
    if (fromManifest) {
      return fromManifest.endsWith("px") ? fromManifest : `${fromManifest}px`;
    }
    const fromStyle = String(node.style.gap || node.style.rowGap || "").trim();
    if (fromStyle) return fromStyle;
    return fallback;
  }

  function normalizeMetricCompoundSections(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(".mei-compose-warning-panel, .mei-compose-compound-section")
      .forEach((section) => {
        if (!(section instanceof HTMLElement)) return;
        section.style.display = "grid";
        // 边距真源：layout_budget_manifest / panel_shell；勿再硬编码 gap=2px、padding=0
        // （0332 space_1：section/area 均为 4px）。仅在未声明时回退旧默认。
        if (
          !section.getAttribute("data-mei-panel-shell-applied") &&
          !String(section.style.padding || "").trim()
        ) {
          section.style.padding = "0";
        }
        section.style.margin = "0";
        section.style.gap = resolveManifestGapPx(section, "2px");
        section.style.borderRadius = "0";
        section.style.gridTemplateRows = "auto minmax(0, 1fr)";
        section.style.gridTemplateAreas = '"title" "body"';
        section.style.border = "1px solid rgba(56, 160, 240, 0.32)";
        section.style.minHeight = "0";
        section.style.height = "100%";
        const head = section.querySelector(
          '[data-preview-scope$="/title_zone"], [data-preview-scope$="/head"], [data-preview-scope$="/title"]:not([data-preview-scope$="/title/title"])',
        );
        // Prefer section-level content hosts. Never promote a nested
        // compound-metric (e.g. 行政检查 AI 底栏) to section body — that
        // pulls it out of its grid slot and collapses the multi-block layout.
        const content =
          section.querySelector(
            ':scope > [data-preview-scope$="/content_zone"], :scope > [data-preview-scope$="/body"]',
          ) ||
          section.querySelector(
            '[data-preview-scope$="/content_zone"], [data-preview-scope$="/body"]',
          ) ||
          section.querySelector('[data-preview-scope$="/enforcement_strip_layout"]') ||
          section.querySelector('[data-preview-scope$="/enforcement_body"]') ||
          section.querySelector(".mei-compose-enforcement-strip") ||
          section.querySelector('[data-preview-scope*="supervision-stats"]') ||
          section.querySelector(".mei-compose-metric-triptych");
        if (head instanceof HTMLElement) {
          head.style.gridArea = "title";
          head.style.margin = "0";
          head.style.padding = "0";
          head.style.border = "none";
          head.style.borderRadius = "0";
        }
        if (content instanceof HTMLElement) {
          content.style.gridArea = "body";
          content.style.display = content.classList.contains("mei-compose-metric-triptych")
            ? "grid"
            : content.style.display;
          content.style.width = "100%";
          content.style.margin = "0";
          if (
            !content.getAttribute("data-mei-panel-shell-applied") &&
            !String(content.style.padding || "").trim()
          ) {
            content.style.padding = "0";
          }
          content.style.gap = resolveManifestGapPx(content, "2px");
          content.style.border = "none";
          content.style.borderRadius = "0";
          content.style.minHeight = "0";
          content.style.height = "100%";
          content.style.alignSelf = "stretch";
          if (content.classList.contains("mei-compose-metric-triptych")) {
            content.style.gridTemplateRows = "1fr";
            content.querySelectorAll(":scope > *").forEach((child) => {
              if (!(child instanceof HTMLElement)) return;
              child.style.height = "100%";
              child.style.minHeight = "0";
              child.style.alignSelf = "stretch";
            });
          }
        }
      });
  }

  function applyWarningSupervisionComposeClasses(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll('[data-preview-scope$="/warning"][data-mei-ui-role="section"]').forEach(
      (section) => {
        if (section instanceof HTMLElement) {
          section.classList.add("mei-compose-warning-panel");
        }
      },
    );
    root
      .querySelectorAll('[data-preview-scope$="/warning/supervision-stats"]')
      .forEach((content) => {
        if (!(content instanceof HTMLElement)) return;
        const role = String(content.getAttribute("data-mei-ui-role") || "").toLowerCase();
        if (role !== "content" && role !== "slot") return;
        content.classList.add("mei-compose-metric-triptych");
      });
  }

  const COMPOUND_METRIC_SLOT_AREAS = {
    top: "top",
    b0: "b0",
    b1: "b1",
    b2: "b2",
    // long_metric_compound (行政检查 AI 底栏)
    main: "main",
    rtop: "rtop",
    rbottom: "rbottom",
  };

  const ENFORCEMENT_STRIP_AREAS = {
    first: "first",
    second: "second",
    third: "third",
    compound: "compound",
    enforcement_units_card: "first",
    enforcement_personnel_card: "second",
    enforcement_items_card: "third",
    enforcement_objects_card: "compound",
  };

  function applyEnforcementStripComposeClasses(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(
        '[data-preview-scope$="/enforcement_strip_layout"], [data-preview-scope$="/enforcement_body"]',
      )
      .forEach((strip) => {
        if (!(strip instanceof HTMLElement)) return;
        const role = String(strip.getAttribute("data-mei-ui-role") || "").toLowerCase();
        if (role && role !== "content" && role !== "slot" && role !== "section") return;
        const scope = String(strip.getAttribute("data-preview-scope") || "").trim();
        if (!/\/enforcement_(strip_layout|body)$/.test(scope)) return;
        strip.classList.add("mei-compose-enforcement-strip");
        strip.style.width = "100%";
        strip.style.gridArea = "body";
        strip.style.alignSelf = "stretch";
        const section = strip.closest('[data-mei-ui-role="section"]');
        if (section instanceof HTMLElement) {
          section.classList.add("mei-compose-compound-section");
        }
        strip
          .querySelectorAll(
            ':scope > [data-mei-ui-role="slot"], :scope > [data-mei-ui-role="content"], :scope > section, :scope > div.mei-compose-node',
          )
          .forEach((child) => {
            if (!(child instanceof HTMLElement)) return;
            const scope = String(child.getAttribute("data-preview-scope") || "");
            const suffix = scope.split("/").filter(Boolean).pop() || "";
            const area =
              ENFORCEMENT_STRIP_AREAS[suffix] ||
              String(child.getAttribute("data-mei-panel-area") || "").trim();
            if (area && ENFORCEMENT_STRIP_AREAS[area]) {
              child.style.gridArea = ENFORCEMENT_STRIP_AREAS[area];
            }
          });
      });
  }

  function isSectionLevelCompoundHost(content, section) {
    if (!(content instanceof HTMLElement) || !(section instanceof HTMLElement)) return false;
    const parent = content.parentElement;
    if (!(parent instanceof HTMLElement)) return false;
    if (parent === section) return true;
    const parentScope = String(parent.getAttribute("data-preview-scope") || "");
    if (/\/(content_zone|body)$/.test(parentScope)) return true;
    // Nested under multi-block hosts (inspection-stats / block_ai / strip compound
    // slot) must not re-skin the whole section as a single compound panel.
    return false;
  }

  function applyCompoundMetricComposeClasses(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll('[data-mei-content-kind="compound-metric"]').forEach((content) => {
      if (!(content instanceof HTMLElement)) return;
      content.classList.add("mei-compose-metric-compound");
      const section = content.closest('[data-mei-ui-role="section"]');
      if (section instanceof HTMLElement && isSectionLevelCompoundHost(content, section)) {
        section.classList.add("mei-compose-compound-section");
      }
      const slotSuffixes = [];
      content.querySelectorAll(':scope > [data-mei-ui-role="slot"]').forEach((slot) => {
        if (!(slot instanceof HTMLElement)) return;
        const scope = String(slot.getAttribute("data-preview-scope") || "");
        const suffix = scope.split("/").filter(Boolean).pop() || "";
        const area = COMPOUND_METRIC_SLOT_AREAS[suffix];
        if (area) {
          slot.style.gridArea = area;
          slotSuffixes.push(area);
        }
      });
      // long compound: main | rtop / rbottom — ensure host grid when budget not yet applied
      const isLongCompound =
        slotSuffixes.includes("main") &&
        slotSuffixes.includes("rtop") &&
        slotSuffixes.includes("rbottom");
      if (isLongCompound) {
        content.style.display = "grid";
        content.style.gridTemplateColumns = "1.05fr 1.95fr";
        content.style.gridTemplateRows = "minmax(0, 1fr) minmax(0, 1fr)";
        content.style.gridTemplateAreas = '"main rtop" "main rbottom"';
        // Zero gap so the shared slot-frame background reads as one card.
        content.style.gap = "0";
        content.style.minHeight = "0";
        content.style.height = "100%";
        content.style.width = "100%";
        content.style.alignSelf = "stretch";
        content.style.overflow = "hidden";
      }
    });
  }

  function clipChartSlotsToHost(root) {
    if (!(root instanceof HTMLElement)) return;
    // Constrain chart slots and fit compact chart canvas to the slot box so
    // fixed chartHeight cannot spill into the next grid row.
    root
      .querySelectorAll(
        '[data-preview-scope$="/chart"][data-mei-ui-role="slot"], [data-preview-scope$="/chart.column"][data-mei-ui-role="content"], [data-preview-scope$="/chart.ranking"][data-mei-ui-role="content"], [data-preview-scope$="/rank"][data-mei-ui-role="slot"], [data-preview-scope$="/matter_rank"]',
      )
      .forEach((el) => {
        if (!(el instanceof HTMLElement)) return;
        el.style.minHeight = "0";
        el.style.overflow = "hidden";
        el.style.maxHeight = "100%";
        el.style.width = "100%";
        el.style.height = "100%";
        el.style.alignSelf = "stretch";
        const host = el.querySelector(
          ":scope > .component-host, :scope > .panel-body-cell > .component-host",
        );
        if (host instanceof HTMLElement) {
          host.style.minHeight = "0";
          host.style.overflow = "hidden";
          host.style.width = "100%";
          host.style.height = "100%";
        }
        const chart =
          el.matches?.("mei-chart-column, mei-chart-bar, mei-chart-line, mei-chart-ranking")
            ? el
            : el.querySelector?.(
                "mei-chart-column, mei-chart-bar, mei-chart-line, mei-chart-ranking",
              );
        if (!(chart instanceof HTMLElement)) return;
        // Chart blocks must not keep default preview-card padding — it shrinks
        // the host below chartHeight and clips the x-axis.
        const chartCard = chart.closest(".preview-card");
        if (chartCard instanceof HTMLElement) {
          chartCard.classList.add("preview-card-bare");
          chartCard.style.padding = "0";
          chartCard.style.gap = "0";
          chartCard.style.boxShadow = "none";
        }
        chart.style.minHeight = "0";
        chart.style.maxHeight = "100%";
        chart.style.height = "100%";
        chart.style.width = "100%";
        chart.style.overflow = "hidden";
        let wantsFill = false;
        try {
          const earlyProps = parseHostProps(chart);
          wantsFill =
            earlyProps.fillHeight === true ||
            earlyProps.fillHeight === "true" ||
            earlyProps.fill_height === true ||
            earlyProps.fill_height === "true";
        } catch (_) {}
        chart.style.display = wantsFill ? "flex" : "block";
        if (wantsFill) {
          chart.style.flexDirection = "column";
          chart.style.boxSizing = "border-box";
        }
        // Prefer the host box (design px) over visual rect — stage scale would
        // otherwise under-report height via getBoundingClientRect.
        const hostBox = host instanceof HTMLElement ? host : el;
        const slotH = Math.max(
          0,
          Math.floor(
            hostBox.clientHeight ||
              el.clientHeight ||
              el.getBoundingClientRect().height ||
              0,
          ),
        );
        if (slotH < 8) return;
        // Fill the design-px host box. Stage CSS scale shrinks visual rect, but
        // echarts must size to clientHeight so the canvas occupies the full slot.
        const fitH = Math.max(40, slotH - 4);
        try {
          const props = parseHostProps(chart);
          // fillHeight：由 slot 拉伸，不再注入固定 chartHeight（否则会在大格里留白或与 Fill-down 打架）。
          wantsFill =
            props.fillHeight === true ||
            props.fillHeight === "true" ||
            props.fill_height === true ||
            props.fill_height === "true";
          const next = wantsFill
            ? { ...props, compact: true }
            : { ...props, compact: true, chartHeight: fitH };
          if (fitH <= 180) {
            if (next.gridTop == null && next.grid_top == null) next.gridTop = 12;
            if (next.gridBottom == null && next.grid_bottom == null) next.gridBottom = 16;
          }
          const changed = wantsFill
            ? props.compact !== true ||
              Number(props.gridTop ?? props.grid_top ?? NaN) !== Number(next.gridTop) ||
              Number(props.gridBottom ?? props.grid_bottom ?? NaN) !== Number(next.gridBottom)
            : Number(props.chartHeight) !== fitH ||
              props.compact !== true ||
              Number(props.gridTop ?? props.grid_top ?? NaN) !== Number(next.gridTop) ||
              Number(props.gridBottom ?? props.grid_bottom ?? NaN) !== Number(next.gridBottom);
          if (changed) {
            applyPropsToHost(chart, next);
          }
        } catch (_) {}
        const wrap = chart.shadowRoot?.querySelector?.(".wrap");
        if (wrap instanceof HTMLElement) {
          wrap.style.height = "100%";
          wrap.style.maxHeight = "100%";
          wrap.style.minHeight = "0";
          wrap.style.overflow = "hidden";
          wrap.style.boxSizing = "border-box";
          if (wantsFill) {
            const headEl = wrap.querySelector(".head");
            const hasHead =
              headEl instanceof HTMLElement && getComputedStyle(headEl).display !== "none";
            wrap.style.display = "flex";
            wrap.style.flexDirection = "column";
            wrap.style.gridTemplateRows = "";
            void hasHead;
          }
        }
        const chartBox =
          chart.shadowRoot?.querySelector?.(".chart") || chart.chartEl || null;
        if (chartBox instanceof HTMLElement) {
          if (wantsFill) {
            chartBox.style.minHeight = "0";
            chartBox.style.flex = "1 1 auto";
            chartBox.style.height = "auto";
            chartBox.style.maxHeight = "none";
          } else {
            chartBox.style.minHeight = `${fitH}px`;
            chartBox.style.height = `${fitH}px`;
            chartBox.style.maxHeight = `${fitH}px`;
          }
        }
        const errorEl = chart.shadowRoot?.querySelector?.(".error");
        if (errorEl instanceof HTMLElement && !String(errorEl.textContent || "").trim()) {
          errorEl.style.display = "none";
        }
        const resizeH = wantsFill
          ? Math.max(
              40,
              Math.floor(
                (chartBox instanceof HTMLElement &&
                  (chartBox.clientHeight || chartBox.getBoundingClientRect().height)) ||
                  Math.max(40, slotH - 28),
              ),
            )
          : fitH;
        try {
          chart.chart?.resize?.({
            width: Math.max(1, hostBox.clientWidth || chart.clientWidth || el.clientWidth),
            height: resizeH,
          });
        } catch (_) {}
        if (wantsFill && chartBox instanceof HTMLElement) {
          // echarts 可能把 canvas 写成固定 px；再对齐一次父盒。
          const canvas = chartBox.querySelector("canvas");
          if (canvas instanceof HTMLElement) {
            try {
              chart.chart?.resize?.({
                width: Math.max(1, chartBox.clientWidth || hostBox.clientWidth),
                height: Math.max(40, chartBox.clientHeight || resizeH),
              });
            } catch (_) {}
          }
        }
      });
    root
      .querySelectorAll(
        '[data-mei-content-kind="chart-summary"], [data-preview-scope$="/block_counts"], [data-preview-scope$="/inspection-stats"]',
      )
      .forEach((el) => {
        if (!(el instanceof HTMLElement)) return;
        el.style.minHeight = "0";
        el.style.overflow = "hidden";
      });
    // cockpit 内嵌可视化：运行时兜底去掉 compose 默认 14px 圆角底
    root
      .querySelectorAll(
        "mei-cockpit-donut-trio, mei-cockpit-park-amount-list, mei-cockpit-scroll-list, mei-cockpit-data-table, mei-cockpit-metric-progress",
      )
      .forEach((el) => {
        if (!(el instanceof HTMLElement)) return;
        const card = el.closest(".preview-card");
        if (!(card instanceof HTMLElement)) return;
        // Progress bars mount inside metric cards; do not wipe SVG slot chrome
        // (e.g. metric-bg-clean on 无违规) owned by the parent metric frame.
        if (
          card.getAttribute("data-mei-slot-frame-bg") === "true" ||
          card.hasAttribute("data-mei-metric-card")
        ) {
          return;
        }
        card.classList.add("preview-card-bare");
        card.style.padding = "0";
        card.style.gap = "0";
        card.style.boxShadow = "none";
        card.style.background = "transparent";
        card.style.borderRadius = "0";
      });
  }

  function rebindMetricCardHosts(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll('[data-mei-metric-card="true"] mei-text, [data-mei-metric-card="true"] MEI-TEXT').forEach(
      (node) => {
        if (!(node instanceof HTMLElement)) return;
        const props = parseHostProps(node);
        if (!String(props.metric_role || "").trim()) return;
        if (typeof node._bind === "function") {
          try {
            node._bind();
          } catch (_) {}
        }
      },
    );
  }

  /** Plain mei.text / chart / data-table hosts may connect before eval props land; force a rebind. */
  function rebindAuthoredComponentHosts(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll("mei-text, MEI-TEXT").forEach((node) => {
      if (!(node instanceof HTMLElement)) return;
      const props = parseHostProps(node);
      if (String(props.metric_role || props.metricRole || "").trim()) return;
      if (typeof props.content !== "string" || !props.content.trim()) return;
      if (typeof node._bind === "function") {
        try {
          node._bind();
        } catch (_) {}
      }
    });
    root
      .querySelectorAll(
        'mei-chart-column, MEI-CHART-COLUMN, [data-mei-use-key="chart.column"], [data-mei-use-key="chart.ranking"]',
      )
      .forEach((node) => {
        if (!(node instanceof HTMLElement)) return;
        const props = parseHostProps(node);
        if (!props || typeof props !== "object") return;
        if (typeof node.refresh === "function") {
          try {
            node.refresh();
          } catch (_) {}
        } else if (typeof node._bind === "function") {
          try {
            node._bind();
          } catch (_) {}
        } else if (typeof node.render === "function") {
          try {
            node.render();
          } catch (_) {}
        }
      });
    root
      .querySelectorAll(
        'mei-cockpit-data-table, MEI-COCKPIT-DATA-TABLE, [data-mei-use-key="cockpit.data-table"]',
      )
      .forEach((node) => {
        if (!(node instanceof HTMLElement)) return;
        if (typeof node._bind === "function") {
          try {
            node._bind();
          } catch (_) {}
        } else if (typeof node.refresh === "function") {
          try {
            node.refresh();
          } catch (_) {}
        }
      });
  }

  function bindEvalSlots(root, evalDocs, options) {
    if (!(root instanceof HTMLElement)) return false;
    const evalSlotCount = Array.isArray(evalDocs)
      ? evalDocs.reduce(
          (sum, doc) => sum + Object.keys(doc?.slots || {}).length,
          0,
        )
      : 0;
    // Include slot count so a structure-only first bind (0 slots) does not skip a
    // later bind once deferred import / layer-batch delivers eval documents.
    const bindDigest = `${String(options?.digest || "").trim()}|slots:${evalSlotCount}`;
    if (
      bindDigest &&
      root.getAttribute("data-mei-eval-bind-digest") === bindDigest &&
      root.getAttribute("data-mei-compose-materialized") === "1"
    ) {
      boot.renderPipelineMark?.("bind_eval_slots:skip", { digest: bindDigest });
      return true;
    }
    const startedAt = typeof performance !== "undefined" ? performance.now() : Date.now();
    const domIndex = buildComposeDomIndex(root);
    boot.renderPipelineMark?.("bind_eval_slots:begin", {
      documents: Array.isArray(evalDocs) ? evalDocs.length : 0,
      scopes: domIndex.byScope.size,
    });
    let bound = 0;
    const sceneMountByMetric = sceneMountsByMetricId(evalDocs);
    for (const doc of evalDocs || []) {
      const slots = doc.slots || {};
      for (const [scopeKey, entry] of Object.entries(slots)) {
        if (isDuplicateMetricCardLeafScope(scopeKey)) continue;
        const container = resolveEvalSlotContainer(root, scopeKey, domIndex);
        if (!(container instanceof HTMLElement)) continue;
        const mounts = Array.isArray(entry?.mounts) ? entry.mounts : [];
        const useKeys = Array.isArray(entry?.use_keys) ? entry.use_keys : [];
        const componentMounts = Array.isArray(entry?.component_mounts)
          ? entry.component_mounts
          : [];
        const allowEval =
          typeof boot.devEvalAllowsEvalScope !== "function" ||
          boot.devEvalAllowsEvalScope(scopeKey);
        if (!allowEval) {
          container.setAttribute("data-mei-dev-eval-placeholder", "1");
          container.setAttribute("data-mei-dev-eval-scope", scopeKey);
        }
        const headScope = isSectionHeadScope(scopeKey);
        const slotLabel = resolveEvalSlotLabel(entry);
        const headChrome = entry?.head_chrome;
        if (headScope && headChrome && typeof headChrome === "object") {
          applyHeadChromeFromSlot(container, headChrome);
        } else if (headScope && slotLabel) {
          applyHeadSlotLabel(container, slotLabel);
        }
        const bindComponentMounts =
          componentMounts.length > 0 &&
          (!headScope || shouldBindHeadComponentMounts(componentMounts));
        const contentKind = String(entry?.content_kind || "").trim().toLowerCase();
        // Compound hosts are layout shells; child cards bind their own mounts.
        if (bindComponentMounts && contentKind !== "compound-metric") {
          const filteredMounts = filterComponentMountsForScope(scopeKey, componentMounts);
          let host = findComponentHostForScope(container, scopeKey, useKeys);
          if (!(host instanceof HTMLElement)) {
            host = ensureMetricCardComponentHost(container);
          }
          if (host instanceof HTMLElement) {
            bound += ensureComponentHostChildren(
              host,
              filteredMounts,
              sceneMountByMetric,
              scopeKey,
              allowEval,
            );
          }
          applyMetricCardShellFromMounts(container, filteredMounts);
        } else if (!componentMounts.length && isStackMetricEvalEntry(entry, container)) {
          const metricSection = resolveMetricCardSection(container);
          const host =
            ensureMetricCardComponentHost(container) ||
            metricSection?.querySelector(".component-host") ||
            container.querySelector(".component-host");
          const structureLabel = String(
            container.getAttribute("data-mei-structure-label") ||
              metricSection?.getAttribute("data-mei-structure-label") ||
              "",
          ).trim();
          const synthesized = buildSyntheticStackMetricMounts(
            slotLabel || structureLabel,
            entry?.content_kind,
            allowEval ? inferSceneMountForScope(scopeKey, sceneMountByMetric) : null,
          );
          if (host instanceof HTMLElement) {
            bound += ensureComponentHostChildren(
              host,
              synthesized,
              sceneMountByMetric,
              scopeKey,
              allowEval,
            );
          }
          applyMetricCardShellFromMounts(container, synthesized);
        }
        if (!headScope && entry?.panel_shell && typeof entry.panel_shell === "object") {
          applyPanelShellFromSlot(container, entry.panel_shell);
        }
        mounts.forEach((mount, index) => {
          const mountMetricId = String(
            mount?.metric_id ||
              mount?.props?.metric_id ||
              mount?.props?.content?.metric_id ||
              mount?.props?.content?.id ||
              "",
          ).trim();
          const allowMetric =
            allowEval &&
            (!mountMetricId ||
              typeof boot.devEvalAllowsMetric !== "function" ||
              boot.devEvalAllowsMetric(mountMetricId, scopeKey));
          const props = mountPropsForEval(mount, scopeKey, mount, allowMetric);
          const host = findHostForMount(root, mount, scopeKey, useKeys, index, container);
          if (host instanceof HTMLElement) {
            applyPropsToHost(host, props);
            bound += 1;
            return;
          }
          const useKey = String(useKeys[index] || "").trim();
          if (useKey) {
            const section = createBlockSection(
              useKey,
              scopeKey === "scene:default" ? "" : scopeKey,
              "content",
            );
            applyPropsToHost(section.querySelector(".component-host"), props);
            container.appendChild(section);
            bound += 1;
          }
        });
      }
    }
    root.querySelectorAll('[data-mei-metric-card="true"]').forEach((card) => {
      normalizeMetricCardSection(card);
    });
    applyWarningSupervisionComposeClasses(root);
    applyEnforcementStripComposeClasses(root);
    applyCompoundMetricComposeClasses(root);
    clearNestedCompoundSlotFrames(root);
    normalizeMetricCompoundSections(root);
    clipChartSlotsToHost(root);
    if (root.querySelector("mei-chart-column, [data-mei-use-key^='chart.']")) {
      const clipWhenIdle = () => {
        try {
          clipChartSlotsToHost(root);
        } catch (_) {}
      };
      if (typeof global.requestIdleCallback === "function") {
        global.requestIdleCallback(clipWhenIdle, { timeout: 1000 });
      } else {
        global.setTimeout(clipWhenIdle, 80);
      }
    }
    normalizeScreenHeaderBrandBlocks(root);
    promoteSectionHeadMeiTextNodes(root);
    applyRailHeadTitlesFromEval(root, evalDocs);
    root.querySelectorAll('[data-mei-section-head-normalized="1"]').forEach((head) => {
      const h3 = head.querySelector("h3");
      const text = String(h3?.textContent || "").trim().toLowerCase();
      if (!text || isIgnoredSectionHeadLabel(text) || text === "板块标题") {
        head.removeAttribute("data-mei-section-head-normalized");
      }
    });
    normalizeAllSectionHeadSlots(root);
    suppressDuplicateMetricCardLeafSlots(root);
    suppressDecomposedMetricCardDuplicateSlots(root);
    normalizeMapStageHintPointerEvents(root);
    normalizeT1InteractivePointerEvents(root);
    normalizeMapViewportPointerEvents(root);
    root.querySelectorAll('[data-mei-ui-role="plane"], .mei-compose-plane').forEach((plane) => {
      normalizeT0BasemapPlane(plane);
    });
    applyDevEvalPlaceholders(root);
    // mei.text connects before authored data-props settle; rebind after placeholders.
    rebindAuthoredComponentHosts(root);
    if (bindDigest) root.setAttribute("data-mei-eval-bind-digest", bindDigest);
    boot.renderPipelineMark?.("bind_eval_slots:end", {
      bound,
      durationMs: Math.round(
        (typeof performance !== "undefined" ? performance.now() : Date.now()) - startedAt,
      ),
    });
    return bound > 0;
  }

  function applyDevEvalPlaceholders(root) {
    if (!(root instanceof HTMLElement)) return;
    if (typeof boot.devEvalAllowsPreviewScope !== "function") return;
    const config = boot.devEvalReadConfig?.() || {};
    if (config.profile === "full") return;
    root.querySelectorAll("[data-preview-scope], [data-mei-preview-scope]").forEach((el) => {
      if (!(el instanceof HTMLElement)) return;
      const scope =
        el.getAttribute("data-preview-scope") ||
        el.getAttribute("data-mei-preview-scope") ||
        "";
      if (!scope || boot.devEvalAllowsPreviewScope(scope)) return;
      el.setAttribute("data-mei-dev-eval-placeholder", "1");
      el.querySelectorAll(".component-host, [data-props], mei-text, .mei-text").forEach((host) => {
        if (!(host instanceof HTMLElement)) return;
        const ownerScope = host.closest("[data-preview-scope], [data-mei-preview-scope]");
        if (ownerScope !== el) return;
        const hostProps = parseHostProps(host);
        const contentObj =
          hostProps.content &&
          typeof hostProps.content === "object" &&
          !Array.isArray(hostProps.content)
            ? hostProps.content
            : null;
        const authored =
          (typeof hostProps.content === "string" && hostProps.content.trim().length > 0) ||
          (typeof hostProps.text === "string" && hostProps.text.trim().length > 0) ||
          (typeof hostProps.html === "string" && hostProps.html.trim().length > 0) ||
          (contentObj &&
            (contentObj.label != null ||
              contentObj.value != null ||
              contentObj.unit != null ||
              contentObj.desc != null));
        // Authored deck/static mei.text already carries content — mark scope only,
        // never clobber light-DOM text or overwrite data-props with `--`.
        if (authored) return;
        host.setAttribute("data-mei-dev-eval-placeholder", "1");
        if (!host.getAttribute("data-props")) {
          host.setAttribute(
            "data-props",
            JSON.stringify(boot.devEvalPlaceholderProps?.({}) || { text: "--", value: "--" }),
          );
        }
        if (/mei-text|metric|label|value/i.test(host.tagName + host.className)) {
          const text = String(host.textContent || "").trim();
          if (!text || text === "—" || text === "-") host.textContent = "--";
        }
      });
    });
  }

  function normalizeMapStageHintPointerEvents(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(
        '[data-preview-scope*="stage-aperture-hint"], [data-preview-scope*="map_stage/hint"]',
      )
      .forEach((el) => {
        if (el instanceof HTMLElement) {
          el.style.pointerEvents = "none";
        }
      });
  }

  function normalizeMapViewportPointerEvents(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(
        '[data-preview-scope$="/map_viewport"], [data-preview-scope*="/map_viewport/"], [data-preview-scope*="/map-viewport"], [data-preview-scope*="map_stage_overlay"], [data-preview-scope$="/map_stage"]',
      )
      .forEach((el) => {
        if (!(el instanceof HTMLElement)) return;
        const scope = el.getAttribute("data-preview-scope") || "";
        if (isMapViewportPointerTransparentScope(scope)) {
          el.style.pointerEvents = "none";
        }
      });
    root
      .querySelectorAll(
        '[data-preview-scope*="map-tools"], [data-preview-scope*="map_tools"], [data-mei-panel-name="map-tools-slot"]',
      )
      .forEach((el) => {
        if (el instanceof HTMLElement) {
          el.style.pointerEvents = "auto";
        }
      });
  }

  function isMapViewportPointerTransparentScope(scope) {
    const normalized = String(scope || "").trim().toLowerCase();
    if (!normalized) return false;
    // T0 底图 map_stage 必须接收 pan/zoom；仅 T1 观察窗/叠层透传。
    if (normalized === "t0" || normalized.startsWith("t0/")) return false;
    if (
      normalized.endsWith("/map_viewport") ||
      normalized.includes("/map_viewport/") ||
      normalized.includes("/map-viewport")
    ) {
      return !(
        normalized.includes("map-tools") ||
        normalized.includes("map_tools")
      );
    }
    if (
      normalized.includes("map-interaction-surface") ||
      normalized.includes("map_stage_overlay") ||
      normalized.endsWith("/map_stage") ||
      normalized.includes("/map_stage/")
    ) {
      return !(
        normalized.includes("map-tools") ||
        normalized.includes("map_tools")
      );
    }
    if (
      normalized.includes("stage_aperture") ||
      normalized.includes("stage-aperture") ||
      normalized.includes("viewport_frame") ||
      normalized.includes("world_viewport")
    ) {
      return true;
    }
    return false;
  }

  function shouldT1UnitReceivePointerEvents(scope) {
    const normalized = String(scope || "").trim().toLowerCase();
    if (!normalized || normalized.includes("layout_debug")) return false;
    if (isMapViewportPointerTransparentScope(normalized)) return false;
    // center_rail 本体必须保持 none，否则子级 map_viewport(none) 上的点击会落在父级上，挡死 T0 底图。
    if (
      normalized === "t1/center_rail" ||
      normalized.endsWith("/center_rail") ||
      normalized.includes("center_rail/map")
    ) {
      return false;
    }
    // 中栏仅已知可点业务 section；其余中栏默认透传。
    // 非地图交互内容应通过作者 pointer_events / data-mei-pointer-events 声明，勿在此加业务名。
    if (normalized.includes("center_rail")) {
      return (
        normalized.includes("playback") ||
        normalized.includes("indicator_system") ||
        normalized.includes("realtime_table") ||
        normalized.includes("realtime_warning") ||
        normalized.includes("center_top")
      );
    }
    // Default allow for T1 app content outside center_rail.
    // Historical cockpit rails/header remain clickable; map scopes already denied above.
    return true;
  }

  function authorPointerEventsDeclaration(el) {
    if (!(el instanceof HTMLElement)) return "";
    const attr = String(el.getAttribute("data-mei-pointer-events") || "").trim();
    if (attr) return attr;
    return "";
  }

  /** Self or nearest ancestor author declaration (non-map center_rail content). */
  function resolveAuthorPointerEvents(el) {
    if (!(el instanceof HTMLElement)) return "";
    const self = authorPointerEventsDeclaration(el);
    if (self) return self;
    const anc = el.closest?.("[data-mei-pointer-events]");
    if (anc instanceof HTMLElement && anc !== el) {
      return authorPointerEventsDeclaration(anc);
    }
    return "";
  }

  function normalizeT1InteractivePointerEvents(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(
        '.mei-compose-plane[data-mei-plane="T1"], .mei-compose-plane[data-mei-plane="t1"]',
      )
      .forEach((plane) => {
        if (!(plane instanceof HTMLElement)) return;
        // T1 plane itself must not capture; only opted-in sections receive events.
        plane.style.pointerEvents = "none";
        plane.querySelectorAll(".mei-compose-section, .mei-compose-slot, .mei-compose-region").forEach((el) => {
          if (!(el instanceof HTMLElement)) return;
          const scope = el.getAttribute("data-preview-scope") || "";
          if (isMapViewportPointerTransparentScope(scope)) {
            el.style.pointerEvents = "none";
            return;
          }
          const authorPe = resolveAuthorPointerEvents(el);
          if (authorPe) {
            el.style.pointerEvents = authorPe;
            return;
          }
          el.style.pointerEvents = shouldT1UnitReceivePointerEvents(scope) ? "auto" : "none";
        });
        // Author-declared hit targets inside center_rail (panels / hosts / WCs).
        plane.querySelectorAll("[data-mei-pointer-events]").forEach((el) => {
          if (!(el instanceof HTMLElement)) return;
          if (el.classList.contains("mei-compose-section") || el.classList.contains("mei-compose-slot") || el.classList.contains("mei-compose-region")) {
            return; // already handled above
          }
          const pe = authorPointerEventsDeclaration(el);
          if (pe) el.style.pointerEvents = pe;
        });
        // Descendants under an author-auto ancestor (e.g. component-host under drill section).
        plane.querySelectorAll("[data-mei-pointer-events='auto']").forEach((anc) => {
          if (!(anc instanceof HTMLElement)) return;
          anc.querySelectorAll(".component-host, .preview-card, .mei-compose-block").forEach((el) => {
            if (!(el instanceof HTMLElement)) return;
            if (el.hasAttribute("data-mei-pointer-events")) return;
            const scope = el.getAttribute("data-preview-scope") || "";
            if (scope && isMapViewportPointerTransparentScope(scope)) return;
            el.style.pointerEvents = "auto";
          });
        });
        // 观察窗透传子树（含 aperture / interaction surface / frame）强制 none
        plane
          .querySelectorAll(
            '[data-preview-scope*="map-interaction-surface"], [data-preview-scope*="map_interaction_surface"], [data-preview-scope*="stage-aperture"], [data-preview-scope*="stage_aperture"], [data-preview-scope*="viewport_frame"], [data-mei-panel-name="map-interaction-surface"], [data-mei-panel-name="stage-aperture-frame"]',
          )
          .forEach((el) => {
            if (el instanceof HTMLElement) el.style.pointerEvents = "none";
          });
        // 工具挂点始终可点
        plane
          .querySelectorAll(
            '[data-preview-scope*="map-tools"], [data-preview-scope*="map_tools"], [data-mei-panel-name="map-tools-slot"]',
          )
          .forEach((el) => {
            if (el instanceof HTMLElement) el.style.pointerEvents = "auto";
          });
      });
  }

  function ensureRuntimeComponentScripts(assets) {
    if (!Array.isArray(assets) || typeof boot.syncPreviewWorkspaceScripts !== "function") {
      return Promise.resolve(false);
    }
    const urls = [];
    for (const asset of assets) {
      const script = String(asset?.script || "").trim();
      if (!script) continue;
      urls.push(`/workspace-components/${script}`);
    }
    if (!urls.length) return Promise.resolve(false);
    return Promise.resolve(boot.syncPreviewWorkspaceScripts(urls, null)).then(() => true);
  }

  function scheduleComponentScriptWake(assets) {
    void ensureRuntimeComponentScripts(assets).then((loaded) => {
      if (!loaded) return;
      const root =
        document.getElementById("mei-compose-root") ||
        document.querySelector("[data-mei-compose-materialized], [data-mei-compose-placeholder]");
      if (!(root instanceof HTMLElement)) return;
      rebindAuthoredComponentHosts(root);
      notifyPreviewComposed(root);
    });
  }

  function applyRuntimePlans(plansLayer) {
    const doc = extractLayerDocument(plansLayer);
    if (!doc) return false;
    global.__mei = global.__mei || {};
    if (doc.layer_plan != null) global.__mei.layer_plan = doc.layer_plan;
    if (doc.presentation_map != null) global.__mei.presentation_map = doc.presentation_map;
    if (doc.world_plan != null) global.__mei.world_plan = doc.world_plan;
    if (doc.map_projection != null) global.__mei.map_projection = doc.map_projection;
    if (doc.overlay_defaults != null) {
      global.__mei.overlay_defaults = doc.overlay_defaults;
      global.__mei.t2_overlay_defaults = doc.overlay_defaults;
      global.__mei.page_overlay_defaults = doc.overlay_defaults;
    }
    if (doc.presentation_map != null) {
      try {
        if (typeof boot.stageSurface?.syncFromLocation === "function") {
          boot.stageSurface.syncFromLocation();
        }
      } catch (_) {}
    }    if (Array.isArray(doc.component_assets) && doc.component_assets.length) {
      global.__mei.component_assets = doc.component_assets;
      // Thin-shell HTML may have been primed from an older meibundle that lacked
      // cockpit.data-table. Runtime.plans still lists the full set — load any
      // missing modules so custom elements upgrade after F5.
      scheduleComponentScriptWake(doc.component_assets);
    }
    if (doc.theme_layout != null && typeof doc.theme_layout === "object") {
      global.__mei.theme_layout = doc.theme_layout;
    }
    if (doc.layout_budget_manifest != null) {
      // runtime.plans is the compile-time authority for grid budgets (e.g. status-flow
      // content hosts). Mark source so later bootstrap/eval-pack cannot clobber it
      // with a stale localStorage artifact that omits Content-role entries.
      global.__mei.layout_budget_manifest = doc.layout_budget_manifest;
      global.__mei.__layout_budget_source = "runtime.plans";
    }
    return true;
  }

  function hasMaterializedPreview(root) {
    if (!(root instanceof HTMLElement)) return false;
    if (root.getAttribute("data-mei-compose-materialized") === "1") return true;
    return !!root.querySelector(
      "[data-mei-frame-viewport], [data-mei-use-key], .preview-viewport, .mei-structure-tree",
    );
  }

  async function ensurePresentationMap(ctx) {
    const PRESENTATION_MAP_SCHEMA = "mei-presentation-map-v1";
    const acceptPresentationMap = (map) => {
      if (!map || typeof map !== "object") return null;
      if (!Object.keys(map).length) return null;
      const ver = String(map.schemaVersion || map.schema_version || "").trim();
      if (ver !== PRESENTATION_MAP_SCHEMA) {
        console.warn("[preview-materializer] unsupported presentation_map schema", ver);
        return null;
      }
      return map;
    };
    const existing = acceptPresentationMap(global.__mei?.presentation_map);
    if (existing) {
      return;
    }
    const appId = String(ctx?.app_id || ctx?.appId || "").trim();
    const sceneId = String(ctx?.scene_id || ctx?.sceneId || "home").trim() || "home";
    if (!appId || typeof global.fetch !== "function") return;
    try {
      const url = `/api/presentation/map/${encodeURIComponent(appId)}?scene=${encodeURIComponent(sceneId)}`;
      const response = await global.fetch(url, { credentials: "same-origin" });
      if (!response.ok) return;
      const payload = await response.json();
      const map = acceptPresentationMap(
        payload?.presentation_map || payload?.map || payload,
      );
      if (!map) return;
      global.__mei = global.__mei || {};
      global.__mei.presentation_map = map;
      try {
        if (typeof boot.stageSurface?.syncFromLocation === "function") {
          boot.stageSurface.syncFromLocation();
        }
      } catch (_) {}
    } catch (error) {
      console.warn("[preview-materializer] ensurePresentationMap skipped", error);
    }
  }

  async function ensureBootstrapBeforeInject(ctx) {
    if (typeof boot.ensureBootstrapSeeded !== "function") return;
    const appId = String(ctx?.appId || ctx?.app_id || "").trim();
    const sceneId = String(ctx?.sceneId || ctx?.scene_id || "home").trim() || "home";
    if (!appId) return;
    try {
      await boot.ensureBootstrapSeeded(ctx, {
        client_revision:
          ctx?.client_revision ||
          ctx?.clientRevision ||
          boot.readBootstrapMeta?.("mei-bootstrap-client-revision") ||
          "",
      });
    } catch (error) {
      console.warn("[preview-materializer] ensureBootstrapSeeded skipped", error);
    }
  }

  function composeRevisionDigest(composeAxes) {
    return String(
      composeAxes?.surface_revision_digest ||
        composeAxes?.surfaceRevisionDigest ||
        global.__mei?.view_revision_envelope?.surface_revision_digest ||
        global.__mei?.scene_manifest_refs?.surface_revision_digest ||
        "",
    ).trim();
  }

  function finalizeClientPreview(root, layers, composeAxes) {
    if (!(root instanceof HTMLElement) || !layers) return false;
    applyRuntimePlans(layers["runtime.plans"]);
    applyStagePlanMetadata(root);
    const projection = String(
      composeAxes?.review_projection || composeAxes?.reviewProjection || "",
    )
      .trim()
      .toLowerCase();
    const bindEvalContent =
      !projection || projection.includes("full") || projection === "live" || projection === "static";
    if (bindEvalContent) {
      bindEvalSlots(root, collectEvalDocs(layers), {
        digest: composeRevisionDigest(composeAxes),
      });
    }
    normalizeDocumentComposeLayout(root);
    root.setAttribute("data-mei-compose-materialized", "1");
    root.removeAttribute("data-mei-compose-placeholder");
    root.removeAttribute("aria-busy");
    // Align with materializePreview: structure may arrive before eval. After
    // rebinding slots, notify components (e.g. cockpit.data-table) that wait
    // on meilang:preview-updated / prefetch, otherwise F5 cold start leaves
    // tables empty until a later SPA rematerialize.
    notifyPreviewComposed(root);
    return true;
  }

  async function materializePlaceholderPreview(ctx, root, layers, options) {
    if (!(root instanceof HTMLElement)) return { ok: false, source: null };
    if (root.getAttribute("data-mei-compose-placeholder") !== "1") {
      return { ok: false, source: null };
    }
    const opts = options || {};
    const composeAxes = {
      ...(opts.composeAxes || {}),
      forceRematerialize: true,
    };
    try {
      await ensureBootstrapBeforeInject(ctx);
      await ensurePresentationMap(ctx);
      if (typeof boot.renderPipelineMark === "function") {
        boot.renderPipelineMark("preview_compose:begin");
      }
      if (layers && boot.viewCompositor?.composeFromLayers) {
        const composed = boot.viewCompositor.composeFromLayers(root, layers, composeAxes);
        if (composed && isClientLayerMaterialized(root)) {
          if (typeof boot.renderPipelineMark === "function") {
            boot.renderPipelineMark("preview_compose:end", { source: "compose" });
          }
          return { ok: true, source: "compose" };
        }
      }
      if (typeof boot.renderPipelineMark === "function") {
        boot.renderPipelineMark("preview_compose:end", { source: "miss" });
      }
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("preview-compose-miss", {
          message: "composeFromLayers failed or layers missing",
        });
      }
      return { ok: false, source: null };
    } catch (error) {
      if (typeof boot.renderPipelineMark === "function") {
        boot.renderPipelineMark("preview_compose:end", {
          source: "error",
          message: String(error?.message || error || "materialize failed"),
        });
      }
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("preview-compose-miss", {
          message: String(error?.message || error || "materialize failed"),
        });
      }
      return { ok: false, source: null };
    }
  }

  function materializePreview(root, layers, composeAxes) {
    if (!(root instanceof HTMLElement) || !layers) return false;
    currentTagLookup = buildComponentTagLookup(layers);
    const structure = extractLayerDocument(layers["structure.full"]);
    if (!structure) return false;

    applyRuntimePlans(layers["runtime.plans"]);
    void ensurePresentationMap({
      app_id: global.__mei?.scene_manifest_refs?.app_id,
      scene_id: global.__mei?.scene_manifest_refs?.scene_id,
    });

    cleanupComposeStructureArtifacts(root);
    buildStructureTree(root, structure, composeAxes || {});
    applyStagePlanMetadata(root);
    applyComposeStructureLayout(root, structure);
    applyComposeThemeLayout(root);

    const projection = String(
      composeAxes?.review_projection || composeAxes?.reviewProjection || "",
    ).trim()
      .toLowerCase();
    const bindEvalContent =
      !projection || projection.includes("full") || projection === "live" || projection === "static";
    if (bindEvalContent) {
      bindEvalSlots(root, collectEvalDocs(layers), {
        digest: composeRevisionDigest(composeAxes),
      });
    }

    normalizeDocumentComposeLayout(root);
    root.setAttribute("data-mei-compose-materialized", "1");
    notifyPreviewComposed(root);
    return true;
  }

  function isClientLayerMaterialized(root) {
    if (!(root instanceof HTMLElement)) return false;
    if (root.getAttribute("data-mei-compose-materialized") === "1") return true;
    return !!root.querySelector("[data-mei-compose-materialized='1']");
  }

  function isSsrInjectedPreviewRoot(root) {
    if (!(root instanceof HTMLElement)) return false;
    if (isClientLayerMaterialized(root)) return false;
    return canSkipClientCompose(root, { surface: "app" });
  }

  function isThinShellComposePlaceholder(root) {
    return root instanceof HTMLElement && root.getAttribute("data-mei-compose-placeholder") === "1";
  }

  function canSkipClientCompose(root, ctx) {
    if (!(root instanceof HTMLElement)) return false;
    if (isThinShellComposePlaceholder(root)) return false;
    if (ctx?.temp_stage || ctx?.tempStage) return false;
    if (String(ctx?.scope || "").trim()) return false;
    if (isClientLayerMaterialized(root)) return true;
    if (!hasMaterializedPreview(root)) return false;
    const targetApp = String(
      ctx?.app_id || ctx?.appId || global.document?.body?.getAttribute("data-app-id") || "",
    ).trim();
    let urlApp = "";
    try {
      urlApp = String(global.location.pathname.match(/^\/apps\/([^/]+)/)?.[1] || "").trim();
    } catch (_) {}
    if (targetApp && urlApp && targetApp !== urlApp) return false;
    const targetScene = String(ctx?.scene_id || ctx?.sceneId || "").trim();
    const rootScene = String(
      root.getAttribute("data-scene") ||
        global.document?.body?.getAttribute("data-scene-id") ||
        "",
    ).trim();
    if (targetScene && rootScene && targetScene !== rootScene) return false;
    const surface = String(ctx?.surface || ctx?.mode || "app")
      .trim()
      .toLowerCase();
    if (surface === "app") {
      return Array.from(root.querySelectorAll("[data-mei-plane], [data-mei-tier]")).some((el) =>
        /^t1$/i.test(
          String(el.getAttribute("data-mei-plane") || el.getAttribute("data-mei-tier") || ""),
        ),
      );
    }
    return true;
  }

  function readResidentStructureDocument() {
    const store = boot.layerStore;
    if (!store || typeof store.takeLayerByRef !== "function") return null;
    try {
      const byName =
        typeof store.takeLayer === "function" ? store.takeLayer("structure.full") : null;
      if (byName && typeof byName === "object") return byName;
    } catch (_) {}
    try {
      const refs = boot.lastViewRevision?.assembly_plan?.layer_refs || {};
      const pref = refs["structure.full"];
      if (pref) {
        const doc = store.takeLayerByRef(pref);
        if (doc && typeof doc === "object") return doc;
      }
    } catch (_) {}
    return null;
  }

  function normalizePlaneToken(value) {
    return String(value || "")
      .trim()
      .toLowerCase()
      .replaceAll("_", "-");
  }

  /**
   * Enumerate structure nodes for the current/visible plane from resident structure.full.
   * Scoped manifests only contain the slice — out-of-slice nodes are not invented.
   */
  function listStructureForPlane(planeId, options = {}) {
    const doc = options.document || readResidentStructureDocument();
    if (!doc || !Array.isArray(doc.nodes)) return [];
    const want = normalizePlaneToken(planeId || options.plane || boot.activePlaneId || "t1");
    const roles = new Set(
      (options.roles || ["region", "section", "slot", "content", "plane"]).map((role) =>
        String(role || "")
          .trim()
          .toLowerCase(),
      ),
    );
    return doc.nodes
      .filter((node) => {
        const role = String(node.ui_role || "").trim().toLowerCase();
        if (!roles.has(role)) return false;
        const plane = normalizePlaneToken(node.plane || "");
        if (!want || want === "all") return true;
        if (plane && plane === want) return true;
        const scope = normalizePlaneToken(node.preview_scope || "");
        return scope.split("/").includes(want);
      })
      .map((node) => ({
        node_id: String(node.node_id || "").trim(),
        preview_scope: String(node.preview_scope || "").trim(),
        ui_role: String(node.ui_role || "").trim().toLowerCase(),
        label: String(node.label || "").trim(),
        plane: String(node.plane || "").trim(),
        panel_id: String(node.panel_id || node.preview_scope || "").trim(),
        parent_id: String(node.parent_id || "").trim(),
        children: Array.isArray(node.children) ? node.children.slice() : [],
      }));
  }

  boot.previewMaterializer = {
    materializePreview,
    buildStructureTree,
    applyRuntimePlans,
    applyComposeThemeLayout,
    bindEvalSlots,
    hasMaterializedPreview,
    isClientLayerMaterialized,
    isSsrInjectedPreviewRoot,
    canSkipClientCompose,
    collectEvalDocs,
    finalizeClientPreview,
    materializePlaceholderPreview,
    ensureBootstrapBeforeInject,
    refreshComposeMaps,
    listStructureForPlane,
    readResidentStructureDocument,
    stampStructureIdentity,
  };
  boot.hasMaterializedPreview = hasMaterializedPreview;
  boot.listStructureForPlane = listStructureForPlane;
})(typeof window !== "undefined" ? window : globalThis);
