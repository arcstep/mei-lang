(function (global) {
  const MAX_ENTRIES = 20;
  const STORAGE_PREFIX = "mei_visit_perf_v1:";

  function readUsername() {
    const meta = document.querySelector('meta[name="mei-auth-user"]');
    const fromMeta = meta ? String(meta.getAttribute("content") || "").trim() : "";
    if (fromMeta) return fromMeta;
    const bodyUser = document.body?.dataset?.meiAuthUser;
    if (bodyUser) return String(bodyUser).trim();
    return "anonymous";
  }

  function storageKey() {
    return STORAGE_PREFIX + readUsername();
  }

  function list() {
    try {
      const raw = global.localStorage.getItem(storageKey());
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed : [];
    } catch (_) {
      return [];
    }
  }

  function append(record) {
    if (!record || typeof record !== "object") return list();
    const enriched = enrichRecord(record);
    const items = list();
    items.unshift(enriched);
    const trimmed = items.slice(0, MAX_ENTRIES);
    try {
      global.localStorage.setItem(storageKey(), JSON.stringify(trimmed));
    } catch (_) {
      /* ignore quota */
    }
    try {
      global.document?.dispatchEvent(
        new CustomEvent("mei:visit-history-updated", { detail: { record, items: trimmed } }),
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

  function resolveAppIdFromPathname(pathname) {
    const parts = String(pathname || "")
      .split("/")
      .filter(Boolean);
    if (parts[0] !== "apps") return "";
    const routeSlug = parts[1] || "";
    const known = new Set(["access", "manage", "build", "presentation", "slides", "upload", "config"]);
    if (known.has(routeSlug) && parts[2]) return parts[2];
    return routeSlug;
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
    const appChip = document.querySelector(".status-chip-app");
    const appChipText = appChip ? String(appChip.textContent || "").trim() : "";
    const appTitle = appChipText.replace(/^应用\s*/, "").trim();
    return {
      workspace: readMeta("mei-workspace-label"),
      routeMode: document.body?.dataset?.meiView || readMeta("mei-view") || "",
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
      (base.kind === "drilldown" ? String(base.path || base.label || "").trim() : "");
    return {
      ...base,
      workspace: base.workspace || ctx.workspace,
      routeMode: base.routeMode || ctx.routeMode,
      appId: base.appId || ctx.appId,
      appTitle: base.appTitle || ctx.appTitle,
      pathname: base.pathname || ctx.pathname,
      href: base.href || ctx.href,
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
      `- 性能: 渲染 ${item.renderMs}ms · 求值 ${item.evalMs}ms · 总计 ${item.totalMs}ms`,
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
        const ok = call?.ok === false ? " FAIL" : "";
        lines.push(`  ${index + 1}. [${kind}] ${url} · HTTP ${status} · ${ms}${ok}`);
      });
    }
    return lines.join("\n");
  }

  function formatAllForAgent(items) {
    const listItems = Array.isArray(items) ? items : [];
    if (!listItems.length) return "# MeiLang 访问历史\n\n（暂无记录）";
    return (
      `# MeiLang 访问历史（${listItems.length} 条）\n\n` +
      listItems.map((item) => formatRecordForAgent(item)).join("\n\n")
    );
  }

  const api = {
    MAX_ENTRIES,
    readUsername,
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
