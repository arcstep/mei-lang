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
  const COORDINATION_CHANNEL = "mei:host-events:v2";
  const LEADER_LOCK = "mei:host-events-leader:v2";
  const LEADER_LEASE_KEY = "mei:host-events-leader-lease:v2";
  const LEASE_TTL_MS = 12_000;
  const LEASE_HEARTBEAT_MS = 4_000;
  const ELECTION_RETRY_MS = 2_000;
  const tabId =
    global.crypto?.randomUUID?.() ||
    `tab-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  let lastChromeDigest = "";
  let chromeRefreshInFlight = null;
  let eventSource = null;
  let coordinationChannel = null;
  let leader = false;
  let leaderKind = "";
  let releaseWebLock = null;
  let webLockRequestInFlight = false;
  let leaseHeartbeatTimer = 0;
  let electionRetryTimer = 0;
  let messageCounter = 0;
  let dirtyWhileHidden = true;
  let coordinatorStopped = false;
  const seenMessages = new Set();

  function currentAppId() {
    const parsed = global.__mei?.view_revision_envelope?.app_id;
    if (parsed) return String(parsed).trim();
    const path = String(global.location?.pathname || "");
    let match = path.match(/^\/apps\/([^/]+)/);
    if (match) return decodeURIComponent(match[1]);
    match = path.match(/^\/admin\/apps\/([^/]+)/);
    if (match) return decodeURIComponent(match[1]);
    const fromDom =
      global.document?.body?.getAttribute?.("data-app-id") ||
      global.document?.getElementById?.("mei-view-host")?.getAttribute?.("data-app-id") ||
      global.document?.querySelector?.("[data-app-id]")?.getAttribute?.("data-app-id") ||
      "";
    return String(fromDom || "").trim();
  }

  function shellNavFromLocation() {
    const path = String(global.location?.pathname || "");
    if (path === "/runtime" || path.startsWith("/runtime/") || path.startsWith("/mcg")) {
      return "runtime";
    }
    if (path === "/home" || path === "/") return "home";
    if (path === "/share" || path.startsWith("/share/")) return "share";
    return "";
  }

  function surfaceFromLocation() {
    const params = new URLSearchParams(global.location?.search || "");
    const fromQuery = String(params.get("surface") || "")
      .trim()
      .toLowerCase();
    if (fromQuery) return fromQuery;
    const path = String(global.location?.pathname || "");
    if (path.startsWith("/admin/apps/")) return "admin";
    const composeRoot = global.document?.getElementById?.("mei-compose-root");
    const fromCompose = String(
      composeRoot?.getAttribute?.("data-mei-compose-root") ||
        composeRoot?.getAttribute?.("data-route-mode") ||
        "",
    )
      .trim()
      .toLowerCase();
    if (fromCompose) return fromCompose;
    return "app";
  }

  function adminIdFromLocation() {
    const params = new URLSearchParams(global.location?.search || "");
    const fromQuery = String(params.get("adminId") || params.get("admin_id") || "").trim();
    if (fromQuery) return fromQuery;
    const path = String(global.location?.pathname || "");
    const match = path.match(/^\/admin\/apps\/[^/]+\/([^/]+)\/([^/]+)\/?$/);
    if (match) {
      return `${decodeURIComponent(match[1])}.${decodeURIComponent(match[2])}`;
    }
    const host = global.document?.getElementById?.("mei-view-host");
    const resource = String(host?.getAttribute?.("data-resource-id") || "").trim();
    const module = String(host?.getAttribute?.("data-module-id") || "").trim();
    if (resource && module) return `${resource}.${module}`;
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
      global.document?.getElementById?.("mei-view-host")?.getAttribute?.("data-scene-id") ||
      "home";
    const surface = surfaceFromLocation();
    const chrome = params.get("chrome") || "";
    const adminId = adminIdFromLocation();
    const shellNav = shellNavFromLocation();
    const query = new URLSearchParams();
    if (shellNav) {
      query.set("shellNav", shellNav);
    } else {
      if (pathApp) query.set("appId", pathApp);
      if (scene) query.set("scene", scene);
      if (surface) query.set("surface", surface);
      if (chrome) query.set("chrome", chrome);
      if (adminId) query.set("adminId", adminId);
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
        if (typeof boot?.refreshStatusBarChips === "function") {
          boot.refreshStatusBarChips();
        }
        if (typeof boot?.refreshVisitHistoryPanel === "function") {
          boot.refreshVisitHistoryPanel();
        }
        const chromeDetail = {
          digest,
          runningAppIds: data?.runningAppIds || [],
          payload,
        };
        try {
          global.document?.dispatchEvent?.(
            new CustomEvent("mei:host-chrome-refreshed", { detail: chromeDetail }),
          );
        } catch (_error) {
          // ignore
        }
        global.dispatchEvent(
          new CustomEvent("mei:host-chrome-refreshed", { detail: chromeDetail }),
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

  function dispatchPayload(eventType, payload) {
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

  function dispatch(eventType, event) {
    dispatchPayload(eventType, eventPayload(event));
  }

  function isVisible() {
    return global.document?.visibilityState !== "hidden";
  }

  function isFocused() {
    return typeof global.document?.hasFocus !== "function" || global.document.hasFocus();
  }

  function nextMessageId() {
    messageCounter += 1;
    return `${tabId}:${Date.now().toString(36)}:${messageCounter.toString(36)}`;
  }

  function rememberMessage(messageId) {
    if (!messageId || seenMessages.has(messageId)) return false;
    seenMessages.add(messageId);
    if (seenMessages.size > 256) {
      const oldest = seenMessages.values().next().value;
      if (oldest) seenMessages.delete(oldest);
    }
    return true;
  }

  function relayEvent(eventType, payload) {
    const message = {
      kind: "host-event",
      sender: tabId,
      messageId: nextMessageId(),
      type: eventType,
      payload,
    };
    rememberMessage(message.messageId);
    coordinationChannel?.postMessage?.(message);
  }

  function handleRelayedEvent(message) {
    if (!rememberMessage(String(message?.messageId || ""))) return;
    if (!isVisible()) {
      dirtyWhileHidden = true;
      return;
    }
    dispatchPayload(String(message.type || ""), message.payload || {});
  }

  function dispatchResync(reason) {
    dirtyWhileHidden = false;
    void refreshTopbarChrome({ force: true, reason });
    global.dispatchEvent(
      new CustomEvent("mei:host-event", {
        detail: { type: "host-resync", payload: { reason } },
      }),
    );
  }

  function closeEventStream() {
    if (!eventSource) return;
    try {
      eventSource.close();
    } catch (_error) {
      // Closing is best-effort during page teardown.
    }
    eventSource = null;
  }

  function openEventStream() {
    if (!leader || !isVisible() || typeof global.EventSource !== "function") return null;
    if (eventSource) return eventSource;
    const query = new URLSearchParams({
      clientId: tabId,
      leader: leaderKind || "unknown",
    });
    const source = new global.EventSource(`${EVENTS_API}?${query.toString()}`);
    eventSource = source;
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
      source.addEventListener(type, (event) => {
        const payload = eventPayload(event);
        dispatchPayload(type, payload);
        relayEvent(type, payload);
      });
    }
    return source;
  }

  function clearLeaseHeartbeat() {
    if (!leaseHeartbeatTimer) return;
    global.clearInterval?.(leaseHeartbeatTimer);
    leaseHeartbeatTimer = 0;
  }

  function clearElectionRetry() {
    if (!electionRetryTimer) return;
    global.clearTimeout?.(electionRetryTimer);
    electionRetryTimer = 0;
  }

  function broadcastLeadership(kind, reason) {
    coordinationChannel?.postMessage?.({
      kind,
      sender: tabId,
      leaderKind,
      reason,
    });
  }

  function becomeLeader(kind) {
    if (coordinatorStopped || !isVisible()) return false;
    if (leader && leaderKind === kind) {
      openEventStream();
      return true;
    }
    closeEventStream();
    leader = true;
    leaderKind = kind;
    clearElectionRetry();
    broadcastLeadership("leader-acquired", kind);
    dispatchResync(`leader-acquired:${kind}`);
    openEventStream();
    return true;
  }

  function readLease() {
    try {
      const raw = global.localStorage?.getItem?.(LEADER_LEASE_KEY);
      if (!raw) return null;
      const value = JSON.parse(raw);
      if (!value || typeof value.holder !== "string") return null;
      return value;
    } catch (_error) {
      return null;
    }
  }

  function writeLease() {
    try {
      global.localStorage?.setItem?.(
        LEADER_LEASE_KEY,
        JSON.stringify({ holder: tabId, expiresAt: Date.now() + LEASE_TTL_MS }),
      );
      return readLease()?.holder === tabId;
    } catch (_error) {
      return false;
    }
  }

  function leaseStorageAvailable() {
    try {
      return Boolean(global.localStorage?.getItem && global.localStorage?.setItem);
    } catch (_error) {
      return false;
    }
  }

  function releaseLease() {
    clearLeaseHeartbeat();
    try {
      if (readLease()?.holder === tabId) {
        global.localStorage?.removeItem?.(LEADER_LEASE_KEY);
      }
    } catch (_error) {
      // Storage can disappear in privacy modes.
    }
  }

  function releaseLeadership(reason) {
    const wasLeader = leader;
    const previousKind = leaderKind;
    leader = false;
    leaderKind = "";
    closeEventStream();
    if (previousKind === "lease") releaseLease();
    if (releaseWebLock) {
      const release = releaseWebLock;
      releaseWebLock = null;
      release();
    }
    if (wasLeader) broadcastLeadership("leader-released", reason);
  }

  function scheduleElectionRetry(delay = ELECTION_RETRY_MS) {
    if (coordinatorStopped || !isVisible()) return;
    if (electionRetryTimer) {
      if (delay > 0) return;
      clearElectionRetry();
    }
    electionRetryTimer = global.setTimeout?.(() => {
      electionRetryTimer = 0;
      tryElection();
    }, delay);
  }

  function tryLeaseLeadership() {
    const current = readLease();
    if (current && current.holder !== tabId && Number(current.expiresAt || 0) > Date.now()) {
      scheduleElectionRetry();
      return false;
    }
    if (!writeLease()) {
      scheduleElectionRetry();
      return false;
    }
    becomeLeader("lease");
    clearLeaseHeartbeat();
    leaseHeartbeatTimer =
      global.setInterval?.(() => {
        if (!leader || leaderKind !== "lease" || !isVisible()) {
          releaseLeadership("lease-ineligible");
          scheduleElectionRetry();
          return;
        }
        if (readLease()?.holder !== tabId || !writeLease()) {
          releaseLeadership("lease-lost");
          scheduleElectionRetry(0);
        }
      }, LEASE_HEARTBEAT_MS) || 0;
    return true;
  }

  function requestWebLock() {
    const locks = global.navigator?.locks;
    if (typeof locks?.request !== "function") return false;
    if (webLockRequestInFlight || (leader && leaderKind === "web-lock")) return true;
    webLockRequestInFlight = true;
    Promise.resolve(
      locks.request(LEADER_LOCK, { mode: "exclusive", ifAvailable: true }, async (lock) => {
        webLockRequestInFlight = false;
        if (!lock || coordinatorStopped || !isVisible()) {
          scheduleElectionRetry();
          return;
        }
        await new Promise((resolve) => {
          releaseWebLock = resolve;
          becomeLeader("web-lock");
        });
        releaseWebLock = null;
        if (leaderKind === "web-lock") {
          releaseLeadership("web-lock-released");
        }
      }),
    ).catch((error) => {
      webLockRequestInFlight = false;
      console.warn("[host-events] leader lock failed; using lease fallback", error);
      if (leaseStorageAvailable()) tryLeaseLeadership();
      else scheduleElectionRetry();
    });
    return true;
  }

  function tryElection() {
    if (coordinatorStopped) return false;
    if (!isVisible()) {
      releaseLeadership("hidden");
      return false;
    }
    if (leader) {
      openEventStream();
      return true;
    }
    if (requestWebLock()) return true;
    if (leaseStorageAvailable()) return tryLeaseLeadership();
    if (isFocused()) return becomeLeader("focused-fallback");
    scheduleElectionRetry();
    return false;
  }

  function handleCoordinationMessage(event) {
    const message = event?.data || event;
    if (!message || message.sender === tabId) return;
    if (message.kind === "host-event") {
      handleRelayedEvent(message);
      return;
    }
    if (message.kind === "leader-released") {
      scheduleElectionRetry(0);
      return;
    }
    if (
      message.kind === "leader-acquired" &&
      leaderKind === "focused-fallback" &&
      String(message.sender) < tabId
    ) {
      releaseLeadership("fallback-tie-break");
      scheduleElectionRetry();
    }
  }

  function setupCoordinationChannel() {
    if (coordinationChannel || typeof global.BroadcastChannel !== "function") return;
    try {
      coordinationChannel = new global.BroadcastChannel(COORDINATION_CHANNEL);
      if (typeof coordinationChannel.addEventListener === "function") {
        coordinationChannel.addEventListener("message", handleCoordinationMessage);
      } else {
        coordinationChannel.onmessage = handleCoordinationMessage;
      }
    } catch (error) {
      console.warn("[host-events] cross-tab channel unavailable", error);
      coordinationChannel = null;
    }
  }

  function onVisibilityChange() {
    if (!isVisible()) {
      dirtyWhileHidden = true;
      releaseLeadership("hidden");
      clearElectionRetry();
      return;
    }
    if (dirtyWhileHidden) dispatchResync("visible");
    tryElection();
  }

  function onStorage(event) {
    if (event?.key !== LEADER_LEASE_KEY) return;
    if (leaderKind === "lease" && readLease()?.holder !== tabId) {
      releaseLeadership("lease-replaced");
    }
    scheduleElectionRetry(0);
  }

  function connect() {
    coordinatorStopped = false;
    setupCoordinationChannel();
    tryElection();
    return eventSource;
  }

  function disconnect(reason = "manual") {
    releaseLeadership(reason);
    clearElectionRetry();
  }

  function startCoordinator() {
    coordinatorStopped = false;
    setupCoordinationChannel();
    dispatchResync("startup");
    global.setTimeout?.(() => void refreshTopbarChrome({ force: true }), 600);
    tryElection();
  }

  global.document?.addEventListener?.("visibilitychange", onVisibilityChange);
  global.document?.addEventListener?.("mei:shell-layer-applied", () => {
    void refreshTopbarChrome({ force: true });
  });
  global.addEventListener?.("focus", () => {
    if (dirtyWhileHidden) dispatchResync("focus");
    tryElection();
  });
  global.addEventListener?.("blur", () => {
    if (leaderKind === "focused-fallback") releaseLeadership("blur");
  });
  global.addEventListener?.("storage", onStorage);
  global.addEventListener?.("pagehide", (event) => {
    dirtyWhileHidden = true;
    releaseLeadership("pagehide");
    if (!event?.persisted) {
      coordinatorStopped = true;
      coordinationChannel?.close?.();
      coordinationChannel = null;
    }
  });
  global.addEventListener?.("pageshow", () => {
    startCoordinator();
  });
  global.addEventListener?.("beforeunload", () => {
    coordinatorStopped = true;
    disconnect("beforeunload");
  });

  global.MeiHostRuntimeEvents = {
    appliesToCurrentApp,
    applyToken,
    claimApplyEvent,
    refreshTopbarChrome,
    shellNavFromLocation,
    connect,
    disconnect,
    dispatchPayload,
    handleCoordinationMessage,
    isLeader: () => leader,
    diagnostics: () => ({
      tabId,
      leader,
      leaderKind,
      eventSourceOpen: Boolean(eventSource),
      visible: isVisible(),
      dirtyWhileHidden,
    }),
  };

  startCoordinator();
})(typeof window !== "undefined" ? window : globalThis);
