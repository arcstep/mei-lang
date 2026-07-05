/**
 * Client-only session draft layers (theme.tokens.session / layout.overlay.session).
 * Not included in server manifest digest.
 */
(function initDraftLayerStore(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const STORAGE_PREFIX = "mei-draft-layer";

  function ensureDraftSessionId() {
    const cookieKey = "mei-draft-session";
    const match = String(document.cookie || "").match(/mei-draft-session=([^;]+)/);
    if (match && match[1]) return decodeURIComponent(match[1].trim());
    const id = `web-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
    document.cookie = `${cookieKey}=${encodeURIComponent(id)};path=/;SameSite=Lax`;
    return id;
  }

  function storageKey(appId, layerId) {
    const sessionId = ensureDraftSessionId();
    return `${STORAGE_PREFIX}:${String(appId || "").trim()}:${layerId}:${sessionId}`;
  }

  function readJson(key) {
    try {
      const raw = global.sessionStorage?.getItem(key);
      if (!raw) return null;
      return JSON.parse(raw);
    } catch (_) {
      return null;
    }
  }

  function writeJson(key, value) {
    try {
      global.sessionStorage?.setItem(key, JSON.stringify(value));
      return true;
    } catch (_) {
      return false;
    }
  }

  function removeJson(key) {
    try {
      global.sessionStorage?.removeItem(key);
    } catch (_) {}
  }

  function normalizeOverlayPatches(doc) {
    if (!doc || typeof doc !== "object") return {};
    if (doc.patches && typeof doc.patches === "object") return { ...doc.patches };
    const entries = doc.entries;
    if (entries && typeof entries === "object") return { ...entries };
    const tuning = doc.tuning;
    if (tuning && typeof tuning === "object") return { ...tuning };
    return {};
  }

  function overlayDocFromPatches(patches) {
    return { patches: { ...(patches || {}) } };
  }

  function themeDocFromTokens(tokens) {
    const colors = tokens?.colors && typeof tokens.colors === "object" ? tokens.colors : {};
    const fonts = tokens?.fonts && typeof tokens.fonts === "object" ? tokens.fonts : {};
    return { colors: { ...colors }, fonts: { ...fonts } };
  }

  function mergeThemeDocs(persisted, session) {
    const base = themeDocFromTokens(persisted);
    const overlay = themeDocFromTokens(session);
    return {
      colors: { ...base.colors, ...overlay.colors },
      fonts: { ...base.fonts, ...overlay.fonts },
    };
  }

  function mergeOverlayDocs(persisted, session) {
    const basePatches = normalizeOverlayPatches(persisted);
    const sessionPatches = normalizeOverlayPatches(session);
    return overlayDocFromPatches({ ...basePatches, ...sessionPatches });
  }

  function readLayoutOverlaySession(appId) {
    return readJson(storageKey(appId, "layout.overlay.session"));
  }

  function readThemeTokensSession(appId) {
    return readJson(storageKey(appId, "theme.tokens.session"));
  }

  function putLayoutOverlayPatches(appId, tuning) {
    const app = String(appId || "").trim();
    if (!app || !tuning || typeof tuning !== "object") return false;
    const key = storageKey(app, "layout.overlay.session");
    const current = normalizeOverlayPatches(readJson(key));
    const next = overlayDocFromPatches({ ...current, ...tuning });
    writeJson(key, next);
    return true;
  }

  function putThemeTokensPatch(appId, tokens) {
    const app = String(appId || "").trim();
    if (!app || !tokens || typeof tokens !== "object") return false;
    const key = storageKey(app, "theme.tokens.session");
    const current = themeDocFromTokens(readJson(key));
    const next = themeDocFromTokens({
      colors: { ...current.colors, ...(tokens.colors || {}) },
      fonts: { ...current.fonts, ...(tokens.fonts || {}) },
    });
    writeJson(key, next);
    return true;
  }

  function getSessionLayers(appId) {
    return {
      layoutOverlay: readLayoutOverlaySession(appId),
      themeTokens: readThemeTokensSession(appId),
    };
  }

  function clearSession(appId) {
    const app = String(appId || "").trim();
    if (!app) return;
    removeJson(storageKey(app, "layout.overlay.session"));
    removeJson(storageKey(app, "theme.tokens.session"));
  }

  function hasSessionDraft(appId) {
    const layers = getSessionLayers(appId);
    const overlayPatches = normalizeOverlayPatches(layers.layoutOverlay);
    const theme = themeDocFromTokens(layers.themeTokens);
    return (
      Object.keys(overlayPatches).length > 0 ||
      Object.keys(theme.colors).length > 0 ||
      Object.keys(theme.fonts).length > 0
    );
  }

  global.MeiDraftLayerStore = {
    ensureDraftSessionId,
    putLayoutOverlayPatches,
    putThemeTokensPatch,
    getSessionLayers,
    clearSession,
    hasSessionDraft,
    mergeThemeDocs,
    mergeOverlayDocs,
    normalizeOverlayPatches,
  };
  boot.draftLayerStore = global.MeiDraftLayerStore;
})(typeof window !== "undefined" ? window : globalThis);
