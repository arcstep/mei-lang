(() => {
  const root = typeof window !== "undefined" ? window : globalThis;
  const boot = (root.__meiLangBoot = root.__meiLangBoot || {});
  const SELECT_EVENT = "mei:object-select";
  const CHANGE_EVENT = "mei:object-selection-change";
  const SUPPORTED_MODES = new Set(["replace", "add", "remove", "clear"]);

  let selection = {
    objectIds: [],
    primaryObjectId: "",
    source: "",
    mode: "replace",
  };

  function normalizeObjectId(value) {
    return String(value || "").trim();
  }

  function hasOwn(value, key) {
    return Object.prototype.hasOwnProperty.call(value, key);
  }

  function normalizeObjectIds(value) {
    const values = Array.isArray(value) ? value : value == null ? [] : [value];
    const seen = new Set();
    return values
      .map(normalizeObjectId)
      .filter((objectId) => objectId && !seen.has(objectId) && seen.add(objectId));
  }

  function cloneSecondary(value) {
    if (value === undefined) return undefined;
    if (typeof structuredClone === "function") {
      try {
        return structuredClone(value);
      } catch (_) {
        /* fall through */
      }
    }
    if (Array.isArray(value)) return value.map(cloneSecondary);
    if (value && typeof value === "object") {
      return Object.fromEntries(
        Object.entries(value).map(([key, entry]) => [key, cloneSecondary(entry)]),
      );
    }
    return value;
  }

  function snapshot() {
    const current = {
      objectIds: selection.objectIds.slice(),
      primaryObjectId: selection.primaryObjectId,
      source: selection.source,
      mode: selection.mode,
    };
    if (hasOwn(selection, "secondary")) {
      current.secondary = cloneSecondary(selection.secondary);
    }
    return current;
  }

  function sameSelection(left, right) {
    if (
      left.primaryObjectId !== right.primaryObjectId ||
      left.source !== right.source ||
      left.mode !== right.mode ||
      left.objectIds.length !== right.objectIds.length
    ) {
      return false;
    }
    if (left.objectIds.some((objectId, index) => objectId !== right.objectIds[index])) {
      return false;
    }
    return JSON.stringify(left.secondary) === JSON.stringify(right.secondary);
  }

  function dispatchChange() {
    root.dispatchEvent(
      new CustomEvent(CHANGE_EVENT, {
        detail: snapshot(),
      }),
    );
  }

  function select(input = {}) {
    const detail = input && typeof input === "object" ? input : { objectId: input };
    const modeValue = String(detail.mode || "replace").trim().toLowerCase();
    const mode = SUPPORTED_MODES.has(modeValue) ? modeValue : "replace";
    const source = String(detail.source || "").trim();
    const requested = normalizeObjectIds([
      ...(Array.isArray(detail.objectIds) ? detail.objectIds : []),
      detail.objectId,
      detail.object_id,
    ]);
    const explicitPrimary = normalizeObjectId(
      detail.primaryObjectId || detail.primary_object_id,
    );
    if ((mode === "replace" || mode === "add") && explicitPrimary && !requested.includes(explicitPrimary)) {
      requested.push(explicitPrimary);
    }

    let objectIds;
    if (mode === "clear") {
      objectIds = [];
    } else if (mode === "add") {
      objectIds = normalizeObjectIds([...selection.objectIds, ...requested]);
    } else if (mode === "remove") {
      const removed = new Set(requested);
      objectIds = selection.objectIds.filter((objectId) => !removed.has(objectId));
    } else {
      objectIds = requested;
    }

    const primaryObjectId =
      mode !== "clear" && explicitPrimary && objectIds.includes(explicitPrimary)
        ? explicitPrimary
        : objectIds.includes(selection.primaryObjectId)
          ? selection.primaryObjectId
          : objectIds[0] || "";
    const next = {
      objectIds,
      primaryObjectId,
      source,
      mode,
    };
    if (mode !== "clear") {
      if (hasOwn(detail, "secondary")) {
        next.secondary = cloneSecondary(detail.secondary);
      } else if (mode !== "replace" && hasOwn(selection, "secondary")) {
        next.secondary = cloneSecondary(selection.secondary);
      }
    }

    const previous = snapshot();
    selection = next;
    if (!sameSelection(previous, next)) {
      dispatchChange();
    }
    return snapshot();
  }

  function onObjectSelect(event) {
    select(event?.detail || {});
  }

  function install() {
    if (boot.objectSelectionRuntimeMounted) return api;
    boot.objectSelectionRuntimeMounted = true;
    root.addEventListener(SELECT_EVENT, onObjectSelect);
    return api;
  }

  const api = {
    boot: install,
    getSelection: snapshot,
    select,
    get selection() {
      return snapshot();
    },
    replace(detail = {}) {
      return select({ ...detail, mode: "replace" });
    },
    add(detail = {}) {
      return select({ ...detail, mode: "add" });
    },
    remove(detail = {}) {
      return select({ ...detail, mode: "remove" });
    },
    clear(detail = {}) {
      return select({ ...detail, mode: "clear" });
    },
  };

  root.MeiObjectSelection = api;
  boot.objectSelectionRuntime = api;
  boot.bootObjectSelectionRuntime = install;
  install();
})();
