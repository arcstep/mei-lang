/**
 * Phase 5 Presenter Session — multi-axis prefs + NarrationCatalog binding.
 * Axes: track | transport | caption | speaker_notes | voice | actions
 * Does not write back to AOT / Stage MDX.
 */
(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  const PRESETS = {
    browse: {
      track: "off",
      transport: "manual",
      caption: false,
      speaker_notes: false,
      voice: false,
      actions: false,
    },
    hint: {
      track: "selected",
      transport: "manual",
      caption: true,
      speaker_notes: false,
      voice: false,
      actions: true,
    },
    presenter: {
      track: "selected",
      transport: "manual",
      caption: true,
      speaker_notes: true,
      voice: false,
      actions: true,
    },
    auto: {
      track: "selected",
      transport: "auto",
      caption: true,
      speaker_notes: false,
      voice: true,
      actions: true,
    },
    silent: {
      track: "selected",
      transport: "manual",
      caption: false,
      speaker_notes: false,
      voice: false,
      actions: true,
    },
  };

  const state = {
    stageId: "",
    catalogKey: "",
    trackId: null,
    cueIndex: -1,
    prefs: { ...PRESETS.browse },
    preset: "browse",
  };

  function parseStageId() {
    const surface = boot.stageSurface;
    if (surface?.parseStageIdFromPath) return surface.parseStageIdFromPath();
    return String(window.__mei?.active_scene_id || "home").trim() || "home";
  }

  function catalogForStage(stageId) {
    const catalogs = window.__mei?.narration_catalogs || {};
    const key = `narration:${stageId}`;
    const direct = catalogs[key];
    if (direct) return { key, catalog: direct };
    // Fallback: any catalog whose id ends with stage
    for (const [k, v] of Object.entries(catalogs)) {
      if (k.includes(stageId)) return { key: k, catalog: v };
    }
    return { key, catalog: null };
  }

  function cueCount(catalog) {
    if (!catalog || !Array.isArray(catalog.tracks)) return 0;
    return catalog.tracks.reduce(
      (n, t) => n + (Array.isArray(t.cues) ? t.cues.length : 0),
      0,
    );
  }

  function defaultTrackId(catalog) {
    if (!catalog || !Array.isArray(catalog.tracks) || !catalog.tracks.length) {
      return null;
    }
    return String(catalog.tracks[0].id || "default");
  }

  function catalogToManifest(catalog) {
    if (!catalog || cueCount(catalog) === 0) return null;
    const track = catalog.tracks.find((t) => Array.isArray(t.cues) && t.cues.length) ||
      catalog.tracks[0];
    if (!track) return null;
    const steps = (track.cues || []).map((cue, idx) => ({
      id: cue.id || `cue-${idx}`,
      target: cue.target?.id || cue.target || "",
      targetKind: cue.target?.kind || "slot",
      caption: cue.caption || "",
      speakerNotes: cue.speaker_notes || cue.speakerNotes || "",
      actions: Array.isArray(cue.actions) ? cue.actions : [],
      timingMs: cue.timing_ms ?? cue.timingMs ?? null,
      source: "narration_catalog",
    }));
    return {
      title: track.id || "Narration",
      steps,
      sourceKind: "narration_catalog",
    };
  }

  function applyPreset(name) {
    const preset = PRESETS[name] || PRESETS.browse;
    state.preset = PRESETS[name] ? name : "browse";
    state.prefs = { ...preset };
    if (state.prefs.track === "off") {
      state.trackId = null;
      state.cueIndex = -1;
    }
    return getSnapshot();
  }

  function resetForStage(stageId) {
    const id = String(stageId || parseStageId()).trim();
    const { key, catalog } = catalogForStage(id);
    state.stageId = id;
    state.catalogKey = key;
    state.cueIndex = -1;
    const count = cueCount(catalog);
    if (count === 0) {
      applyPreset("browse");
      state.trackId = null;
    } else {
      applyPreset("presenter");
      state.trackId = defaultTrackId(catalog);
      state.cueIndex = 0;
    }
    return getSnapshot();
  }

  function stop() {
    state.cueIndex = -1;
    state.prefs = { ...state.prefs, transport: "manual" };
    if (state.preset === "auto") state.preset = "presenter";
    return getSnapshot();
  }

  function hasNavigableCues() {
    if (state.prefs.track === "off") return false;
    const { catalog } = catalogForStage(state.stageId || parseStageId());
    return cueCount(catalog) > 0;
  }

  function getSnapshot() {
    const { catalog } = catalogForStage(state.stageId || parseStageId());
    return {
      stageId: state.stageId,
      catalogKey: state.catalogKey,
      trackId: state.trackId,
      cueIndex: state.cueIndex,
      prefs: { ...state.prefs },
      preset: state.preset,
      cueCount: cueCount(catalog),
      hasNavigableCues: hasNavigableCues(),
      manifest: state.prefs.track === "off" ? null : catalogToManifest(catalog),
    };
  }

  function setPref(axis, value) {
    if (!(axis in state.prefs)) return getSnapshot();
    state.prefs[axis] = value;
    if (axis === "track" && value === "off") {
      state.trackId = null;
      state.cueIndex = -1;
    }
    return getSnapshot();
  }

  boot.presenterSession = {
    PRESETS,
    resetForStage,
    applyPreset,
    stop,
    getSnapshot,
    setPref,
    catalogForStage,
    catalogToManifest,
    hasNavigableCues,
    parseStageId,
  };

  // Initial bind for current stage (no cross-stage inheritance).
  if (document.readyState === "loading") {
    document.addEventListener(
      "DOMContentLoaded",
      () => resetForStage(parseStageId()),
      { once: true },
    );
  } else {
    resetForStage(parseStageId());
  }
})();
