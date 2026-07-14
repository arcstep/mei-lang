/**
 * Phase 5 Stage Surface dispatcher — viewport (cockpit) vs paged (slides).
 * Single Access mount; profile selects host behaviour (not a second mount tree).
 */
(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  function readRegistryStages() {
    const mei = window.__mei || {};
    const reg = mei.stage_registry;
    if (reg && Array.isArray(reg.stages)) return reg.stages;
    return [];
  }

  function parseStageIdFromPath(pathname) {
    const path = String(pathname || window.location.pathname || "");
    const stageMatch = path.match(/^\/apps\/[^/]+\/([^/?#]+)/);
    if (stageMatch) {
      const seg = String(stageMatch[1] || "").trim();
      const reserved = new Set([
        "view",
        "layout",
        "prototype",
        "app",
        "access",
        "build",
        "manage",
      ]);
      if (seg && !reserved.has(seg.toLowerCase())) return seg;
    }
    return String(window.__mei?.active_scene_id || "home").trim() || "home";
  }

  function resolveStageMeta(stageId) {
    const id = String(stageId || parseStageIdFromPath()).trim();
    const fromReg = readRegistryStages().find(
      (s) => String(s?.stage_id || "") === id,
    );
    if (fromReg) {
      return {
        stageId: id,
        profile: String(fromReg.profile || "cockpit"),
        surface: String(fromReg.surface || (fromReg.profile === "slides" ? "paged" : "viewport")),
      };
    }
    const programs = window.__mei?.stage_programs || {};
    const program = programs[id];
    if (program) {
      return {
        stageId: id,
        profile: String(program.profile || "cockpit"),
        surface: String(program.surface || "viewport"),
      };
    }
    const routes = Array.isArray(window.__mei?.scene_routes)
      ? window.__mei.scene_routes
      : [];
    const route = routes.find((r) => String(r?.scene_id || "") === id);
    if (route) {
      const kind = String(route.kind || "").toLowerCase();
      const slides = kind === "presentation";
      return {
        stageId: id,
        profile: slides ? "slides" : "cockpit",
        surface: slides ? "paged" : "viewport",
      };
    }
    return { stageId: id, profile: "cockpit", surface: "viewport" };
  }

  function applyStageSurface(meta) {
    const surface = String(meta?.surface || "viewport");
    const profile = String(meta?.profile || "cockpit");
    const stageId = String(meta?.stageId || "");
    const body = document.body;
    if (body instanceof HTMLElement) {
      body.setAttribute("data-mei-stage-surface", surface);
      body.setAttribute("data-mei-stage-profile", profile);
      if (stageId) body.setAttribute("data-mei-stage-id", stageId);
    }
    const compose = document.getElementById("mei-compose-root");
    if (compose instanceof HTMLElement) {
      compose.setAttribute("data-mei-stage-surface", surface);
      compose.setAttribute("data-mei-stage-profile", profile);
    }
    return { surface, profile, stageId };
  }

  function syncFromLocation() {
    const meta = resolveStageMeta(parseStageIdFromPath());
    return applyStageSurface(meta);
  }

  boot.stageSurface = {
    readRegistryStages,
    parseStageIdFromPath,
    resolveStageMeta,
    applyStageSurface,
    syncFromLocation,
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => syncFromLocation(), {
      once: true,
    });
  } else {
    syncFromLocation();
  }
})();
