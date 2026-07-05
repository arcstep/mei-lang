  // Shell HTML snapshot cache removed; revision-first layer assembly is the only restore path.
  function snapshotStorageKey(ctx) {
    if (typeof boot.sceneRevisionCacheKey === "function") {
      return boot.sceneRevisionCacheKey(ctx);
    }
    const dataMode = String(ctx.dataMode || "").trim().toLowerCase();
    const reviewProjection = String(ctx.reviewProjection || "").trim().toLowerCase();
    const chrome = String(ctx.chrome || "").trim().toLowerCase();
    return [ctx.appId, ctx.sceneId, ctx.mode || "app", chrome, dataMode, reviewProjection]
      .filter(Boolean)
      .join(":");
  }

  function legacySnapshotStorageKey(ctx) {
    return snapshotStorageKey(ctx);
  }

  async function loadSceneShellSnapshot() {
    return null;
  }

  async function persistSceneShellSnapshot() {
    return false;
  }

  async function tryRestoreSceneShellFromCache() {
    return null;
  }

  async function saveCurrentSceneShellSnapshot() {
    return false;
  }

  function buildSceneShellSnapshot() {
    return null;
  }

  function restoreSceneShellSnapshot() {
    return false;
  }

  function buildDocFromSceneShellSnapshot() {
    return null;
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

  function pruneSceneShellSessionStorage() {
    return { pruned: 0, totalBytesEstimate: 0, entries: 0 };
  }

  boot.buildSceneShellSnapshot = buildSceneShellSnapshot;
  boot.collectHeadJsonScripts = collectHeadJsonScripts;
  boot.loadSceneShellSnapshot = loadSceneShellSnapshot;
  boot.persistSceneShellSnapshot = persistSceneShellSnapshot;
  boot.restoreSceneShellSnapshot = restoreSceneShellSnapshot;
  boot.tryRestoreSceneShellFromCache = tryRestoreSceneShellFromCache;
  boot.buildDocFromSceneShellSnapshot = buildDocFromSceneShellSnapshot;
  boot.saveCurrentSceneShellSnapshot = saveCurrentSceneShellSnapshot;
  boot.snapshotStorageKey = snapshotStorageKey;
  boot.legacySnapshotStorageKey = legacySnapshotStorageKey;
  boot.pruneSceneShellSessionStorage = pruneSceneShellSessionStorage;
  boot.SCENE_SHELL_MAX_ENTRIES = 0;
  boot.SCENE_SHELL_MAX_AGE_MS = 0;
