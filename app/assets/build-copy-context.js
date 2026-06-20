/**
 * Build view: copy Markdown agent context + load previews.
 */
(function (global) {
  "use strict";

  function copyText(value) {
    if (navigator.clipboard && global.isSecureContext) {
      return navigator.clipboard.writeText(value);
    }
    const ta = document.createElement("textarea");
    ta.value = value;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    return Promise.resolve();
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
    return {
      appPath: shell.getAttribute("data-app-path") || "",
      node: shell.getAttribute("data-build-node") || "",
      tab: shell.getAttribute("data-build-tab") || "overview",
    };
  }

  function bindCopyButtons() {
    document.querySelectorAll("[data-app-path][data-node][data-intent]").forEach((btn) => {
      if (btn.__buildCopyBound) return;
      btn.__buildCopyBound = true;
      btn.addEventListener("click", async () => {
        const appId = btn.getAttribute("data-app-path");
        const node = btn.getAttribute("data-node");
        const tab = btn.getAttribute("data-tab") || "overview";
        const intent = btn.getAttribute("data-intent") || "lock_node";
        try {
          const md = await fetchMarkdown({ app_id: appId, node, tab, intent, include_readiness: "1" });
          await copyText(md);
          btn.textContent = "已复制";
          setTimeout(() => {
            btn.textContent = intent === "full" ? "复制 Agent 上下文" : "复制 Markdown 简报";
          }, 1500);
        } catch (err) {
          console.error("build copy failed", err);
        }
      });
    });
    document.querySelectorAll(".build-copy-provenance[data-copy-text]").forEach((btn) => {
      if (btn.__provCopyBound) return;
      btn.__provCopyBound = true;
      btn.addEventListener("click", () => {
        copyText(btn.getAttribute("data-copy-text") || "");
      });
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
        include_graph: "semantic,eval",
        include_readiness: "1",
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
    bindCopyButtons();
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
