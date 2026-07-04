/**
 * Build view: copy Markdown agent context + load previews.
 */
(function (global) {
  "use strict";

  async function copyText(value) {
    const text = String(value || "");
    if (!text) return false;
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        return true;
      }
    } catch (_) {}
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.left = "-9999px";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      document.body.removeChild(ta);
      return ok;
    } catch (_) {
      return false;
    }
  }

  function copyButtonLabel(btn, intent) {
    if (btn.id === "build-copy-agent-context-top") {
      return "复制场景原型调试上下文";
    }
    return intent === "full" ? "复制场景原型调试上下文" : "复制 Markdown 简报";
  }

  function reviewAxesFromShell(shell) {
    if (!shell) return { dataMode: "", reviewProjection: "" };
    return {
      dataMode: shell.getAttribute("data-data-mode") || "",
      reviewProjection: shell.getAttribute("data-review-projection") || "",
    };
  }

  function reviewAxesFromButton(btn) {
    return {
      dataMode: btn.getAttribute("data-data-mode") || "",
      reviewProjection: btn.getAttribute("data-review-projection") || "",
    };
  }

  async function handleCopyClick(btn) {
    const appId = btn.getAttribute("data-app-path");
    const node = btn.getAttribute("data-node");
    const tab =
      btn.getAttribute("data-tab") ||
      document.querySelector(".shell[data-build-node]")?.getAttribute("data-build-tab") ||
      "overview";
    const intent = btn.getAttribute("data-intent") || "lock_node";
    const defaultLabel = copyButtonLabel(btn, intent);
    if (!appId || !node) {
      console.warn("build copy: missing app_id or node on button", btn);
      return;
    }
    const prevLabel = btn.textContent;
    btn.disabled = true;
    try {
      const shell = document.querySelector(".shell[data-build-node]");
      const axes = {
        ...reviewAxesFromShell(shell),
        ...reviewAxesFromButton(btn),
      };
      const params = {
        app_id: appId,
        node,
        tab,
        intent,
        include_readiness: "1",
      };
      if (axes.dataMode) params.data_mode = axes.dataMode;
      if (axes.reviewProjection) params.review_projection = axes.reviewProjection;
      if (intent === "full") {
        params.include_graph = "semantic,eval,mcg,mrg";
      }
      const md = await fetchMarkdown(params);
      const ok = await copyText(md);
      btn.textContent = ok ? "已复制" : "复制失败";
      setTimeout(() => {
        btn.textContent = prevLabel || defaultLabel;
        btn.disabled = false;
      }, 1500);
    } catch (err) {
      console.error("build copy failed", err);
      btn.textContent = "复制失败";
      setTimeout(() => {
        btn.textContent = prevLabel || defaultLabel;
        btn.disabled = false;
      }, 1500);
    }
  }

  async function fetchMarkdown(params) {
    const qs = new URLSearchParams(params);
    const res = await fetch("/api/build/context/export?" + qs.toString());
    if (!res.ok) {
      throw new Error("export failed: " + res.status + " " + (await res.text()));
    }
    return res.text();
  }

  function shellContext() {
    const shell = document.querySelector("[data-build-node]");
    if (!shell) return null;
    const axes = reviewAxesFromShell(shell);
    return {
      appPath: shell.getAttribute("data-app-path") || "",
      node: shell.getAttribute("data-build-node") || "",
      tab: shell.getAttribute("data-build-tab") || "overview",
      dataMode: axes.dataMode,
      reviewProjection: axes.reviewProjection,
    };
  }

  function installCopyDelegation() {
    if (document.__buildCopyDelegationBound) return;
    document.__buildCopyDelegationBound = true;
    document.addEventListener("click", (event) => {
      const provBtn = event.target.closest(".build-copy-provenance[data-copy-text]");
      if (provBtn) {
        event.preventDefault();
        void copyText(provBtn.getAttribute("data-copy-text") || "");
        return;
      }
      const btn = event.target.closest("[data-app-path][data-node][data-intent]");
      if (!btn) return;
      event.preventDefault();
      void handleCopyClick(btn);
    });
  }

  async function refreshAgentPreview() {
    const pre = document.getElementById("build-agent-context-preview");
    const ctx = shellContext();
    if (!pre || !ctx || !ctx.appPath || !ctx.node) return;
    try {
      pre.textContent = "加载中…";
      pre.textContent = await fetchMarkdown({
        app_id: ctx.appPath,
        node: ctx.node,
        tab: "agent",
        intent: "full",
        include_graph: "semantic,eval,mcg,mrg",
        include_readiness: "1",
        ...(ctx.dataMode ? { data_mode: ctx.dataMode } : {}),
        ...(ctx.reviewProjection ? { review_projection: ctx.reviewProjection } : {}),
      });
    } catch (err) {
      pre.textContent = String(err);
    }
  }

  async function refreshGraphPanels(kind) {
    const ctx = shellContext();
    if (!ctx || !ctx.appPath || !ctx.node) return;
    const kinds = kind ? [kind] : ["semantic", "eval"];
    for (const graphKind of kinds) {
      const el = document.querySelector(
        '.build-graph-markdown[data-graph-kind="' + graphKind + '"]',
      );
      if (!el) continue;
      try {
        el.textContent = "加载图摘要…";
        el.textContent = await fetchMarkdown({
          app_id: ctx.appPath,
          node: ctx.node,
          tab: graphKind,
          intent: "debug_eval",
          include_graph: graphKind,
        });
      } catch (err) {
        el.textContent = String(err);
      }
    }
  }

  async function refreshOverviewGate() {
    const gateHost = document.getElementById("build-overview-gate");
    if (!gateHost) return;
    try {
      const res = await fetch("/api/host/readiness");
      if (!res.ok) {
        gateHost.textContent = "Gate 状态暂不可用";
        return;
      }
      const readiness = await res.json();
      const ctx = shellContext();
      const app = (readiness.apps || []).find((entry) => entry.app_id === (ctx && ctx.appPath));
      gateHost.textContent = [
        readiness.phase ? "host_phase=" + readiness.phase : "",
        app ? "app_phase=" + app.phase : "",
        readiness.access_ready != null ? "access_ready=" + readiness.access_ready : "",
      ]
        .filter(Boolean)
        .join(" · ") || "Gate 状态已加载";
    } catch (_) {
      gateHost.textContent = "";
    }
  }

  async function refreshArtifactPanels() {
    const ctx = shellContext();
    if (!ctx || !ctx.appPath || !ctx.node) return;

    const gateHost = document.getElementById("build-overview-gate");
    const artifactSummary = document.getElementById("build-artifact-summary");
    try {
      const md = await fetchMarkdown({
        app_id: ctx.appPath,
        node: ctx.node,
        tab: "artifact",
        intent: "debug_artifact",
        include_readiness: "1",
      });
      if (artifactSummary) {
        artifactSummary.textContent = md.split("### 建议 Agent 任务")[0].trim();
      }
      if (gateHost) {
        const gateLine = md
          .split("\n")
          .find((line) => line.includes("**Gate**") || line.includes("Gate"));
        gateHost.textContent = gateLine ? gateLine.replace(/^-\s*/, "") : "Gate 状态已加载";
      }
    } catch (err) {
      if (artifactSummary) artifactSummary.textContent = String(err);
    }

    try {
      const res = await fetch("/api/host/readiness");
      if (!res.ok || !artifactSummary) return;
      const readiness = await res.json();
      const app = (readiness.apps || []).find((entry) => entry.app_id === ctx.appPath);
      const cacheBits = [
        readiness.phase ? "host_phase=" + readiness.phase : "",
        app ? "app_phase=" + app.phase : "",
        readiness.access_ready != null ? "access_ready=" + readiness.access_ready : "",
      ].filter(Boolean);
      if (cacheBits.length) {
        artifactSummary.textContent += "\n\n" + cacheBits.join(" · ");
      }
    } catch (_) {}
  }

  function refreshBuildPanelForTab(tab) {
    const slug = String(tab || "").trim().toLowerCase();
    if (slug === "agent") {
      refreshAgentPreview();
      return;
    }
    if (slug === "semantic" || slug === "eval") {
      refreshGraphPanels(slug);
      return;
    }
    if (slug === "artifact") {
      refreshArtifactPanels();
      return;
    }
    if (slug === "overview") {
      refreshOverviewGate();
    }
  }

  function initBuildCopyContext() {
    installCopyDelegation();
    const ctx = shellContext();
    refreshBuildPanelForTab(ctx ? ctx.tab : "overview");
  }

  global.__meiBuildCopyContextInit = initBuildCopyContext;
  global.__meiBuildCopyContextRefresh = refreshBuildPanelForTab;

  document.addEventListener("mei:manage-tab-change", (event) => {
    const tab = event && event.detail ? event.detail.tab : "";
    refreshBuildPanelForTab(tab);
  });
})(typeof window !== "undefined" ? window : globalThis);
