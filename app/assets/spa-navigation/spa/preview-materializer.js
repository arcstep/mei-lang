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
    const label = String(node?.label || "").trim().toLowerCase();
    if (label.startsWith("viewport:")) return true;
    const scope = String(node?.preview_scope || "").trim().toLowerCase();
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
      hints.add(base);
      hints.add(`${base}_card`);
    }
    return [...hints];
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

  function isSectionHeadScope(scopeKey) {
    const scope = String(scopeKey || "").trim().toLowerCase();
    return scope.endsWith("/head") || scope.endsWith("/head/mei.text");
  }

  function resolveEvalSlotLabel(entry) {
    const label = String(entry?.label || "").trim();
    if (!label || label.toLowerCase() === "head") return "";
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
        if (scope.includes(token)) {
          const score = token.length;
          if (score > bestScore) {
            best = mount;
            bestScore = score;
          }
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
    if (!label || label.toLowerCase() === "head") return false;
    const isHead =
      container.matches?.('[data-preview-scope$="/head"]') ||
      container.matches?.('[data-preview-scope$="/head/mei.text"]');
    const headSlot = isHead
      ? container
      : container.closest('[data-preview-scope$="/head"], [data-preview-scope$="/head/mei.text"]') ||
        container;
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
        const scopeSelector = `[data-preview-scope$="${CSS.escape(hint)}"], [data-preview-scope*="${CSS.escape(hint)}"], [data-mei-panel-id$="${CSS.escape(hint)}"]`;
        const scopeEl = searchRoot.querySelector(scopeSelector);
        if (!(scopeEl instanceof HTMLElement)) {
          continue;
        }
        const host =
          scopeEl.querySelector(".component-host.metric-card") ||
          scopeEl.querySelector('[data-mei-metric-card="true"] .component-host') ||
          scopeEl.querySelector(".component-host");
        if (host instanceof HTMLElement) {
          return host;
        }
      }
    }
    const useKey = String(useKeys[index] || useKeys[0] || "").trim();
    if (useKey && scopeKey !== "scene:default") {
      const scopeRoot =
        scopeKey === "scene:default" || !container ? root : container;
      const block = scopeRoot.querySelector(`[data-mei-use-key="${CSS.escape(useKey)}"]`);
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
    const scope = String(scopeKey || "").trim().toLowerCase();
    if (!scope.endsWith("/head") && !scope.endsWith("/head/mei.text")) {
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
      return Boolean(String(props.metric_role || props.metricRole || "").trim());
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
    if (template !== "stack" && template !== "stack_desc") return;
    const card = target.closest("[data-mei-metric-card]") || target;
    const titleRatio =
      card.getAttribute("data-mei-metric-title-ratio") || "2";
    const contentRatio =
      card.getAttribute("data-mei-metric-content-ratio") || "3";
    const style = target.style;
    style.display = "grid";
    style.gridTemplateColumns = "auto auto";
    style.gridTemplateRows = `${ratioFrTrack(titleRatio, 1)} ${ratioFrTrack(contentRatio, 1)}`;
    style.gridTemplateAreas = '"label label" "value unit"';
    style.alignItems = "stretch";
    style.justifyItems = "center";
    style.justifyContent = "center";
    style.gap = "0";
    style.boxSizing = "border-box";
    style.minHeight = "0";
    style.minWidth = "0";
    style.height = "100%";
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
    const roleNodes = host
      ? Array.from(host.querySelectorAll("mei-text, MEI-TEXT")).filter((node) => {
          const role = String(parseHostProps(node).metric_role || "").trim();
          return role === "label" || role === "value" || role === "unit";
        })
      : Array.from(bodyCell.querySelectorAll("mei-text, MEI-TEXT")).filter((node) => {
          const role = String(parseHostProps(node).metric_role || "").trim();
          return role === "label" || role === "value" || role === "unit";
        });
    if (!roleNodes.length && bodyCell.querySelector(":scope > .component-card")) {
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

  function applyHeadChromeFromSlot(headEl, headChrome) {
    if (!(headEl instanceof HTMLElement) || !headChrome || typeof headChrome !== "object") {
      return false;
    }
    const title = String(headChrome.title || "").trim() || "板块标题";
    const classes = Array.isArray(headChrome.heading_classes)
      ? headChrome.heading_classes.filter(Boolean).join(" ")
      : "panel-heading panel-heading-plain panel-heading-compact";
    const cellStyle = String(headChrome.cell_style || "").trim();
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
    if (target.classList.contains("mei-compose-content-group")) {
      target = target;
    } else if (!target.classList.contains("preview-card") && !target.classList.contains("mei-compose-block")) {
      const content = target.closest('[data-mei-ui-role="content"]');
      if (content instanceof HTMLElement) target = content;
    }
    applyContainerVisualStyle(target, panelShell.props);
    target.setAttribute("data-mei-panel-shell-applied", "1");
    return true;
  }

  function normalizeSectionHeadSlot(headSlot) {
    if (!(headSlot instanceof HTMLElement)) return;
    if (headSlot.getAttribute("data-mei-section-head-chrome") === "1") return;
    const scope = String(headSlot.getAttribute("data-preview-scope") || "");
    if (!scope.endsWith("/head") && !scope.endsWith("/head/mei.text")) return;

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
      if (current && current.toLowerCase() !== "head") {
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
      titleText = label && label.toLowerCase() !== "head" ? label.replace(/_/g, " ") : "板块标题";
    }

    headSlot.className = "mei-compose-slot preview-card preview-card-bare mei-compose-section-head";
    headSlot.innerHTML = buildSectionHeadMarkup(titleText);
    headSlot.setAttribute("data-mei-section-head-normalized", "1");
  }

  function normalizeAllSectionHeadSlots(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll('[data-preview-scope$="/head"], [data-preview-scope$="/head/mei.text"]').forEach((headSlot) => {
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
    section.className = "preview-card mei-compose-block";
    const key = String(useKey || "").trim();
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
      return section;
    }

    if (role === "content") {
      if (isViewportMetaContentNode(node)) {
        const placeholder = document.createElement("div");
        placeholder.className = "mei-compose-viewport-meta";
        placeholder.hidden = true;
        if (scope) placeholder.setAttribute("data-preview-scope", scope);
        return placeholder;
      }
      const keys = Array.isArray(node.use_keys) && node.use_keys.length
        ? node.use_keys
        : node.content_kind
          ? [node.content_kind]
          : [];
      if (keys.length === 1) {
        const key = keys[0];
        if (isMetricTemplateKind(key)) {
          const scopeLower = scope.toLowerCase();
          if (scopeLower.includes("/hint/") || scopeLower.includes("stage-aperture-hint")) {
            return createBlockSection("mei.text", scope, node.ui_role);
          }
          return createMetricCardSection(scope, node.ui_role, node.label);
        }
        return createBlockSection(key, scope, node.ui_role);
      }
      if (keys.length > 1) {
        const wrap = document.createElement("div");
        wrap.className = "mei-compose-content-group";
        if (scope) wrap.setAttribute("data-preview-scope", scope);
        if (node.label) wrap.setAttribute("data-mei-structure-label", String(node.label));
        keys.forEach((key) => wrap.appendChild(createBlockSection(key, scope, node.ui_role)));
        return wrap;
      }
    }

    const tag =
      role === "slot" || role === "section" || role === "region" ? "section" : "div";
    const el = document.createElement(tag);
    el.className = `mei-compose-node mei-compose-${role || "node"}`;
    if (scope) el.setAttribute("data-preview-scope", scope);
    if (node.panel_id) {
      el.setAttribute("data-mei-panel-id", String(node.panel_id));
    } else if ((role === "slot" || role === "section" || role === "region") && scope) {
      el.setAttribute("data-mei-panel-id", scope);
    }
    el.setAttribute("data-mei-ui-role", String(node.ui_role || ""));
    if (node.label) {
      el.setAttribute("data-mei-structure-label", String(node.label));
    }
    const planeCode = String(node.plane || "").trim();
    if (planeCode) {
      el.setAttribute("data-mei-plane", planeCode);
      el.setAttribute("data-mei-tier", planeCode);
    }
    return el;
  }

  function mountTargetForParent(parentEl) {
    return parentEl;
  }

  function buildStructureTree(root, structureDoc, options) {
    if (!(root instanceof HTMLElement)) return false;
    const doc = extractLayerDocument(structureDoc);
    const allNodes = Array.isArray(doc?.nodes) ? doc.nodes : [];
    if (!allNodes.length) return false;

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
          : nodes
              .filter((candidate) => candidate.parent_id === node.node_id)
              .map((candidate) => candidate.node_id);
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

  const DEFAULT_COCKPIT_FOCUS_INSET = Object.freeze({
    top: "84px",
    left: "0px",
    right: "532px",
    bottom: "0px",
  });

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
      return { ...DEFAULT_COCKPIT_FOCUS_INSET };
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
      if (isUnresolvedMeiRef(viewport.focusInset)) {
        viewport.focusInset = { ...DEFAULT_COCKPIT_FOCUS_INSET };
      }
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
      active_scene_id: runtimeRef.scene_id,
      active_target_file:
        viewport?.getAttribute("data-target-file") ||
        shell?.getAttribute("data-compile-target") ||
        "src/scene/home/assembly.mei",
      entry_target:
        viewport?.getAttribute("data-target-file") ||
        shell?.getAttribute("data-compile-target") ||
        "src/scene/home/assembly.mei",
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

  function findScopeContainer(root, scopeKey) {
    const scope = String(scopeKey || "").trim();
    if (!scope || scope === "scene:default") {
      return root;
    }
    for (const candidate of scopeLookupCandidates(scope)) {
      const el =
        root.querySelector(`[data-preview-scope="${CSS.escape(candidate)}"]`) ||
        root.querySelector(`[data-mei-panel-id="${CSS.escape(candidate)}"]`);
      if (el instanceof HTMLElement) {
        return el;
      }
    }
    return null;
  }

  function resolveEvalSlotContainer(root, scopeKey) {
    let container = findScopeContainer(root, scopeKey);
    if (container instanceof HTMLElement) {
      return container;
    }
    const scope = String(scopeKey || "").trim();
    if (scope.endsWith("/head/mei.text")) {
      return findScopeContainer(root, scope.replace(/\/mei\.text$/, ""));
    }
    return null;
  }

  function promoteSectionHeadMeiTextNodes(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll('[data-preview-scope*="/head/mei.text"]').forEach((node) => {
      if (!(node instanceof HTMLElement)) return;
      const scope = String(node.getAttribute("data-preview-scope") || "").trim();
      if (!scope.endsWith("/head/mei.text")) return;
      const title = String(
        node.getAttribute("data-mei-eval-label") ||
          node.getAttribute("data-mei-structure-label") ||
          "",
      ).trim();
      if (!title || title.toLowerCase() === "head") return;
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
        if (!String(scopeKey || "").endsWith("/head/mei.text")) continue;
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

  function applyBackgroundInlineStyle(style, background) {
    if (!style || background == null) return;
    if (typeof background === "string") {
      const value = String(background).trim();
      if (!value) return;
      if (
        value.startsWith("linear-gradient") ||
        value.startsWith("radial-gradient") ||
        value.startsWith("repeating-linear-gradient") ||
        value.startsWith("repeating-radial-gradient")
      ) {
        style.backgroundImage = value;
      } else {
        style.background = value;
      }
      return;
    }
    if (typeof background !== "object") return;
    const image = String(background.image || "").trim();
    if (image) {
      if (
        image.startsWith("linear-gradient") ||
        image.startsWith("radial-gradient") ||
        image.startsWith("repeating-linear-gradient") ||
        image.startsWith("repeating-radial-gradient")
      ) {
        style.backgroundImage = image;
      } else {
        style.backgroundImage = `url("${image.replace(/"/g, '\\"')}")`;
      }
      const size = String(background.size || "").trim();
      if (size) style.backgroundSize = size;
      const position = String(background.position || "").trim();
      if (position) style.backgroundPosition = position;
      const repeat = String(background.repeat || "").trim();
      if (repeat) style.backgroundRepeat = repeat;
      return;
    }
    const color = String(background.color || "").trim();
    if (color) style.background = color;
  }

  function applyContainerVisualStyle(el, props) {
    if (!(el instanceof HTMLElement) || !props || typeof props !== "object") return;
    const style = el.style;
    applyBackgroundInlineStyle(style, props.background);
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
      style.pointerEvents = pointerEvents.trim();
    }
    if (props.__mei_metric_template != null) {
      el.setAttribute("data-mei-metric-template", String(props.__mei_metric_template));
    }
    if (props.__mei_metric_density != null) {
      el.setAttribute("data-mei-metric-density", String(props.__mei_metric_density));
    }
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
      const shellProps = shellMount.props;
      if (String(shellProps.chrome || "").trim() === "bare") {
        section.classList.add("preview-card-bare");
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
      target =
        host.querySelector("[data-mei-use-key]") ||
        host.firstElementChild ||
        host;
    }
    if (!(target instanceof HTMLElement)) return;
    if (target.getAttribute("data-props") === serialized) return;
    target.setAttribute("data-props", serialized);
    if (typeof target._bind === "function") {
      try {
        target._bind();
      } catch (_) {}
    } else if (typeof target.render === "function") {
      try {
        target.render();
      } catch (_) {}
    }
  }

  function ensureComponentHostChildren(host, mounts, sceneMountByMetric) {
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
      const props = enrichRuntimeMetricRef(
        enrichComposeComponentProps(rawProps),
        sceneMount,
      );
      const metricRole = String(props.metric_role || props.metricRole || "").trim();
      const tag = resolveComponentTag(useKey);
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
      let target = host.querySelector(selector);
      if (!(target instanceof HTMLElement) && tag) {
        target = document.createElement(tag);
        target.setAttribute("data-mei-use-key", useKey);
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
        container.querySelector(`[data-preview-scope="${CSS.escape(scope)}"] .component-host`) ||
        container.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`);
      if (scoped instanceof HTMLElement) {
        return scoped.classList?.contains("component-host")
          ? scoped
          : scoped.querySelector(".component-host") || scoped;
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
    const fallback = container.querySelector(".component-host");
    return fallback instanceof HTMLElement ? fallback : null;
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
    if (
      normalized.includes("home_header") ||
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

  function applyT1GridLayout(container, units, grid) {
    if (!(container instanceof HTMLElement) || !units.length || !grid) return;
    container.style.display = "grid";
    container.style.width = "100%";
    container.style.height = "100%";
    container.style.minHeight = "0";
    container.style.gridTemplateRows = grid.rows;
    container.style.gridTemplateColumns = grid.columns;
    container.style.gridTemplateAreas = grid.areas;

    units.forEach((unit) => {
      const scope = unit.getAttribute("data-preview-scope") || "";
      if (isOverlayRegionScope(scope)) {
        unit.style.gridArea = grid.overlayArea;
        unit.style.position = "relative";
        unit.style.pointerEvents = "none";
        unit.style.minHeight = "0";
        unit.style.minWidth = "0";
        return;
      }
      const area = resolveRegionGridArea(scope);
      if (area) unit.style.gridArea = area;
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

    if (layoutRegions.length === 1) {
      const region = layoutRegions[0];
      const nested = layoutUnitsFor(region).filter((unit) => {
        const scope = unit.getAttribute("data-preview-scope") || "";
        return !isLayoutDebugScope(scope);
      });
      const nestedScopes = nested.map((unit) => unit.getAttribute("data-preview-scope") || "");
      const nestedGrid = inferT1PlaneGrid(nestedScopes);
      if (nestedGrid && nested.length > 1) {
        return { container: region, units: nested, grid: nestedGrid };
      }
    }

    const regionScopes = layoutRegions.map(
      (region) => region.getAttribute("data-preview-scope") || "",
    );
    const multiRegionGrid = inferT1PlaneGrid(regionScopes);
    if (multiRegionGrid && layoutRegions.length > 1) {
      return { container: planeEl, units: layoutRegions, grid: multiRegionGrid };
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
        return { container: region, units: sections, grid: sectionGrid };
      }
    }

    if (multiRegionGrid) {
      return { container: planeEl, units: layoutRegions, grid: multiRegionGrid };
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
      planeEl.style.gridTemplate = '"stage" 1fr / 1fr';
      regions.forEach((region) => {
        region.style.gridArea = "stage";
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
      planeEl.style.gridTemplate = '"main" 1fr / 1fr';
      regions.forEach((region) => {
        region.style.gridArea = "main";
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
    applyT1GridLayout(layout.container, layout.units, layout.grid);
  }

  function wrapStructureTreeInSceneViewport(root, tree, structureDoc) {
    const vpMeta = resolveSceneViewportMeta(structureDoc);
    if (!(tree instanceof HTMLElement) || !(root instanceof HTMLElement)) return false;

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
        if (el.classList.contains("component-host") || el.classList.contains("preview-card")) {
          el.style.position = "absolute";
          el.style.inset = "0";
        }
      }
      el = el.parentElement;
    }
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
      if (!rail.style.gridTemplateRows) {
        rail.style.display = "grid";
        rail.style.gridTemplateRows = `repeat(${sections.length}, minmax(0, 1fr))`;
      }
      rail.style.minHeight = "0";
      rail.style.height = "100%";
      rail.style.overflow = "hidden";
      if (!rail.style.rowGap && !rail.style.gap) {
        rail.style.rowGap = "12px";
      }
      sections.forEach((section) => {
        section.style.minHeight = "0";
        section.style.minWidth = "0";
        section.style.overflow = "hidden";
      });
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
    applyRailRegionSectionLayouts(root);
    normalizeT1InteractivePointerEvents(tree);
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

    tree.querySelectorAll('[data-mei-ui-role="plane"], .mei-compose-plane').forEach((plane) => {
      if (!(plane instanceof HTMLElement)) return;
      applyPlaneRegionLayout(plane);
    });

    normalizeComposeCockpitLayouts(root);

    if (global.MeiProjectionDepth?.applyLayoutBudgetManifest) {
      global.MeiProjectionDepth.applyLayoutBudgetManifest(root.ownerDocument || document);
    }
    return true;
  }

  function notifyPreviewComposed(root) {
    if (!(root instanceof HTMLElement)) return;
    root.removeAttribute("data-mei-compose-placeholder");
    root.removeAttribute("aria-busy");
    try {
      if (typeof global.__meiSyncRuntimeQueryAppContext === "function") {
        global.__meiSyncRuntimeQueryAppContext({ clearCaches: false });
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

  function bindEvalSlots(root, evalDocs) {
    if (!(root instanceof HTMLElement)) return false;
    let bound = 0;
    const sceneMountByMetric = sceneMountsByMetricId(evalDocs);
    for (const doc of evalDocs || []) {
      const slots = doc.slots || {};
      for (const [scopeKey, entry] of Object.entries(slots)) {
        if (isDuplicateMetricCardLeafScope(scopeKey)) continue;
        const container = resolveEvalSlotContainer(root, scopeKey);
        if (!(container instanceof HTMLElement)) continue;
        const mounts = Array.isArray(entry?.mounts) ? entry.mounts : [];
        const useKeys = Array.isArray(entry?.use_keys) ? entry.use_keys : [];
        const componentMounts = Array.isArray(entry?.component_mounts)
          ? entry.component_mounts
          : [];
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
        if (bindComponentMounts) {
          const filteredMounts = filterComponentMountsForScope(scopeKey, componentMounts);
          let host = findComponentHostForScope(container, scopeKey, useKeys);
          if (!(host instanceof HTMLElement)) {
            host = ensureMetricCardComponentHost(container);
          }
          if (host instanceof HTMLElement) {
            bound += ensureComponentHostChildren(host, filteredMounts, sceneMountByMetric);
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
            inferSceneMountForScope(scopeKey, sceneMountByMetric),
          );
          if (host instanceof HTMLElement) {
            bound += ensureComponentHostChildren(host, synthesized, sceneMountByMetric);
          }
          applyMetricCardShellFromMounts(container, synthesized);
        }
        if (!headScope && entry?.panel_shell && typeof entry.panel_shell === "object") {
          applyPanelShellFromSlot(container, entry.panel_shell);
        }
        mounts.forEach((mount, index) => {
          const props = enrichRuntimeMetricRef(
            enrichComposeComponentProps(propsFromMount(mount)),
            mount,
          );
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
    rebindMetricCardHosts(root);
    applyWarningSupervisionComposeClasses(root);
    promoteSectionHeadMeiTextNodes(root);
    applyRailHeadTitlesFromEval(root, evalDocs);
    root.querySelectorAll('[data-mei-section-head-normalized="1"]').forEach((head) => {
      const h3 = head.querySelector("h3");
      const text = String(h3?.textContent || "").trim().toLowerCase();
      if (!text || text === "head" || text === "板块标题") {
        head.removeAttribute("data-mei-section-head-normalized");
      }
    });
    normalizeAllSectionHeadSlots(root);
    suppressDuplicateMetricCardLeafSlots(root);
    normalizeMapStageHintPointerEvents(root);
    normalizeT1InteractivePointerEvents(root);
    return bound > 0;
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

  function shouldT1UnitReceivePointerEvents(scope) {
    const normalized = String(scope || "").trim().toLowerCase();
    if (!normalized || normalized.includes("layout_debug")) return false;
    if (
      normalized.includes("stage_aperture") ||
      normalized.includes("stage-aperture") ||
      normalized.includes("viewport_frame") ||
      normalized.includes("world_viewport")
    ) {
      return false;
    }
    if (normalized.endsWith("/map_stage") || normalized.includes("/map_stage/")) {
      return false;
    }
    return (
      normalized.includes("right_rail") ||
      normalized.includes("left_rail") ||
      normalized.includes("header") ||
      normalized.includes("center_rail") ||
      normalized.includes("center_top") ||
      normalized.includes("realtime_center")
    );
  }

  function normalizeT1InteractivePointerEvents(root) {
    if (!(root instanceof HTMLElement)) return;
    root
      .querySelectorAll(
        '.mei-compose-plane[data-mei-plane="T1"], .mei-compose-plane[data-mei-plane="t1"]',
      )
      .forEach((plane) => {
        if (!(plane instanceof HTMLElement)) return;
        plane.querySelectorAll(".mei-compose-section, .mei-compose-slot").forEach((el) => {
          if (!(el instanceof HTMLElement)) return;
          const scope = el.getAttribute("data-preview-scope") || "";
          if (shouldT1UnitReceivePointerEvents(scope)) {
            el.style.pointerEvents = "auto";
          }
        });
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
    if (Array.isArray(doc.component_assets) && doc.component_assets.length) {
      global.__mei.component_assets = doc.component_assets;
    }
    if (doc.theme_layout != null && typeof doc.theme_layout === "object") {
      global.__mei.theme_layout = doc.theme_layout;
    }
    if (doc.layout_budget_manifest != null) {
      global.__mei.layout_budget_manifest = doc.layout_budget_manifest;
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
    const existing = global.__mei?.presentation_map;
    if (existing && typeof existing === "object" && Object.keys(existing).length > 0) {
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
      const map = payload?.presentation_map || payload?.map || payload;
      if (!map || typeof map !== "object") return;
      global.__mei = global.__mei || {};
      global.__mei.presentation_map = map;
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

  function finalizeClientPreview(root, layers, composeAxes) {
    if (!(root instanceof HTMLElement) || !layers) return false;
    const projection = String(
      composeAxes?.review_projection || composeAxes?.reviewProjection || "",
    )
      .trim()
      .toLowerCase();
    const bindEvalContent =
      !projection || projection.includes("full") || projection === "live" || projection === "static";
    if (bindEvalContent) {
      bindEvalSlots(root, collectEvalDocs(layers));
    }
    root.setAttribute("data-mei-compose-materialized", "1");
    root.removeAttribute("data-mei-compose-placeholder");
    root.removeAttribute("aria-busy");
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
    applyComposeStructureLayout(root, structure);
    applyComposeThemeLayout(root);

    const projection = String(
      composeAxes?.review_projection || composeAxes?.reviewProjection || "",
    ).trim()
      .toLowerCase();
    const bindEvalContent =
      !projection || projection.includes("full") || projection === "live" || projection === "static";
    if (bindEvalContent) {
      bindEvalSlots(root, collectEvalDocs(layers));
    }

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
  };
  boot.hasMaterializedPreview = hasMaterializedPreview;
})(typeof window !== "undefined" ? window : globalThis);
