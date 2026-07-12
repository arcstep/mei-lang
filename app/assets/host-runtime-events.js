/**
 * Bounded host-control SSE client shared by Runtime and Access surfaces.
 * EventSource reconnects automatically; session-scoped dedupe prevents reload loops.
 */
(function (global) {
  "use strict";

  const EVENTS_API = "/api/host/events";
  const RELOAD_KEY_PREFIX = "mei:host-event-applied:v1:";

  function currentAppId() {
    const parsed = global.__mei?.view_revision_envelope?.app_id;
    if (parsed) return String(parsed);
    const match = global.location?.pathname?.match(/^\/apps\/([^/]+)/);
    return match ? decodeURIComponent(match[1]) : "";
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
  }

  function connect() {
    if (typeof global.EventSource !== "function") return null;
    const source = new global.EventSource(EVENTS_API);
    for (const type of [
      "job-phase",
      "profile-applied",
      "revision-published",
      "generation-activated",
      "generation-rolled-back",
    ]) {
      source.addEventListener(type, (event) => dispatch(type, event));
    }
    return source;
  }

  global.MeiHostRuntimeEvents = {
    appliesToCurrentApp,
    applyToken,
    claimApplyEvent,
    connect,
  };

  connect();
})(typeof window !== "undefined" ? window : globalThis);
