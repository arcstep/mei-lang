/**
 * ViewCompositor: compose review_projection depth without refetching structure.full.
 * Layout overlays come from ops.themes.*.layout (+ theme.layout session draft) and
 * manifest / layout_policy projection.
 */
(function initViewCompositor(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  const PROJECTION_MAX_ROLE = {
    plane_region: "region",
    plane_region_section: "section",
    plane_region_section_slot: "slot",
    content: "content",
    live_full: "content",
    static_full: "content",
  };

  function roleDepth(role) {
    const map = { plane: 0, region: 1, section: 2, slot: 3, content: 4 };
    return map[String(role || "").toLowerCase()] ?? 99;
  }

  function nodesForProjection(structureDoc, projection) {
    const maxRole = PROJECTION_MAX_ROLE[String(projection || "").toLowerCase()] || "content";
    const maxDepth = roleDepth(maxRole);
    const nodes = Array.isArray(structureDoc?.nodes) ? structureDoc.nodes : [];
    return nodes.filter((node) => roleDepth(node.ui_role) <= maxDepth);
  }

  function clearComposeArtifacts(root) {
    if (!(root instanceof HTMLElement)) return;
    root.querySelectorAll(".mei-compose-hidden").forEach((el) => {
      if (!(el instanceof HTMLElement)) return;
      el.classList.remove("mei-compose-hidden");
      el.removeAttribute("hidden");
    });
  }

  function themeLayoutScopeToPreview(scope) {
    const normalized = String(scope || "")
      .trim()
      .replace(/^\/+|\/+$/g, "");
    const t1 = normalized.match(/^home\/T1\/(.+)$/i);
    if (t1) return `t1/${t1[1]}`;
    const t1lower = normalized.match(/^home\/t1\/(.+)$/i);
    if (t1lower) return `t1/${t1lower[1]}`;
    return normalized;
  }

  function resolveLayoutOverlayNode(root, scope) {
    if (!(root instanceof HTMLElement)) return null;
    const candidates = [
      String(scope || "").trim(),
      themeLayoutScopeToPreview(scope),
    ].filter(Boolean);
    for (const key of candidates) {
      const node = root.querySelector(`[data-preview-scope="${CSS.escape(key)}"]`);
      if (node instanceof HTMLElement) return node;
    }
    return null;
  }

  function applyThemeAndOverlay(root, themeTokens, layoutOverlay) {
    if (!(root instanceof HTMLElement)) return;
    const colors = themeTokens?.colors || {};
    const fonts = themeTokens?.fonts || {};
    Object.keys(colors).forEach((token) => {
      root.style.setProperty(`--mei-${token}`, String(colors[token]));
    });
    Object.keys(fonts).forEach((token) => {
      root.style.setProperty(`--mei-font-${token}`, String(fonts[token]));
    });
    const patches = layoutOverlay?.patches;
    if (patches && typeof patches === "object") {
      root.setAttribute("data-layout-overlay", JSON.stringify(patches));
      Object.entries(patches).forEach(([scope, patch]) => {
        if (!patch || typeof patch !== "object") return;
        const node = resolveLayoutOverlayNode(root, scope);
        if (!(node instanceof HTMLElement)) return;
        const paddingProfile = patch.paddingProfile ?? patch.padding_profile;
        if (paddingProfile) {
          node.dataset.manifestPaddingProfile = String(paddingProfile);
        }
        const sectionRows = patch.sectionRows || patch.section_rows;
        const manifestEntry = globalThis.__mei?.layout_budget_manifest?.entries?.[scope];
        const manifestGridRows =
          manifestEntry?.grid_template_rows ?? manifestEntry?.gridTemplateRows;
        if (
          Array.isArray(sectionRows) &&
          sectionRows.length > 0 &&
          !manifestGridRows
        ) {
          node.style.display = "grid";
          node.style.gridTemplateRows = sectionRows.map((row) => String(row)).join(" ");
          node.style.minHeight = "0";
          node.style.height = "100%";
          node.style.overflow = "hidden";
          node.dataset.manifestSectionRows = sectionRows.join(",");
        }
        const columns = patch.columns;
        if (columns && typeof columns === "object") {
          const left = columns.left || columns.left_rail;
          const center = columns.center || columns.center_rail;
          const right = columns.right || columns.right_rail;
          if (left && center && right) {
            node.style.display = "grid";
            node.style.gridTemplateColumns = `${left} ${center} ${right}`;
            node.style.minHeight = "0";
            node.style.height = "100%";
          }
        }
        const headerHeight = patch.headerHeight ?? patch.header_height;
        if (headerHeight != null && headerHeight !== "") {
          node.style.setProperty("--mei-t1-header-height", String(headerHeight));
        }
        const gapFr = patch.gap ?? patch.stripGap;
        if (gapFr != null && gapFr !== "") {
          node.style.gap = String(gapFr).endsWith("px") ? String(gapFr) : `${gapFr}px`;
        }
      });
    }
  }

  function extractLayerDocument(layerValue) {
    if (!layerValue) return null;
    if (Array.isArray(layerValue.nodes) || layerValue.schema_version) {
      return layerValue;
    }
    if (layerValue.document) return layerValue.document;
    return layerValue;
  }

  function ensureStructureSkeleton(root, structureDoc) {
    if (!(root instanceof HTMLElement)) return false;
    if (
      root.querySelector(
        "[data-preview-scope], [data-mei-ui-role], [data-mei-frame-viewport], [data-mei-use-key], .preview-viewport",
      )
    ) {
      return true;
    }
    const doc = extractLayerDocument(structureDoc);
    const nodes = Array.isArray(doc?.nodes) ? doc.nodes : [];
    for (const node of nodes) {
      const scope = String(node.preview_scope || "").trim();
      if (!scope) continue;
      const el = document.createElement("div");
      el.setAttribute("data-preview-scope", scope);
      el.setAttribute("data-mei-ui-role", String(node.ui_role || ""));
      el.className = `mei-compose-node mei-compose-${String(node.ui_role || "node").toLowerCase()}`;
      root.appendChild(el);
    }
    return nodes.length > 0;
  }

  function pickManifestShellLayer(_surface) {
    const layers = globalThis.__mei?.scene_manifest_refs?.layers;
    if (!layers || typeof layers !== "object") return null;
    // Stage-only Access: only shell.app is materialized.
    return layers["shell.app"] || null;
  }

  function isPlaceholderShellDoc(doc) {
    if (!doc) return true;
    const top = String(doc.topbar_html || "").trim();
    if (!top) return true;
    return top.includes('class="mei-shell-topbar"') && top.length < 240;
  }

  function applyShellLayer(root, shellLayer) {
    if (!(root instanceof HTMLElement)) return;
    let doc = extractLayerDocument(shellLayer);
    if (isPlaceholderShellDoc(doc)) {
      const manifestDoc = extractLayerDocument(pickManifestShellLayer());
      if (manifestDoc && !isPlaceholderShellDoc(manifestDoc)) {
        doc = manifestDoc;
      }
    }
    if (!doc) return;
    const topbar = String(doc.topbar_html || "").trim();
    const statusbar = String(doc.statusbar_html || "").trim();
    const signature = String(
      shellLayer?.content_hash ||
        shellLayer?.artifact_id ||
        doc.revision_digest ||
        `${topbar.length}:${statusbar.length}:${doc.tab || ""}:${doc.chrome || ""}`,
    );
    if (signature && root.getAttribute("data-mei-shell-digest") === signature) return;
    const startedAt = typeof performance !== "undefined" ? performance.now() : Date.now();
    boot.renderPipelineMark?.("apply_chrome:begin");
    if (doc.tab) root.setAttribute("data-tab", String(doc.tab));
    if (doc.chrome) root.setAttribute("data-chrome", String(doc.chrome));
    if (doc.route_mode) root.setAttribute("data-route-mode", String(doc.route_mode));
    const topSlot = global.document?.getElementById?.("mei-host-topbar-slot");
    const bottomSlot = global.document?.getElementById?.("mei-host-statusbar-slot");
    if (topbar && topSlot instanceof HTMLElement) {
      topSlot.innerHTML = topbar;
    } else if (topbar && !global.document?.querySelector?.(".topbar-shell, .mei-shell-topbar")) {
      const wrap = document.createElement("div");
      wrap.innerHTML = topbar;
      const bar = wrap.firstElementChild;
      const host = global.document?.getElementById?.("mei-compose-host") || root;
      if (bar && host instanceof HTMLElement) host.prepend(bar);
    }
    if (statusbar && bottomSlot instanceof HTMLElement) {
      bottomSlot.innerHTML = statusbar;
    } else if (statusbar && !global.document?.querySelector?.(".statusbar-shell, .statusbar")) {
      const wrap = document.createElement("div");
      wrap.innerHTML = statusbar;
      const bar = wrap.firstElementChild;
      const host = global.document?.getElementById?.("mei-compose-host") || root;
      if (bar && host instanceof HTMLElement) host.append(bar);
    }
    if (signature) root.setAttribute("data-mei-shell-digest", signature);
    boot.renderPipelineMark?.("apply_chrome:end", {
      durationMs: Math.round(
        (typeof performance !== "undefined" ? performance.now() : Date.now()) - startedAt,
      ),
    });
    try {
      global.document?.dispatchEvent?.(
        new CustomEvent("mei:shell-layer-applied", {
          detail: { signature, routeMode: doc.route_mode || null },
        }),
      );
    } catch (_error) {
      // ignore
    }
  }

  function resolveAppId(composeAxes) {
    const fromAxes = String(composeAxes?.app_id || composeAxes?.appId || "").trim();
    if (fromAxes) return fromAxes;
    return String(
      global.document?.querySelector?.(".shell[data-app-path]")?.getAttribute("data-app-path") || "",
    ).trim();
  }

  function mergePersistedAndSession(persistedTheme, persistedOverlay, appId) {
    const store = global.MeiDraftLayerStore || boot.draftLayerStore;
    const session = store?.getSessionLayers?.(appId) || {};
    const themeEffective = store?.mergeThemeDocs
      ? store.mergeThemeDocs(persistedTheme, session.themeTokens)
      : persistedTheme;
    const overlayEffective = store?.mergeOverlayDocs
      ? store.mergeOverlayDocs(persistedOverlay, session.themeLayout)
      : persistedOverlay;
    return { themeEffective, overlayEffective };
  }

  function surfaceSlugFromComposeAxes(composeAxes) {
    const fromAxes = String(composeAxes?.surface || composeAxes?.mode || "")
      .trim()
      .toLowerCase();
    if (fromAxes) return fromAxes;
    if (typeof boot.parseViewContext === "function") {
      const ctx = boot.parseViewContext(global.location.href);
      return String(ctx?.surface || ctx?.mode || "app")
        .trim()
        .toLowerCase();
    }
    return "app";
  }

  function pickShellLayer(layers, _composeAxes) {
    if (!layers || typeof layers !== "object") return null;
    // Stage-only Access: only shell.app is materialized.
    return layers["shell.app"] || null;
  }

  function recomposeFromLayerStore(appId, composeAxes) {
    const root =
      global.document?.querySelector?.(".preview-pane-scroll, .preview-pane, #mei-compose-host") ||
      null;
    if (!(root instanceof HTMLElement)) return false;
    const layers = {};
    const manifestLayers = globalThis.__mei?.scene_manifest_refs?.layers;
    if (manifestLayers && typeof manifestLayers === "object") {
      Object.assign(layers, manifestLayers);
    }
    if (boot.layerStore?.listHoldings) {
      const sceneId =
        String(composeAxes?.scene_id || composeAxes?.sceneId || "").trim() ||
        String(new URL(global.location.href).searchParams.get("scene") || "home").trim() ||
        "home";
      const holdings = boot.layerStore.listHoldings(appId, sceneId);
      if (Array.isArray(holdings)) {
        holdings.forEach((holding) => {
          const doc = boot.layerStore.takeLayerByRef?.(holding);
          if (doc && holding?.layer_id) {
            layers[holding.layer_id] = doc;
          }
        });
      }
    }
    return composeFromLayers(root, layers, { ...(composeAxes || {}), app_id: appId });
  }

  function resolveEvalBootstrapSeed(evalRaw) {
    if (
      Array.isArray(globalThis.__mei?.bootstrap_metrics) &&
      globalThis.__mei.bootstrap_metrics.length > 0
    ) {
      return {
        client_revision: globalThis.__mei.client_revision || "",
        scope: globalThis.__mei.bootstrap_scope || "",
        workset_id: globalThis.__mei.bootstrap_compile_epoch || "",
        metrics: globalThis.__mei.bootstrap_metrics,
      };
    }
    const docSeed = evalRaw?.document?.bootstrap_seed;
    if (docSeed && typeof docSeed === "object") {
      return docSeed;
    }
    if (evalRaw && typeof evalRaw === "object" && evalRaw.bootstrap_seed) {
      return evalRaw.bootstrap_seed;
    }
    const manifestEval =
      globalThis.__mei?.scene_manifest_refs?.layers?.["eval.slot_group.scene:default"];
    if (manifestEval?.bootstrap_seed) {
      return manifestEval.bootstrap_seed;
    }
    return null;
  }

  function isWorkspacePreviewRoot(root, composeAxes) {
    const surface = surfaceSlugFromComposeAxes(composeAxes);
    if (typeof boot.isWorkspaceComposeSurface === "function" && !boot.isWorkspaceComposeSurface(surface)) {
      return false;
    }
    const workspaceRoot = global.document?.querySelector?.("#mei-surface-workspace .preview-pane-scroll");
    return (
      root instanceof HTMLElement &&
      workspaceRoot instanceof HTMLElement &&
      (root === workspaceRoot || !!root.closest?.("#mei-surface-workspace"))
    );
  }

  function hasEstablishedWorkspacePreview(root) {
    if (!(root instanceof HTMLElement)) return false;
    return !!root.querySelector(
      "[data-mei-frame-viewport], [data-preview-scope], .preview-viewport, .preview-board-mounted",
    );
  }

  function composeFromLayers(root, layers, composeAxes) {
    if (!(root instanceof HTMLElement) || !layers) return false;
    const projection =
      composeAxes?.review_projection ||
      composeAxes?.reviewProjection ||
      "live_full";
    const structure = extractLayerDocument(layers["structure.full"]);
    if (!structure) return false;
    applyShellLayer(root, pickShellLayer(layers, composeAxes));
    const materializer = boot.previewMaterializer;
    const forceRematerialize = composeAxes?.forceRematerialize === true;
    const workspacePreviewRoot = isWorkspacePreviewRoot(root, composeAxes);
    const thinShellPlaceholder =
      root instanceof HTMLElement && root.getAttribute("data-mei-compose-placeholder") === "1";
    const preserveWorkspaceDom =
      !forceRematerialize &&
      workspacePreviewRoot &&
      hasEstablishedWorkspacePreview(root);
    const keepSsrPreview =
      !forceRematerialize &&
      root.getAttribute("data-mei-compose-placeholder") !== "1" &&
      typeof materializer?.canSkipClientCompose === "function" &&
      materializer.canSkipClientCompose(root, {
        surface: composeAxes?.route_mode || composeAxes?.routeMode || "app",
      });
    const shouldMaterializePreview =
      !keepSsrPreview &&
      !preserveWorkspaceDom &&
      typeof materializer?.materializePreview === "function";
    if (shouldMaterializePreview && forceRematerialize && root instanceof HTMLElement) {
      root.innerHTML = "";
      root.removeAttribute("data-mei-compose-materialized");
      root.removeAttribute("data-compose-projection");
      root.removeAttribute("data-review-projection");
      root.removeAttribute("data-review-projection-active");
    }
    if (shouldMaterializePreview) {
      materializer.materializePreview(root, layers, composeAxes);
    } else if (!keepSsrPreview && !preserveWorkspaceDom && !thinShellPlaceholder) {
      ensureStructureSkeleton(root, structure);
    }
    if (
      !shouldMaterializePreview &&
      typeof materializer?.applyRuntimePlans === "function" &&
      layers["runtime.plans"]
    ) {
      materializer.applyRuntimePlans(layers["runtime.plans"]);
    }
    const projectionSlug = String(projection || "").trim().toLowerCase();
    const bindEvalContent =
      !projectionSlug ||
      projectionSlug.includes("full") ||
      roleDepth(PROJECTION_MAX_ROLE[projectionSlug] || "content") >= roleDepth("content");
    const evalAlreadyBound =
      shouldMaterializePreview &&
      root instanceof HTMLElement &&
      root.getAttribute("data-mei-compose-materialized") === "1";
    if (
      bindEvalContent &&
      !evalAlreadyBound &&
      typeof materializer?.bindEvalSlots === "function"
    ) {
      const evalDocs =
        typeof materializer.collectEvalDocs === "function"
          ? materializer.collectEvalDocs(layers)
          : [];
      materializer.bindEvalSlots(root, evalDocs);
    }
    const themeDoc = extractLayerDocument(layers["theme.tokens"]);
    const overlayDoc = extractLayerDocument(layers["layout.overlay"]);
    const appId = resolveAppId(composeAxes);
    const { themeEffective, overlayEffective: sessionOverlay } = mergePersistedAndSession(
      themeDoc,
      overlayDoc,
      appId,
    );
    let overlayEffective = sessionOverlay;
    const runtimeTheme = globalThis.__mei?.theme_layout;
    if (runtimeTheme && typeof runtimeTheme === "object" && !Array.isArray(runtimeTheme)) {
      const basePatches =
        overlayEffective?.patches && typeof overlayEffective.patches === "object"
          ? overlayEffective.patches
          : {};
      overlayEffective = {
        ...(overlayEffective || {}),
        patches: { ...basePatches, ...runtimeTheme },
      };
    }
    const evalRaw = layers["eval.slot_group.scene:default"] || null;
    const bootstrapSeed = resolveEvalBootstrapSeed(evalRaw);
    if (bootstrapSeed && globalThis.__mei) {
      globalThis.__mei.bootstrap_seed = bootstrapSeed;
      if (Array.isArray(bootstrapSeed.metrics) && typeof boot.applyBootstrapPayload === "function") {
        boot.applyBootstrapPayload({
          clientRevision: bootstrapSeed.client_revision || bootstrapSeed.clientRevision,
          bootstrapScope: bootstrapSeed.scope || bootstrapSeed.bootstrapScope,
          metrics: bootstrapSeed.metrics,
          appId: globalThis.__mei.bootstrap_app_id || appId,
        });
      }
      if (
        bootstrapSeed.client_revision &&
        typeof boot.ensureBootstrapSeeded === "function"
      ) {
        void boot.ensureBootstrapSeeded(
          {
            appId,
            sceneId:
              String(composeAxes?.scene_id || composeAxes?.sceneId || "").trim() || "home",
          },
          { client_revision: bootstrapSeed.client_revision },
        );
      }
    }
    composePreview(root, structure, projection, themeEffective, overlayEffective);
    if (typeof materializer?.applyComposeThemeLayout === "function") {
      materializer.applyComposeThemeLayout(root);
    }
    return true;
  }

  function composePreview(root, structureDoc, projection, themeTokens, layoutOverlay) {
    clearComposeArtifacts(root);
    if (root instanceof HTMLElement) {
      root.setAttribute("data-compose-projection", String(projection || "live_full"));
      if (global.MeiProjectionDepth?.applyReviewProjectionChrome) {
        global.MeiProjectionDepth.applyReviewProjectionChrome(root, {
          reviewProjection: projection,
        });
      }
    }
    applyThemeAndOverlay(root, themeTokens, layoutOverlay);
    const visible = nodesForProjection(structureDoc, projection);
    return { visibleCount: visible.length, projection };
  }

  boot.viewCompositor = {
    nodesForProjection,
    composePreview,
    composeFromLayers,
    ensureStructureSkeleton,
    applyShellLayer,
    applyThemeAndOverlay,
    clearComposeArtifacts,
    pickShellLayer,
    recomposeFromLayerStore,
    mergePersistedAndSession,
    isPlaceholderShellDoc,
  };
})(typeof window !== "undefined" ? window : globalThis);
