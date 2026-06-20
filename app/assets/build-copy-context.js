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
    return res.text();
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
    if (!pre) return;
    const appId = pre.getAttribute("data-app-path");
    const node = pre.getAttribute("data-node");
    const tab = pre.getAttribute("data-tab") || "agent";
    try {
      pre.textContent = await fetchMarkdown({
        app_id: appId,
        node,
        tab,
        intent: "full",
        include_graph: "semantic,eval",
        include_readiness: "1",
      });
    } catch (err) {
      pre.textContent = String(err);
    }
  }

  async function refreshGraphPanels() {
    document.querySelectorAll(".build-graph-markdown[data-graph-kind]").forEach(async (el) => {
      const node = el.getAttribute("data-node");
      const kind = el.getAttribute("data-graph-kind");
      const shell = document.querySelector("[data-build-node]");
      const appPath = shell && shell.getAttribute("data-app-path");
      if (!appPath || !node) return;
      try {
        const md = await fetchMarkdown({
          app_id: appPath,
          node,
          tab: kind === "eval" ? "eval" : "semantic",
          intent: "debug_eval",
          include_graph: kind,
        });
        el.textContent = md;
      } catch (err) {
        el.textContent = String(err);
      }
    });
  }

  async function refreshArtifactPanels() {
    const shell = document.querySelector("[data-build-node]");
    const appPath = shell && shell.getAttribute("data-app-path");
    const node = shell && shell.getAttribute("data-build-node");
    if (!appPath || !node) return;

    const gateHost = document.getElementById("build-overview-gate");
    const artifactSummary = document.getElementById("build-artifact-summary");
    try {
      const md = await fetchMarkdown({
        app_id: appPath,
        node,
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
      if (gateHost) gateHost.textContent = String(err);
    }

    try {
      const res = await fetch("/api/host/readiness");
      if (!res.ok || !artifactSummary) return;
      const readiness = await res.json();
      const app = (readiness.apps || []).find((entry) => entry.app_id === appPath);
      const cacheBits = [
        readiness.phase ? `host_phase=${readiness.phase}` : "",
        app ? `app_phase=${app.phase}` : "",
        readiness.access_ready != null ? `access_ready=${readiness.access_ready}` : "",
      ].filter(Boolean);
      if (cacheBits.length) {
        artifactSummary.textContent += "\n\n" + cacheBits.join(" · ");
      }
    } catch (_) {}
  }

  function initBuildCopyContext() {
    bindCopyButtons();
    refreshAgentPreview();
    refreshGraphPanels();
    refreshArtifactPanels();
  }

  global.__meiBuildCopyContextInit = initBuildCopyContext;
})(typeof window !== "undefined" ? window : globalThis);
