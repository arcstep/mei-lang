use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::UiRouteMode;

pub(super) fn panel_view(compiled: &CompiledApp, route_mode: UiRouteMode) -> impl IntoView {
    view! {
        <div
            id="meilang-opencode-root"
            class="opencode-panel"
            data-app-id=compiled.app_id.clone()
            data-entry-target=compiled.entry_target.clone()
            data-mode=route_mode.slug()
        >
            <div class="opencode-section">
                <div class="opencode-line">
                    <span class="opencode-label">"状态"</span>
                    <strong id="meilang-opencode-status" class="opencode-badge opencode-badge-idle">"加载中"</strong>
                </div>
                <p id="meilang-opencode-summary" class="opencode-summary">
                    "正在读取 OpenCode 配置与运行态..."
                </p>
            </div>
            <div class="opencode-actions">
                <button id="meilang-opencode-refresh" class="opencode-btn opencode-btn-muted" type="button">
                    "刷新"
                </button>
                <button id="meilang-opencode-start" class="opencode-btn" type="button">
                    "启动"
                </button>
                <button id="meilang-opencode-stop" class="opencode-btn opencode-btn-danger" type="button">
                    "停止"
                </button>
            </div>
            <div class="opencode-section">
                <div class="panel-heading">
                    <h3>"配置"</h3>
                    <p>"provider / model / env"</p>
                </div>
                <ul id="meilang-opencode-config" class="opencode-list">
                    <li>"正在读取配置..."</li>
                </ul>
            </div>
            <div class="opencode-section">
                <div class="panel-heading">
                    <h3>"运行态"</h3>
                    <p>"managed server health"</p>
                </div>
                <ul id="meilang-opencode-runtime" class="opencode-list">
                    <li>"正在读取运行态..."</li>
                </ul>
            </div>
            <div class="opencode-section">
                <div class="panel-heading">
                    <h3>"当前上下文"</h3>
                    <p>"host shell context"</p>
                </div>
                <ul class="opencode-list">
                    <li>{format!("当前应用：{}", compiled.app_id)}</li>
                    <li>{format!("入口脚本：{}", compiled.entry_target)}</li>
                    <li>{format!("模式：{}", route_mode.slug())}</li>
                </ul>
            </div>
        </div>
    }
}

pub(super) const BOOTSTRAP_SCRIPT: &str = r#"
(function () {
  const root = document.getElementById("meilang-opencode-root");
  if (!root) return;

  const els = {
    status: document.getElementById("meilang-opencode-status"),
    summary: document.getElementById("meilang-opencode-summary"),
    config: document.getElementById("meilang-opencode-config"),
    runtime: document.getElementById("meilang-opencode-runtime"),
    refresh: document.getElementById("meilang-opencode-refresh"),
    start: document.getElementById("meilang-opencode-start"),
    stop: document.getElementById("meilang-opencode-stop"),
  };

  const state = {
    config: null,
    runtime: null,
    health: null,
    loading: false,
  };

  function escapeHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  async function fetchJson(url, init) {
    const response = await fetch(url, init);
    if (!response.ok) {
      throw new Error(url + " -> " + response.status);
    }
    return response.json();
  }

  function setButtonState(disabled) {
    if (els.refresh) els.refresh.disabled = disabled;
    if (els.start) els.start.disabled = disabled;
    if (els.stop) els.stop.disabled = disabled;
  }

  function renderList(target, items) {
    if (!target) return;
    target.innerHTML = items.map((item) => "<li>" + item + "</li>").join("");
  }

  function renderStatus() {
    if (!els.status || !els.summary) return;
    const runtime = state.runtime;
    const health = state.health;
    const config = state.config;
    let label = "未配置";
    let badge = "opencode-badge opencode-badge-idle";
    if (state.loading) {
      label = "刷新中";
      badge = "opencode-badge opencode-badge-busy";
    } else if (runtime && runtime.running && health && health.healthy) {
      label = "运行中";
      badge = "opencode-badge opencode-badge-ok";
    } else if (runtime && runtime.running) {
      label = "已启动";
      badge = "opencode-badge opencode-badge-warn";
    } else if (config && config.runtime_env_ready) {
      label = "可启动";
      badge = "opencode-badge opencode-badge-warn";
    }
    els.status.className = badge;
    els.status.textContent = label;

    const parts = [];
    if (runtime && runtime.server_url) parts.push("server=" + runtime.server_url);
    if (health && health.version) parts.push("health=v" + health.version);
    if (config && config.default_model) parts.push("model=" + config.default_model);
    if (config && Array.isArray(config.missing_env) && config.missing_env.length > 0) {
      parts.push("缺少环境变量：" + config.missing_env.join(", "));
    }
    els.summary.textContent = parts.length > 0 ? parts.join(" · ") : "OpenCode 宿主桥接已接入，等待进一步配置。";
  }

  function renderConfig() {
    const config = state.config;
    if (!config) {
      renderList(els.config, ["尚未读取到配置"]);
      return;
    }
    const items = [
      "provider: " + escapeHtml(config.provider_name || config.provider_id || "-"),
      "default_model: " + escapeHtml(config.default_model || "-"),
      "base_url: " + escapeHtml(config.base_url || "-"),
      "project_config_present: " + escapeHtml(String(config.project_config_present === true)),
      "runtime_env_ready: " + escapeHtml(String(config.runtime_env_ready === true)),
    ];
    if (Array.isArray(config.missing_env) && config.missing_env.length > 0) {
      items.push("missing_env: " + escapeHtml(config.missing_env.join(", ")));
    }
    renderList(els.config, items);
  }

  function renderRuntime() {
    const runtime = state.runtime;
    const health = state.health;
    if (!runtime) {
      renderList(els.runtime, ["尚未读取到运行态"]);
      return;
    }
    const items = [
      "running: " + escapeHtml(String(runtime.running === true)),
      "server_url: " + escapeHtml(runtime.server_url || "-"),
      "pid: " + escapeHtml(runtime.pid != null ? String(runtime.pid) : "-"),
      "working_directory: " + escapeHtml(runtime.working_directory || "-"),
      "health: " + escapeHtml(health && health.healthy ? "ok" : "offline"),
    ];
    if (runtime.last_exit) {
      items.push(
        "last_exit: " +
          escapeHtml(
            String(runtime.last_exit.kind || "-") +
              " / " +
              String(runtime.last_exit.code != null ? runtime.last_exit.code : "-")
          )
      );
    }
    renderList(els.runtime, items);
  }

  async function refreshAll() {
    state.loading = true;
    setButtonState(true);
    renderStatus();
    try {
      const [config, runtime] = await Promise.all([
        fetchJson("/api/opencode/config"),
        fetchJson("/api/opencode/runtime"),
      ]);
      state.config = config;
      state.runtime = runtime;
      if (runtime && runtime.running) {
        try {
          state.health = await fetchJson("/api/opencode/health");
        } catch (_) {
          state.health = null;
        }
      } else {
        state.health = null;
      }
    } catch (error) {
      state.health = null;
      if (els.summary) {
        els.summary.textContent = "读取 OpenCode 状态失败：" + String(error.message || error);
      }
    } finally {
      state.loading = false;
      setButtonState(false);
      renderStatus();
      renderConfig();
      renderRuntime();
    }
  }

  async function startServer() {
    setButtonState(true);
    if (els.summary) els.summary.textContent = "正在启动 OpenCode 服务...";
    try {
      await fetchJson("/api/opencode/start", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ port: 4099 }),
      });
    } catch (error) {
      if (els.summary) {
        els.summary.textContent = "启动失败：" + String(error.message || error);
      }
    }
    await refreshAll();
  }

  async function stopServer() {
    setButtonState(true);
    if (els.summary) els.summary.textContent = "正在停止 OpenCode 服务...";
    try {
      await fetchJson("/api/opencode/stop", {
        method: "POST",
      });
    } catch (error) {
      if (els.summary) {
        els.summary.textContent = "停止失败：" + String(error.message || error);
      }
    }
    await refreshAll();
  }

  if (els.refresh) els.refresh.addEventListener("click", refreshAll);
  if (els.start) els.start.addEventListener("click", startServer);
  if (els.stop) els.stop.addEventListener("click", stopServer);

  refreshAll();
  window.setInterval(refreshAll, 8000);
})();
"#;
