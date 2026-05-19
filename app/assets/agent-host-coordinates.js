/**
 * Agent preview / send / SSE 共用的宿主坐标（scene-first）。
 * 与后端 `BridgePromptRequest` 字段对齐。
 */
(function (g) {
  "use strict";

  function buildAgentHostCoordinates(api) {
    const root = api.root;
    const state = api.state || {};
    const ext =
      typeof g !== "undefined" && g.MeiAgentScopeParams ? g.MeiAgentScopeParams : null;
    const route = api.normalizeRouteMode(root.dataset.mode);
    const mode = api.normalizeAgentMode(state.agentMode);
    let resourceVisibility = "";
    if (ext && typeof ext.effectiveResourceVisibility === "function") {
      const sel = document.getElementById("author-resource-visibility-select");
      const rawSel = sel && "value" in sel ? String(sel.value || "").trim() : "";
      resourceVisibility = ext.effectiveResourceVisibility(rawSel, route, mode);
    } else if (api.CTX && typeof api.CTX.currentResourceVisibility === "function") {
      resourceVisibility = api.CTX.currentResourceVisibility();
    }
    return {
      app_id: String(root.dataset.app || api.currentAppKey() || ""),
      scene_id: String(api.currentSceneId() || ""),
      target_file: String(api.currentTargetKey() || ""),
      route_mode: route,
      mode: mode,
      resource_visibility: resourceVisibility,
    };
  }

  function applyToUrlSearchParams(params, coords) {
    if (!params || !coords) return params;
    if (coords.app_id) params.set("app_id", coords.app_id);
    if (coords.target_file) params.set("target_file", coords.target_file);
    params.set("route_mode", coords.route_mode || "manage");
    params.set("mode", coords.mode || "build");
    if (coords.resource_visibility) {
      params.set("resource_visibility", coords.resource_visibility);
    }
    if (coords.scene_id) params.set("scene_id", coords.scene_id);
    return params;
  }

  g.MeiAgentHostCoordinates = {
    build: buildAgentHostCoordinates,
    applyToUrlSearchParams: applyToUrlSearchParams,
  };
})(typeof globalThis !== "undefined" ? globalThis : this);
