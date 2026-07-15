/** Thunder 首页事件总线：selectedEventId + playbackAt（P0 原型真换数）。 */

export const THUNDER_EVENT_ACTIVATE = "mei:thunder-event-activate";
export const THUNDER_EVENT_STATE = "mei:thunder-event-state";
export const THUNDER_SLICE_SELECT = "mei:thunder-slice-select";

const STORE_KEY = "__meiThunderEventStore";

function emptyStore() {
  return {
    catalog: null,
    catalogUrl: "",
    eventId: "",
    event: null,
    playbackAt: "",
    playbackAtMin: 0,
    level: "",
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

export function resolveDefaultEvent(catalog) {
  const events = Array.isArray(catalog?.events) ? catalog.events : [];
  const preferred = String(catalog?.defaultEventId || "").trim();
  if (preferred) {
    const hit = findEvent(catalog, preferred);
    if (hit) return hit;
  }
  return events.find((item) => String(item?.status || "").toLowerCase() === "active") || events[0] || null;
}

export function publishThunderState(partial = {}, source = "bridge") {
  const store = getThunderStore();
  Object.assign(store, partial || {});
  const detail = {
    eventId: store.eventId,
    event: store.event,
    playbackAt: store.playbackAt,
    playbackAtMin: store.playbackAtMin,
    level: store.level,
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
  const playbackAt = String(options.playbackAt || event.defaultSlice || event.slices?.[event.slices.length - 1] || "").trim();
  const level =
    String(options.level || event.sliceLevels?.[playbackAt] || event.level || "").trim();
  return publishThunderState(
    {
      eventId: String(event.id || "").trim(),
      event,
      playbackAt,
      playbackAtMin: hhmmToMinutes(playbackAt),
      level,
    },
    options.source || "activate",
  );
}

export function selectThunderSlice(playbackAt, options = {}) {
  const store = getThunderStore();
  const event = store.event;
  if (!event) return null;
  const at = String(playbackAt || "").trim();
  const level = String(event.sliceLevels?.[at] || event.level || store.level || "").trim();
  return publishThunderState(
    {
      playbackAt: at,
      playbackAtMin: hhmmToMinutes(at),
      level,
    },
    options.source || "slice",
  );
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
  if (text.includes("红")) return { fg: "#f87171", bg: "rgba(248, 113, 113, 0.22)", border: "rgba(248, 113, 113, 0.65)" };
  if (text.includes("橙")) return { fg: "#fb923c", bg: "rgba(251, 146, 60, 0.22)", border: "rgba(251, 146, 60, 0.65)" };
  if (text.includes("黄")) return { fg: "#facc15", bg: "rgba(250, 204, 21, 0.18)", border: "rgba(250, 204, 21, 0.55)" };
  return { fg: "#7dd3fc", bg: "rgba(56, 160, 240, 0.18)", border: "rgba(56, 160, 240, 0.45)" };
}

export function listTitleOf(event) {
  if (!event) return "";
  return String(event.listTitle || `${event.id} · ${event.title || ""}`).trim();
}

/** catalog 事件 id → prototype/{lifecycle,efield,lightning}/ 文件键 */
export function fixtureKeyForEventId(eventId) {
  const id = String(eventId || "").trim();
  const map = {
    "260709-01": "EVT-20260709-01",
    "260708-17": "EVT-20260709-01",
    "260707-09": "EVT-20260709-01",
    "EVT-20260709-01": "EVT-20260709-01",
  };
  return map[id] || "EVT-20260709-01";
}

