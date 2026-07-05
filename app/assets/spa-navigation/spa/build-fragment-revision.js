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

  function legacyFragmentHtmlEnabled() {
    return global.__mei?.allow_legacy_fragment_html === true;
  }

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

  function resolveBuildFragmentNode(urlLike) {
    try {
      const url = new URL(urlLike, global.location.href);
      const fromQuery = String(url.searchParams.get("node") || "").trim();
      if (fromQuery) return fromQuery;
    } catch (_) {}
    const shell = global.document?.querySelector?.(".shell[data-build-node]");
    if (shell instanceof HTMLElement) {
      const fromDom = String(shell.getAttribute("data-build-node") || "").trim();
      if (fromDom) return fromDom;
    }
    return "";
  }

  function buildFragmentRevisionCacheKey(urlLike) {
    try {
      const url = new URL(urlLike, global.location.href);
      const parts = url.pathname.split("/").filter(Boolean);
      const appId = parts[2] || "";
      const shell = global.document?.querySelector?.(".shell[data-build-node]");
      const shellAxes =
        shell instanceof HTMLElement
          ? {
              data_mode: String(shell.getAttribute("data-data-mode") || "")
                .trim()
                .toLowerCase(),
              review_projection: String(shell.getAttribute("data-review-projection") || "")
                .trim()
                .toLowerCase(),
            }
          : { data_mode: "", review_projection: "" };
      return boot.surfaceRevisionKey({
        surface: "build",
        app_id: appId,
        node: resolveBuildFragmentNode(urlLike),
        data_mode:
          String(url.searchParams.get("data_mode") || "").trim().toLowerCase() ||
          shellAxes.data_mode,
        review_projection:
          String(url.searchParams.get("review_projection") || "").trim().toLowerCase() ||
          shellAxes.review_projection,
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

  function readSsrEmbeddedBuildRevision() {
    const digest = String(
      document
        .querySelector('meta[name="mei-build-fragment-revision-digest"]')
        ?.getAttribute("content") || "",
    ).trim();
    if (!digest) return null;
    const cacheKey = String(
      document.querySelector('meta[name="mei-build-fragment-cache-key"]')?.getAttribute("content") ||
        "",
    ).trim();
    const revision = {
      revision_digest: digest,
      cache_key: cacheKey || undefined,
    };
    return typeof boot.normalizeRevision === "function"
      ? boot.normalizeRevision(revision)
      : revision;
  }

  async function fetchBuildFragmentRevision(urlLike, options) {
    const opts = options || {};
    if (opts.skipRemoteWhenValid) {
      const localRevision = readBuildFragmentRevision(urlLike);
      const cached =
        localRevision && typeof boot.readBuildFragmentHtml === "function"
          ? readBuildFragmentHtml(urlLike, localRevision)
          : null;
      if (localRevision && cached?.preview_html) {
        global.__meiBuildRevisionSkippedNetwork = 1;
        if (typeof boot.cacheDiagTrace === "function") {
          boot.cacheDiagTrace("build-revision-skip-network", {
            reason: "local-fragment-hit",
            revision_digest: localRevision.revision_digest,
          });
        }
        return typeof boot.normalizeRevision === "function"
          ? boot.normalizeRevision(localRevision)
          : localRevision;
      }
      const ssr = readSsrEmbeddedBuildRevision();
      if (
        ssr &&
        localRevision &&
        typeof boot.revisionsMatch === "function" &&
        boot.revisionsMatch(localRevision, ssr)
      ) {
        global.__meiBuildRevisionSkippedNetwork = 1;
        if (typeof boot.cacheDiagTrace === "function") {
          boot.cacheDiagTrace("build-revision-skip-network", {
            reason: "ssr-digest-match",
            revision_digest: ssr.revision_digest,
          });
        }
        return typeof boot.normalizeRevision === "function"
          ? boot.normalizeRevision(localRevision)
          : localRevision;
      }
      if (ssr?.revision_digest) {
        rememberBuildFragmentRevision(urlLike, ssr);
        global.__meiBuildRevisionSkippedNetwork = 1;
        return ssr;
      }
    }
    const url = new URL(urlLike, global.location.href);
    const parts = url.pathname.split("/").filter(Boolean);
    const appId = parts[2] || "";
    const node = resolveBuildFragmentNode(urlLike);
    if (!node) {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("build-revision-miss-node", { url: urlLike });
      }
      return null;
    }
    const params = new URLSearchParams({
      app_id: appId,
      node,
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
    if (!legacyFragmentHtmlEnabled()) return {};
    try {
      const raw =
        global.localStorage.getItem(fragmentHtmlStoreKey) ||
        global.sessionStorage.getItem(fragmentHtmlStoreKey);
      return raw ? JSON.parse(raw) : {};
    } catch (_) {
      return {};
    }
  }

  function writeFragmentHtmlStore(store) {
    if (!legacyFragmentHtmlEnabled()) return;
    const payload = JSON.stringify(store || {});
    try {
      global.localStorage.setItem(fragmentHtmlStoreKey, payload);
      return;
    } catch (error) {
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("build-fragment-store-fallback", {
          reason: String(error?.message || error || "localStorage-failed"),
        });
      }
    }
    try {
      global.sessionStorage.setItem(fragmentHtmlStoreKey, payload);
    } catch (_) {}
  }

  function fragmentHtmlCacheKey(urlLike, revision) {
    const base = buildFragmentRevisionCacheKey(urlLike);
    const digest = String(revision?.revision_digest || revision?.cache_key || "").trim();
    return digest ? `${base}:${digest}` : base;
  }

  function rememberBuildFragmentHtml(urlLike, revision, payload) {
    if (!legacyFragmentHtmlEnabled()) return;
    const key = fragmentHtmlCacheKey(urlLike, revision);
    if (!key) return;
    const store = readFragmentHtmlStore();
    const entry = {
      drilldown_script: String(payload?.drilldown_script || ""),
      workspace_scripts: Array.isArray(payload?.workspace_scripts)
        ? payload.workspace_scripts
        : [],
      node: payload?.node || "",
      focus: payload?.focus || "",
      revision,
      scene_manifest: payload?.scene_manifest || null,
      compose_defaults: payload?.compose_defaults || null,
    };
    if (payload?.preview_html) {
      entry.preview_html = String(payload.preview_html);
    }
    store[key] = entry;
    if (typeof boot.pruneRevisionStore === "function") {
      boot.pruneRevisionStore(store, key, FRAGMENT_HTML_MAX);
    }
    writeFragmentHtmlStore(store);
  }

  function readBuildFragmentHtml(urlLike, revision) {
    if (!legacyFragmentHtmlEnabled()) return null;
    const key = fragmentHtmlCacheKey(urlLike, revision);
    if (!key) return null;
    const store = readFragmentHtmlStore();
    return store[key] || null;
  }

  boot.buildFragmentRevisionCacheKey = buildFragmentRevisionCacheKey;
  boot.resolveBuildFragmentNode = resolveBuildFragmentNode;
  boot.readBuildFragmentHtml = readBuildFragmentHtml;
  boot.rememberBuildFragmentHtml = rememberBuildFragmentHtml;
  boot.fetchBuildFragmentRevision = fetchBuildFragmentRevision;
  boot.rememberBuildFragmentRevision = rememberBuildFragmentRevision;
  boot.readBuildFragmentRevision = readBuildFragmentRevision;
  boot.buildFragmentRevisionStillValid = buildFragmentRevisionStillValid;
  boot.readSsrEmbeddedBuildRevision = readSsrEmbeddedBuildRevision;

  global.MeiBuildFragmentRevision = {
    buildFragmentRevisionCacheKey,
    resolveBuildFragmentNode,
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
      resolveBuildFragmentNode,
    });
  }
})(window);
