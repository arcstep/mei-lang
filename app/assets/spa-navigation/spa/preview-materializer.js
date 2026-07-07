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

  function isViewportNode(node) {
    if (node?.frame_viewport) return true;
    const haystack = `${node?.node_id || ""} ${node?.preview_scope || ""} ${node?.label || ""}`.toLowerCase();
    return haystack.includes("viewport") || haystack.includes("world_viewport");
  }

  function applyFrameViewportMeta(el, meta, docLevel) {
    const vp = meta || docLevel;
    if (!vp || !(el instanceof HTMLElement)) return;
    el.setAttribute("data-mei-frame-viewport", "true");
    el.classList.add("preview-viewport", "preview-surface");
    if (vp.design_width != null) el.setAttribute("data-design-width", String(vp.design_width));
    if (vp.design_height != null) el.setAttribute("data-design-height", String(vp.design_height));
    if (vp.scale_mode) el.setAttribute("data-scale-mode", String(vp.scale_mode));
    if (vp.overflow_mode) el.setAttribute("data-overflow-mode", String(vp.overflow_mode));
    if (vp.aspect_ratio) el.setAttribute("data-aspect-ratio", String(vp.aspect_ratio));
    if (vp.target_file) el.setAttribute("data-target-file", String(vp.target_file));
    if (vp.scene_id) el.setAttribute("data-scene-id", String(vp.scene_id));
    if (vp.route_mode) el.setAttribute("data-route-mode", String(vp.route_mode));
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
      stage.className = "preview-stage";
      stageShell.appendChild(stage);
      section.appendChild(stageShell);
      section.__meiStageTarget = stage;
      return section;
    }

    if (role === "content") {
      const keys = Array.isArray(node.use_keys) && node.use_keys.length
        ? node.use_keys
        : node.content_kind
          ? [node.content_kind]
          : [];
      if (keys.length === 1) {
        return createBlockSection(keys[0], scope, node.ui_role);
      }
      if (keys.length > 1) {
        const wrap = document.createElement("div");
        wrap.className = "mei-compose-content-group";
        if (scope) wrap.setAttribute("data-preview-scope", scope);
        keys.forEach((key) => wrap.appendChild(createBlockSection(key, scope, node.ui_role)));
        return wrap;
      }
    }

    const tag = role === "slot" || role === "section" ? "section" : "div";
    const el = document.createElement(tag);
    el.className = `mei-compose-node mei-compose-${role || "node"}`;
    if (scope) el.setAttribute("data-preview-scope", scope);
    if (node.panel_id) {
      el.setAttribute("data-mei-panel-id", String(node.panel_id));
    } else if ((role === "slot" || role === "section") && scope) {
      el.setAttribute("data-mei-panel-id", scope);
    }
    el.setAttribute("data-mei-ui-role", String(node.ui_role || ""));
    if (node.plane) el.setAttribute("data-mei-plane", String(node.plane));
    return el;
  }

  function mountTargetForParent(parentEl) {
    if (parentEl?.__meiStageTarget instanceof HTMLElement) {
      return parentEl.__meiStageTarget;
    }
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

  function findScopeContainer(root, scopeKey) {
    const scope = String(scopeKey || "").trim();
    if (!scope || scope === "scene:default") {
      return root;
    }
    return (
      root.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`) ||
      root.querySelector(`[data-mei-panel-id="${CSS.escape(scope)}"]`) ||
      null
    );
  }

  function applyPropsToHost(host, props) {
    if (!(host instanceof HTMLElement)) return;
    const serialized = JSON.stringify(props || {});
    let target =
      host.querySelector("[data-mei-use-key]") ||
      host.firstElementChild ||
      host;
    if (!(target instanceof HTMLElement)) return;
    target.setAttribute("data-props", serialized);
  }

  function bindEvalSlots(root, evalDocs) {
    if (!(root instanceof HTMLElement)) return false;
    let bound = 0;
    for (const doc of evalDocs || []) {
      const slots = doc.slots || {};
      for (const [scopeKey, entry] of Object.entries(slots)) {
        const container = findScopeContainer(root, scopeKey);
        if (!(container instanceof HTMLElement)) continue;
        const mounts = Array.isArray(entry?.mounts) ? entry.mounts : [];
        const useKeys = Array.isArray(entry?.use_keys) ? entry.use_keys : [];
        const hosts = Array.from(container.querySelectorAll(".component-host"));
        mounts.forEach((mount, index) => {
          const props = propsFromMount(mount);
          const useKey = String(useKeys[index] || "").trim();
          let host = null;
          if (useKey) {
            const block = container.querySelector(`[data-mei-use-key="${CSS.escape(useKey)}"]`);
            host = block?.querySelector?.(".component-host") || block;
          }
          if (!(host instanceof HTMLElement)) {
            host = hosts[index] || hosts[0] || null;
          }
          if (host instanceof HTMLElement) {
            applyPropsToHost(host, props);
            bound += 1;
          } else if (useKey) {
            const section = createBlockSection(useKey, scopeKey === "scene:default" ? "" : scopeKey, "content");
            applyPropsToHost(section.querySelector(".component-host"), props);
            container.appendChild(section);
            bound += 1;
          }
        });
      }
    }
    return bound > 0;
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
    return true;
  }

  function hasMaterializedPreview(root) {
    if (!(root instanceof HTMLElement)) return false;
    if (root.getAttribute("data-mei-compose-materialized") === "1") return true;
    return !!root.querySelector(
      "[data-mei-frame-viewport], [data-mei-use-key], .preview-viewport, .mei-structure-tree",
    );
  }

  function injectPreviewSurfaceHtml(root, surfaceHtml) {
    const html = String(surfaceHtml || "").trim();
    if (!(root instanceof HTMLElement) || !html) return false;
    root.innerHTML = html;
    root.removeAttribute("data-mei-compose-placeholder");
    root.removeAttribute("aria-busy");
    root.removeAttribute("data-mei-compose-materialized");
    return true;
  }

  async function fetchScenePreviewFragment(ctx, options) {
    const opts = options || {};
    const appId = String(ctx?.appId || ctx?.app_id || "").trim();
    const sceneId = String(ctx?.sceneId || ctx?.scene_id || "home").trim() || "home";
    if (!appId) return null;
    const params = new URLSearchParams({ app: appId, scene: sceneId, format: "html" });
    const dataMode = String(ctx?.dataMode || ctx?.data_mode || "").trim();
    const reviewProjection = String(ctx?.reviewProjection || ctx?.review_projection || "").trim();
    const chrome = String(ctx?.chrome || "").trim();
    if (dataMode) params.set("data_mode", dataMode);
    if (reviewProjection) params.set("review_projection", reviewProjection);
    if (chrome) params.set("chrome", chrome);
    const controller = opts.signal ? null : new AbortController();
    const signal = opts.signal || controller?.signal;
    const response = await fetch(`/api/host/scene-fragment?${params.toString()}`, {
      credentials: "same-origin",
      headers: { Accept: "application/json", "x-mei-spa-nav": "1" },
      signal,
    });
    if (!response.ok) {
      throw new Error(`scene preview fragment failed: ${response.status}`);
    }
    return await response.json();
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

  async function tryInjectCachedSurface(ctx, root, options) {
    if (!(root instanceof HTMLElement)) return false;
    const cached = await boot.previewSurfaceCache?.tryGetCachedSurface?.(ctx, options);
    if (!cached?.surfaceHtml) return false;
    if (typeof boot.renderPipelineMark === "function") {
      boot.renderPipelineMark("preview_fragment:begin");
    }
    const ok = injectPreviewSurfaceHtml(root, cached.surfaceHtml);
    if (ok && typeof boot.renderPipelineMark === "function") {
      boot.renderPipelineMark("preview_fragment:end", {
        bytes: cached.bytes || cached.surfaceHtml.length,
        source: "idb",
      });
    }
    return ok;
  }

  function preferComposePreview() {
    return global.__mei?.prefer_compose_preview === true;
  }

  function composePreviewMaterialized(root) {
    if (!(root instanceof HTMLElement)) return false;
    if (typeof boot.hasMaterializedPreview === "function" && !boot.hasMaterializedPreview(root)) {
      return false;
    }
    return !!root.querySelector(
      "[data-mei-frame-viewport], [data-preview-scope], [data-mei-use-key], .preview-viewport",
    );
  }

  async function storeSurfaceHtmlCache(ctx, surfaceHtml, options) {
    const html = String(surfaceHtml || "").trim();
    if (!html || !boot.previewSurfaceCache?.storeCachedSurface) return;
    void boot.previewSurfaceCache.storeCachedSurface(ctx, html, {
      ...(options || {}),
      source: "fragment",
    });
  }

  async function fetchAndInjectFragment(ctx, root, options) {
    const fragment = await fetchScenePreviewFragment(ctx, options);
    if (!fragment?.surfaceHtml) return false;
    const ok = injectPreviewSurfaceHtml(root, fragment.surfaceHtml);
    if (ok) {
      void storeSurfaceHtmlCache(ctx, fragment.surfaceHtml, options);
      if (typeof boot.renderPipelineMark === "function") {
        boot.renderPipelineMark("preview_fragment:end", {
          bytes: fragment.surfaceHtml.length,
          source: "network",
        });
      }
    }
    return ok;
  }

  async function hydratePlaceholderFromFragment(ctx, root, options) {
    if (!(root instanceof HTMLElement)) return false;
    if (root.getAttribute("data-mei-compose-placeholder") !== "1") return false;
    if (boot.previewMaterializer?.isSsrInjectedPreviewRoot?.(root) === true) return true;
    const opts = options || {};
    try {
      await ensureBootstrapBeforeInject(ctx);
      if (opts.skipIdb !== true) {
        const fromCache = await tryInjectCachedSurface(ctx, root, opts);
        if (fromCache) return true;
      }
      if (typeof boot.renderPipelineMark === "function") {
        boot.renderPipelineMark("preview_fragment:begin");
      }
      return await fetchAndInjectFragment(ctx, root, opts);
    } catch (error) {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("preview-fragment-hydrate-miss", {
          message: String(error?.message || error || "fragment hydrate failed"),
        });
      }
      return false;
    }
  }

  async function materializePlaceholderPreview(ctx, root, layers, options) {
    if (!(root instanceof HTMLElement)) return { ok: false, source: null };
    if (root.getAttribute("data-mei-compose-placeholder") !== "1") {
      return { ok: false, source: null };
    }
    if (isSsrInjectedPreviewRoot(root)) {
      return { ok: true, source: "ssr_preview" };
    }
    const opts = options || {};
    try {
      await ensureBootstrapBeforeInject(ctx);
      const fromCache = await tryInjectCachedSurface(ctx, root, opts);
      if (fromCache) return { ok: true, source: "idb" };

      if (preferComposePreview() && layers && boot.viewCompositor?.composeFromLayers) {
        const composeAxes = {
          ...(opts.composeAxes || {}),
          forceRematerialize: opts.forceRematerialize === true,
        };
        const composed = boot.viewCompositor.composeFromLayers(root, layers, composeAxes);
        if (composed && composePreviewMaterialized(root)) {
          return { ok: true, source: "compose" };
        }
      }

      const hydrated = await hydratePlaceholderFromFragment(ctx, root, {
        ...opts,
        skipIdb: true,
      });
      return { ok: hydrated, source: hydrated ? "fragment" : null };
    } catch (error) {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("preview-materialize-miss", {
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

    root.querySelectorAll(".mei-structure-tree").forEach((el) => el.remove());
    buildStructureTree(root, structure, composeAxes || {});

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
    return true;
  }

  function isClientLayerMaterialized(root) {
    if (!(root instanceof HTMLElement)) return false;
    if (root.getAttribute("data-mei-compose-materialized") === "1") return true;
    return !!root.querySelector("[data-mei-compose-materialized='1']");
  }

  function isSsrInjectedPreviewRoot(root) {
    if (!hasMaterializedPreview(root)) return false;
    return !isClientLayerMaterialized(root);
  }

  boot.previewMaterializer = {
    materializePreview,
    buildStructureTree,
    applyRuntimePlans,
    bindEvalSlots,
    hasMaterializedPreview,
    isClientLayerMaterialized,
    isSsrInjectedPreviewRoot,
    collectEvalDocs,
    injectPreviewSurfaceHtml,
    fetchScenePreviewFragment,
    hydratePlaceholderFromFragment,
    materializePlaceholderPreview,
    ensureBootstrapBeforeInject,
  };
  boot.hasMaterializedPreview = hasMaterializedPreview;
})(typeof window !== "undefined" ? window : globalThis);
