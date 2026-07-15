const QUERY_STATE_LS_PREFIX = "mei:query-state:v1:";
const QUERY_STATE_STORE_KEY = "__meiQueryStateStore";

function queryStateStorageKey(appId, sceneId) {
  const app = String(appId || "").trim() || "_";
  const scene = String(sceneId || "").trim() || "home";
  return `${QUERY_STATE_LS_PREFIX}${app}:${scene}`;
}

export function readPersistedQueryStateStore(appId, sceneId) {
  if (typeof window === "undefined") return null;
  try {
    const raw = localStorage.getItem(queryStateStorageKey(appId, sceneId));
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch (_) {
    return null;
  }
}

export function writePersistedQueryStateStore(appId, sceneId, store) {
  if (typeof window === "undefined") return false;
  try {
    if (!store || typeof store !== "object" || Object.keys(store).length === 0) {
      localStorage.removeItem(queryStateStorageKey(appId, sceneId));
      return true;
    }
    localStorage.setItem(queryStateStorageKey(appId, sceneId), JSON.stringify(store));
    return true;
  } catch (_) {
    return false;
  }
}

export function hydrateQueryStateStore(appId, sceneId) {
  if (typeof window === "undefined") return 0;
  const persisted = readPersistedQueryStateStore(appId, sceneId);
  if (!persisted) return 0;
  if (!window[QUERY_STATE_STORE_KEY] || typeof window[QUERY_STATE_STORE_KEY] !== "object") {
    window[QUERY_STATE_STORE_KEY] = {};
  }
  let restored = 0;
  for (const [key, value] of Object.entries(persisted)) {
    window[QUERY_STATE_STORE_KEY][key] = value;
    restored += 1;
  }
  return restored;
}

export function persistQueryStateStore(appId, sceneId) {
  if (typeof window === "undefined") return false;
  const store = window[QUERY_STATE_STORE_KEY];
  return writePersistedQueryStateStore(appId, sceneId, store);
}

export function installQueryStatePersistence(appId, sceneId) {
  if (typeof window === "undefined") return () => {};
  hydrateQueryStateStore(appId, sceneId);
  const onChange = () => {
    persistQueryStateStore(appId, sceneId);
  };
  document.addEventListener("mei:query-state-change", onChange);
  window.addEventListener("pagehide", onChange);
  return () => {
    document.removeEventListener("mei:query-state-change", onChange);
    window.removeEventListener("pagehide", onChange);
  };
}
