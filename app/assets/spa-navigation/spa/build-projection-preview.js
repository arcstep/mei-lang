/**
 * Build view: isolated projection / overlay preview mount.
 */
(function (global) {
  "use strict";

  function initBuildProjectionPreview() {
    const host = document.getElementById("build-projection-preview-host");
    if (!host || host.__bound) return;
    host.__bound = true;
    const sceneId = host.getAttribute("data-scene-id");
    const projectionId = host.getAttribute("data-projection-id");
    if (!sceneId || !projectionId) return;

    const script = document.getElementById("mei-scene-drilldown-context");
    if (!script) return;
    let context = {};
    try {
      context = JSON.parse(script.textContent || "{}");
    } catch (_) {
      return;
    }
    const byScene = context.scene_projection_assembly_by_id || {};
    const assembly = byScene[sceneId];
    if (!assembly) return;

    const boot = global.__meiLangBoot || {};
    const open =
      (global.MeiDrilldown && global.MeiDrilldown.openProjectionPreview) ||
      boot.openSceneProjection;
    if (typeof open !== "function") return;

    if (global.MeiDrilldown && typeof global.MeiDrilldown.openProjectionPreview === "function") {
      global.MeiDrilldown.openProjectionPreview({
        sceneId,
        projectionId,
        assembly,
        isolated: true,
      });
      return;
    }

    const popup =
      (assembly.overlays && assembly.overlays[projectionId]) ||
      (assembly.boards && assembly.boards[projectionId]) ||
      {};
    boot.openSceneProjection({
      scene_id: sceneId,
      projection_id: projectionId,
      popup,
      __mei_build_isolated: true,
    });
  }

  global.__meiBuildProjectionPreviewInit = initBuildProjectionPreview;
})(typeof window !== "undefined" ? window : globalThis);
