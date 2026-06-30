/**
 * Agent preview / send / SSE 共用的宿主坐标（scene-first）。
 * 与后端 `BridgePromptRequest` 字段对齐。
 */
(function (g) {
  "use strict";

  const HOST_RUNTIME_PROTOCOL_SCHEMA = "mei-host-runtime-protocol-v1";
  const HOST_RUNTIME_CONTRACT_SCHEMA = "mei-host-runtime-contract-v1";

  function safeObject(value) {
    return value && typeof value === "object" && !Array.isArray(value) ? value : null;
  }

  function resolveHostSurface(routeMode) {
    const mode = String(routeMode || "").trim().toLowerCase();
    return mode === "access" || mode === "copilot" ? "access_host" : "authoring_host";
  }

  function buildHostProtocol(routeMode, mode) {
    return {
      schema: HOST_RUNTIME_PROTOCOL_SCHEMA,
      surface: resolveHostSurface(routeMode),
      route_mode: String(routeMode || "manage"),
      mode: String(mode || "build"),
    };
  }

  function resolveBrowserContext(api, options) {
    const opts = safeObject(options) || {};
    if (safeObject(opts.browserContext)) return opts.browserContext;
    if (api && typeof api.collectBrowserContext === "function") {
      const ctx = api.collectBrowserContext();
      if (safeObject(ctx)) return ctx;
    }
    return null;
  }

  function buildAgentHostCoordinates(api, options) {
    const root = api.root;
    const state = api.state || {};
    const opts = safeObject(options) || {};
    const ext =
      typeof g !== "undefined" && g.MeiAgentScopeParams ? g.MeiAgentScopeParams : null;
    const route = api.normalizeRouteMode(root.dataset.mode);
    const mode = api.normalizeAgentMode(state.agentMode);
    let resourceVisibility = String(opts.resourceVisibility || "").trim();
    if (!resourceVisibility) {
      if (ext && typeof ext.effectiveResourceVisibility === "function") {
        const sel = document.getElementById("author-resource-visibility-select");
        const rawSel = sel && "value" in sel ? String(sel.value || "").trim() : "";
        resourceVisibility = ext.effectiveResourceVisibility(rawSel, route, mode);
      } else if (api.CTX && typeof api.CTX.currentResourceVisibility === "function") {
        resourceVisibility = api.CTX.currentResourceVisibility();
      }
    }
    const coords = {
      app_id: String(root.dataset.app || api.currentAppKey() || ""),
      scene_id: String(api.currentSceneId() || ""),
      target_file: String(api.currentTargetKey() || ""),
      route_mode: route,
      mode: mode,
      resource_visibility: resourceVisibility,
      host_protocol: buildHostProtocol(route, mode),
      host_contract_schema: HOST_RUNTIME_CONTRACT_SCHEMA,
    };
    const browserContext = resolveBrowserContext(api, opts);
    if (browserContext) {
      coords.browser_context = browserContext;
    }
    const copilot =
      typeof g !== "undefined" && g.MeiCopilot && typeof g.MeiCopilot.context === "function"
        ? g.MeiCopilot.context()
        : null;
    if (copilot && typeof copilot === "object") {
      coords.presentation_id = String(copilot.presentationId || "").trim();
      coords.presentation_step_id = String(copilot.stepId || "").trim();
      coords.presentation_composition = String(copilot.composition || "").trim();
      coords.presentation_viewpoint = String(copilot.viewpoint || "").trim();
      if (!coords.browser_context) coords.browser_context = {};
      coords.browser_context.copilot = copilot;
    }
    return coords;
  }

  function applyToUrlSearchParams(params, coords) {
    if (!params || !coords) return params;
    if (coords.app_id) params.set("app_id", coords.app_id);
    if (coords.target_file) params.set("target_file", coords.target_file);
    params.set("route_mode", coords.route_mode || "access");
    params.set("mode", coords.mode || "build");
    if (coords.resource_visibility) {
      params.set("resource_visibility", coords.resource_visibility);
    }
    if (coords.scene_id) params.set("scene_id", coords.scene_id);
    if (coords.browser_context && typeof coords.browser_context === "object") {
      try {
        params.set("browser_context", JSON.stringify(coords.browser_context));
      } catch (_) {}
    }
    if (coords.host_protocol && typeof coords.host_protocol === "object") {
      try {
        params.set("host_protocol", JSON.stringify(coords.host_protocol));
      } catch (_) {}
    }
    if (coords.presentation_id) params.set("presentation_id", coords.presentation_id);
    if (coords.presentation_step_id) params.set("presentation_step_id", coords.presentation_step_id);
    if (coords.presentation_composition) {
      params.set("presentation_composition", coords.presentation_composition);
    }
    if (coords.presentation_viewpoint) {
      params.set("presentation_viewpoint", coords.presentation_viewpoint);
    }
    if (coords.host_contract_schema) {
      params.set("host_contract_schema", String(coords.host_contract_schema));
    }
    return params;
  }

  function buildPromptRequestBody(api, text, options) {
    const coords = buildAgentHostCoordinates(api, options);
    const body = {
      text: String(text || ""),
      app_id: coords.app_id,
      scene_id: coords.scene_id,
      target_file: coords.target_file,
      mode: coords.mode,
      route_mode: coords.route_mode,
      agent: coords.mode,
      resource_visibility: coords.resource_visibility,
    };
    if (coords.browser_context && typeof coords.browser_context === "object") {
      body.browser_context = coords.browser_context;
    }
    if (coords.host_protocol && typeof coords.host_protocol === "object") {
      body.host_protocol = coords.host_protocol;
    }
    if (coords.presentation_id) body.presentation_id = coords.presentation_id;
    if (coords.presentation_step_id) body.presentation_step_id = coords.presentation_step_id;
    if (coords.presentation_composition) {
      body.presentation_composition = coords.presentation_composition;
    }
    if (coords.presentation_viewpoint) body.presentation_viewpoint = coords.presentation_viewpoint;
    if (coords.host_contract_schema) {
      body.host_contract_schema = String(coords.host_contract_schema);
    }
    return body;
  }

  g.MeiAgentHostCoordinates = {
    build: buildAgentHostCoordinates,
    applyToUrlSearchParams: applyToUrlSearchParams,
    buildPromptRequestBody: buildPromptRequestBody,
  };
})(typeof globalThis !== "undefined" ? globalThis : this);
