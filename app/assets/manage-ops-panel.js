(function initManageOpsPanel() {
  const root = document.getElementById("manage-ops-panel");
  if (!root) return;
  const appId = root.dataset.appId || "";
  if (!appId) return;

  const statusEl = root.querySelector("[data-ops-status]");
  const bodyEl = root.querySelector("[data-ops-body]");
  const refreshBtn = root.querySelector("[data-ops-refresh]");

  async function loadOpsConfig() {
    if (statusEl) statusEl.textContent = "加载中…";
    try {
      const response = await fetch(`/api/ops/config/${encodeURIComponent(appId)}`, {
        credentials: "same-origin",
      });
      const payload = await response.json();
      if (!response.ok) {
        throw new Error(payload.error || `HTTP ${response.status}`);
      }
      if (statusEl) {
        statusEl.textContent = `rev ${payload.journal_revision || 0}`;
      }
      if (bodyEl) {
        bodyEl.textContent = JSON.stringify(payload.config?.ops || {}, null, 2);
      }
    } catch (error) {
      if (statusEl) statusEl.textContent = "加载失败";
      if (bodyEl) bodyEl.textContent = String(error?.message || error);
    }
  }

  if (refreshBtn) {
    refreshBtn.addEventListener("click", loadOpsConfig);
  }
  loadOpsConfig();
})();
