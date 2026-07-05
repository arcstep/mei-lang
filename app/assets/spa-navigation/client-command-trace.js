/**
 * Tag user-initiated fetches so host-shell terminal logs can group them under ▶ 路由 / 开发导航 / …
 * (server-side only — no browser console output)
 */
(function initClientCommandTrace(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const TRACE_API = "/api/host/client-trace";
  const CMD_TTL_MS = 45_000;
  const HEADER_ID = "x-mei-client-cmd-id";
  const HEADER_KIND = "x-mei-client-cmd-kind";
  const HEADER_LABEL = "x-mei-client-cmd-label";

  let seq = 0;
  let activeCommand = null;
  let fetchHookInstalled = false;

  function normalizeKind(kind) {
    return String(kind || "CMD")
      .trim()
      .toUpperCase()
      .replace(/[^A-Z0-9_]+/g, "_")
      .slice(0, 32);
  }

  function nextCommandId() {
    seq += 1;
    return `cmd-${String(seq).padStart(4, "0")}`;
  }

  function pruneActiveCommand() {
    if (!activeCommand) return null;
    if (Date.now() - activeCommand.startedAt > CMD_TTL_MS) {
      activeCommand = null;
    }
    return activeCommand;
  }

  function annotateFetchInit(init) {
    const cmd = pruneActiveCommand();
    if (!cmd) return init;
    const base = init && typeof init === "object" ? { ...init } : {};
    const headers = new Headers(base.headers || undefined);
    headers.set(HEADER_ID, cmd.id);
    headers.set(HEADER_KIND, cmd.kind);
    if (cmd.label) headers.set(HEADER_LABEL, cmd.label.slice(0, 180));
    base.headers = headers;
    return base;
  }

  function beaconCommand(cmd) {
    if (!cmd?.id) return;
    try {
      const body = JSON.stringify({
        id: cmd.id,
        kind: cmd.kind,
        label: cmd.label || "",
      });
      if (typeof navigator !== "undefined" && typeof navigator.sendBeacon === "function") {
        const blob = new Blob([body], { type: "application/json" });
        if (navigator.sendBeacon(TRACE_API, blob)) return;
      }
      void fetch(TRACE_API, {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body,
        keepalive: true,
      });
    } catch (_) {}
  }

  function beginClientCommand(options) {
    const opts = options && typeof options === "object" ? options : {};
    const kind = normalizeKind(opts.kind || "CMD");
    const label = String(opts.label || "").trim();
    const cmd = {
      id: String(opts.id || nextCommandId()),
      kind,
      label,
      startedAt: Date.now(),
    };
    activeCommand = cmd;
    beaconCommand(cmd);
    return cmd.id;
  }

  function endClientCommand(id) {
    const cmd = activeCommand;
    if (!cmd || (id && cmd.id !== id)) return;
    activeCommand = null;
  }

  function installClientCommandFetchHook() {
    if (fetchHookInstalled || typeof global.fetch !== "function") return;
    fetchHookInstalled = true;
    const nativeFetch = global.fetch.bind(global);
    global.fetch = function meiClientCommandFetch(input, init) {
      const nextInit = annotateFetchInit(init);
      if (input instanceof Request) {
        const cmd = pruneActiveCommand();
        if (cmd) {
          const headers = new Headers(input.headers);
          headers.set(HEADER_ID, cmd.id);
          headers.set(HEADER_KIND, cmd.kind);
          if (cmd.label) headers.set(HEADER_LABEL, cmd.label.slice(0, 180));
          input = new Request(input, { headers });
        }
        return nativeFetch(input, nextInit);
      }
      return nativeFetch(input, nextInit);
    };
  }

  function wrapBeginDrilldownLoadSession() {
    if (typeof boot.beginDrilldownLoadSession !== "function") return;
    if (wrapBeginDrilldownLoadSession._wrapped) return;
    wrapBeginDrilldownLoadSession._wrapped = true;
    const original = boot.beginDrilldownLoadSession.bind(boot);
    boot.beginDrilldownLoadSession = function wrappedBeginDrilldownLoadSession(options) {
      const opts = options && typeof options === "object" ? options : {};
      const boardLike = /board|看板/i.test(String(opts.label || opts.path || ""));
      beginClientCommand({
        kind: boardLike ? "BOARD" : "DRILLDOWN",
        label: String(opts.label || opts.path || "下钻"),
      });
      return original(options);
    };
  }

  installClientCommandFetchHook();

  function wrapBeginLoadingProgressSession() {
    if (typeof boot.beginLoadingProgressSession !== "function") return;
    if (wrapBeginLoadingProgressSession._wrapped) return;
    wrapBeginLoadingProgressSession._wrapped = true;
    const original = boot.beginLoadingProgressSession.bind(boot);
    boot.beginLoadingProgressSession = function wrappedBeginLoadingProgressSession(
      navigationId,
      url,
    ) {
      if (navigationId !== boot.INITIAL_LOAD_NAVIGATION_ID && typeof boot.beginClientCommand === "function") {
        let kind = "ROUTE";
        try {
          const path = new URL(String(url || global.location.href), global.location.href).pathname;
          if (path.startsWith("/apps/build/") || path.startsWith("/apps/manage/")) {
            kind = "BUILD_NAV";
          }
        } catch (_) {}
        boot.beginClientCommand({ kind, label: String(url || global.location.href) });
      }
      return original(navigationId, url);
    };
  }

  function installClientCommandWrappers() {
    wrapBeginLoadingProgressSession();
    wrapBeginDrilldownLoadSession();
  }

  boot.beginClientCommand = beginClientCommand;
  boot.endClientCommand = endClientCommand;
  boot.annotateClientFetchInit = annotateFetchInit;
  boot.installClientCommandWrappers = installClientCommandWrappers;

  global.__meiClientCommand = {
    begin: beginClientCommand,
    end: endClientCommand,
    active: () => pruneActiveCommand(),
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", installClientCommandWrappers, { once: true });
  } else {
    installClientCommandWrappers();
  }
})(
  typeof window !== "undefined" ? window : typeof globalThis !== "undefined" ? globalThis : {},
);
