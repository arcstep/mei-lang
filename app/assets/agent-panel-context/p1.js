/**
 * 上下文预览 API 查询、资源 inventory 展示、模型探测。由 agent-panel 主文件装配 `CTX`。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallContextPreview = function (api) {
    function currentScopeParams() {
      const browserContext = collectBrowserContext();
      const host =
        typeof globalThis !== "undefined" && globalThis.MeiAgentHostCoordinates;
      if (host && typeof host.build === "function") {
        const coords = host.build(api, {
          resourceVisibility: currentResourceVisibility(),
          browserContext: browserContext,
        });
        const params = new URLSearchParams();
        if (host.applyToUrlSearchParams) {
          return host.applyToUrlSearchParams(params, coords);
        }
        if (coords.app_id) params.set("app_id", coords.app_id);
        if (coords.target_file) params.set("target_file", coords.target_file);
        params.set("route_mode", coords.route_mode);
        params.set("mode", coords.mode);
        params.set("resource_visibility", coords.resource_visibility);
        if (coords.scene_id) params.set("scene_id", coords.scene_id);
        if (coords.browser_context) {
          try {
            params.set("browser_context", JSON.stringify(coords.browser_context));
          } catch (_) {}
        }
        return params;
      }
      const params = new URLSearchParams();
      const app = api.currentAppKey();
      const sceneId = api.currentSceneId();
      const routeMode = api.normalizeRouteMode(api.root.dataset.mode);
      const mode = api.normalizeAgentMode(api.state.agentMode);
      const target = api.currentTargetKey();
      if (app) params.set("app_id", app);
      if (target) params.set("target_file", target);
      params.set("route_mode", routeMode);
      params.set("mode", mode);
      params.set("resource_visibility", currentResourceVisibility());
      if (sceneId) params.set("scene_id", sceneId);
      if (browserContext) {
        try {
          params.set("browser_context", JSON.stringify(browserContext));
        } catch (_) {}
      }
      return params;
    }

    function defaultResourceVisibilityFromRoute() {
      const route = api.normalizeRouteMode(api.root.dataset.mode);
      const mode = api.normalizeAgentMode(api.state.agentMode);
      const ext =
        typeof globalThis !== "undefined" && globalThis.MeiAgentScopeParams;
      if (ext && typeof ext.defaultResourceVisibilityFromRoute === "function") {
        return ext.defaultResourceVisibilityFromRoute(route, mode);
      }
      if (route === "access" && mode === "ask") return "allow_scene_reachable";
      if (route === "manage" && mode === "ask") return "allow_direct_refs";
      if (route === "manage" && mode === "build") return "allow_direct_refs";
      return "local_only";
    }

    function currentResourceVisibility() {
      const sel = document.getElementById("author-resource-visibility-select");
      const route = api.normalizeRouteMode(api.root.dataset.mode);
      const mode = api.normalizeAgentMode(api.state.agentMode);
      const rawSel = sel && "value" in sel ? String(sel.value || "").trim() : "";
      const ext =
        typeof globalThis !== "undefined" && globalThis.MeiAgentScopeParams;
      if (ext && typeof ext.effectiveResourceVisibility === "function") {
        return ext.effectiveResourceVisibility(rawSel, route, mode);
      }
      if (rawSel) return rawSel;
      return defaultResourceVisibilityFromRoute();
    }

    function safeTrim(value) {
      return String(value || "").trim();
    }

    function collectActiveQueryStateIds(limit) {
      const ids = [];
      const nodes = document.querySelectorAll("[data-props]");
      for (let i = 0; i < nodes.length; i += 1) {
        if (ids.length >= limit) break;
        const node = nodes[i];
        const raw = node && node.getAttribute ? String(node.getAttribute("data-props") || "") : "";
        if (!raw) continue;
        try {
          const parsed = JSON.parse(raw);
          const id = safeTrim(parsed && (parsed.query_state || parsed.queryState));
          if (!id) continue;
          if (ids.indexOf(id) >= 0) continue;
          ids.push(id);
        } catch (_) {}
      }
      return ids;
    }

    function compactQueryStateEntry(id, raw) {
      const data = raw && typeof raw === "object" ? raw : {};
      const rawFilters = data.filters && typeof data.filters === "object" ? data.filters : {};
      const filters = {};
      Object.keys(rawFilters)
        .sort()
        .slice(0, 12)
        .forEach(function (key) {
          const k = safeTrim(key);
          const v = safeTrim(rawFilters[key]);
          if (k && v) filters[k] = v;
        });
      const search = safeTrim(data.search);
      const filterIntents = Array.isArray(data.filter_intents)
        ? data.filter_intents.slice(0, 12).map(function (entry) {
            return {
              dimension: safeTrim(entry && entry.dimension),
              value: safeTrim(entry && entry.value),
              source: safeTrim(entry && entry.source),
            };
          }).filter(function (entry) {
            return entry.dimension && entry.value;
          })
        : [];
      return {
        id: id,
        filters: filters,
        search: search || undefined,
        filter_intents: filterIntents.length ? filterIntents : undefined,
      };
    }

    function collectBrowserContext() {
      const store =
        typeof window !== "undefined" &&
        window.__meiQueryStateStore &&
        typeof window.__meiQueryStateStore === "object"
          ? window.__meiQueryStateStore
          : null;
      const activeQueryStateIds = collectActiveQueryStateIds(8);
      const queryStates = [];
      if (store) {
        activeQueryStateIds.forEach(function (id) {
          queryStates.push(compactQueryStateEntry(id, store[id]));
        });
      }
      const query = new URLSearchParams(window.location.search || "");
      const viewTab = safeTrim(query.get("tab") || api.root.dataset.viewTab || "preview");
      const overlayOpen =
        document.body &&
        document.body.classList &&
        document.body.classList.contains("access-drilldown-open");
      const sessionPatchState =
        typeof window !== "undefined" &&
        window.__meiAccessSessionPatchState &&
        typeof window.__meiAccessSessionPatchState === "object"
          ? window.__meiAccessSessionPatchState
          : null;
      const sessionPatch = sessionPatchState
        ? {
            schema: String(sessionPatchState.schema || "mei_session_patch_v1"),
            active_offer_count: Number(sessionPatchState.offer_count || 0),
            active_op_count: Number(sessionPatchState.op_count || 0),
          }
        : undefined;
      return {
        schema: "access_browser_context_v1",
        view_tab: viewTab || "preview",
        overlay_open: !!overlayOpen,
        active_query_state_ids: activeQueryStateIds,
        query_states: queryStates,
        session_patch: sessionPatch,
      };
    }

    function formatContextScopeText(payload) {
      const app = String((payload && payload.app_id) || api.currentAppKey() || "-");
      const scene = String((payload && payload.scene_id) || api.currentSceneId() || "-");
      const target = String((payload && payload.target_file) || api.currentTargetKey() || "-");
      let line =
        "scope: app=" + app + " | scene=" + scene + " | file=" + target;
      const prof = payload && payload.profile_summary ? String(payload.profile_summary).trim() : "";
      if (prof) line += "\n" + prof;
      const sb = payload && payload.scope_boundary;
      if (sb && typeof sb === "object") {
        line +=
          "\n边界: binding=" +
          String(sb.binding_scope || "-") +
          " | resource_visibility=" +
          String(sb.resource_visibility || "-") +
          " | edit=" +
          String(sb.edit_scope || "-");
      }
      const digest =
        payload && payload.scope_digest ? String(payload.scope_digest).trim() : "";
      if (digest) line += "\ndigest: " + digest;
      const browserContext = payload && payload.browser_context_echo;
      if (browserContext && typeof browserContext === "object") {
        const activeIds = Array.isArray(browserContext.active_query_state_ids)
          ? browserContext.active_query_state_ids.length
          : 0;
        line += "\nbrowser_context: view_tab=" + String(browserContext.view_tab || "-");
        line += " | query_states=" + String(activeIds);
        const sessionPatch =
          browserContext.session_patch && typeof browserContext.session_patch === "object"
            ? browserContext.session_patch
            : null;
        if (sessionPatch) {
          line +=
            " | session_patch_offers=" +
            String(sessionPatch.active_offer_count || 0) +
            " | session_patch_ops=" +
            String(sessionPatch.active_op_count || 0);
        }
      }
      const hostProtocol = payload && payload.host_protocol_echo;
      if (hostProtocol && typeof hostProtocol === "object") {
        line +=
          "\nhost_protocol: " +
          String(hostProtocol.schema || "-") +
          " | surface=" +
          String(hostProtocol.surface || "-");
      }
      const hostContract = payload && payload.host_contract;
      if (hostContract && typeof hostContract === "object") {
        line +=
          "\nhost_contract: " +
          String(hostContract.schema_version || "-") +
          " | protocol=" +
          String(hostContract.protocol_schema || "-");
      }
      return line;
    }

    function formatContextSkillText(payload) {
      const skill = payload && payload.skill_status ? payload.skill_status : null;
      if (!skill || typeof skill !== "object") {
        return "skill: (none)";
      }
      const mode = skill.installed ? (skill.stale ? "已安装(待同步)" : "已安装") : "仅源目录";
      const rev = String(skill.revision || "").trim();
      return "skill: " + mode + (rev ? " | rev=" + rev : "");
    }

    function formatContextToolsText(payload) {
      const native = Array.isArray(payload && payload.native_tool_names)
        ? payload.native_tool_names
        : [];
      const tools = Array.isArray(payload && payload.query_tools) ? payload.query_tools : [];
      const runtimeCaps = Array.isArray(payload && payload.runtime_capabilities)
        ? payload.runtime_capabilities
        : [];
      const parts = [];
      if (native.length) {
        parts.push(
          "Native LLM tools:\n" +
            native.map(function (n) {
              return "- " + String(n || "");
            }).join("\n"),
        );
      }
      if (!tools.length) {
        return parts.length ? parts.join("\n\n") : "(none)";
      }
      const catalog = tools
        .map(function (tool) {
          const id = String(tool && tool.id ? tool.id : "unknown");
          const purpose = String(tool && tool.purpose ? tool.purpose : "");
          const input = String(tool && tool.input ? tool.input : "");
          return "- " + id + (purpose ? " | " + purpose : "") + (input ? "\n  input: " + input : "");
        })
        .join("\n");
      parts.push("Resource query tools:\n" + catalog);
      if (runtimeCaps.length) {
        parts.push(
          "Runtime capabilities:\n" +
            runtimeCaps
              .map(function (cap) {
                return "- " + String(cap || "");
              })
              .join("\n"),
        );
      }
