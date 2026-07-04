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
        chrome: String(url.searchParams.get("chrome") || "").trim().toLowerCase(),
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
    return boot.surfaceRevisionKey({
      surface: ctx.mode || "app",
      app_id: ctx.appId,
      scene_id: ctx.sceneId,
      data_mode: (() => {
        try {
          return String(
            new URL(ctx.url || window.location.href).searchParams.get("data_mode") || "",
          )
            .trim()
            .toLowerCase();
        } catch (_) {
          return "";
        }
      })(),
      review_projection: (() => {
        try {
          return String(
            new URL(ctx.url || window.location.href).searchParams.get("review_projection") || "",
          )
            .trim()
            .toLowerCase();
        } catch (_) {
          return "";
        }
      })(),
      chrome: (() => {
        try {
          return String(new URL(ctx.url || window.location.href).searchParams.get("chrome") || "")
            .trim()
            .toLowerCase();
        } catch (_) {
          return "";
        }
      })(),
    });
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
      const chrome = String(url.searchParams.get("chrome") || "").trim();
      if (dataMode) params.set("data_mode", dataMode);
      if (reviewProjection) params.set("review_projection", reviewProjection);
      if (chrome) params.set("chrome", chrome);
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
  boot.sceneRevisionCacheKey = sceneRevisionCacheKey;
