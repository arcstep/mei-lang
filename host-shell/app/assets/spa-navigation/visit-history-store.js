(function (global) {
  const MAX_ENTRIES = 20;
  const STORAGE_PREFIX = "mei_visit_perf_v2:";
  const LEGACY_STORAGE_PREFIX = "mei_visit_perf_v1:";

  function readUsername() {
    const meta = document.querySelector('meta[name="mei-auth-user"]');
    const fromMeta = meta ? String(meta.getAttribute("content") || "").trim() : "";
    if (fromMeta) return fromMeta;
    const bodyUser = document.body?.dataset?.meiAuthUser;
    if (bodyUser) return String(bodyUser).trim();
    return "anonymous";
  }

  function resolveAppIdFromPathname(pathname) {
    const path = String(pathname || "");
    const prefixes = [
      "/apps/app/",
      "/apps/access/",
      "/apps/layout/",
      "/apps/prototype/",
    ];
    for (const prefix of prefixes) {
      if (!path.startsWith(prefix)) continue;
      let rest = path.slice(prefix.length);
      for (const marker of ["/scene/", "/tour/", "/presentation/"]) {
        const idx = rest.indexOf(marker);
        if (idx >= 0) {
          rest = rest.slice(0, idx);
        }
      }
      const slashQ = rest.indexOf("/?");
      if (slashQ >= 0) rest = rest.slice(0, slashQ);
      rest = rest.replace(/\/+$/, "");
      if (rest) return rest;
      break;
    }
    const parts = path.split("/").filter(Boolean);
    if (parts[0] !== "apps") return "";
    if (parts.length >= 3 && parts[2] === "view") {
      return parts[1] || "";
    }
    if (parts.length >= 2) {
      const reserved = new Set([
        "view",
        "layout",
        "prototype",
        "app",
        "access",
        "build",
        "manage",
        "upload",
        "config",
        "runtime",
      ]);
      if (!reserved.has(String(parts[1] || "").toLowerCase())) {
        return parts[1] || "";
      }
    }
    if (parts.length >= 3) {
      const surface = String(parts[2] || "").toLowerCase();
      if (
        surface === "layout" ||
        surface === "prototype" ||
        surface === "app" ||
        surface === "config" ||
        surface === "upload" ||
        surface === "runtime"
      ) {
        return parts[1] || "";
      }
    }
    return parts[1] || "";
  }

  function currentAppId(hint) {
    const hinted = String(hint || "").trim();
    if (hinted) return hinted;
    const root = document.querySelector("[data-app]");
    const fromDataset = root ? String(root.dataset.app || "").trim() : "";
    if (fromDataset) return fromDataset;
    let pathname = "";
    try {
      pathname =
        typeof global.location !== "undefined" ? String(global.location.pathname || "") : "";
    } catch (_) {}
    return resolveAppIdFromPathname(pathname) || "_global";
  }

  function storageKey(appIdHint) {
    const appId = currentAppId(appIdHint);
    return STORAGE_PREFIX + readUsername() + ":" + appId;
  }

  function readListForKey(key) {
    try {
      const raw = global.localStorage.getItem(key);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed : [];
    } catch (_) {
      return [];
    }
  }

  function migrateLegacyIfNeeded(appIdHint) {
    const username = readUsername();
    const legacyKey = LEGACY_STORAGE_PREFIX + username;
    let legacyItems;
    try {
      const raw = global.localStorage.getItem(legacyKey);
      if (!raw) return;
      legacyItems = JSON.parse(raw);
      if (!Array.isArray(legacyItems) || !legacyItems.length) {
        global.localStorage.removeItem(legacyKey);
        return;
      }
    } catch (_) {
      return;
    }
    const buckets = new Map();
    legacyItems.forEach((item) => {
      if (!item || typeof item !== "object") return;
      const appId = String(item.appId || "").trim() || "_global";
      if (!buckets.has(appId)) buckets.set(appId, []);
      buckets.get(appId).push(item);
    });
    buckets.forEach((items, appId) => {
      const key = STORAGE_PREFIX + username + ":" + appId;
      const existing = readListForKey(key);
      const merged = items.concat(existing).slice(0, MAX_ENTRIES);
      try {
        global.localStorage.setItem(key, JSON.stringify(merged));
      } catch (_) {
        /* ignore quota */
      }
    });
    try {
      global.localStorage.removeItem(legacyKey);
    } catch (_) {}
  }

  function list(appIdHint) {
    migrateLegacyIfNeeded(appIdHint);
    return readListForKey(storageKey(appIdHint));
  }

  function append(record) {
    if (!record || typeof record !== "object") return list();
    const enriched = enrichRecord(record);
    const appId = currentAppId(enriched.appId);
    enriched.appId = appId === "_global" ? enriched.appId || "" : appId;
    migrateLegacyIfNeeded(appId);
    const items = readListForKey(storageKey(appId));
    items.unshift(enriched);
    const trimmed = items.slice(0, MAX_ENTRIES);
    try {
      global.localStorage.setItem(storageKey(appId), JSON.stringify(trimmed));
    } catch (_) {
      /* ignore quota */
    }
    try {
      global.document?.dispatchEvent(
        new CustomEvent("mei:visit-history-updated", {
          detail: { record: enriched, items: trimmed, appId },
        }),
      );
    } catch (_) {}
    return trimmed;
  }

  function kindLabel(kind) {
    const map = {
      navigation: "导航",
      drilldown: "下钻",
      initial: "首屏",
    };
    return map[String(kind || "")] || String(kind || "访问");
  }

  function readMeta(name) {
    if (typeof document === "undefined") return "";
    const node = document.querySelector(`meta[name="${name}"]`);
    return node ? String(node.getAttribute("content") || "").trim() : "";
  }

  function collectVisitContext(urlHint) {
    const hrefBase =
      typeof global.location !== "undefined" ? global.location.href : "";
    let url;
    try {
      url = new URL(String(urlHint || hrefBase), hrefBase || "http://localhost");
    } catch (_) {
      url = new URL(hrefBase || "http://localhost");
    }
    const appPathItem = document.querySelector(".app-current-path-item");
    const appTitle = appPathItem ? String(appPathItem.textContent || "").trim() : "";
    return {
      workspace: readMeta("mei-workspace-label"),
      routeMode:
        document.body?.dataset?.surface ||
        document.body?.dataset?.meiView ||
        readMeta("mei-view") ||
        "",
      appId: resolveAppIdFromPathname(url.pathname),
      appTitle,
      pathname: url.pathname,
      href: url.href,
      scene: String(url.searchParams.get("scene") || "").trim(),
      file: String(url.searchParams.get("file") || "").trim(),
      authUser: readMeta("mei-auth-user"),
    };
  }

  function enrichRecord(record, extras) {
    const base = record && typeof record === "object" ? { ...record } : {};
    const opts = extras && typeof extras === "object" ? extras : {};
    const ctx = collectVisitContext(opts.url || base.href || base.path);
    const scene =
      String(opts.scene || base.scene || ctx.scene || "").trim() ||
      (base.kind === "drilldown"
        ? String(opts.scope || base.scope || base.path || base.label || "").trim()
        : "");
    const pathname =
      String(opts.pathname || base.pathname || ctx.pathname || "").trim() ||
      (base.kind === "drilldown" && opts.url
        ? (() => {
            try {
              return new URL(String(opts.url), ctx.href || window.location.href).pathname;
            } catch (_) {
              return "";
            }
          })()
        : "");
    return {
      ...base,
      workspace: base.workspace || ctx.workspace,
      routeMode: base.routeMode || ctx.routeMode,
      appId: base.appId || ctx.appId,
      appTitle: base.appTitle || ctx.appTitle,
      pathname: pathname || base.pathname || ctx.pathname,
      href: String(opts.url || base.href || ctx.href || "").trim() || base.href || ctx.href,
      scene,
      file: String(opts.file || base.file || ctx.file || "").trim(),
      authUser: base.authUser || ctx.authUser,
      apiCalls: Array.isArray(opts.apiCalls)
        ? opts.apiCalls.slice(0, 20)
        : Array.isArray(base.apiCalls)
          ? base.apiCalls
          : [],
      apiFailed: Number.isFinite(Number(opts.apiFailed))
        ? Number(opts.apiFailed)
        : Number(base.apiFailed) || 0,
      handlerReadyMs: Number.isFinite(Number(opts.handlerReadyMs))
        ? Number(opts.handlerReadyMs)
        : Number(base.handlerReadyMs) || 0,
      readyReason: String(opts.readyReason || base.readyReason || "").trim(),
    };
  }

  function formatRecordForAgent(item) {
    if (!item || typeof item !== "object") return "";
    const normalized =
      typeof boot.normalizeVisitPerfTotals === "function"
        ? boot.normalizeVisitPerfTotals(item)
        : item;
    const atIso = new Date(Number(item.at) || 0).toISOString();
    const lines = [
      `## 访问记录 ${item.id || ""}`.trim(),
      `- 时间: ${atIso}`,
      `- 类型: ${kindLabel(item.kind)}`,
      `- 结果: ${item.outcome || "—"}`,
      `- 工作区: ${item.workspace || "—"}`,
      `- 应用: ${item.appTitle || "—"} (${item.appId || "—"})`,
      `- 路由模式: ${item.routeMode || "—"}`,
      `- 访问路径: ${item.pathname || "—"}`,
      `- 完整 URL: ${item.href || item.path || "—"}`,
      `- 场景: ${item.scene || "—"}`,
      `- 文件: ${item.file || "—"}`,
      `- 标签: ${item.label || "—"}`,
      `- 性能: 渲染 ${normalized.renderMs}ms · 求值 ${normalized.evalMs}ms · 总计 ${normalized.totalMs}ms`,
      `- 后台 API: ${item.apiTotal || 0} 次${item.apiFailed ? `（失败 ${item.apiFailed}）` : ""}`,
      `- SSR 就绪: ${item.handlerReadyMs ? `${item.handlerReadyMs}ms` : "—"}`,
      `- 进度 UI: ${item.uiShown ? "已显示" : "未提示(<1s)"}`,
    ];
    if (item.readyReason) {
      lines.push(`- 就绪原因: ${item.readyReason}`);
    }
    if (Array.isArray(item.apiCalls) && item.apiCalls.length) {
      lines.push("- API 明细:");
      item.apiCalls.forEach((call, index) => {
        const kind = call?.kind || "api";
        const url = call?.url || "—";
        const status = call?.status != null ? String(call.status) : "—";
        const ms = Number.isFinite(Number(call?.ms)) ? `${call.ms}ms` : "—";
        const clientTag = call?.clientHit
          ? call?.url?.includes("/client-cache/metric_session")
            ? " · session_cache"
            : " · client_cache"
          : "";
        const serverCacheTag =
          Number(call?.responseCacheHit) === 1
            ? " · L1"
            : Number(call?.resultArtifactHit) === 1
              ? " · artifact"
              : "";
        const ok = call?.ok === false ? " FAIL" : "";
        lines.push(`  ${index + 1}. [${kind}] ${url} · HTTP ${status} · ${ms}${clientTag}${serverCacheTag}${ok}`);
      });
    }
    return lines.join("\n");
  }

  function formatAllForAgent(items, appIdHint) {
    const listItems = Array.isArray(items) ? items : list(appIdHint);
    const appId = currentAppId(appIdHint);
    const ctx = collectVisitContext();
    const appLabel = ctx.appTitle || appId;
    if (!listItems.length) {
      return `# MeiLang 访问历史 · ${appLabel}\n\n（暂无记录）`;
    }
    return (
      `# MeiLang 访问历史 · ${appLabel}（${listItems.length} 条）\n\n` +
      listItems.map((item) => formatRecordForAgent(item)).join("\n\n")
    );
  }

  const api = {
    MAX_ENTRIES,
    readUsername,
    currentAppId,
    resolveAppIdFromPathname,
    list,
    append,
    kindLabel,
    collectVisitContext,
    enrichRecord,
    formatRecordForAgent,
    formatAllForAgent,
  };

  global.MeiVisitHistoryStore = api;
  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  boot.appendVisitHistory = append;
  boot.listVisitHistory = list;
  boot.visitHistoryKindLabel = kindLabel;
  boot.enrichVisitHistoryRecord = enrichRecord;
  boot.formatVisitHistoryForAgent = formatRecordForAgent;
  boot.formatAllVisitHistoryForAgent = formatAllForAgent;
})(
  typeof window !== "undefined" ? window : typeof globalThis !== "undefined" ? globalThis : {},
);
