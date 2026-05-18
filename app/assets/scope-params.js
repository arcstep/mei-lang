/**
 * 与后端 `agent_scope_profile::default_resource_visibility` 对齐的纯函数，
 * 供 `agent-panel.js` 与 Node 单测共用（通过 globalThis.MeiAgentScopeParams）。
 */
(function (g) {
  const api = {
    defaultResourceVisibilityFromRoute: function (route, mode) {
      const r = String(route || "")
        .trim()
        .toLowerCase();
      const m = String(mode || "")
        .trim()
        .toLowerCase();
      const normMode = m || "build";
      if (r === "access" && normMode === "ask") return "allow_scene_reachable";
      if (r === "manage" && normMode === "ask") return "allow_direct_refs";
      if (r === "manage" && normMode === "build") return "allow_direct_refs";
      return "local_only";
    },
    /** 与 `agent-panel.js` 的 `normalizeTargetKey` 对齐，用于 scene_id 是否随 preview 请求发送。 */
    normTargetKeyForScope: function (raw) {
      return String(raw || "")
        .trim()
        .replace(/\\/g, "/")
        .replace(/^\.\/+/, "");
    },
    /**
     * 当预览目标与「scene 路由锚点」不一致时（例如 data/dataset/**），不附带 scene_id，
     * 避免触发无意义的 scope 校验失败。
     */
    shouldAttachSceneIdToScopeQuery: function (targetKey, sceneRouteTarget) {
      const tgt = api.normTargetKeyForScope(targetKey);
      const sceneT = api.normTargetKeyForScope(sceneRouteTarget);
      return !tgt || (sceneT && tgt === sceneT);
    },
    /** 显式 UI 选择优先，否则走 route+mode 默认。 */
    effectiveResourceVisibility: function (selectValue, route, mode) {
      const s = String(selectValue || "").trim();
      if (s) return s;
      return api.defaultResourceVisibilityFromRoute(route, mode);
    },
    /** 模拟 agent-panel 的 URLSearchParams 构造（不含 scene 省略逻辑，单测只验 mode/route/visibility）。 */
    scopeQueryCore: function (app, target, route, mode, resourceVisibility) {
      const params = new URLSearchParams();
      if (app) params.set("app_id", app);
      if (target) params.set("target_file", target);
      params.set("route_mode", route);
      params.set("mode", mode);
      params.set("resource_visibility", resourceVisibility);
      return params;
    },
  };
  g.MeiAgentScopeParams = api;
})(typeof globalThis !== "undefined" ? globalThis : this);
