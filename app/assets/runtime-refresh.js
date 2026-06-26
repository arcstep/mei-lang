(function initRuntimeObservatoryRefresh() {
  const shell = document.querySelector("[data-runtime-node]");
  if (!shell) return;
  const appPath = shell.getAttribute("data-app-path") || "";
  const refreshBtn = document.getElementById("runtime-refresh-btn");
  let timer = null;

  async function refreshSnapshot() {
    if (!appPath) return;
    const url = `/api/runtime/snapshot?appId=${encodeURIComponent(appPath)}`;
    try {
      const response = await fetch(url, {
        headers: { Accept: "application/json" },
        credentials: "same-origin",
      });
      if (!response.ok) return;
      const payload = await response.json();
      const script = document.getElementById("mei-runtime-observability-tree");
      if (script && payload && Array.isArray(payload.roots)) {
        script.textContent = JSON.stringify(payload.roots);
      }
      const detail = document.getElementById("runtime-detail-json");
      if (detail && payload) {
        detail.textContent = JSON.stringify(payload, null, 2);
      }
    } catch (_error) {
      /* ignore transient refresh errors */
    }
  }

  if (refreshBtn) {
    refreshBtn.addEventListener("click", () => {
      refreshSnapshot();
    });
  }

  refreshSnapshot();
  timer = window.setInterval(refreshSnapshot, 5000);
  window.addEventListener(
    "pagehide",
    () => {
      if (timer) window.clearInterval(timer);
    },
    { once: true },
  );
})();
