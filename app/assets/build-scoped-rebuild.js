/**
 * Build view scoped AOT rebuild (POST /api/host/build).
 */
(function (global) {
  "use strict";

  function shellCompileContext() {
    const shell = document.querySelector(".shell[data-compile-target], .shell[data-app-path]");
    if (!shell) return null;
    return {
      appId: String(
        shell.getAttribute("data-app-path") ||
          shell.getAttribute("data-app-id") ||
          "",
      ).trim(),
      sceneId: String(shell.getAttribute("data-compile-scene") || "").trim(),
      targetFile: String(shell.getAttribute("data-compile-target") || "").trim(),
    };
  }

  async function runScopedRebuild(ctx, trigger) {
    const appId = String((ctx && ctx.appId) || "").trim();
    if (!appId) {
      throw new Error("缺少 appId，无法重建 scope");
    }
    const body = { appId, mode: "build" };
    const sceneId = String((ctx && ctx.sceneId) || "").trim();
    const targetFile = String((ctx && ctx.targetFile) || "").trim();
    if (sceneId) body.sceneId = sceneId;
    if (targetFile) body.targetFile = targetFile;

    if (trigger) {
      trigger.disabled = true;
      trigger.dataset.meiRebuildPrev = trigger.textContent || "";
      trigger.textContent = "重建中…";
    }

    try {
      const res = await fetch("/api/host/build", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || res.statusText || "scoped rebuild failed");
      }
      global.dispatchEvent(new CustomEvent("meilang:scoped-rebuild-complete"));
      global.location.reload();
    } finally {
      if (trigger) {
        trigger.disabled = false;
        if (trigger.dataset.meiRebuildPrev) {
          trigger.textContent = trigger.dataset.meiRebuildPrev;
        }
      }
    }
  }

  function bindScopedRebuildControls(root) {
    const scope = root || document;
    scope.querySelectorAll("[data-mei-scoped-rebuild]").forEach((btn) => {
      if (btn.__meiScopedRebuildBound) return;
      btn.__meiScopedRebuildBound = true;
      btn.addEventListener("click", () => {
        const ctx = {
          appId: btn.getAttribute("data-app-id") || shellCompileContext()?.appId,
          sceneId: btn.getAttribute("data-scene-id") || shellCompileContext()?.sceneId,
          targetFile:
            btn.getAttribute("data-target-file") || shellCompileContext()?.targetFile,
        };
        runScopedRebuild(ctx, btn).catch((err) => {
          global.alert(String(err && err.message ? err.message : err));
        });
      });
    });
  }

  function bindOverviewGate() {
    const gateHost = document.getElementById("build-overview-gate");
    if (!gateHost || gateHost.querySelector("[data-mei-scoped-rebuild]")) return;
    const ctx = shellCompileContext();
    if (!ctx || !ctx.appId) return;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "mei-btn mei-btn--sm mt-2";
    btn.setAttribute("data-mei-scoped-rebuild", "1");
    btn.setAttribute("data-app-id", ctx.appId);
    if (ctx.sceneId) btn.setAttribute("data-scene-id", ctx.sceneId);
    if (ctx.targetFile) btn.setAttribute("data-target-file", ctx.targetFile);
    btn.textContent = "重建此 scope";
    gateHost.appendChild(btn);
    bindScopedRebuildControls(gateHost);
  }

  function initBuildScopedRebuild() {
    bindScopedRebuildControls(document);
    bindOverviewGate();
  }

  global.MeiBuildScopedRebuild = {
    runScopedRebuild,
    initBuildScopedRebuild,
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initBuildScopedRebuild);
  } else {
    initBuildScopedRebuild();
  }

  global.addEventListener("meilang:preview-updated", initBuildScopedRebuild);
})(typeof window !== "undefined" ? window : globalThis);
