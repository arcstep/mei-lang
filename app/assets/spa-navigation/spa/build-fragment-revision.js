/**
 * Build fragment revision helpers (shared contract with access-like scene revision).
 */
(function initBuildFragmentRevision(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const BUILD_FRAGMENT_REVISION_API = "/api/build/fragment-revision";
  const revisionStoreKey = "mei-build-fragment-revisions";
  const fragmentHtmlStoreKey = "mei-build-fragment-html";
  const FRAGMENT_HTML_MAX = 8;

  function readRevisionStore() {
    try {
      const raw = global.sessionStorage.getItem(revisionStoreKey);
      return raw ? JSON.parse(raw) : {};
    } catch (_) {
      return {};
    }
  }

  function writeRevisionStore(store) {
    try {
      global.sessionStorage.setItem(revisionStoreKey, JSON.stringify(store || {}));
    } catch (_) {}
  }

  function buildFragmentRevisionCacheKey(urlLike) {
    try {
      const url = new URL(urlLike, global.location.href);
      const parts = url.pathname.split("/").filter(Boolean);
      const appId = parts[2] || "";
      return boot.surfaceRevisionKey({
        surface: "build",
        app_id: appId,
        node: String(url.searchParams.get("node") || "").trim(),
        data_mode: String(url.searchParams.get("data_mode") || "").trim().toLowerCase(),
        review_projection: String(url.searchParams.get("review_projection") || "")
          .trim()
          .toLowerCase(),
        focus: String(url.searchParams.get("focus") || "").trim(),
        scope: String(url.searchParams.get("scope") || "").trim(),
      });
    } catch (_) {
      return "";
    }
  }

  function rememberBuildFragmentRevision(urlLike, revision) {
    const key = buildFragmentRevisionCacheKey(urlLike);
    if (!key || !revision) return;
    const store = readRevisionStore();
    store[key] = revision;
    if (typeof boot.pruneRevisionStore === "function") {
      boot.pruneRevisionStore(store, key, 48);
    }
    writeRevisionStore(store);
  }

  function readBuildFragmentRevision(urlLike) {
    const key = buildFragmentRevisionCacheKey(urlLike);
    if (!key) return null;
    const store = readRevisionStore();
    return store[key] || null;
  }

  function buildFragmentRevisionStillValid(urlLike, remoteRevision) {
    const localRevision = readBuildFragmentRevision(urlLike);
    if (!localRevision || !remoteRevision) return false;
    return typeof boot.revisionsMatch === "function"
      ? boot.revisionsMatch(localRevision, remoteRevision)
      : false;
  }

  async function fetchBuildFragmentRevision(urlLike, options) {
    const opts = options || {};
    if (opts.skipRemoteWhenValid) {
      const localRevision = readBuildFragmentRevision(urlLike);
      const cached =
        localRevision && typeof boot.readBuildFragmentHtml === "function"
          ? boot.readBuildFragmentHtml(urlLike, localRevision)
          : null;
      if (localRevision && cached?.preview_html) {
        if (typeof boot.cacheDiagTrace === "function") {
          boot.cacheDiagTrace("build-revision-skip-network", {
            revision_digest: localRevision.revision_digest,
          });
        }
        return typeof boot.normalizeRevision === "function"
          ? boot.normalizeRevision(localRevision)
          : localRevision;
      }
    }
    const url = new URL(urlLike, global.location.href);
    const parts = url.pathname.split("/").filter(Boolean);
    const appId = parts[2] || "";
    const params = new URLSearchParams({
      app_id: appId,
      node: String(url.searchParams.get("node") || "").trim(),
    });
    const focus = url.searchParams.get("focus");
    const scope = url.searchParams.get("scope");
    const dataMode = url.searchParams.get("data_mode");
    const reviewProjection = url.searchParams.get("review_projection");
    if (focus) params.set("focus", focus);
    if (scope) params.set("scope", scope);
    if (dataMode) params.set("data_mode", dataMode);
    if (reviewProjection) params.set("review_projection", reviewProjection);
    const controller = opts.signal ? null : new AbortController();
    const signal = opts.signal || controller?.signal;
    const timer =
      controller && Number.isFinite(opts.timeoutMs)
        ? setTimeout(() => controller.abort(), opts.timeoutMs)
        : null;
    try {
      const response = await fetch(`${BUILD_FRAGMENT_REVISION_API}?${params.toString()}`, {
        credentials: "same-origin",
        headers: {
          Accept: "application/json",
          ...(typeof ensureDraftSessionId === "function"
            ? { "x-mei-draft-session": ensureDraftSessionId() }
            : {}),
        },
        signal,
      });
      if (!response.ok) {
        throw new Error(`build fragment revision failed: ${response.status}`);
      }
      const revision = await response.json();
      rememberBuildFragmentRevision(urlLike, revision);
      return typeof boot.normalizeRevision === "function"
        ? boot.normalizeRevision(revision)
        : revision;
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  function readFragmentHtmlStore() {
    try {
      const raw = global.sessionStorage.getItem(fragmentHtmlStoreKey);
      return raw ? JSON.parse(raw) : {};
    } catch (_) {
      return {};
    }
  }

  function writeFragmentHtmlStore(store) {
    try {
      global.sessionStorage.setItem(fragmentHtmlStoreKey, JSON.stringify(store || {}));
    } catch (_) {}
  }

  function fragmentHtmlCacheKey(urlLike, revision) {
    const base = buildFragmentRevisionCacheKey(urlLike);
    const digest = String(revision?.revision_digest || revision?.cache_key || "").trim();
    return digest ? `${base}:${digest}` : base;
  }

  function rememberBuildFragmentHtml(urlLike, revision, payload) {
    const key = fragmentHtmlCacheKey(urlLike, revision);
    if (!key || !payload?.preview_html) return;
    const store = readFragmentHtmlStore();
    store[key] = {
      preview_html: String(payload.preview_html || ""),
      drilldown_script: String(payload.drilldown_script || ""),
      workspace_scripts: Array.isArray(payload.workspace_scripts)
        ? payload.workspace_scripts
        : [],
      node: payload.node || "",
      focus: payload.focus || "",
      revision,
    };
    if (typeof boot.pruneRevisionStore === "function") {
      boot.pruneRevisionStore(store, key, FRAGMENT_HTML_MAX);
    }
    writeFragmentHtmlStore(store);
  }

  function readBuildFragmentHtml(urlLike, revision) {
    const key = fragmentHtmlCacheKey(urlLike, revision);
    if (!key) return null;
    const store = readFragmentHtmlStore();
    return store[key] || null;
  }

  boot.buildFragmentRevisionCacheKey = buildFragmentRevisionCacheKey;
  boot.readBuildFragmentHtml = readBuildFragmentHtml;
  boot.rememberBuildFragmentHtml = rememberBuildFragmentHtml;
  boot.fetchBuildFragmentRevision = fetchBuildFragmentRevision;
  boot.rememberBuildFragmentRevision = rememberBuildFragmentRevision;
  boot.readBuildFragmentRevision = readBuildFragmentRevision;
  boot.buildFragmentRevisionStillValid = buildFragmentRevisionStillValid;

  global.MeiBuildFragmentRevision = {
    buildFragmentRevisionCacheKey,
    readBuildFragmentHtml,
    rememberBuildFragmentHtml,
    fetchBuildFragmentRevision,
    rememberBuildFragmentRevision,
    readBuildFragmentRevision,
    buildFragmentRevisionStillValid,
  };

  if (global.MeiBuildNavigation && typeof global.MeiBuildNavigation === "object") {
    Object.assign(global.MeiBuildNavigation, {
      fetchBuildFragmentRevision,
      readBuildFragmentRevision,
      readBuildFragmentHtml,
      rememberBuildFragmentRevision,
      rememberBuildFragmentHtml,
      buildFragmentRevisionStillValid,
      buildFragmentRevisionCacheKey,
    });
  }
})(window);
