  const SCENE_SHELL_DB = "mei-scene-shell-cache-v1";
  const SCENE_SHELL_STORE = "snapshots";
  const SCENE_SHELL_DB_VERSION = 1;
  const SCENE_SHELL_LS_PREFIX = "mei:scene-shell:v1:";
  const SCENE_SHELL_MAX_ENTRIES = 12;
  const SCENE_SHELL_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;

  function openSceneShellDb() {
    if (typeof indexedDB === "undefined") {
      return Promise.resolve(null);
    }
    return new Promise((resolve) => {
      try {
        const request = indexedDB.open(SCENE_SHELL_DB, SCENE_SHELL_DB_VERSION);
        request.onupgradeneeded = () => {
          const db = request.result;
          if (!db.objectStoreNames.contains(SCENE_SHELL_STORE)) {
            db.createObjectStore(SCENE_SHELL_STORE, { keyPath: "key" });
          }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => resolve(null);
      } catch (_) {
        resolve(null);
      }
    });
  }

  function snapshotStorageKey(ctx) {
    return `${ctx.appId}:${ctx.sceneId}:${ctx.mode || "app"}`;
  }

  function collectHeadJsonScripts() {
    const scripts = {};
    for (const id of ["mei-scene-drilldown-context", "mei-host-runtime-capabilities", "mei-layer-plan", "mei-presentation-map"]) {
      const node = document.getElementById(id);
      if (node) {
        scripts[id] = node.textContent || "";
      }
    }
    return scripts;
  }

  function collectSceneBundleMeta(doc) {
    const source = doc || document;
    const bundle = source.querySelector('script[data-mei-scene-bundle="true"]');
    if (!bundle) return null;
    return {
      src: bundle.getAttribute("src") || "",
      revision: bundle.getAttribute("data-mei-persistent-script") || bundle.getAttribute("src") || "",
    };
  }

  function buildSceneShellSnapshot(ctx, revision, doc) {
    const sourceDoc = doc || document;
    const shell = sourceDoc.querySelector(".shell");
    if (!shell) return null;
    return {
      key: snapshotStorageKey(ctx),
      appId: ctx.appId,
      sceneId: ctx.sceneId,
      mode: ctx.mode || "app",
      revision,
      savedAtMs: Date.now(),
      title: sourceDoc.title || document.title,
      bodyClassName: sourceDoc.body?.className || document.body.className,
      shellHtml: shell.innerHTML,
      headScripts: collectHeadJsonScripts(),
      sceneBundle: collectSceneBundleMeta(sourceDoc),
      url: ctx.url || window.location.href,
    };
  }

  async function listSceneShellSnapshots(db) {
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(SCENE_SHELL_STORE, "readonly");
        const request = tx.objectStore(SCENE_SHELL_STORE).getAll();
        request.onsuccess = () => resolve(request.result || []);
        request.onerror = () => resolve([]);
      } catch (_) {
        resolve([]);
      }
    });
  }

  function estimateSnapshotBytes(snapshot) {
    try {
      return JSON.stringify(snapshot || {}).length;
    } catch (_) {
      return 0;
    }
  }

  async function pruneSceneShellDb(db) {
    const now = Date.now();
    let pruned = 0;
    const all = await listSceneShellSnapshots(db);
    const expiredKeys = all
      .filter((item) => now - Number(item?.savedAtMs || 0) > SCENE_SHELL_MAX_AGE_MS)
      .map((item) => item.key);
    if (expiredKeys.length > 0) {
      await new Promise((resolve) => {
        try {
          const tx = db.transaction(SCENE_SHELL_STORE, "readwrite");
          const store = tx.objectStore(SCENE_SHELL_STORE);
          for (const key of expiredKeys) {
            store.delete(key);
            pruned += 1;
          }
          tx.oncomplete = () => resolve(true);
          tx.onerror = () => resolve(false);
        } catch (_) {
          resolve(false);
        }
      });
    }
    let remaining = await listSceneShellSnapshots(db);
    if (remaining.length > SCENE_SHELL_MAX_ENTRIES) {
      remaining = remaining.sort(
        (a, b) => Number(a?.savedAtMs || 0) - Number(b?.savedAtMs || 0),
      );
      const excess = remaining.length - SCENE_SHELL_MAX_ENTRIES;
      const deleteKeys = remaining.slice(0, excess).map((item) => item.key);
      await new Promise((resolve) => {
        try {
          const tx = db.transaction(SCENE_SHELL_STORE, "readwrite");
          const store = tx.objectStore(SCENE_SHELL_STORE);
          for (const key of deleteKeys) {
            store.delete(key);
            pruned += 1;
          }
          tx.oncomplete = () => resolve(true);
          tx.onerror = () => resolve(false);
        } catch (_) {
          resolve(false);
        }
      });
      remaining = await listSceneShellSnapshots(db);
    }
    let totalBytesEstimate = 0;
    for (const item of remaining) {
      totalBytesEstimate += estimateSnapshotBytes(item);
    }
    return { pruned, totalBytesEstimate, entries: remaining.length };
  }

  function pruneSceneShellSessionStorage() {
    let pruned = 0;
    const entries = [];
    try {
      for (let i = 0; i < sessionStorage.length; i += 1) {
        const key = sessionStorage.key(i);
        if (!key || !key.startsWith(SCENE_SHELL_LS_PREFIX)) continue;
        const raw = sessionStorage.getItem(key);
        let savedAtMs = 0;
        try {
          const parsed = JSON.parse(raw || "{}");
          savedAtMs = Number(parsed?.savedAtMs || 0);
        } catch (_) {
          savedAtMs = 0;
        }
        entries.push({ key, savedAtMs });
      }
    } catch (_) {
      return { pruned: 0, totalBytesEstimate: 0, entries: 0 };
    }
    const now = Date.now();
    for (const entry of entries) {
      if (now - entry.savedAtMs > SCENE_SHELL_MAX_AGE_MS) {
        try {
          sessionStorage.removeItem(entry.key);
          pruned += 1;
        } catch (_) {
          /* ignore */
        }
      }
    }
    const survivors = entries
      .filter((entry) => now - entry.savedAtMs <= SCENE_SHELL_MAX_AGE_MS)
      .sort((a, b) => a.savedAtMs - b.savedAtMs);
    if (survivors.length > SCENE_SHELL_MAX_ENTRIES) {
      const excess = survivors.length - SCENE_SHELL_MAX_ENTRIES;
      for (let i = 0; i < excess; i += 1) {
        try {
          sessionStorage.removeItem(survivors[i].key);
          pruned += 1;
        } catch (_) {
          /* ignore */
        }
      }
    }
    let totalBytesEstimate = 0;
    let entryCount = 0;
    try {
      for (let i = 0; i < sessionStorage.length; i += 1) {
        const key = sessionStorage.key(i);
        if (!key || !key.startsWith(SCENE_SHELL_LS_PREFIX)) continue;
        entryCount += 1;
        totalBytesEstimate += String(sessionStorage.getItem(key) || "").length;
      }
    } catch (_) {
      /* ignore */
    }
    if (pruned > 0) {
      window.__meiBrowserRuntimeDiag?.record?.("scene_shell_pruned", {
        pruned,
        entries: entryCount,
        totalBytesEstimate,
      });
    }
    return { pruned, totalBytesEstimate, entries: entryCount };
  }

  async function persistSceneShellSnapshot(snapshot) {
    if (!snapshot || !snapshot.key) return false;
    const db = await openSceneShellDb();
    if (db) {
      await new Promise((resolve) => {
        try {
          const tx = db.transaction(SCENE_SHELL_STORE, "readwrite");
          tx.objectStore(SCENE_SHELL_STORE).put(snapshot);
          tx.oncomplete = () => resolve(true);
          tx.onerror = () => resolve(false);
        } catch (_) {
          resolve(false);
        }
      });
      const pruneStats = await pruneSceneShellDb(db);
      window.__meiBrowserRuntimeDiag?.record?.("scene_shell_persist", {
        key: snapshot.key,
        ...pruneStats,
      });
      try {
        db.close();
      } catch (_) {}
      return true;
    }
    try {
      const compact = {
        key: snapshot.key,
        revision_digest: snapshot.revision?.revision_digest,
        cache_key: snapshot.revision?.cache_key,
        savedAtMs: snapshot.savedAtMs,
        title: snapshot.title,
        bodyClassName: snapshot.bodyClassName,
        shellHtml: snapshot.shellHtml,
      };
      sessionStorage.setItem(`${SCENE_SHELL_LS_PREFIX}${snapshot.key}`, JSON.stringify(compact));
      const pruneStats = pruneSceneShellSessionStorage();
      window.__meiBrowserRuntimeDiag?.record?.("scene_shell_persist", {
        key: snapshot.key,
        ...pruneStats,
      });
      return true;
    } catch (_) {
      return false;
    }
  }

  async function loadSceneShellSnapshot(ctx) {
    const key = snapshotStorageKey(ctx);
    const db = await openSceneShellDb();
    if (db) {
      const snapshot = await new Promise((resolve) => {
        try {
          const tx = db.transaction(SCENE_SHELL_STORE, "readonly");
          const request = tx.objectStore(SCENE_SHELL_STORE).get(key);
          request.onsuccess = () => resolve(request.result || null);
          request.onerror = () => resolve(null);
        } catch (_) {
          resolve(null);
        }
      });
      try {
        db.close();
      } catch (_) {}
      if (snapshot) return snapshot;
    }
    try {
      const raw = sessionStorage.getItem(`${SCENE_SHELL_LS_PREFIX}${key}`);
      if (!raw) return null;
      const compact = JSON.parse(raw);
      return compact && compact.shellHtml ? compact : null;
    } catch (_) {
      return null;
    }
  }

  function applyHeadJsonScripts(scripts) {
    if (!scripts || typeof scripts !== "object") return;
    for (const [id, content] of Object.entries(scripts)) {
      const node = document.getElementById(id);
      if (!node) continue;
      node.textContent = content || "";
    }
    try {
      delete window.__meiSceneDrilldownContext;
      delete window.__meiHostRuntimeCapabilities;
    } catch (_) {}
  }

  function restoreSceneShellSnapshot(snapshot, url, replaceHistory) {
    const shell = document.querySelector(".shell");
    if (!shell || !snapshot?.shellHtml) return false;
    shell.innerHTML = snapshot.shellHtml;
    if (snapshot.title) {
      document.title = snapshot.title;
    }
    if (snapshot.bodyClassName) {
      document.body.className = snapshot.bodyClassName;
    }
    applyHeadJsonScripts(snapshot.headScripts);
    if (url) {
      if (replaceHistory) {
        window.history.replaceState({}, "", url);
      } else {
        window.history.pushState({}, "", url);
      }
    }
    window.__meiShellRestoredFromCache = 1;
    return true;
  }

  function buildDocFromSceneShellSnapshot(snapshot) {
    if (!snapshot?.shellHtml) return null;
    const html = `<!DOCTYPE html><html><head><title>${snapshot.title || ""}</title></head><body><div class="shell">${snapshot.shellHtml}</div></body></html>`;
    return new DOMParser().parseFromString(html, "text/html");
  }

  async function tryRestoreSceneShellFromCache(ctx, revision, url, replaceHistory) {
    const snapshot = await loadSceneShellSnapshot(ctx);
    if (!snapshot) return null;
    if (!boot.revisionsMatch(snapshot.revision, revision)) return null;
    const restored = restoreSceneShellSnapshot(snapshot, url, replaceHistory);
    if (!restored) return null;
    return buildDocFromSceneShellSnapshot(snapshot);
  }

  async function saveCurrentSceneShellSnapshot(ctx, revision, doc) {
    const snapshot = buildSceneShellSnapshot(ctx, revision, doc);
    if (!snapshot) return false;
    return persistSceneShellSnapshot(snapshot);
  }

  boot.buildSceneShellSnapshot = buildSceneShellSnapshot;
  boot.collectHeadJsonScripts = collectHeadJsonScripts;
  boot.loadSceneShellSnapshot = loadSceneShellSnapshot;
  boot.persistSceneShellSnapshot = persistSceneShellSnapshot;
  boot.restoreSceneShellSnapshot = restoreSceneShellSnapshot;
  boot.tryRestoreSceneShellFromCache = tryRestoreSceneShellFromCache;
  boot.buildDocFromSceneShellSnapshot = buildDocFromSceneShellSnapshot;
  boot.saveCurrentSceneShellSnapshot = saveCurrentSceneShellSnapshot;
  boot.pruneSceneShellSessionStorage = pruneSceneShellSessionStorage;
  boot.SCENE_SHELL_MAX_ENTRIES = SCENE_SHELL_MAX_ENTRIES;
  boot.SCENE_SHELL_MAX_AGE_MS = SCENE_SHELL_MAX_AGE_MS;
