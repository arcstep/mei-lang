(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.statusBarMounted) return;
  boot.statusBarMounted = true;

  let refreshTimer = null;
  let probeFailureStreak = 0;
  let probeLastSuccessAtMs = 0;
  let probeHasResult = false;
  const PROBE_RED_AFTER_STREAK = 3;
  const PROBE_RED_AFTER_MS = 20000;
  const PROBE_COLD_START_RED_AFTER_STREAK = 5;

  function agentUtils() {
    return window.MeiAgentPanelUtils || null;
  }

  function isAgentBlocked() {
    const U = agentUtils();
    return !!(U && typeof U.areAgentRequestsBlocked === "function" && U.areAgentRequestsBlocked());
  }

  function readMeta(name) {
    const node = document.querySelector('meta[name="' + name + '"]');
    return node ? String(node.getAttribute("content") || "").trim() : "";
  }

  function els() {
    return {
      compliance: document.getElementById("mei-status-compliance"),
      hostVersion: document.getElementById("mei-status-host-version"),
      modelService: document.getElementById("mei-status-model-service"),
    };
  }

  function applyComplianceChip() {
    const nodes = els();
    if (!nodes.compliance) return;
    const parts = [
      readMeta("mei-host-icp-record"),
      readMeta("mei-host-psb-record"),
      readMeta("mei-host-copyright"),
    ].filter(Boolean);
    if (!parts.length) {
      nodes.compliance.hidden = true;
      nodes.compliance.textContent = "";
      return;
    }
    nodes.compliance.hidden = false;
    const text = parts.join(" · ");
    setChip(nodes.compliance, text, "neutral", text);
  }

  function applyHostVersionChip() {
    const nodes = els();
    const label = readMeta("mei-host-version-label");
    const version = readMeta("mei-host-version");
    const text = label || (version ? "Mei " + version : "");
    if (!text) return;
    setChip(nodes.hostVersion, text, "neutral", text);
  }

  function hasTargets() {
    const nodes = els();
    return !!nodes.modelService;
  }

  async function fetchJson(url, options) {
    const response = await fetch(url, options);
    if (!response.ok) {
      let detail = "";
      try {
        detail = (await response.clone().text()).trim();
      } catch (_) {}
      const error = new Error(
        detail ? "request failed: " + response.status + " " + detail : "request failed: " + response.status,
      );
      error.httpStatus = response.status;
      throw error;
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
    const nowMs = Date.now();
    if (!payload || typeof payload !== "object") {
      probeFailureStreak += 1;
      probeHasResult = true;
      const shouldAlertCold = probeFailureStreak >= PROBE_COLD_START_RED_AFTER_STREAK;
      return {
        text: shouldAlertCold ? "模型服务 异常" : "模型服务 连接中",
        tone: shouldAlertCold ? "danger" : "info",
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
      probeFailureStreak = 0;
      probeLastSuccessAtMs = nowMs;
      probeHasResult = true;
      return {
        text: "模型服务 在线",
        tone: "good",
        title: "探测成功 · " + titleBase,
      };
    }
    probeFailureStreak += 1;
    probeHasResult = true;
    const error = String(payload.error || "").trim();
    const withinGrace =
      probeLastSuccessAtMs > 0 && nowMs - probeLastSuccessAtMs < PROBE_RED_AFTER_MS;
    const transientFailure = probeLastSuccessAtMs > 0
      ? probeFailureStreak < PROBE_RED_AFTER_STREAK || withinGrace
      : probeFailureStreak < PROBE_COLD_START_RED_AFTER_STREAK;
    if (transientFailure) {
      return {
        text: "模型服务 连接中",
        tone: "info",
        title: (error ? error + " · " : "") + "正在尝试连接 · " + titleBase,
      };
    }
    return {
      text: "模型服务 异常",
      tone: "danger",
      title: (error ? error + " · " : "") + titleBase,
    };
  }

  function blockFromHttpStatus(status) {
    const U = agentUtils();
    if (!U || typeof U.blockAgentRequests !== "function") return;
    if (status === 401) {
      U.blockAgentRequests("session_expired");
      return;
    }
    if (status === 403) {
      U.blockAgentRequests("capability");
    }
  }

  async function refresh() {
    if (!hasTargets()) return;
    if (isAgentBlocked()) {
      stop();
      return;
    }
    const nodes = els();
    if (!probeHasResult) {
      setChip(nodes.modelService, "模型服务 探测中", "info", "正在探测当前默认模型服务连接");
    }
    try {
      const probe = await fetchJson("/api/agent/model/probe", { credentials: "same-origin" });
      const summary = modelServiceSummary(probe, "");
      setChip(nodes.modelService, summary.text, summary.tone, summary.title);
    } catch (error) {
      const status = Number(error && error.httpStatus) || 0;
      if (status === 401 || status === 403) {
        blockFromHttpStatus(status);
        const U = agentUtils();
        const message =
          U && typeof U.agentRequestsBlockMessage === "function"
            ? U.agentRequestsBlockMessage(status === 403 ? "capability" : "session_expired")
            : status === 403
              ? "当前账号无权限访问模型探测"
              : "登录已失效";
        setChip(nodes.modelService, "模型服务 已暂停", "danger", message);
        stop();
        return;
      }
      const summary = modelServiceSummary(null, String(error && error.message ? error.message : error || ""));
      setChip(nodes.modelService, summary.text, summary.tone, summary.title);
    }
  }

  function startInterval() {
    if (refreshTimer) {
      clearInterval(refreshTimer);
    }
    refreshTimer = window.setInterval(function () {
      if (document.visibilityState === "hidden") return;
      if (isAgentBlocked()) {
        stop();
        return;
      }
      refresh();
    }, 60000);
  }

  async function start() {
    applyComplianceChip();
    applyHostVersionChip();
    if (!hasTargets()) return;

    const U = agentUtils();
    if (U && typeof U.resolveAgentAuthGate === "function") {
      try {
        const gate = await U.resolveAgentAuthGate();
        if (!gate.allowed) {
          if (typeof U.blockAgentRequests === "function") {
            U.blockAgentRequests(gate.reason);
          }
          const message =
            typeof U.agentRequestsBlockMessage === "function"
              ? U.agentRequestsBlockMessage(gate.reason)
              : "助手鉴权未通过，模型探测已暂停";
          setChip(els().modelService, "模型服务 已暂停", "neutral", message);
          return;
        }
      } catch (_) {
        if (typeof U.blockAgentRequests === "function") {
          U.blockAgentRequests("session_check_error");
        }
        setChip(els().modelService, "模型服务 已暂停", "neutral", "无法确认登录状态");
        return;
      }
    }

    refresh();
    startInterval();
  }

  function stop() {
    if (refreshTimer) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
    boot.statusBarMounted = false;
  }

  function onAgentAuthBlocked() {
    stop();
    const nodes = els();
    if (!nodes.modelService) return;
    const U = agentUtils();
    const reason =
      U && typeof U.agentRequestsBlockMessage === "function"
        ? U.agentRequestsBlockMessage()
        : "助手请求已暂停";
    setChip(nodes.modelService, "模型服务 已暂停", "neutral", reason);
  }

  document.addEventListener("mei:agent-auth-blocked", onAgentAuthBlocked);

  boot.disposeStatusBar = function () {
    document.removeEventListener("mei:agent-auth-blocked", onAgentAuthBlocked);
    stop();
  };
  start();
})();
