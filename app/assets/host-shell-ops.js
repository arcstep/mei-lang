/**
 * mei-host-shell phase-1 ops panel: status / reload / prebuild.
 * Only activates when GET /api/host/ops/status returns hostShellOps=true.
 */
(function (global) {
  "use strict";

  const STATUS_URL = "/api/host/ops/status";
  const RELOAD_URL = "/api/host/ops/reload";
  const PREBUILD_URL = "/api/host/ops/prebuild";
  let pollTimer = null;

  function mountRoot() {
    return document.getElementById("host-shell-ops-mount");
  }

  function phaseLabel(phase) {
    const map = {
      starting: "启动中",
      bound: "已绑定（MCG）",
      ready: "就绪（含 MRG）",
    };
    return map[phase] || phase || "-";
  }

  function readyBadge(label, ready) {
    const cls = ready ? "host-shell-ops-badge is-ready" : "host-shell-ops-badge";
    return `<span class="${cls}">${label}: ${ready ? "是" : "否"}</span>`;
  }

  function renderPanel(status) {
    const root = mountRoot();
    if (!root) return;
    const job = status.job || null;
    const lastJob = status.lastJob || null;
    const jobLine = job
      ? `<p class="host-shell-ops-job">进行中：${job.kind} …</p>`
      : lastJob
        ? `<p class="host-shell-ops-job">最近：${lastJob.kind} · ${lastJob.status}${
            lastJob.message ? " · " + lastJob.message : ""
          }${lastJob.error ? " · " + lastJob.error : ""}</p>`
        : "";
    root.innerHTML = `
<section class="host-shell-ops-panel mei-surface-panel-muted mei-border-default" aria-label="Host 运维">
  <div class="host-shell-ops-head">
    <strong class="host-shell-ops-title">Host 运维</strong>
    <span class="host-shell-ops-sub">${status.displayLabel || ""}</span>
  </div>
  <dl class="host-shell-ops-meta">
    <div><dt>Shell</dt><dd>${(status.binary && status.binary.build_version) || "-"}</dd></div>
    <div><dt>Toolchain</dt><dd>${(status.toolchain && status.toolchain.active) || "-"}</dd></div>
    <div><dt>Env active</dt><dd>${(status.env && status.env.active) || "-"}</dd></div>
    <div><dt>Phase</dt><dd>${phaseLabel(status.phase)}</dd></div>
  </dl>
  <div class="host-shell-ops-badges">
    ${readyBadge("accessReady", !!status.accessReady)}
    ${readyBadge("warmupReady", !!status.warmupReady)}
  </div>
  ${jobLine}
  <div class="host-shell-ops-actions">
    <button type="button" class="mei-btn mei-btn--sm" data-host-ops="reload">重新加载（reload）</button>
    <button type="button" class="mei-btn mei-btn--sm mei-btn--accent" data-host-ops="prebuild">完整预构建（prebuild）</button>
    <button type="button" class="mei-btn mei-btn--sm" data-host-ops="refresh">刷新状态</button>
  </div>
</section>`;
    bindActions(root, status);
  }

  function setBusy(root, busy) {
    root.querySelectorAll("[data-host-ops]").forEach((btn) => {
      if (btn.getAttribute("data-host-ops") === "refresh") return;
      btn.disabled = busy;
    });
  }

  async function fetchStatus() {
    const res = await fetch(STATUS_URL, { headers: { Accept: "application/json" } });
    if (!res.ok) return null;
    const data = await res.json();
    return data && data.hostShellOps ? data : null;
  }

  async function refreshPanel() {
    const status = await fetchStatus();
    if (!status) return false;
    renderPanel(status);
    if (status.job && status.job.status === "running") {
      schedulePoll();
    } else if (pollTimer) {
      clearTimeout(pollTimer);
      pollTimer = null;
    }
    return true;
  }

  function schedulePoll() {
    if (pollTimer) return;
    pollTimer = global.setTimeout(async () => {
      pollTimer = null;
      await refreshPanel();
    }, 1500);
  }

  async function postReload(root) {
    setBusy(root, true);
    try {
      const res = await fetch(RELOAD_URL, { method: "POST" });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body.error || res.statusText || "reload failed");
      await refreshPanel();
      global.location.reload();
    } catch (err) {
      global.alert(String(err && err.message ? err.message : err));
      await refreshPanel();
    } finally {
      setBusy(root, false);
    }
  }

  async function postPrebuild(root) {
    if (
      !global.confirm(
        "将执行完整 prebuild（compile + import + data + warmup + finalize），可能需要较长时间。继续？",
      )
    ) {
      return;
    }
    setBusy(root, true);
    try {
      const res = await fetch(PREBUILD_URL, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ policy: "standard" }),
      });
      const body = await res.json().catch(() => ({}));
      if (!res.ok && res.status !== 202) {
        throw new Error(body.error || res.statusText || "prebuild failed");
      }
      schedulePoll();
      await refreshPanel();
    } catch (err) {
      global.alert(String(err && err.message ? err.message : err));
      await refreshPanel();
    } finally {
      setBusy(root, false);
    }
  }

  function bindActions(root, status) {
    root.querySelectorAll("[data-host-ops]").forEach((btn) => {
      if (btn.__hostOpsBound) return;
      btn.__hostOpsBound = true;
      btn.addEventListener("click", () => {
        const action = btn.getAttribute("data-host-ops");
        if (action === "reload") postReload(root);
        else if (action === "prebuild") postPrebuild(root);
        else if (action === "refresh") refreshPanel();
      });
    });
    if (status.job && status.job.status === "running") {
      setBusy(root, true);
    }
  }

  async function initHostShellOps() {
    const root = mountRoot();
    if (!root) return;
    const ok = await refreshPanel();
    if (!ok) {
      root.remove();
    }
  }

  global.MeiHostShellOps = { refreshPanel, initHostShellOps };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initHostShellOps);
  } else {
    initHostShellOps();
  }
})(typeof window !== "undefined" ? window : globalThis);
