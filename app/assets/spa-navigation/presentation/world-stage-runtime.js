(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const WORLD_STAGE_EVENT = "mei:presentation-world-action";
  const WORLD_STAGE_SELECTORS = [
    "mei-cockpit-basemap-stage",
    "mei-map-maplibre",
    "mei-world-stage",
  ].join(",");

  function readDatasetValue(node, key) {
    if (!(node instanceof HTMLElement)) return "";
    return String(node.dataset?.[key] || "").trim();
  }

  function nearestStageMeta(node) {
    if (!(node instanceof HTMLElement)) {
      return null;
    }
    const block = node.closest("[data-mei-block-id]");
    const panel = node.closest("[data-mei-panel-id]");
    return {
      node,
      block,
      panel,
      panelId: readDatasetValue(panel, "meiPanelId"),
      blockId: readDatasetValue(block, "meiBlockId"),
      viewFamily:
        readDatasetValue(block, "meiViewFamily") || readDatasetValue(panel, "meiViewFamily"),
      stageKind:
        readDatasetValue(block, "meiStageKind") || readDatasetValue(panel, "meiStageKind"),
      worldRef:
        readDatasetValue(block, "meiWorldRef") || readDatasetValue(panel, "meiWorldRef"),
      entityId:
        readDatasetValue(block, "meiEntityId") || readDatasetValue(panel, "meiEntityId"),
      groupId:
        readDatasetValue(block, "meiGroupId") || readDatasetValue(panel, "meiGroupId"),
      cameraPreset:
        readDatasetValue(block, "meiCameraPreset") || readDatasetValue(panel, "meiCameraPreset"),
    };
  }

  function collectWorldStageHosts() {
    return Array.from(document.querySelectorAll(WORLD_STAGE_SELECTORS))
      .filter((node) => node instanceof HTMLElement)
      .map((node) => nearestStageMeta(node))
      .filter(Boolean);
  }

  function scoreStageHost(host, target) {
    let score = 0;
    if (!host || !target) return score;
    if (target.worldRef && host.worldRef === target.worldRef) score += 6;
    if (target.viewFamily && host.viewFamily === target.viewFamily) score += 4;
    if (target.stageKind && host.stageKind === target.stageKind) score += 3;
    if (target.entityId && host.entityId === target.entityId) score += 2;
    if (target.groupId && host.groupId === target.groupId) score += 2;
    if (target.cameraPreset && host.cameraPreset === target.cameraPreset) score += 1;
    if (!target.viewFamily && host.viewFamily) score += 1;
    if (!target.worldRef && host.worldRef) score += 1;
    return score;
  }

  function resolveStageHost(target) {
    const hosts = collectWorldStageHosts();
    if (!hosts.length) return null;
    const scored = hosts
      .map((host) => ({
        host,
        score: scoreStageHost(host, target),
      }))
      .sort((left, right) => right.score - left.score);
    if (scored[0]?.score > 0) {
      return scored[0].host;
    }
    return hosts[0];
  }

  function normalizeWorldTarget(target) {
    if (!target || typeof target !== "object") return null;
    const normalized = {
      type: String(target.type || target.kind || "").trim(),
      viewpointId: String(target.viewpointId || target.viewpoint || "").trim(),
      viewFamily: String(target.viewFamily || target.view_family || "").trim(),
      stageKind: String(target.stageKind || target.stage_kind || "").trim(),
      worldRef: String(target.worldRef || target.world_ref || "").trim(),
      entityId: String(target.entityId || target.entity_id || "").trim(),
      groupId: String(target.groupId || target.group_id || "").trim(),
      cameraPreset: String(target.cameraPreset || target.camera_preset || "").trim(),
    };
    if (
      !normalized.type &&
      !normalized.viewpointId &&
      !normalized.viewFamily &&
      !normalized.worldRef &&
      !normalized.entityId &&
      !normalized.groupId &&
      !normalized.cameraPreset
    ) {
      return null;
    }
    return normalized;
  }

  function applyWorldTarget(target) {
    const normalized = normalizeWorldTarget(target);
    if (!normalized) return false;
    const host = resolveStageHost(normalized);
    if (!host?.node) return false;
    boot.lastResolvedWorldStageHost = {
      panelId: host.panelId,
      blockId: host.blockId,
      viewFamily: host.viewFamily,
      stageKind: host.stageKind,
      worldRef: host.worldRef,
    };
    if (typeof host.node.applyWorldTarget === "function") {
      return Boolean(host.node.applyWorldTarget(normalized, host));
    }
    host.node.dispatchEvent(
      new CustomEvent("mei:apply-world-target", {
        detail: normalized,
        bubbles: false,
      }),
    );
    return true;
  }

  function onWorldAction(event) {
    applyWorldTarget(event?.detail);
  }

  function installWorldStageRuntime() {
    if (boot.worldStageRuntimeMounted) return;
    boot.worldStageRuntimeMounted = true;
    window.addEventListener(WORLD_STAGE_EVENT, onWorldAction);
    boot.worldStageRuntime = {
      applyWorldTarget,
      collectWorldStageHosts,
      resolveStageHost,
    };
  }

  installWorldStageRuntime();
})();
