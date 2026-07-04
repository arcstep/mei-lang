  const SCENE_REVISION_API = "/api/host/scene-revision";

  function parseAccessSceneContext(urlLike) {
    try {
      const url = new URL(urlLike, window.location.href);
      const match = url.pathname.match(
        /^\/apps\/(?:app|access|run|presentation|slides|copilot)\/([^/]+)\/scene\/([^/]+)/,
      );
      if (!match) return null;
      const mode = url.pathname.split("/")[2] || "app";
      return {
        appId: decodeURIComponent(match[1]),
        sceneId: decodeURIComponent(match[2]),
        mode,
        dataMode: String(url.searchParams.get("data_mode") || "").trim().toLowerCase(),
        reviewProjection: String(url.searchParams.get("review_projection") || "")
          .trim()
          .toLowerCase(),
        url: url.href,
      };
    } catch (_) {
      return null;
    }
  }

  function sceneRevisionCacheKey(ctx) {
    const dataMode = (() => {
      try {
        return String(new URL(ctx.url || window.location.href).searchParams.get("data_mode") || "")
          .trim()
          .toLowerCase();
      } catch (_) {
        return "";
      }
    })();
    const reviewProjection = (() => {
      try {
        return String(
          new URL(ctx.url || window.location.href).searchParams.get("review_projection") || "",
        )
          .trim()
          .toLowerCase();
      } catch (_) {
        return "";
      }
    })();
    return [ctx.appId, ctx.sceneId, ctx.mode, dataMode, reviewProjection]
      .filter(Boolean)
      .join(":");
  }

  function revisionsMatch(localRevision, remoteRevision) {
    if (!localRevision || !remoteRevision) return false;
    if (localRevision.revision_digest && remoteRevision.revision_digest) {
      return localRevision.revision_digest === remoteRevision.revision_digest;
    }
    if (localRevision.cache_key && remoteRevision.cache_key) {
      return localRevision.cache_key === remoteRevision.cache_key;
    }
    return (
      localRevision.registry_revision === remoteRevision.registry_revision &&
      localRevision.client_revision === remoteRevision.client_revision &&
      localRevision.data_generation === remoteRevision.data_generation &&
      localRevision.scene_bundle_revision === remoteRevision.scene_bundle_revision
    );
  }

  async function fetchSceneRevision(ctx, options) {
    const opts = options || {};
    const params = new URLSearchParams({
      app: ctx.appId,
      scene: ctx.sceneId,
      mode: ctx.mode || "app",
    });
    try {
      const url = new URL(ctx.url || window.location.href);
      const dataMode = String(url.searchParams.get("data_mode") || "").trim();
      const reviewProjection = String(url.searchParams.get("review_projection") || "").trim();
      if (dataMode) params.set("data_mode", dataMode);
      if (reviewProjection) params.set("review_projection", reviewProjection);
    } catch (_) {}
    const controller = opts.signal ? null : new AbortController();
    const signal = opts.signal || controller?.signal;
    const timer =
      controller && Number.isFinite(opts.timeoutMs)
        ? setTimeout(() => controller.abort(), opts.timeoutMs)
        : null;
    try {
      const response = await fetch(`${SCENE_REVISION_API}?${params.toString()}`, {
        credentials: "same-origin",
        headers: { Accept: "application/json" },
        signal,
      });
      if (!response.ok) {
        throw new Error(`scene revision failed: ${response.status}`);
      }
      return await response.json();
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  boot.parseAccessSceneContext = parseAccessSceneContext;
  boot.fetchSceneRevision = fetchSceneRevision;
  boot.revisionsMatch = revisionsMatch;
  boot.sceneRevisionCacheKey = sceneRevisionCacheKey;
