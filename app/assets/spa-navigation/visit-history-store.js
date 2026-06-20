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
    const items = list();
    items.unshift(record);
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

  const api = {
    MAX_ENTRIES,
    readUsername,
    list,
    append,
    kindLabel,
  };

  global.MeiVisitHistoryStore = api;
  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  boot.appendVisitHistory = append;
  boot.listVisitHistory = list;
  boot.visitHistoryKindLabel = kindLabel;
})(
  typeof window !== "undefined" ? window : typeof globalThis !== "undefined" ? globalThis : {},
);
