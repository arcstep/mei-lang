/**
 * 宿主登录态滑动续期：在 JWT 到期前静默调用 POST /api/auth/refresh。
 * 挂载 `window.MeiHostAuthSession`，在所有 --auth 已登录页面 bundle 中紧随 host-http-feedback.js 加载。
 */
(function () {
  if (window.MeiHostAuthSession) return;

  const DEFAULT_REFRESH_LEAD_MS = 30 * 60 * 1000;
  const MIN_REFRESH_DELAY_MS = 5000;

  let refreshTimerId = 0;
  let refreshInFlight = false;
  let lastSessionPayload = null;

  function readMeta(name) {
    const node = document.querySelector('meta[name="' + name + '"]');
    return node ? String(node.getAttribute("content") || "").trim() : "";
  }

  function readHostCapabilities() {
    const raw =
      document.body?.dataset?.meiAuthCapabilities ||
      readMeta("mei-auth-capabilities") ||
      "{}";
    try {
      const parsed = JSON.parse(raw);
      return parsed && typeof parsed === "object" ? parsed : {};
    } catch (_) {
      return {};
    }
  }

  function isAuthDisabledProfile(caps) {
    return Boolean(
      caps &&
        caps.access_view &&
        caps.config_upload &&
        caps.build_view &&
        caps.access_agent &&
        caps.agent_control &&
        caps.authoring_agent,
    );
  }

  function isLoggedIn() {
    const fromBody = String(document.body?.dataset?.meiAuthLoggedIn || "").trim();
    if (fromBody) return fromBody === "1";
    return readMeta("mei-auth-logged-in") === "1";
  }

  function shouldStart() {
    if (!isLoggedIn()) return false;
    return !isAuthDisabledProfile(readHostCapabilities());
  }

  function clearRefreshTimer() {
    if (refreshTimerId) {
      clearTimeout(refreshTimerId);
      refreshTimerId = 0;
    }
  }

  function refreshLeadMs(payload) {
    const fromPayload = Number(payload && payload.refreshLeadSeconds);
    if (Number.isFinite(fromPayload) && fromPayload > 0) {
      return fromPayload * 1000;
    }
    return DEFAULT_REFRESH_LEAD_MS;
  }

  function dispatchSessionRefreshed(payload) {
    try {
      document.dispatchEvent(
        new CustomEvent("mei:auth-session-refreshed", {
          detail: payload && typeof payload === "object" ? payload : {},
        }),
      );
    } catch (_) {}
  }

  function scheduleRefreshFromPayload(payload) {
    clearRefreshTimer();
    if (!payload || !payload.authenticated || !payload.expiresAt) {
      return;
    }
    const expMs = Number(payload.expiresAt) * 1000;
    if (!Number.isFinite(expMs) || expMs <= 0) return;
    const leadMs = refreshLeadMs(payload);
    const delayMs = expMs - Date.now() - leadMs;
    if (delayMs <= 0) {
      refreshSession("lead_elapsed").catch(function () {});
      return;
    }
    refreshTimerId = setTimeout(function () {
      refreshTimerId = 0;
      refreshSession("scheduled").catch(function () {});
    }, Math.max(MIN_REFRESH_DELAY_MS, delayMs));
  }

  async function fetchSession() {
    const response = await fetch("/api/auth/session", { credentials: "same-origin" });
    if (!response.ok) {
      throw new Error("session check failed: " + response.status);
    }
    return response.json();
  }

  async function refreshSession(_reason) {
    if (refreshInFlight) return lastSessionPayload;
    refreshInFlight = true;
    try {
      const response = await fetch("/api/auth/refresh", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
      });
      if (!response.ok) {
        throw new Error("session refresh failed: " + response.status);
      }
      const payload = await response.json();
      const sessionPayload = {
        enabled: true,
        authenticated: true,
        expiresAt: payload.expiresAt,
        jwtTtlSeconds: payload.jwtTtlSeconds,
        refreshLeadSeconds: payload.refreshLeadSeconds,
        user: payload.user || null,
      };
      lastSessionPayload = sessionPayload;
      scheduleRefreshFromPayload(sessionPayload);
      dispatchSessionRefreshed(sessionPayload);
      return sessionPayload;
    } finally {
      refreshInFlight = false;
    }
  }

  async function bootstrap() {
    if (!shouldStart()) return;
    try {
      const payload = await fetchSession();
      lastSessionPayload = payload;
      if (!(payload && payload.authenticated)) return;
      scheduleRefreshFromPayload(payload);
    } catch (_) {}
  }

  function onVisibilityChange() {
    if (document.visibilityState !== "visible") return;
    if (!shouldStart()) return;
    const payload = lastSessionPayload;
    if (!payload || !payload.authenticated || !payload.expiresAt) {
      bootstrap().catch(function () {});
      return;
    }
    const leadMs = refreshLeadMs(payload);
    const refreshAtMs = Number(payload.expiresAt) * 1000 - leadMs;
    if (Date.now() >= refreshAtMs) {
      refreshSession("visibility").catch(function () {});
    }
  }

  document.addEventListener("visibilitychange", onVisibilityChange);

  window.MeiHostAuthSession = {
    bootstrap: bootstrap,
    refreshSession: refreshSession,
    scheduleRefreshFromPayload: scheduleRefreshFromPayload,
    getLastSessionPayload: function () {
      return lastSessionPayload;
    },
    dispose: function () {
      clearRefreshTimer();
      document.removeEventListener("visibilitychange", onVisibilityChange);
    },
  };

  bootstrap().catch(function () {});
})();
