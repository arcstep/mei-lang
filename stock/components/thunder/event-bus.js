/** Thunder 时间轴导演：window + playhead 驱动地图/右栏；catalog 仅过渡。 */

export const THUNDER_EVENT_ACTIVATE = "mei:thunder-event-activate";
export const THUNDER_EVENT_STATE = "mei:thunder-event-state";
export const THUNDER_SLICE_SELECT = "mei:thunder-slice-select";
export const OBJECT_SELECTION_CHANGE = "mei:object-selection-change";

const STORE_KEY = "__meiThunderEventStore";
const OBJECT_SELECTION_SOURCE = "thunder";
export const STORM_EVENT_TYPE = "thunder.StormEvent";

/** 默认 live 窗长 */
export const LIVE_WINDOW_MS = 30 * 60_000;
/** 地图近窗（相对 playhead）正常显示 */
export const PLAYHEAD_NEAR_MS = 30 * 60_000;

function emptyStore() {
  return {
    catalog: null,
    catalogUrl: "",
    eventId: "",
    event: null,
    playbackAt: "",
    playbackAtMin: 0,
    /** ISO 墙钟游标（主时钟） */
    playhead: "",
    level: "",
    tBiz: "",
    windowStart: "",
    windowEnd: "",
    selectedSiteIds: [],
    monitorSites: [],
    efieldRefLines: [3, 7, 9],
    alertRows: [],
    historyRows: [],
    pgError: "",
    /** live | history */
    mode: "live",
    playing: false,
    /** 历史播放：墙钟 1s 推进的时间轴毫秒；live 默认 正常(=1000) */
    playSpeed: 1_000,
    /** 分钟/刻度档：1 | 5 | 10（默认 1 分钟） */
    zoomMinutes: 1,
  };
}

export function getThunderStore() {
  if (typeof window === "undefined") {
    return emptyStore();
  }
  if (!window[STORE_KEY]) {
    window[STORE_KEY] = emptyStore();
  }
  return window[STORE_KEY];
}

export function hhmmToMinutes(hhmm) {
  const text = String(hhmm || "").trim();
  const m = text.match(/^(\d{1,2}):(\d{2})$/);
  if (!m) return 0;
  return Number(m[1]) * 60 + Number(m[2]);
}

export function findEvent(catalog, eventId) {
  const events = Array.isArray(catalog?.events) ? catalog.events : [];
  const id = String(eventId || "").trim();
  if (!id) return null;
  return events.find((item) => String(item?.id || "").trim() === id) || null;
}

export function eventLocator(event) {
  const authored = event?.locator && typeof event.locator === "object"
    ? event.locator
    : {};
  const objectKey = String(
    authored.objectKey ?? authored.object_key ?? event?.id ?? "",
  ).trim();
  if (!objectKey) return null;
  return {
    objectType: String(
      authored.objectType ?? authored.object_type ?? STORM_EVENT_TYPE,
    ).trim(),
    objectKey,
  };
}

export function findEventByLocator(catalog, descriptor) {
  const events = Array.isArray(catalog?.events) ? catalog.events : [];
  const type = String(
    descriptor?.objectType ?? descriptor?.object_type ?? "",
  ).trim();
  if (type && type !== STORM_EVENT_TYPE) return null;
  const key = String(
    descriptor?.objectKey ??
      descriptor?.object_key ??
      descriptor?.identityValues?.event_id ??
      descriptor?.identity_values?.event_id ??
      "",
  ).trim();
  return key ? findEvent(catalog, key) : null;
}

export function getMeiObjectSelection() {
  if (typeof window === "undefined") return null;
  const api = window.MeiObjectSelection;
  if (!api || typeof api !== "object") return null;
  if (typeof api.getSelection === "function") {
    return api.getSelection();
  }
  return api.selection && typeof api.selection === "object" ? api.selection : null;
}

export function selectedStormEventDescriptor(selection = getMeiObjectSelection()) {
  const objects = Array.isArray(selection?.objects) ? selection.objects : [];
  return objects.find(
    (item) =>
      String(item?.objectType ?? item?.object_type ?? "").trim() ===
      STORM_EVENT_TYPE,
  ) || null;
}

export function dispatchThunderInteraction(
  event,
  playbackAt,
  intents = ["select"],
  source = OBJECT_SELECTION_SOURCE,
) {
  const locator = eventLocator(event);
  if (!locator || typeof window === "undefined") return null;
  const detail = {
    ...locator,
    source,
    secondary: {
      playbackAt: String(playbackAt || "").trim(),
    },
  };
  const interaction =
    window.MeiInteraction || window.__meiLangBoot?.interactionRuntime;
  if (interaction?.dispatchMany) {
    return interaction.dispatchMany(intents, detail);
  }
  window.dispatchEvent(new CustomEvent("mei:object-select", { detail }));
  return detail;
}

export function resolveDefaultEvent(catalog) {
  const events = Array.isArray(catalog?.events) ? catalog.events : [];
  const preferred = String(catalog?.defaultEventId || "").trim();
  if (preferred) {
    const hit = findEvent(catalog, preferred);
    if (hit) return hit;
  }
  return events.find((item) => String(item?.status || "").toLowerCase() === "active") || events[0] || null;
}

function playheadFields(playheadIso) {
  const d = playheadIso ? new Date(playheadIso) : null;
  if (!d || Number.isNaN(d.getTime())) {
    return { playhead: "", playbackAt: "", playbackAtMin: 0 };
  }
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return {
    playhead: d.toISOString(),
    playbackAt: `${hh}:${mm}`,
    playbackAtMin: d.getHours() * 60 + d.getMinutes(),
  };
}

export function publishThunderState(partial = {}, source = "bridge") {
  const store = getThunderStore();
  const next = { ...(partial || {}) };
  if (next.playhead != null && next.playbackAt == null) {
    Object.assign(next, playheadFields(next.playhead));
  }
  Object.assign(store, next);
  const detail = {
    eventId: store.eventId,
    event: store.event,
    playbackAt: store.playbackAt,
    playbackAtMin: store.playbackAtMin,
    playhead: store.playhead,
    level: store.level,
    tBiz: store.tBiz,
    windowStart: store.windowStart,
    windowEnd: store.windowEnd,
    selectedSiteIds: store.selectedSiteIds,
    monitorSites: store.monitorSites,
    efieldRefLines: store.efieldRefLines,
    alertRows: store.alertRows,
    historyRows: store.historyRows,
    pgError: store.pgError,
    mode: store.mode,
    playing: store.playing,
    zoomMinutes: store.zoomMinutes,
    source,
  };
  if (typeof window !== "undefined") {
    window.dispatchEvent(
      new CustomEvent(THUNDER_EVENT_STATE, {
        bubbles: true,
        composed: true,
        detail,
      }),
    );
  }
  return detail;
}

export function activateThunderEvent(event, options = {}) {
  if (!event) return null;
  const playhead =
    options.playhead ||
    event.playhead ||
    event.started_at ||
    options.playbackAt ||
    event.defaultSlice ||
    "";
  const ph = playheadFields(playhead);
  const level =
    String(options.level || event.level || event.max_level || "").trim();
  const detail = publishThunderState(
    {
      eventId: String(event.id || event.site_id || "").trim(),
      event,
      level,
      ...ph,
    },
    options.source || "activate",
  );
  if (options.publishSelection !== false) {
    dispatchThunderInteraction(
      event,
      detail.playbackAt,
      options.intents || ["select"],
      `${OBJECT_SELECTION_SOURCE}:${options.source || "activate"}`,
    );
  }
  return detail;
}

export function selectThunderSlice(playbackAt, options = {}) {
  const store = getThunderStore();
  const at = String(playbackAt || "").trim();
  let playhead = options.playhead || "";
  if (!playhead && store.windowStart && at) {
    const base = new Date(store.windowStart);
    if (!Number.isNaN(base.getTime())) {
      const [hh, mm] = at.split(":").map(Number);
      const d = new Date(base);
      d.setHours(hh || 0, mm || 0, 0, 0);
      // 若早于窗起点，可能跨日：贴近 windowEnd 日期
      if (d.getTime() < new Date(store.windowStart).getTime() - 60_000) {
        const end = new Date(store.windowEnd || store.windowStart);
        d.setFullYear(end.getFullYear(), end.getMonth(), end.getDate());
        d.setHours(hh || 0, mm || 0, 0, 0);
      }
      playhead = d.toISOString();
    }
  }
  const level = String(store.event?.level || store.level || "").trim();
  return publishThunderState(
    {
      ...playheadFields(playhead || store.playhead),
      level,
    },
    options.source || "slice",
  );
}

/** 生效监测站：空选 = 全部 monitorSites */
export function effectiveSiteIds(store = getThunderStore()) {
  const selected = Array.isArray(store?.selectedSiteIds)
    ? store.selectedSiteIds.map((s) => String(s).trim()).filter(Boolean)
    : [];
  if (selected.length) return selected;
  const all = Array.isArray(store?.monitorSites)
    ? store.monitorSites.map((s) => String(s?.site_id || s).trim()).filter(Boolean)
    : [];
  return all;
}

/** live 播放速度：墙钟 1s = 时间轴 1s */
export const LIVE_PLAY_RATE_MS = 1_000;
/** 进入历史窗时的默认速度：墙钟 1s = 时间轴 1 分钟 */
export const HISTORY_PLAY_RATE_MS = 60_000;

/** 播放速度选项（墙钟 1s 推进的时间轴毫秒） */
export const PLAY_SPEED_OPTIONS = [
  { id: "1x", label: "1×", rateMs: 1_000 },
  { id: "10x", label: "10×", rateMs: 10_000 },
  { id: "60x", label: "60×", rateMs: 60_000 },
  { id: "120x", label: "120×", rateMs: 120_000 },
];

export function resolvePlaySpeedOption(rateMs) {
  const n = Math.max(1000, Number(rateMs) || HISTORY_PLAY_RATE_MS);
  return (
    PLAY_SPEED_OPTIONS.find((opt) => opt.rateMs === n) ||
    PLAY_SPEED_OPTIONS.find((opt) => opt.rateMs === HISTORY_PLAY_RATE_MS) ||
    PLAY_SPEED_OPTIONS[0]
  );
}

/** live 默认窗：[T_biz-30min, T_biz] */
export function liveWindowFromTbiz(tBizIso, windowMs = LIVE_WINDOW_MS) {
  const end = new Date(tBizIso || Date.now());
  if (Number.isNaN(end.getTime())) {
    const now = new Date();
    return {
      tBiz: now.toISOString(),
      windowStart: new Date(now.getTime() - windowMs).toISOString(),
      windowEnd: now.toISOString(),
      playhead: now.toISOString(),
      mode: "live",
      playSpeed: LIVE_PLAY_RATE_MS,
    };
  }
  return {
    tBiz: end.toISOString(),
    windowStart: new Date(end.getTime() - windowMs).toISOString(),
    windowEnd: end.toISOString(),
    playhead: end.toISOString(),
    mode: "live",
    playSpeed: LIVE_PLAY_RATE_MS,
  };
}

/**
 * 历史 streak → 时间窗：至少覆盖段；不足 30min 则居中扩到 30min。
 */
export function windowForHistoryStreak(startedAt, endedAt, minMs = LIVE_WINDOW_MS) {
  let a = new Date(startedAt).getTime();
  let b = new Date(endedAt || startedAt).getTime();
  if (!Number.isFinite(a)) a = Date.now() - minMs;
  if (!Number.isFinite(b) || b < a) b = a;
  if (b - a < minMs) {
    const mid = (a + b) / 2;
    a = mid - minMs / 2;
    b = mid + minMs / 2;
  }
  const startIso = new Date(a).toISOString();
  return {
    mode: "history",
    windowStart: startIso,
    windowEnd: new Date(b).toISOString(),
    playhead: new Date(startedAt).toISOString(),
    playing: false,
    playSpeed: HISTORY_PLAY_RATE_MS,
  };
}

export function subscribeThunderState(handler) {
  if (typeof window === "undefined" || typeof handler !== "function") {
    return () => {};
  }
  const onState = (event) => handler(event?.detail || getThunderStore());
  window.addEventListener(THUNDER_EVENT_STATE, onState);
  return () => window.removeEventListener(THUNDER_EVENT_STATE, onState);
}

export function levelTone(level) {
  const text = String(level || "").trim();
  if (text.includes("红") || text.toLowerCase() === "red") {
    return { fg: "#f87171", bg: "rgba(248, 113, 113, 0.22)", border: "rgba(248, 113, 113, 0.65)" };
  }
  if (text.includes("橙") || text.toLowerCase() === "orange") {
    return { fg: "#fb923c", bg: "rgba(251, 146, 60, 0.22)", border: "rgba(251, 146, 60, 0.65)" };
  }
  if (text.includes("黄") || text.toLowerCase() === "yellow") {
    return { fg: "#facc15", bg: "rgba(250, 204, 21, 0.18)", border: "rgba(250, 204, 21, 0.55)" };
  }
  return { fg: "#7dd3fc", bg: "rgba(56, 160, 240, 0.18)", border: "rgba(56, 160, 240, 0.45)" };
}

export function listTitleOf(event) {
  if (!event) return "";
  return String(
    event.listTitle || event.site_name || `${event.id || ""} · ${event.title || ""}`,
  ).trim();
}

export function fixtureKeyForEventId(eventId) {
  return String(eventId || "").trim();
}
