/**
 * Build view exec REPL panel (metric/query smoke).
 */
(function (global) {
  "use strict";

  function storageKey(appPath, node) {
    return "mei.build.exec." + appPath + "." + node;
  }

  function initBuildExecPanel() {
    const panel = document.getElementById("build-exec-panel");
    if (!panel || panel.__bound) return;
    panel.__bound = true;
    const appPath = panel.getAttribute("data-app-path");
    const node = panel.getAttribute("data-node");
    const output = document.getElementById("build-exec-output");
    const runBtn = document.getElementById("build-exec-run");
    let scope = "warmup";

    panel.querySelectorAll(".build-exec-scope").forEach((btn) => {
      btn.addEventListener("click", () => {
        scope = btn.getAttribute("data-scope") || "warmup";
        panel.querySelectorAll(".build-exec-scope").forEach((b) => b.classList.remove("is-active"));
        btn.classList.add("is-active");
      });
    });

    if (runBtn) {
      runBtn.addEventListener("click", async () => {
        if (!output) return;
        output.textContent = "running…";
        try {
          const res = await fetch(
            "/api/build/context/export?" +
              new URLSearchParams({
                app_id: appPath,
                node,
                tab: "exec",
                intent: "debug_eval",
                scope,
              }).toString()
          );
          const text = await res.text();
          output.textContent = text;
          try {
            sessionStorage.setItem(
              storageKey(appPath, node),
              JSON.stringify({ scope, text, at: Date.now() })
            );
          } catch (_) {}
        } catch (err) {
          output.textContent = String(err);
        }
      });
    }
  }

  global.__meiBuildExecPanelInit = initBuildExecPanel;
})(typeof window !== "undefined" ? window : globalThis);
