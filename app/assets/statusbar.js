(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.statusBarMounted) return;
  boot.statusBarMounted = true;

  let refreshTimer = null;

  function els() {
    return {
      skill: document.getElementById("mei-status-skill"),
      opencode: document.getElementById("mei-status-opencode"),
    };
  }

  function hasTargets() {
    const nodes = els();
    return !!(nodes.skill || nodes.opencode);
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

  function openCodeModeLabel(value) {
    const mode = String(value || "").trim().toLowerCase();
    if (mode === "managed") return "托管";
    if (mode === "external") return "外部";
    return "--";
  }

  function skillSummary(skill) {
    if (!skill || !skill.source_present) {
      return { text: "Skill 缺失", tone: "danger", title: "Skill 源目录不存在" };
    }
    if (skill.installed && skill.stale) {
      return { text: "Skill 待同步", tone: "warn", title: "Skill 已安装，但版本落后于源目录" };
    }
    if (skill.installed) {
      return { text: "Skill 已装", tone: "good", title: "Skill 已安装并可用" };
    }
    return { text: "Skill 源目录", tone: "info", title: "Skill 源目录存在，但尚未安装" };
  }

  function openCodeSummary(state) {
    const config = state.config;
    const runtime = state.runtime;
    const health = state.health;
    const mode = openCodeModeLabel(
      (runtime && runtime.connection_source) || (config && config.preferred_mode) || "",
    );
    let phase = "未配";
    let tone = "neutral";
    if (state.loading) {
      phase = "刷新中";
      tone = "info";
    } else if (health && health.healthy) {
      phase = "在线";
      tone = "good";
    } else if (runtime && runtime.running) {
      phase = mode === "托管" ? "启动中" : "未连";
      tone = mode === "托管" ? "warn" : "danger";
    } else if (
      config &&
      config.preferred_mode === "managed" &&
      config.managed_start_available
    ) {
      phase = "可启动";
      tone = "info";
    }
    const model =
      String(config && (config.completion_model || config.provider_name || config.provider_id) || "").trim() ||
      "";
    const text = "OpenCode " + mode + "·" + phase;
    const title = model ? text + " · " + model : text;
    return { text, tone, title };
  }

  async function refresh() {
    if (!hasTargets()) return;
    const nodes = els();
    const state = { loading: true, config: null, runtime: null, health: null };
    const loadingSummary = openCodeSummary(state);
    setChip(nodes.opencode, loadingSummary.text, loadingSummary.tone, loadingSummary.title);
    try {
      const [config, runtime, skill] = await Promise.all([
        fetchJson("/api/opencode/config"),
        fetchJson("/api/opencode/runtime"),
        fetchJson("/api/opencode/skill"),
      ]);
      state.loading = false;
      state.config = config;
      state.runtime = runtime;
      if (runtime && runtime.running) {
        try {
          state.health = await fetchJson("/api/opencode/health");
        } catch (_) {
          state.health = null;
        }
      }
      const skillView = skillSummary(skill);
      const openCodeView = openCodeSummary(state);
      setChip(nodes.skill, skillView.text, skillView.tone, skillView.title);
      setChip(nodes.opencode, openCodeView.text, openCodeView.tone, openCodeView.title);
    } catch (_) {
      state.loading = false;
      setChip(nodes.skill, "Skill 未知", "warn", "Skill 状态读取失败");
      setChip(nodes.opencode, "OpenCode 未知", "warn", "OpenCode 状态读取失败");
    }
  }

  function start() {
    refresh();
    const manageMode =
      document.body && document.body.classList.contains("manage-mode");
    // 管理页由右侧 OpenCode 面板负责更高频状态同步，避免重复轮询。
    if (manageMode) {
      return;
    }
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
