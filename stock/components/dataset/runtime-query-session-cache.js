const STORAGE_PREFIX = "mei:runtime-query:v1:";

function sessionStorageAvailable() {
  if (typeof window === "undefined" || !window.sessionStorage) {
    return false;
  }
  try {
    const probe = `${STORAGE_PREFIX}__probe__`;
    window.sessionStorage.setItem(probe, "1");
    window.sessionStorage.removeItem(probe);
    return true;
  } catch (_) {
    return false;
  }
}

function hashCacheKey(text) {
  const value = String(text || "");
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function storageKey(appId, cacheKey) {
  const app = String(appId || "").trim() || "default";
  const digest = hashCacheKey(cacheKey);
  return `${STORAGE_PREFIX}${app}:${digest}`;
}

function readJson(key) {
  if (!sessionStorageAvailable()) {
    return null;
  }
  try {
    const raw = window.sessionStorage.getItem(key);
    if (!raw) {
      return null;
    }
    return JSON.parse(raw);
  } catch (_) {
    return null;
  }
}

function writeJson(key, value) {
  if (!sessionStorageAvailable()) {
    return;
  }
  try {
    window.sessionStorage.setItem(key, JSON.stringify(value));
  } catch (_) {
    /* ignore quota errors */
  }
}

function listSessionKeys(appId) {
  if (!sessionStorageAvailable()) {
    return [];
  }
  const prefix = `${STORAGE_PREFIX}${String(appId || "").trim() || "default"}:`;
  const keys = [];
  for (let index = 0; index < window.sessionStorage.length; index += 1) {
    const key = window.sessionStorage.key(index);
    if (key && key.startsWith(prefix)) {
      keys.push(key);
    }
  }
  return keys;
}

function dataGenMatches(expectedDataGen, entryDataGen) {
  const expected = String(expectedDataGen || "").trim();
  const stored = String(entryDataGen || "").trim();
  if (!expected || !stored) {
    return true;
  }
  return expected === stored;
}

export function enumerateSessionRuntimeQueryCaches(appId, expectedDataGen, now = Date.now()) {
  const results = [];
  for (const key of listSessionKeys(appId)) {
    const entry = readJson(key);
    if (!entry || typeof entry !== "object") {
      continue;
    }
    if (!dataGenMatches(expectedDataGen, entry.dataGen)) {
      continue;
    }
    const expiresAt = Number(entry.expiresAt);
    if (!Number.isFinite(expiresAt) || expiresAt <= now) {
      continue;
    }
    const cacheKey = String(entry.cacheKey || "").trim();
    if (!cacheKey || !entry.data) {
      continue;
    }
    results.push({
      cacheKey,
      kind: String(entry.kind || "metric").trim() || "metric",
      data: entry.data,
      expiresAt,
    });
  }
  return results;
}

export function readSessionRuntimeQueryCache(appId, cacheKey, expectedDataGen, now = Date.now()) {
  const normalizedKey = String(cacheKey || "").trim();
  if (!normalizedKey) {
    return null;
  }
  const entry = readJson(storageKey(appId, normalizedKey));
  if (!entry || typeof entry !== "object") {
    return null;
  }
  if (String(entry.cacheKey || "").trim() !== normalizedKey) {
    return null;
  }
  if (!dataGenMatches(expectedDataGen, entry.dataGen)) {
    return null;
  }
  const expiresAt = Number(entry.expiresAt);
  if (!Number.isFinite(expiresAt) || expiresAt <= now) {
    return null;
  }
  return entry;
}

export function writeSessionRuntimeQueryCache(
  appId,
  cacheKey,
  dataGen,
  kind,
  data,
  ttlMs,
  maxEntries = 512,
) {
  const normalizedKey = String(cacheKey || "").trim();
  if (!normalizedKey) {
    return;
  }
  const ttl = Number.isFinite(Number(ttlMs)) && Number(ttlMs) > 0 ? Number(ttlMs) : 300_000;
  const key = storageKey(appId, normalizedKey);
  writeJson(key, {
    cacheKey: normalizedKey,
    dataGen: String(dataGen || "").trim(),
    savedAtMs: Date.now(),
    expiresAt: Date.now() + ttl,
    kind: String(kind || "metric").trim() || "metric",
    data,
  });
  const keys = listSessionKeys(appId);
  if (keys.length <= maxEntries) {
    return;
  }
  const ranked = keys
    .map((candidate) => {
      const entry = readJson(candidate);
      return {
        key: candidate,
        savedAtMs: Number(entry?.savedAtMs) || 0,
      };
    })
    .sort((left, right) => left.savedAtMs - right.savedAtMs);
  const overflow = ranked.length - maxEntries;
  for (let index = 0; index < overflow; index += 1) {
    try {
      window.sessionStorage.removeItem(ranked[index].key);
    } catch (_) {
      /* ignore */
    }
  }
}

export function persistMemoryRuntimeQueryCaches(appId, dataGen, memoryEntries, ttlMs, maxEntries) {
  if (!Array.isArray(memoryEntries) || memoryEntries.length === 0) {
    return;
  }
  for (const entry of memoryEntries) {
    if (!entry || !entry.cacheKey) {
      continue;
    }
    writeSessionRuntimeQueryCache(
      appId,
      entry.cacheKey,
      dataGen,
      entry.kind,
      entry.data,
      ttlMs,
      maxEntries,
    );
  }
}

export function clearSessionRuntimeQueryCaches(appId) {
  for (const key of listSessionKeys(appId)) {
    try {
      window.sessionStorage.removeItem(key);
    } catch (_) {
      /* ignore */
    }
  }
}

export function clientQueryCacheConfig(props) {
  const runtime = props?._mei?.client_query_cache;
  if (!runtime || typeof runtime !== "object") {
    return {
      persist: "sessionStorage",
      ttlMs: 300_000,
      maxEntries: 512,
    };
  }
  return {
    persist: String(runtime.persist || "sessionStorage").trim() || "sessionStorage",
    ttlMs:
      Number.isFinite(Number(runtime.ttl_ms ?? runtime.ttlMs)) && Number(runtime.ttl_ms ?? runtime.ttlMs) > 0
        ? Number(runtime.ttl_ms ?? runtime.ttlMs)
        : 300_000,
    maxEntries:
      Number.isFinite(Number(runtime.max_entries ?? runtime.maxEntries)) &&
      Number(runtime.max_entries ?? runtime.maxEntries) > 0
        ? Number(runtime.max_entries ?? runtime.maxEntries)
        : 512,
  };
}
