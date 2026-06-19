/**
 * 宿主心跳：轮询 /api/host/heartbeat，在服务端失联或版本变更时提示用户刷新。
 */
(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.hostHeartbeatMounted) return;
  boot.hostHeartbeatMounted = true;

  const ROOT_ID = "mei-host-heartbeat-root";
  const HEARTBEAT_URL = "/api/host/heartbeat";
  const POLL_MS = 30000;
  const POLL_FAST_MS = 8000;
  const DISCONNECT_AFTER_STREAK = 2;

  let timerId = 0;
  let failureStreak = 0;
  let alertKind = "";
  let remoteVersion = "";
  let probeInFlight = false;

  function readMeta(name) {
    const node = document.querySelector('meta[name="' + name + '"]');
    return node ? String(node.getAttribute("content") || "").trim() : "";
  }

  function pageBuildVersion() {
    return readMeta("mei-host-version");
  }

  function shouldRun() {
    if (document.body && document.body.dataset.meiCompileShell === "true") {
      return false;
    }
    return !!pageBuildVersion();
  }

  function ensureRoot() {
    let root = document.getElementById(ROOT_ID);
    if (!root) {
      root = document.createElement("div");
      root.id = ROOT_ID;
      root.setAttribute("aria-live", "assertive");
      root.setAttribute("aria-relevant", "additions");
      document.body.appendChild(root);
    }
    return root;
  }

  function clearBanner() {
    const root = document.getElementById(ROOT_ID);
    if (root) root.innerHTML = "";
  }

  function renderBanner(kind) {
    const root = ensureRoot();
    const pageVersion = pageBuildVersion();
    const isVersion = kind === "version";
    const title = isVersion ? "服务端已更新" : "服务端失联";
    const message = isVersion
      ? "当前页面版本为 " +
        (pageVersion || "未知") +
        "，服务端版本为 " +
        (remoteVersion || "未知") +
        "。请刷新页面以加载新版本。"
      : "无法连接宿主服务，请检查网络或联系管理员后刷新页面。";
    const toneClass = isVersion
      ? " mei-host-heartbeat-banner--version"
      : " mei-host-heartbeat-banner--offline";

    root.innerHTML =
      '<div class="mei-host-heartbeat-banner' +
      toneClass +
      '" role="alert">' +
      '<div class="mei-host-heartbeat-banner__icon" aria-hidden="true">' +
      (isVersion ? "↑" : "!") +
      '</div><div class="mei-host-heartbeat-banner__body">' +
      '<div class="mei-host-heartbeat-banner__title"></div>' +
      '<div class="mei-host-heartbeat-banner__message"></div>' +
      '<div class="mei-host-heartbeat-banner__actions">' +
      '<button type="button" class="mei-host-heartbeat-banner__btn mei-host-heartbeat-banner__btn--primary" data-action="reload">刷新页面</button>' +
      (isVersion
        ? ""
        : '<button type="button" class="mei-host-heartbeat-banner__btn" data-action="retry">重试连接</button>') +
      "</div></div></div>";

    const banner = root.querySelector(".mei-host-heartbeat-banner");
    banner.querySelector(".mei-host-heartbeat-banner__title").textContent = title;
    banner.querySelector(".mei-host-heartbeat-banner__message").textContent = message;
    banner.querySelector('[data-action="reload"]').addEventListener("click", () => {
      window.location.reload();
    });
    const retry = banner.querySelector('[data-action="retry"]');
    if (retry) {
      retry.addEventListener("click", () => {
        tick();
      });
    }
  }

  function setAlert(kind) {
    if (alertKind === kind) return;
    alertKind = kind;
    if (!kind) {
      clearBanner();
      return;
    }
    renderBanner(kind);
    try {
      document.dispatchEvent(
        new CustomEvent("mei:host-heartbeat-alert", {
          detail: { kind, remoteVersion, pageVersion: pageBuildVersion() },
        }),
      );
    } catch (_) {}
  }

  function scheduleNext() {
    if (timerId) clearTimeout(timerId);
    const delay = alertKind === "offline" ? POLL_FAST_MS : POLL_MS;
    timerId = window.setTimeout(tick, delay);
  }

  async function tick() {
    if (!shouldRun() || probeInFlight) return;
    probeInFlight = true;
    try {
      const response = await fetch(HEARTBEAT_URL, {
        method: "GET",
        cache: "no-store",
        credentials: "same-origin",
        headers: { accept: "application/json" },
      });
      if (!response.ok) {
        throw new Error("heartbeat status " + String(response.status));
      }
      const payload = await response.json();
      const nextVersion = String((payload && payload.buildVersion) || "").trim();
      remoteVersion = nextVersion;
      failureStreak = 0;

      const pageVersion = pageBuildVersion();
      if (pageVersion && nextVersion && pageVersion !== nextVersion) {
        setAlert("version");
        return;
      }
      if (alertKind === "offline") {
        setAlert("");
      }
    } catch (_) {
      failureStreak += 1;
      if (failureStreak >= DISCONNECT_AFTER_STREAK) {
        setAlert("offline");
      }
    } finally {
      probeInFlight = false;
      if (shouldRun()) scheduleNext();
    }
  }

  function stop() {
    if (timerId) {
      clearTimeout(timerId);
      timerId = 0;
    }
    boot.hostHeartbeatMounted = false;
  }

  function start() {
    if (!shouldRun()) return;
    tick();
    document.addEventListener("visibilitychange", onVisibilityChange);
  }

  function onVisibilityChange() {
    if (document.visibilityState === "visible" && shouldRun()) {
      tick();
    }
  }

  boot.disposeHostHeartbeat = function () {
    document.removeEventListener("visibilitychange", onVisibilityChange);
    stop();
    clearBanner();
  };

  window.MeiHostHeartbeat = {
    tick,
    getAlertKind: () => alertKind,
  };

  start();
})();
