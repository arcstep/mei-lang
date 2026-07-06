/**
 * Surface readiness gate: observable parity with F5 cold-start for a given surface URL.
 */
(function initSurfaceReady(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const doc = global.document;

  function surfaceSlug(ctx) {
    return String(ctx?.surface || ctx?.mode || "app").trim().toLowerCase();
  }

  function buildComposeAxes(ctx) {
    if (typeof boot.viewRevisionClient?.buildComposeRequest === "function") {
      return boot.viewRevisionClient.buildComposeRequest(ctx);
    }
    if (typeof boot.composeDefaultsForSurface === "function") {
      return boot.composeDefaultsForSurface(ctx);
    }
    return { route_mode: surfaceSlug(ctx), review_projection: "live_full" };
  }

  function bodySurfaceMatches(ctx) {
    const expected = surfaceSlug(ctx);
    const body = doc?.body;
    if (!(body instanceof HTMLElement)) return false;
    const actual = String(
      body.getAttribute("data-surface") || body.getAttribute("data-mei-view") || "",
    )
      .trim()
      .toLowerCase();
    return actual === expected;
  }

  function manifestRouteModeMatches(ctx) {
    const expected = surfaceSlug(ctx);
    const routeMode = String(
      globalThis.__mei?.scene_manifest_refs?.compose_defaults?.route_mode || "",
    )
      .trim()
      .toLowerCase();
    return !routeMode || routeMode === expected;
  }

  function countAppPreviewMarkers(root) {
    if (!(root instanceof HTMLElement)) return 0;
    return root.querySelectorAll(
      "[data-preview-scope], [data-mei-frame-viewport], .preview-viewport, [data-mei-compose-materialized]",
    ).length;
  }

  function countWorkspacePreviewMarkers(root) {
    if (!(root instanceof HTMLElement)) return 0;
    return root.querySelectorAll(
      "[data-preview-scope], [data-mei-frame-viewport], .preview-viewport, .preview-board-mounted",
    ).length;
  }

  function workspaceTreeNodeCount() {
    return (
      doc?.querySelectorAll?.("aside .build-tree-node, .build-tree-shell .build-tree-node")
        ?.length || 0
    );
  }

  function projectionMatches(ctx, root) {
    if (!(root instanceof HTMLElement)) return false;
    const expected = String(buildComposeAxes(ctx).review_projection || "").trim();
    const actual = String(root.getAttribute("data-compose-projection") || "").trim();
    if (!expected) return true;
    if (!actual) return countWorkspacePreviewMarkers(root) > 0;
    return actual === expected;
  }

  function isSurfaceMaterialized(ctx, options) {
    const opts = options || {};
    const surface = surfaceSlug(ctx);
    if (!bodySurfaceMatches(ctx)) return false;
    if (!manifestRouteModeMatches(ctx)) return false;
    const chromeReady =
      typeof boot.hostChromeReady === "function" ? boot.hostChromeReady() : true;
    if (!chromeReady) return false;

    if (surface === "app") {
      const root =
        typeof boot.resolveComposeRoot === "function"
          ? boot.resolveComposeRoot(surface)
          : doc?.getElementById?.("mei-compose-root");
      const markers = countAppPreviewMarkers(root);
      const materialized =
        typeof boot.hasMaterializedPreview === "function" && boot.hasMaterializedPreview(root);
      return markers > 0 || materialized;
    }

    if (
      typeof boot.isWorkspaceComposeSurface === "function" &&
      boot.isWorkspaceComposeSurface(surface)
    ) {
      const root =
        typeof boot.resolveComposeRoot === "function"
          ? boot.resolveComposeRoot(surface)
          : doc?.querySelector?.("#mei-surface-workspace .preview-pane-scroll");
      const markers = countWorkspacePreviewMarkers(root);
      if (markers === 0) return false;
      if (!projectionMatches(ctx, root)) return false;
      if (!opts.relaxTree && workspaceTreeNodeCount() === 0) return false;
      return true;
    }

    return false;
  }

  function surfaceSnapshot(ctx) {
    const surface = surfaceSlug(ctx);
    const root =
      typeof boot.resolveComposeRoot === "function" ? boot.resolveComposeRoot(surface) : null;
    const axes = buildComposeAxes(ctx);
    return {
      surface,
      bodySurface: doc?.body?.getAttribute("data-surface") || "",
      routeMode: globalThis.__mei?.scene_manifest_refs?.compose_defaults?.route_mode || "",
      projection: root instanceof HTMLElement ? root.getAttribute("data-compose-projection") : "",
      expectedProjection: axes.review_projection || "",
      previewMarkers:
        surface === "app" ? countAppPreviewMarkers(root) : countWorkspacePreviewMarkers(root),
      treeNodes: workspaceTreeNodeCount(),
      chromeReady:
        typeof boot.hostChromeReady === "function" ? boot.hostChromeReady() : null,
      ready:
        typeof boot.isSurfaceMaterialized === "function"
          ? boot.isSurfaceMaterialized(ctx, { relaxTree: true })
          : null,
    };
  }

  boot.isSurfaceMaterialized = isSurfaceMaterialized;
  boot.surfaceSnapshot = surfaceSnapshot;
  global.surfaceSnapshot = surfaceSnapshot;
})(typeof window !== "undefined" ? window : globalThis);
