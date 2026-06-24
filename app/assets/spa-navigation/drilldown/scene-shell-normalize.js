  function normalizeProjection(value) {
    const raw = String(value || "overlay")
      .trim()
      .toLowerCase();
    if (raw === "route" || raw === "navigate" || raw === "spa" || raw === "page") {
      return "route";
    }
    return "overlay";
  }

  function normalizeShellLayout(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
    const columns = Array.isArray(raw.columns)
      ? raw.columns.map((entry) => String(entry || "").trim()).filter(Boolean)
      : [];
    const rows = Array.isArray(raw.rows)
      ? raw.rows.map((entry) => String(entry || "").trim()).filter(Boolean)
      : [];
    const areas = Array.isArray(raw.areas)
      ? raw.areas
          .map((row) =>
            Array.isArray(row)
              ? row.map((entry) => String(entry || "").trim()).filter(Boolean)
              : []
          )
          .filter((row) => row.length > 0)
      : [];
    const gap = nonEmptyString(raw.gap);
    const padding = nonEmptyString(raw.padding);
    if (!columns.length && !rows.length && !areas.length && !gap && !padding) return null;
    return {
      columns,
      rows,
      areas,
      gap,
      padding,
    };
  }

  function inferSceneShellLayoutMode(zones = []) {
    const roles = new Set(
      (Array.isArray(zones) ? zones : []).map((zone) => nonEmptyString(zone?.role)).filter(Boolean),
    );
    if (roles.has("tab_bar") && roles.has("tab_content")) return "generic_tabs";
    if (roles.has("row_preview")) return "list_preview";
    if (roles.has("filter") && roles.has("slots")) return "analytics";
    return "";
  }

  function normalizeSceneShellZone(raw, parent = "") {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
    const props = raw.props && typeof raw.props === "object" && !Array.isArray(raw.props) ? raw.props : {};
    const slot = raw.slot && typeof raw.slot === "object" && !Array.isArray(raw.slot) ? raw.slot : {};
    const id = nonEmptyString(raw.id, props.projection_id, props.zone_id);
    const role = nonEmptyString(
      raw.role,
      slot.kind,
      slot.role,
      props.projection_role,
      props.zone_role,
    );
    if (!id || !role) return null;
    const accepts = Array.isArray(raw.accepts)
      ? raw.accepts.map((entry) => nonEmptyString(entry)).filter(Boolean)
      : Array.isArray(slot.accepts)
        ? slot.accepts.map((entry) => nonEmptyString(entry)).filter(Boolean)
        : Array.isArray(props.projection_accepts)
          ? props.projection_accepts.map((entry) => nonEmptyString(entry)).filter(Boolean)
          : [];
    return {
      id,
      role,
      area: nonEmptyString(raw.area, props.area),
      parent,
      source: nonEmptyString(slot.source, props.projection_source, props.source),
      selectionSource: nonEmptyString(
        slot.selection_from,
        slot.selectionFrom,
        props.selection_source,
        props.selectionSource,
      ),
      required: boolValue(raw.required, slot.required, props.projection_required, false),
      max: positiveInt(slot.max, props.projection_max, props.max),
      accepts,
      layout: normalizeShellLayout(raw.layout),
    };
  }

  function collectTopLevelLayoutAreas(layout) {
    const allowed = new Set();
    if (!layout || !Array.isArray(layout.areas)) {
      return allowed;
    }
    layout.areas.forEach((row) => {
      (Array.isArray(row) ? row : []).forEach((area) => {
        const name = String(area || "").trim();
        if (name && name !== ".") {
          allowed.add(name);
        }
      });
    });
    return allowed;
  }

  /** Align with Rust retain_shell_zones_matching_layout: keep nested slot zones under containers. */
  function retainZonesMatchingLayout(layout, zones) {
    if (!layout || !Array.isArray(layout.areas) || !layout.areas.length) {
      return zones;
    }
    const allowed = collectTopLevelLayoutAreas(layout);
    if (!allowed.size) {
      return zones;
    }
    const keptIds = new Set();
    (Array.isArray(zones) ? zones : []).forEach((zone) => {
      const area = nonEmptyString(zone?.area);
      const id = nonEmptyString(zone?.id);
      if (area && allowed.has(area) && id) {
        keptIds.add(id);
      }
    });
    let changed = true;
    while (changed) {
      changed = false;
      (Array.isArray(zones) ? zones : []).forEach((zone) => {
        const id = nonEmptyString(zone?.id);
        if (!id || !keptIds.has(id) || zone?.role !== "container") {
          return;
        }
        const nestedAreas = collectTopLevelLayoutAreas(zone?.layout);
        if (!nestedAreas.size) {
          return;
        }
        (Array.isArray(zones) ? zones : []).forEach((candidate) => {
          const candidateArea = nonEmptyString(candidate?.area);
          const candidateId = nonEmptyString(candidate?.id);
          if (!candidateId || !candidateArea || !nestedAreas.has(candidateArea)) {
            return;
          }
          if (!keptIds.has(candidateId)) {
            keptIds.add(candidateId);
            changed = true;
          }
        });
      });
    }
    changed = true;
    while (changed) {
      changed = false;
      (Array.isArray(zones) ? zones : []).forEach((zone) => {
        const id = nonEmptyString(zone?.id);
        const parent = nonEmptyString(zone?.parent);
        if (!id || keptIds.has(id) || !parent || !keptIds.has(parent)) {
          return;
        }
        keptIds.add(id);
        changed = true;
      });
    }
    return (Array.isArray(zones) ? zones : []).filter((zone) => keptIds.has(nonEmptyString(zone?.id)));
  }

  function normalizeSceneShellContract(rawFrame, rawPanels, rawContract = null) {
    const frame =
      rawFrame && typeof rawFrame === "object" && !Array.isArray(rawFrame) ? rawFrame : null;
    const panels = Array.isArray(rawPanels) ? rawPanels : [];
    const explicitContract =
      rawContract && typeof rawContract === "object" && !Array.isArray(rawContract) ? rawContract : null;
    const zones = [];
    if (Array.isArray(explicitContract?.zones)) {
      explicitContract.zones.forEach((zone) => {
        const normalized = normalizeSceneShellZone(zone, nonEmptyString(zone?.parent));
        if (normalized) {
          zones.push(normalized);
        }
      });
    }
    const collectZones = (items, parent = "") => {
      (Array.isArray(items) ? items : []).forEach((item) => {
        if (!item || typeof item !== "object" || Array.isArray(item) || item.kind !== "panel") return;
        const zone = normalizeSceneShellZone(item, parent);
        if (zone) {
          zones.push(zone);
        }
        const childPanels = Array.isArray(item.blocks)
          ? item.blocks.filter((block) => block && typeof block === "object" && block.kind === "panel")
          : [];
        collectZones(childPanels, nonEmptyString(item.id));
      });
    };
    if (!zones.length) {
      collectZones(panels);
    }
    const layout =
      normalizeShellLayout(explicitContract?.layout) || normalizeShellLayout(frame?.layout);
    const filteredZones = retainZonesMatchingLayout(layout, zones);
    const layoutMode =
      nonEmptyString(explicitContract?.layout_mode, explicitContract?.layoutMode) ||
      inferSceneShellLayoutMode(filteredZones);
    if (!layoutMode && !filteredZones.length && !layout) return null;
    return {
      layoutMode,
      layout,
      zones: filteredZones,
      overlaySize: nonEmptyString(
        explicitContract?.overlay_size,
        explicitContract?.overlaySize,
      ),
    };
  }

  function normalizeSceneLocalNav(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
    const itemsRaw = Array.isArray(raw.items)
      ? raw.items
      : Array.isArray(raw.tabs)
        ? raw.tabs
        : [];
    const items = itemsRaw
      .filter((entry) => entry && typeof entry === "object")
      .map((entry) => ({
        id: normalizeTabId(entry.id || entry.tab || entry.key),
        kind: normalizeTabId(entry.kind || entry.role || entry.id || entry.tab || entry.key),
        role: nonEmptyString(entry.role),
        label: nonEmptyString(entry.label),
      }))
      .filter((entry) => entry.id || entry.kind);
    const kindOrder = Array.from(
      new Set(
        [
          ...(Array.isArray(raw.order_by_kind) ? raw.order_by_kind : []),
          ...(Array.isArray(raw.kind_order) ? raw.kind_order : []),
          ...(Array.isArray(raw.kindOrder) ? raw.kindOrder : []),
          ...items.map((entry) => entry.kind || entry.id),
        ]
          .map((entry) => normalizeTabId(entry))
          .filter(Boolean),
      ),
    );
    const overlaySize = normalizeOverlaySize(
      nonEmptyString(raw.overlay_size, raw.overlaySize),
      "",
    );
    const kind = nonEmptyString(raw.kind);
    const sceneId = nonEmptyString(raw.scene_id, raw.sceneId);
    if (!items.length && !kindOrder.length && !overlaySize && !kind && !sceneId) return null;
    const rowDrilldownPopup = raw.row_drilldown_popup ?? raw.rowDrilldownPopup ?? null;
    const rowDrilldown = raw.row_drilldown ?? raw.rowDrilldown ?? null;
    return {
      kind,
      sceneId,
      hostMode: nonEmptyString(raw.host_mode, raw.hostMode),
      defaultEntry: normalizeTabId(nonEmptyString(raw.default_entry, raw.defaultEntry, raw.defaultEntryTab)),
      includeHero: boolValue(raw.include_hero, raw.includeHero, true),
      overlaySize,
      items,
      kindOrder,
      ...(rowDrilldownPopup && typeof rowDrilldownPopup === "object" && !Array.isArray(rowDrilldownPopup)
        ? { rowDrilldownPopup, row_drilldown_popup: rowDrilldownPopup }
        : {}),
      ...(rowDrilldown && typeof rowDrilldown === "object" && !Array.isArray(rowDrilldown)
        ? { rowDrilldown, row_drilldown: rowDrilldown }
        : {}),
    };
  }

  const DRILLDOWN_OVERLAY_SIZE_CLASSES = [
    "access-drilldown-overlay--size-comfortable",
    "access-drilldown-overlay--size-large",
    "access-drilldown-overlay--size-fullscreen",
  ];
  const DRILLDOWN_PANEL_SIZE_CLASSES = [
    "access-drilldown-overlay-panel--size-comfortable",
    "access-drilldown-overlay-panel--size-large",
    "access-drilldown-overlay-panel--size-fullscreen",
  ];

  function normalizeOverlaySize(raw, fallback = "comfortable") {
    const value = String(raw || fallback || "comfortable")
      .trim()
      .toLowerCase();
    if (value === "large" || value === "fullscreen" || value === "comfortable") {
      return value;
    }
    if (value === "full" || value === "max" || value === "maximum") {
      return "fullscreen";
    }
    if (value === "medium" || value === "default" || value === "moderate") {
      return "comfortable";
    }
    return fallback ? normalizeOverlaySize(fallback, "comfortable") : "comfortable";
  }

  function resolveDrilldownOverlaySize({ popup, boardFields, structuredBoard, sceneShell = null }) {
    const sceneRef =
      popup?.scene && typeof popup.scene === "object" && !Array.isArray(popup.scene) ? popup.scene : {};
    const localNav =
      boardFields?.localNav ||
      normalizeSceneLocalNav(
        popup?.local_nav ||
          popup?.localNav ||
          sceneRef?.local_nav ||
          sceneRef?.localNav,
      );
    const explicit = nonEmptyString(
      popup?.overlay_size,
      popup?.overlaySize,
      sceneRef?.overlay_size,
      sceneRef?.overlaySize,
      localNav?.overlaySize,
    );
    const largeDefault = structuredBoard && sceneShell?.layoutMode !== "generic_tabs";
    if (explicit) {
      return normalizeOverlaySize(explicit, largeDefault ? "large" : "comfortable");
    }
    return largeDefault ? "large" : "comfortable";
  }

  function applyDrilldownOverlaySize(root, config) {
    const overlayEl = root.classList.contains("access-drilldown-overlay")
      ? root
      : root.closest(".access-drilldown-overlay");
    const panelEl = root.querySelector(".access-drilldown-overlay-panel");
    const size = normalizeOverlaySize(
      config?.overlaySize,
      config?.structuredBoard && config?.sceneShell?.layoutMode !== "generic_tabs"
        ? "large"
        : "comfortable",
    );
    if (overlayEl instanceof HTMLElement) {
      overlayEl.classList.remove(...DRILLDOWN_OVERLAY_SIZE_CLASSES);
      overlayEl.classList.add(`access-drilldown-overlay--size-${size}`);
    }
    if (panelEl instanceof HTMLElement) {
      panelEl.classList.remove(...DRILLDOWN_PANEL_SIZE_CLASSES);
      panelEl.classList.add(`access-drilldown-overlay-panel--size-${size}`);
      panelEl.dataset.drilldownOverlaySize = size;
    }
  }

  function resolveSceneLocalNav(sceneFile, runtimeMap = null) {
    const normalized = normalizeDrilldownScenePath(sceneFile);
    if (!normalized) return null;
    if (runtimeMap && typeof runtimeMap === "object" && !Array.isArray(runtimeMap)) {
      const dynamic = normalizeSceneLocalNav(runtimeMap[normalized]);
      if (dynamic) return dynamic;
    }
    return normalizeSceneLocalNav(SCENE_LOCAL_NAV_BY_FILE[normalized]);
  }

  function sceneLocalNavTabIds(localNav) {
    if (!localNav || !Array.isArray(localNav.items)) return [];
    return localNav.items
      .map((entry) => normalizeTabId(entry?.id))
      .filter((tab) => tab && tab !== "hero");
  }

