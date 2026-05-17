(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.statusBarMounted) return;
  boot.statusBarMounted = true;

  let refreshTimer = null;

  function els() {
    return {
      modelService: document.getElementById("mei-status-model-service"),
    };
  }

  function hasTargets() {
    const nodes = els();
    return !!nodes.modelService;
  }

  async function fetchJson(url, options) {
    const response = await fetch(url, options);
    if (!response.ok) {
      throw new Error("request failed: " + response.status);
    }
    return response.json();
  }

  function setChip(node, text, tone, title) {
    if (!node) return;
    node.textContent = text;
    node.title = title || text;
    node.dataset.tone = tone || "neutral";
  }

  function modelServiceSummary(payload, fallbackError) {
    if (!payload || typeof payload !== "object") {
      return {
        text: "模型服务 未知",
        tone: "warn",
        title: fallbackError || "模型服务状态读取失败",
      };
    }
    const provider = String(payload.provider_id || "").trim() || "--";
    const model = String(payload.model_id || "").trim() || "--";
    const reachable = !!payload.reachable;
    const latency = Number(payload.latency_ms || 0);
    const latencyText = Number.isFinite(latency) && latency > 0 ? " · " + String(latency) + "ms" : "";
    const statusCode = Number(payload.status_code || 0);
    const statusText = statusCode > 0 ? " · HTTP " + String(statusCode) : "";
    const titleBase = "provider=" + provider + " · model=" + model + latencyText + statusText;
    if (reachable) {
      return {
        text: "模型服务 在线",
        tone: "good",
        title: "探测成功 · " + titleBase,
      };
    }
    const error = String(payload.error || "").trim();
    return {
      text: "模型服务 异常",
      tone: "danger",
      title: (error ? error + " · " : "") + titleBase,
    };
  }

  async function refresh() {
    if (!hasTargets()) return;
    const nodes = els();
    setChip(nodes.modelService, "模型服务 探测中", "info", "正在探测当前默认模型服务连接");
    try {
      const probe = await fetchJson("/api/opencode/model/probe");
      const summary = modelServiceSummary(probe, "");
      setChip(nodes.modelService, summary.text, summary.tone, summary.title);
    } catch (error) {
      const summary = modelServiceSummary(null, String(error && error.message ? error.message : error || ""));
      setChip(nodes.modelService, summary.text, summary.tone, summary.title);
    }
  }

  function start() {
    refresh();
    if (refreshTimer) {
      clearInterval(refreshTimer);
    }
    refreshTimer = window.setInterval(function () {
      if (document.visibilityState === "hidden") return;
      refresh();
    }, 60000);
  }

  function stop() {
    if (refreshTimer) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
    boot.statusBarMounted = false;
  }

  boot.disposeStatusBar = stop;
  start();
})();
