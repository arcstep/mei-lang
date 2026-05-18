/**
 * 上下文预览 API 查询、资源 inventory 展示、模型探测。由 agent-panel 主文件装配 `CTX`。
 */
(function (global) {
  "use strict";

  global.__meiAgentPanelInstallContextPreview = function (api) {
    function currentScopeParams() {
      const params = new URLSearchParams();
      const app = api.currentAppKey();
      const sceneId = api.currentSceneId();
      const routeMode = api.normalizeRouteMode(api.root.dataset.mode);
      const mode = api.normalizeAgentMode(api.state.agentMode);
      const sceneRouteTarget = api.normalizeTargetKey(String(api.root.dataset.sceneTarget || ""));
      const target = api.currentTargetKey();
      if (app) params.set("app_id", app);
      if (target) params.set("target_file", target);
      params.set("route_mode", routeMode);
      params.set("mode", mode);
      params.set("resource_visibility", currentResourceVisibility());
      const ext =
        typeof globalThis !== "undefined" && globalThis.MeiAgentScopeParams;
      if (ext && typeof ext.shouldAttachSceneIdToScopeQuery === "function") {
        if (ext.shouldAttachSceneIdToScopeQuery(target, sceneRouteTarget)) {
          if (sceneId) params.set("scene_id", sceneId);
        }
      } else {
        const scopedToSceneRoute = !target || (sceneRouteTarget && target === sceneRouteTarget);
        if (scopedToSceneRoute) {
          if (sceneId) params.set("scene_id", sceneId);
        }
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
      return parts.join("\n\n");
    }

    function formatContextPromptText(payload) {
      const system = String((payload && payload.system_prompt) || "").trim();
      if (system) return system;
      const context = String((payload && payload.session_context) || "").trim();
      if (context) return "[Session Context]\n" + context;
      return "(empty)";
    }

    function readContextInventory(payload) {
      const inventory = payload && payload.resource_inventory ? payload.resource_inventory : null;
      if (!inventory || typeof inventory !== "object") {
        return { target: "", total: 0, items: [] };
      }
      return {
        target: String(inventory.target_file || "").trim(),
        total: Number.isFinite(Number(inventory.total_items)) ? Number(inventory.total_items) : 0,
        items: Array.isArray(inventory.items) ? inventory.items : [],
      };
    }

    function groupInventoryItemsByReachTier(items) {
      const tiers = { direct: [], scene: [], other: [] };
      (Array.isArray(items) ? items : []).forEach(function (item) {
        if (!item || typeof item !== "object") return;
        const t = String(item.reach_tier || "other").trim().toLowerCase();
        if (t === "direct") tiers.direct.push(item);
        else if (t === "scene") tiers.scene.push(item);
        else tiers.other.push(item);
      });
      return tiers;
    }

    function renderContextInventory(payload) {
      if (!api.els.contextInventory) return;
      const inventory = readContextInventory(payload);
      const tiers = groupInventoryItemsByReachTier(inventory.items);
      const tierOrder = [
        { key: "direct", label: "直接相关（direct）" },
        { key: "scene", label: "场景可达（scene）" },
        { key: "other", label: "其它（other；/world 会按可见性裁剪）" },
      ];
      api.els.contextInventory.innerHTML = "";
      let anyTier = false;
      const head = document.createElement("div");
      head.className = "text-[10px] text-slate-400";
      head.textContent =
        "target=" + (inventory.target || "-") + " | total=" + String(inventory.total || 0);
      api.els.contextInventory.appendChild(head);

      tierOrder.forEach(function (tierDef, tierIndex) {
        const items = tiers[tierDef.key] || [];
        if (!items.length) return;
        anyTier = true;
        const details = document.createElement("details");
        details.className = "rounded border border-slate-700/60 bg-slate-950/40 px-2 py-1";
        details.open = tierIndex === 0;

        const summary = document.createElement("summary");
        summary.className = "cursor-pointer text-[10px] font-bold text-slate-200";
        summary.textContent = tierDef.label + " (" + String(items.length) + ")";
        details.appendChild(summary);

        const byType = {};
        items.forEach(function (item) {
          const type = String(item.resource_type || "unknown").trim() || "unknown";
          if (!byType[type]) byType[type] = [];
          byType[type].push(item);
        });
        const typeKeys = Object.keys(byType).sort();
        typeKeys.forEach(function (type, typeIndex) {
          const typeItems = byType[type] || [];
          const subDetails = document.createElement("details");
          subDetails.className = "mt-1 rounded border border-slate-700/50 bg-slate-900/35 px-2 py-1";
          subDetails.open = tierIndex === 0 && typeIndex < 2;
          const subSum = document.createElement("summary");
          subSum.className = "cursor-pointer text-[10px] font-bold text-slate-300";
          subSum.textContent = type + " (" + String(typeItems.length) + ")";
          subDetails.appendChild(subSum);
          const list = document.createElement("div");
          list.className = "mt-1 grid gap-1";
          typeItems.forEach(function (item) {
            const row = document.createElement("div");
            row.className = "rounded border border-slate-700/50 bg-slate-900/45 px-1.5 py-1";
            const id = String(item.id || "").trim() || "(no-id)";
            const title = String(item.title || "").trim();
            const summaryText = String(item.summary || "").trim();
            const sourcePath = String(item.source_path || "").trim();
            const refs = Array.isArray(item.references) ? item.references : [];
            const related = item.related_to_target ? " [target]" : "";
            const firstLine = document.createElement("div");
            firstLine.className = "font-mono text-[10px] text-slate-100";
            firstLine.textContent = id + (title ? " · " + title : "") + related;
            row.appendChild(firstLine);
            if (summaryText) {
              const sub = document.createElement("div");
              sub.className = "text-[10px] text-slate-300";
              sub.textContent = summaryText;
              row.appendChild(sub);
            }
            if (sourcePath) {
              const sub = document.createElement("div");
              sub.className = "font-mono text-[10px] text-blue-300";
              sub.textContent = "source: " + sourcePath;
              row.appendChild(sub);
            }
            if (refs.length) {
              const sub = document.createElement("div");
              sub.className = "text-[10px] text-slate-400";
              sub.textContent = "refs: " + refs.slice(0, 8).join(", ");
              row.appendChild(sub);
            }
            list.appendChild(row);
          });
          subDetails.appendChild(list);
          details.appendChild(subDetails);
        });
        api.els.contextInventory.appendChild(details);
      });

      if (!anyTier) {
        api.els.contextInventory.textContent = "(none)";
      }
    }

    function renderContextPreview() {
      if (api.els.contextScope) {
        api.els.contextScope.textContent = formatContextScopeText(api.state.contextPreview);
      }
      if (api.els.contextSkill) {
        api.els.contextSkill.textContent = formatContextSkillText(api.state.contextPreview);
      }
      if (api.els.contextTools) {
        api.els.contextTools.textContent = formatContextToolsText(api.state.contextPreview);
      }
      if (api.els.contextInventory) {
        renderContextInventory(api.state.contextPreview);
      }
      if (api.els.contextPrompt) {
        api.els.contextPrompt.textContent = formatContextPromptText(api.state.contextPreview);
      }
      api.renderDeltaDebugLog();
    }

    async function refreshContextPreview(force) {
      const forceRefresh = Boolean(force);
      if (!forceRefresh && api.state.contextPreviewBackoffUntilMs > Date.now()) {
        return;
      }
      const app = api.currentAppKey();
      if (!app) {
        api.state.contextPreview = null;
        renderContextPreview();
        return;
      }
      try {
        const params = currentScopeParams();
        const scopeKey = params.toString();
        const nowMs = Date.now();
        const sameScope =
          api.state.contextPreviewScopeKey &&
          api.state.contextPreviewScopeKey === scopeKey;
        if (
          !forceRefresh &&
          sameScope &&
          api.state.contextPreviewFetchedAtMs > 0 &&
          nowMs - api.state.contextPreviewFetchedAtMs < 60000
        ) {
          return;
        }
        const payload = await api.$U.fetchJson("/api/agent/context/preview?" + params.toString());
        api.state.contextPreview = payload;
        api.state.contextPreviewScopeKey = scopeKey;
        api.state.contextPreviewFetchedAtMs = nowMs;
        const previewError = String(payload && payload.preview_error ? payload.preview_error : "").trim();
        if (previewError && !forceRefresh) {
          api.state.contextPreviewBackoffUntilMs = Date.now() + 60000;
        } else {
          api.state.contextPreviewBackoffUntilMs = 0;
        }
        renderContextPreview();
      } catch (error) {
        api.state.contextPreview = null;
        api.state.contextPreviewScopeKey = "";
        api.state.contextPreviewFetchedAtMs = 0;
        if (!forceRefresh) {
          api.state.contextPreviewBackoffUntilMs = Date.now() + 60000;
        }
        renderContextPreview();
        api.setInlineNote("读取上下文预览失败：" + String(error.message || error));
      }
    }

    function formatMsTime(value) {
      const stamp = Number(value || 0);
      if (!Number.isFinite(stamp) || stamp <= 0) return "";
      return new Date(stamp).toLocaleString("zh-CN", {
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
      });
    }

    function selectedModelProbeQueryString() {
      const params = new URLSearchParams();
      const mref = api.getSelectedCompletionModelRef();
      if (mref && mref.provider_id) {
        params.set("provider_id", String(mref.provider_id));
      }
      if (mref && mref.model_id) {
        params.set("model_id", String(mref.model_id));
      }
      return params.toString();
    }

    function noteModelProbeResult(probe, atMs) {
      const ts = Number(atMs || Date.now());
      if (probe && probe.reachable) {
        api.state.modelProbeFailureStreak = 0;
        api.state.modelProbeLastSuccessAtMs = ts;
        return;
      }
      api.state.modelProbeFailureStreak = Number(api.state.modelProbeFailureStreak || 0) + 1;
    }

    async function refreshModelProbe(force) {
      if (!api.els.statusModelService) return;
      const forceRefresh = Boolean(force);
      const nowMs = Date.now();
      if (
        !forceRefresh &&
        api.state.modelProbeFetchedAtMs > 0 &&
        nowMs - api.state.modelProbeFetchedAtMs < 30000
      ) {
        return;
      }
      const query = selectedModelProbeQueryString();
      try {
        api.state.modelProbe = await api.$U.fetchJson(
          "/api/agent/model/probe" + (query ? "?" + query : ""),
        );
        api.state.modelProbeFetchedAtMs = nowMs;
        noteModelProbeResult(api.state.modelProbe, nowMs);
      } catch (error) {
        api.state.modelProbe = {
          reachable: false,
          provider_id: "",
          model_id: "",
          base_url: "",
          error: String(error && error.message ? error.message : error || ""),
        };
        api.state.modelProbeFetchedAtMs = nowMs;
        noteModelProbeResult(api.state.modelProbe, nowMs);
      }
      api.renderStatusBarOpenCode();
    }

    return {
      currentScopeParams: currentScopeParams,
      defaultResourceVisibilityFromRoute: defaultResourceVisibilityFromRoute,
      currentResourceVisibility: currentResourceVisibility,
      formatContextScopeText: formatContextScopeText,
      formatContextSkillText: formatContextSkillText,
      formatContextToolsText: formatContextToolsText,
      formatContextPromptText: formatContextPromptText,
      readContextInventory: readContextInventory,
      groupInventoryItemsByReachTier: groupInventoryItemsByReachTier,
      renderContextInventory: renderContextInventory,
      renderContextPreview: renderContextPreview,
      refreshContextPreview: refreshContextPreview,
      formatMsTime: formatMsTime,
      selectedModelProbeQueryString: selectedModelProbeQueryString,
      noteModelProbeResult: noteModelProbeResult,
      refreshModelProbe: refreshModelProbe,
    };
  };
})(window);
