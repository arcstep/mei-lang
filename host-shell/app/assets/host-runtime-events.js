/**
 * Bounded host-control SSE client shared by Runtime and Access surfaces.
 * EventSource reconnects automatically; session-scoped dedupe prevents reload loops.
 *
 * Topbar truth is always Host LaunchManifest (`/api/host/shell-chrome`).
 * Runtime shell.app may only know the current app — never trust it as the multi-app menu.
 */
(function (global) {
  "use strict";

  const EVENTS_API = "/api/host/events";
  const SHELL_CHROME_API = "/api/host/shell-chrome";
  const RELOAD_KEY_PREFIX = "mei:host-event-applied:v1:";
  let lastChromeDigest = "";
  let chromeRefreshInFlight = null;

  function currentAppId() {
    const parsed = global.__mei?.view_revision_envelope?.app_id;
    if (parsed) return String(parsed);
    const match = global.location?.pathname?.match(/^\/apps\/([^/]+)/);
    return match ? decodeURIComponent(match[1]) : "";
  }

  function shellNavFromLocation() {
    const path = String(global.location?.pathname || "");
    if (path === "/runtime" || path.startsWith("/runtime/")) return "runtime";
    if (path === "/home" || path === "/") return "home";
    if (path.startsWith("/config")) return "config";
    if (path.startsWith("/upload")) return "upload";
    if (path.startsWith("/mcg")) return "mcg";
    return "";
  }

  function eventPayload(event) {
    try {
      const envelope = JSON.parse(event.data || "{}");
      return envelope && typeof envelope.payload === "object" ? envelope.payload : {};
    } catch (_error) {
      return {};
    }
  }

  function appliesToCurrentApp(payload) {
    const appId = currentAppId();
    if (!appId) return false;
    if (payload.appId) return payload.appId === appId;
    return Array.isArray(payload.apps) && payload.apps.includes(appId);
  }

  function applyToken(payload) {
    const appId = currentAppId();
    if (payload.profileId || payload.profileRevision) {
      return [appId, payload.profileId || "", payload.profileRevision || ""].join(":");
    }
    return [appId, payload.revision || "", payload.envVersion || ""].join(":");
  }

  function claimApplyEvent(payload) {
    const token = applyToken(payload);
    if (!token || /^:*$/u.test(token)) return false;
    const key = `${RELOAD_KEY_PREFIX}${token}`;
    try {
      if (global.sessionStorage.getItem(key) === "1") return false;
      global.sessionStorage.setItem(key, "1");
    } catch (_error) {
      // Storage can be unavailable in privacy modes. The in-memory token still
      // protects this page lifetime.
      if (claimApplyEvent.memory === token) return false;
      claimApplyEvent.memory = token;
    }
    return true;
  }

  async function refreshAccess(payload) {
    if (!appliesToCurrentApp(payload) || !claimApplyEvent(payload)) return;
    if (payload.runtimePlan && typeof payload.runtimePlan === "object") {
      global.__mei = global.__mei || {};
      global.__mei.dev_eval = {
        ...(global.__mei.dev_eval || {}),
        runtimePlan: payload.runtimePlan,
        appId: currentAppId(),
      };
    }
    const boot = global.__meiLangBoot;
    if (typeof boot?.tryCacheFirstViewRestore === "function") {
      try {
        const result = await boot.tryCacheFirstViewRestore(global.location.href, {
          forceRematerialize: true,
          skipRemoteWhenValid: false,
        });
        if (result?.restored) return;
      } catch (error) {
        console.warn("[host-events] view revision reassembly failed", error);
      }
    }
    global.setTimeout(() => global.location.reload(), 120);
  }

  function chromeQueryFromLocation() {
    const params = new URLSearchParams(global.location?.search || "");
    const pathApp = currentAppId();
    const scene =
      params.get("scene") ||
      global.document?.body?.getAttribute?.("data-scene-id") ||
      "home";
    const surface = params.get("surface") || "app";
    const chrome = params.get("chrome") || "";
    const shellNav = shellNavFromLocation();
    const query = new URLSearchParams();
    if (shellNav) {
      query.set("shellNav", shellNav);
    } else {
      if (pathApp) query.set("appId", pathApp);
      if (scene) query.set("scene", scene);
      if (surface) query.set("surface", surface);
      if (chrome) query.set("chrome", chrome);
    }
    return query.toString();
  }

  async function refreshTopbarChrome(payload) {
    const topSlot = global.document?.getElementById?.("mei-host-topbar-slot");
    if (!topSlot) {
      // Pages without chrome slot still benefit from a light reload signal.
      if (payload?.appId || payload?.instanceId || payload?.force) {
        global.dispatchEvent(
          new CustomEvent("mei:host-apps-changed", { detail: { payload } }),
        );
      }
      return;
    }
    const force = Boolean(payload?.force) || Boolean(payload?.appId) || Boolean(payload?.instanceId);
    if (chromeRefreshInFlight) {
      await chromeRefreshInFlight;
      if (!force) return;
    }
    chromeRefreshInFlight = (async () => {
      try {
        const qs = chromeQueryFromLocation();
        const response = await global.fetch(
          `${SHELL_CHROME_API}${qs ? `?${qs}` : ""}`,
          { credentials: "same-origin", headers: { Accept: "application/json" } },
        );
        if (!response.ok) {
          console.warn("[host-events] shell-chrome refresh failed", response.status);
          return;
        }
        const data = await response.json();
        const digest = String(data?.digest || "");
        if (!force && digest && digest === lastChromeDigest) return;
        lastChromeDigest = digest;
        if (typeof data?.topbarHtml === "string") {
          topSlot.innerHTML = data.topbarHtml;
        }
        const bottomSlot = global.document?.getElementById?.("mei-host-statusbar-slot");
        if (bottomSlot && typeof data?.statusbarHtml === "string") {
          bottomSlot.innerHTML = data.statusbarHtml;
        }
        const boot = global.__meiLangBoot;
        if (typeof boot?.watchTopbarChromeInjection === "function") {
          boot.watchTopbarChromeInjection();
        }
        if (typeof boot?.fixTopbarHrefFromLocation === "function") {
          boot.fixTopbarHrefFromLocation();
        }
        global.dispatchEvent(
          new CustomEvent("mei:host-chrome-refreshed", {
            detail: { digest, runningAppIds: data?.runningAppIds || [], payload },
          }),
        );
        global.dispatchEvent(
          new CustomEvent("mei:host-apps-changed", {
            detail: { payload, runningAppIds: data?.runningAppIds || [] },
          }),
        );
      } catch (error) {
        console.warn("[host-events] shell-chrome refresh error", error);
      }
    })();
    try {
      await chromeRefreshInFlight;
    } finally {
      chromeRefreshInFlight = null;
    }
  }

  function dispatch(eventType, event) {
    const payload = eventPayload(event);
    global.dispatchEvent(
      new CustomEvent("mei:host-event", {
        detail: { type: eventType, payload },
      }),
    );
    if (eventType === "profile-applied" || eventType === "revision-published") {
      void refreshAccess(payload);
    }
    if (
      eventType === "app-started" ||
      eventType === "app-stopped" ||
      eventType === "app-config-switched" ||
      eventType === "app-starting"
    ) {
      void refreshTopbarChrome({ ...payload, force: true });
    }
  }

  function connect() {
    if (typeof global.EventSource !== "function") return null;
    const source = new global.EventSource(EVENTS_API);
    for (const type of [
      "job-phase",
      "builder-phase",
      "profile-applied",
      "revision-published",
      "generation-activated",
      "generation-rolled-back",
      "instance-phase",
      "instance-ready",
      "instance-failed",
      "route-cutover",
      "route-rollback",
      "app-started",
      "app-stopped",
      "app-config-switched",
      "app-starting",
      "app-failed",
    ]) {
      source.addEventListener(type, (event) => dispatch(type, event));
    }
    // Sync immediately, then again after Access compose may overwrite the slot from Runtime shell.app.
    void refreshTopbarChrome({ force: true });
    global.setTimeout(() => void refreshTopbarChrome({ force: true }), 600);
    return source;
  }

  global.document?.addEventListener?.("mei:shell-layer-applied", () => {
    void refreshTopbarChrome({ force: true });
  });

  global.MeiHostRuntimeEvents = {
    appliesToCurrentApp,
    applyToken,
    claimApplyEvent,
    refreshTopbarChrome,
    shellNavFromLocation,
    connect,
  };

  connect();
})(typeof window !== "undefined" ? window : globalThis);
