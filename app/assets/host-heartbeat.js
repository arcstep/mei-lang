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
  const SUCCESS_VISIBLE_MS = 5000;

  let timerId = 0;
  let failureStreak = 0;
  let alertKind = "";
  let remoteVersion = "";
  let probeInFlight = false;
  let lastHeartbeat = null;
  let successTimerId = 0;

  function readMeta(name) {
    const node = document.querySelector('meta[name="' + name + '"]');
    return node ? String(node.getAttribute("content") || "").trim() : "";
  }

  function pageBuildVersion() {
    return readMeta("mei-host-version");
  }

  function formatDurationMs(value) {
    if (value == null || value === "") return "";
    const ms = Number(value);
    if (!Number.isFinite(ms) || ms < 0) return "";
    if (ms < 1000) return Math.round(ms) + "ms";
    if (ms < 60000) return (ms / 1000).toFixed(1) + "s";
    const minutes = Math.floor(ms / 60000);
    const seconds = ((ms % 60000) / 1000).toFixed(1);
    return `${minutes}m ${seconds}s`;
  }

  function heartbeatPhaseLabel(phase) {
    const normalized = String(phase || "").trim().toLowerCase();
    const map = {
      starting: "启动中",
      bound: "已绑定，等待后台构建",
      building: "后台构建中",
      verifying: "校验中",
      degraded: "部分产物未就绪",
      failed: "后台构建失败",
      ready: "已就绪",
      skipped: "已跳过",
    };
    return map[normalized] || normalized || "未知状态";
  }

  function buildAlertCopy(payload) {
    const phase = String(payload?.phase || "").trim().toLowerCase();
    const activeJob = String(payload?.activeJob || "").trim();
    const activeElapsed = formatDurationMs(payload?.activeJobElapsedMs);
    const lastTotal = formatDurationMs(payload?.lastBuildTotalMs);
    const lastCompile = formatDurationMs(payload?.lastBuildCompileMs);
    const lastWarmup = formatDurationMs(payload?.lastBuildWarmupMs);
    const warningCount = Number(payload?.lastWarningCount || 0);
    const title =
      phase === "failed"
        ? "访问态构建失败"
        : phase === "degraded"
          ? "部分访问能力降级"
          : "访问态构建中";
    const lines = [
      `当前状态：${heartbeatPhaseLabel(phase)}`,
      activeJob ? `后台任务：${activeJob}` : "",
      activeElapsed ? `已耗时：${activeElapsed}` : "",
      lastTotal
        ? `最近一次构建：总计 ${lastTotal}${
            lastCompile ? `，编译 ${lastCompile}` : ""
          }${lastWarmup ? `，warmup ${lastWarmup}` : ""}`
        : "",
      warningCount > 0 ? `最近一次构建含 ${warningCount} 条 warning / 降级项。` : "",
      phase === "failed"
        ? "宿主已启动，但部分访问态产物构建失败；请到构建视图检查失败项。"
        : phase === "degraded"
          ? "宿主服务可用；部分页面或指标可能局部降级，未命中缺失产物的功能仍可正常访问。"
          : "宿主已启动，后台正在生成访问态产物；完成前部分页面或指标可能暂时不可用。",
    ].filter(Boolean);
    return { title, message: lines.join(" ") };
  }

  function buildSuccessCopy(payload) {
    const lastTotal = formatDurationMs(payload?.lastBuildTotalMs);
    const lastCompile = formatDurationMs(payload?.lastBuildCompileMs);
    const lastWarmup = formatDurationMs(payload?.lastBuildWarmupMs);
    const fullWarmupReady = payload?.fullWarmupReady === true;
    const message = [
      fullWarmupReady
        ? "OK! 全量访问态产物已就绪，现在可以正常访问页面。"
        : "OK! 关键访问态产物已就绪，后台仍在生成 deferred 产物。",
      lastTotal
        ? `最近一次构建：总计 ${lastTotal}${
            lastCompile ? `，编译 ${lastCompile}` : ""
          }${lastWarmup ? `，warmup ${lastWarmup}` : ""}`
        : "",
    ]
      .filter(Boolean)
      .join(" ");
    return {
      title: fullWarmupReady ? "FULL READY! 全量预热完成" : "ACCESS READY! 访问态已就绪",
      message,
    };
  }

  function shouldRun() {
    if (document.body && document.body.dataset.meiCompileShell === "true") {
      return false;
    }
    if (pageBuildVersion()) return true;
    const path = window.location.pathname || "";
    return (
      path === "/host" ||
      path === "/login" ||
      path.startsWith("/account/") ||
      path.startsWith("/logout")
    );
  }

  function currentPageAppId() {
    const fromRuntime = String(window.__meiRuntimeAppId || "").trim();
    if (fromRuntime) return fromRuntime;
    const match = String(window.location.pathname || "").match(
      /^\/apps\/[^/]+\/([^/]+)/,
    );
    return match ? String(match[1] || "").trim() : "";
  }

  function isShellSurface() {
    const path = window.location.pathname || "";
    return (
      path === "/host" ||
      path === "/login" ||
      path.startsWith("/account/") ||
      path.startsWith("/logout")
    );
  }

  function appAccessReadyFromPayload(payload, appId) {
    if (!payload || !appId) return false;
    const apps = Array.isArray(payload.apps) ? payload.apps : [];
    const entry = apps.find((app) => String(app?.appId || "").trim() === appId);
    if (entry) return entry.accessReady === true;
    return false;
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

  function cancelSuccessTimer() {
    if (successTimerId) {
      clearTimeout(successTimerId);
      successTimerId = 0;
    }
  }

  function renderBanner(kind, payload) {
    const root = ensureRoot();
    const pageVersion = pageBuildVersion();
    const isVersion = kind === "version";
    const isBuild = kind === "build";
    const isSuccess = kind === "success";
    const title = isVersion
      ? "服务端已更新"
      : isSuccess
        ? buildSuccessCopy(payload).title
      : isBuild
        ? buildAlertCopy(payload).title
        : "服务端失联";
    const message = isVersion
      ? "当前页面版本为 " +
        (pageVersion || "未知") +
        "，服务端版本为 " +
        (remoteVersion || "未知") +
        "。请刷新页面以加载新版本。"
      : isSuccess
        ? buildSuccessCopy(payload).message
      : isBuild
        ? buildAlertCopy(payload).message
        : "无法连接宿主服务，请检查网络或联系管理员后刷新页面。";
    const toneClass = isVersion
      ? " mei-host-heartbeat-banner--version"
      : isSuccess
        ? " mei-host-heartbeat-banner--success"
      : " mei-host-heartbeat-banner--offline";

    root.innerHTML =
      '<div class="mei-host-heartbeat-banner' +
      toneClass +
      '" role="alert">' +
      '<div class="mei-host-heartbeat-banner__icon" aria-hidden="true">' +
      (isVersion ? "↑" : isSuccess ? "OK" : "!") +
      '</div><div class="mei-host-heartbeat-banner__body">' +
      '<div class="mei-host-heartbeat-banner__title"></div>' +
      '<div class="mei-host-heartbeat-banner__message"></div>' +
      '<div class="mei-host-heartbeat-banner__actions">' +
      '<button type="button" class="mei-host-heartbeat-banner__btn mei-host-heartbeat-banner__btn--primary" data-action="reload">刷新页面</button>' +
      (isVersion || isSuccess
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
    cancelSuccessTimer();
    alertKind = kind;
    if (!kind) {
      clearBanner();
      return;
    }
    renderBanner(kind, lastHeartbeat);
    if (kind === "success") {
      successTimerId = window.setTimeout(() => {
        successTimerId = 0;
        if (alertKind === "success") {
          setAlert("");
        }
      }, SUCCESS_VISIBLE_MS);
    }
    try {
      document.dispatchEvent(
        new CustomEvent("mei:host-heartbeat-alert", {
          detail: { kind, remoteVersion, pageVersion: pageBuildVersion(), payload: lastHeartbeat },
        }),
      );
    } catch (_) {}
  }

  function scheduleNext() {
    if (timerId) clearTimeout(timerId);
    const delay = alertKind === "offline" || alertKind === "build" ? POLL_FAST_MS : POLL_MS;
    timerId = window.setTimeout(tick, delay);
  }

  function isActivelyBuilding(payload) {
    const phase = String(payload?.phase || "").trim().toLowerCase();
    const activeJob = String(payload?.activeJob || "").trim();
    return (
      activeJob &&
      (phase === "building" || phase === "verifying" || phase === "starting")
    );
  }

  function isHostServiceable(payload) {
    return payload?.hostReady === true || payload?.ready === true;
  }

  function isAccessFullyReady(payload) {
    if (isShellSurface()) {
      return isHostServiceable(payload);
    }
    const appId = currentPageAppId();
    if (appId) {
      return appAccessReadyFromPayload(payload, appId);
    }
    return payload?.accessReady === true || payload?.anyAppAccessReady === true;
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
      lastHeartbeat = payload && typeof payload === "object" ? payload : null;
      const nextVersion = String((payload && payload.buildVersion) || "").trim();
      remoteVersion = nextVersion;
      failureStreak = 0;

      const pageVersion = pageBuildVersion();
      if (pageVersion && nextVersion && pageVersion !== nextVersion) {
        setAlert("version");
        return;
      }
      if (!isHostServiceable(payload)) {
        if (alertKind === "build") {
          renderBanner("build", lastHeartbeat);
        } else {
          setAlert("build");
        }
        return;
      }
      if (
        !isShellSurface() &&
        !isAccessFullyReady(payload) &&
        isActivelyBuilding(payload)
      ) {
        if (alertKind === "build") {
          renderBanner("build", lastHeartbeat);
        } else {
          setAlert("build");
        }
        return;
      }
      if (isAccessFullyReady(payload) && alertKind === "build") {
        setAlert("success");
        return;
      }
      if (alertKind === "build") {
        setAlert("");
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
