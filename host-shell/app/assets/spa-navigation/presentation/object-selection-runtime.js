(() => {
  const root = typeof window !== "undefined" ? window : globalThis;
  const boot = (root.__meiLangBoot = root.__meiLangBoot || {});
  const SELECT_EVENT = "mei:object-select";
  const CHANGE_EVENT = "mei:object-selection-change";
  const DIAGNOSTIC_EVENT = "mei:object-identity-diagnostic";
  const INTERACTION_EVENT = "mei:interaction-intent";
  const INTERACTION_REQUEST_EVENT = "mei:interaction-request";
  const INTERACTION_RESPONSE_EVENT = "mei:interaction-response";
  const INTERACTION_DIAGNOSTIC_EVENT = "mei:interaction-diagnostic";
  const INTERACTION_SCHEMA = "mei-interaction-v1";
  const INTERACTION_INTENTS = new Set([
    "select",
    "open_projection",
    "explain_metric",
    "filter_query",
    "focus_viewpoint",
  ]);
  const SUPPORTED_MODES = new Set(["replace", "add", "remove", "clear"]);

  let selection = {
    objects: [],
    objectIds: [],
    primaryObjectId: "",
    source: "",
    mode: "replace",
  };

  function normalizeObjectId(value) {
    return String(value || "").trim();
  }

  function readPresentationObjectIndex() {
    const injected = root.__mei?.presentation_map?.objectIndex;
    if (injected && typeof injected === "object") return injected;
    if (typeof document === "undefined") return { descriptors: {}, entries: [] };
    const node = document.getElementById("mei-presentation-map");
    if (
      typeof HTMLScriptElement !== "undefined" &&
      node instanceof HTMLScriptElement &&
      node.textContent
    ) {
      try {
        return JSON.parse(node.textContent)?.objectIndex || { descriptors: {}, entries: [] };
      } catch (_) {
        return { descriptors: {}, entries: [] };
      }
    }
    return { descriptors: {}, entries: [] };
  }

  function identityDiagnostic(code, message, input) {
    const detail = { code, severity: code.includes("legacy") ? "warning" : "error", message };
    if (input && typeof input === "object") detail.input = cloneSecondary(input);
    root.dispatchEvent(new CustomEvent(DIAGNOSTIC_EVENT, { detail }));
    if (boot.presentationDebug && typeof console?.warn === "function") {
      console.warn(`[mei] ${code}: ${message}`, input);
    }
  }

  function readField(value, ...keys) {
    for (const key of keys) {
      if (value && Object.prototype.hasOwnProperty.call(value, key)) return value[key];
    }
    return undefined;
  }

  function scalarKey(value) {
    if (typeof value === "string") return `s:${value}`;
    if (typeof value === "number" && Number.isFinite(value)) return `n:${value}`;
    if (typeof value === "boolean") return `b:${value}`;
    return "";
  }

  function locatorMatches(candidate, requested) {
    const candidateType = String(
      readField(candidate, "objectType", "object_type", "object_type_id") || "",
    ).trim();
    const requestedType = String(
      readField(requested, "objectType", "object_type", "object_type_id") || "",
    ).trim();
    if (!candidateType || candidateType !== requestedType) return false;
    for (const keys of [
      ["objectKey", "object_key"],
      ["entityId", "entity_id"],
    ]) {
      const wanted = readField(requested, ...keys);
      if (wanted === undefined || wanted === null || wanted === "") continue;
      if (scalarKey(readField(candidate, ...keys)) !== scalarKey(wanted)) return false;
    }
    const requestedValues = readField(requested, "identityValues", "identity_values");
    if (requestedValues && typeof requestedValues === "object") {
      const candidateValues =
        readField(candidate, "identityValues", "identity_values") || {};
      if (JSON.stringify(candidateValues) !== JSON.stringify(requestedValues)) return false;
    }
    return true;
  }

  function canonicalDescriptor(value) {
    if (!value || typeof value !== "object") return null;
    const objectId = normalizeObjectId(readField(value, "objectId", "object_id"));
    const objectType = String(
      readField(value, "objectType", "object_type", "object_type_id") || "",
    ).trim();
    if (!objectId || !objectType) return null;
    const descriptor = { objectId, objectType, identityStatus: "canonical" };
    for (const [output, keys] of [
      ["objectKey", ["objectKey", "object_key"]],
      ["entityId", ["entityId", "entity_id"]],
      ["sourceRef", ["sourceRef", "source_ref"]],
      ["identityValues", ["identityValues", "identity_values"]],
      ["label", ["label"]],
    ]) {
      const field = readField(value, ...keys);
      if (field !== undefined && field !== null) descriptor[output] = cloneSecondary(field);
    }
    return descriptor;
  }

  function resolveObject(input) {
    const index = readPresentationObjectIndex();
    const descriptors = index?.descriptors || {};
    const value =
      input && typeof input === "object"
        ? readField(input, "descriptor", "object") || input
        : { objectId: input };
    const objectId = normalizeObjectId(readField(value, "objectId", "object_id"));
    if (objectId && descriptors[objectId]) {
      return canonicalDescriptor(descriptors[objectId]);
    }
    const objectType = String(
      readField(value, "objectType", "object_type", "object_type_id") || "",
    ).trim();
    const hasLocator =
      readField(value, "objectKey", "object_key") != null ||
      readField(value, "entityId", "entity_id") != null ||
      readField(value, "identityValues", "identity_values") != null;
    if (objectType && hasLocator) {
      const matched = (Array.isArray(index?.entries) ? index.entries : []).find((entry) =>
        locatorMatches(entry?.locator, value),
      );
      const resolved = matched && descriptors[normalizeObjectId(matched.objectId)];
      if (resolved) return canonicalDescriptor(resolved);
      identityDiagnostic(
        "object_locator_unresolved",
        "locator 未出现在 host 注入的 ObjectIndex 中，禁止在浏览器端生成 objectId",
        value,
      );
      return null;
    }
    if (objectId) {
      identityDiagnostic(
        "legacy_object_id_read_only",
        "未解析的 objectId 仅按 legacy 兼容读取，不可作为新 author intent",
        value,
      );
      return { objectId, identityStatus: "legacy" };
    }
    if (objectType || hasLocator) {
      identityDiagnostic(
        "object_locator_incomplete",
        "对象选择必须同时提供 objectType 与 objectKey/entityId",
        value,
      );
    }
    return null;
  }

  const resolverApi = {
    resolve: resolveObject,
    get index() {
      return cloneSecondary(readPresentationObjectIndex());
    },
  };
  root.MeiObjectResolver = resolverApi;
  boot.objectResolver = resolverApi;

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
      objects: selection.objects.map(cloneSecondary),
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
    if (JSON.stringify(left.objects) !== JSON.stringify(right.objects)) return false;
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
    const requestedInputs = [
      ...(Array.isArray(detail.objects) ? detail.objects : []),
      ...(Array.isArray(detail.descriptors) ? detail.descriptors : []),
      ...(Array.isArray(detail.objectIds) ? detail.objectIds : []),
    ];
    if (
      detail.descriptor ||
      detail.object ||
      detail.objectId ||
      detail.object_id ||
      detail.objectType ||
      detail.object_type ||
      detail.objectKey != null ||
      detail.object_key != null ||
      detail.entityId != null ||
      detail.entity_id != null
    ) {
      requestedInputs.push(detail);
    }
    const requestedObjects = requestedInputs.map(resolveObject).filter(Boolean);
    const requested = normalizeObjectIds(requestedObjects.map((object) => object.objectId));
    const explicitPrimary = normalizeObjectId(
      detail.primaryObjectId || detail.primary_object_id,
    );
    if ((mode === "replace" || mode === "add") && explicitPrimary && !requested.includes(explicitPrimary)) {
      const primaryObject = resolveObject({ objectId: explicitPrimary });
      if (primaryObject) {
        requestedObjects.push(primaryObject);
        requested.push(explicitPrimary);
      }
    }

    let objects;
    if (mode === "clear") {
      objects = [];
    } else if (mode === "add") {
      const byId = new Map(selection.objects.map((object) => [object.objectId, object]));
      requestedObjects.forEach((object) => byId.set(object.objectId, object));
      objects = Array.from(byId.values());
    } else if (mode === "remove") {
      const removed = new Set(requested);
      objects = selection.objects.filter((object) => !removed.has(object.objectId));
    } else {
      objects = requestedObjects.filter(
        (object, index, values) =>
          values.findIndex((candidate) => candidate.objectId === object.objectId) === index,
      );
    }
    const objectIds = objects.map((object) => object.objectId);

    const primaryObjectId =
      mode !== "clear" && explicitPrimary && objectIds.includes(explicitPrimary)
        ? explicitPrimary
        : objectIds.includes(selection.primaryObjectId)
          ? selection.primaryObjectId
          : objectIds[0] || "";
    const next = {
      objects,
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
    if (event?.detail?.__meiInteractionBridge) return;
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

  let objectSet = null;
  const responderHandlers = new Map();
  const interactionSubscribers = new Set();

  function interactionDiagnostic(code, message, input) {
    const detail = { code, severity: "error", message };
    if (input && typeof input === "object") detail.input = cloneSecondary(input);
    root.dispatchEvent(new CustomEvent(INTERACTION_DIAGNOSTIC_EVENT, { detail }));
    if (boot.presentationDebug && typeof console?.warn === "function") {
      console.warn(`[mei] ${code}: ${message}`, input);
    }
  }

  function nonEmpty(value) {
    const text = String(value || "").trim();
    return text || "";
  }

  function normalizeFocus(input) {
    const candidate =
      input?.kind === "object_focus"
        ? input.focus
        : input?.objectFocus || input?.focus || input;
    const requested = [
      ...(Array.isArray(candidate?.objects) ? candidate.objects : []),
      ...(Array.isArray(candidate?.descriptors) ? candidate.descriptors : []),
    ];
    if (
      candidate?.descriptor ||
      candidate?.object ||
      candidate?.objectId ||
      candidate?.object_id ||
      candidate?.objectType ||
      candidate?.object_type ||
      candidate?.objectKey != null ||
      candidate?.entityId != null
    ) {
      requested.push(candidate);
    }
    const objects = requested
      .map(resolveObject)
      .filter(Boolean)
      .filter(
        (object, index, values) =>
          values.findIndex((entry) => entry.objectId === object.objectId) === index,
      );
    if (!objects.length) return null;
    const requestedPrimary = nonEmpty(
      candidate?.primaryObjectId || candidate?.primary_object_id,
    );
    const primaryObjectId = objects.some((object) => object.objectId === requestedPrimary)
      ? requestedPrimary
      : objects[0].objectId;
    return {
      cardinality: objects.length === 1 ? "single" : "multiple",
      objects,
      primaryObjectId,
    };
  }

  function normalizeObjectSet(input) {
    const candidate =
      input?.kind === "object_set" ? input.set : input?.objectSet || input?.set || input;
    if (!candidate || typeof candidate !== "object") return null;
    if (candidate.objectId || candidate.object_id || candidate.objectIds) {
      interactionDiagnostic(
        "object_set_object_id_forbidden",
        "ObjectSet 表示 query/metric/source 集合，不能伪装成 objectId",
        candidate,
      );
      return null;
    }
    const objectType = nonEmpty(
      candidate.objectType || candidate.object_type || candidate.object_type_id,
    );
    const query = candidate.query ?? candidate.filterQuery ?? candidate.filter_query;
    const metric = nonEmpty(candidate.metric || candidate.metricId || candidate.metric_id);
    const sourceRef = candidate.sourceRef || candidate.source_ref;
    if (!objectType || (query == null && !metric && sourceRef == null)) return null;
    const set = { objectType };
    if (query != null) set.query = cloneSecondary(query);
    if (metric) set.metric = metric;
    if (sourceRef != null) set.sourceRef = cloneSecondary(sourceRef);
    return set;
  }

  function interactionObjectType(subject) {
    if (subject?.kind === "object_focus") {
      return nonEmpty(subject.focus?.objects?.[0]?.objectType);
    }
    if (subject?.kind === "object_set") return nonEmpty(subject.set?.objectType);
    return "";
  }

  function readDeclaredResponders() {
    const fromBoot = root.__mei?.presentation_map?.responders;
    if (Array.isArray(fromBoot)) return fromBoot;
    if (typeof document === "undefined") return [];
    const node = document.getElementById?.("mei-presentation-map");
    if (
      typeof HTMLScriptElement !== "undefined" &&
      node instanceof HTMLScriptElement &&
      node.textContent
    ) {
      try {
        const responders = JSON.parse(node.textContent)?.responders;
        return Array.isArray(responders) ? responders : [];
      } catch (_) {
        return [];
      }
    }
    return [];
  }

  function normalizeResponder(spec, handler) {
    const objectType = nonEmpty(spec?.objectType || spec?.object_type);
    const role = nonEmpty(spec?.role);
    const intents = (Array.isArray(spec?.intents) ? spec.intents : [spec?.intent])
      .map(nonEmpty)
      .filter((intent) => INTERACTION_INTENTS.has(intent));
    const id = nonEmpty(spec?.id);
    if (!id || !objectType || !role || !intents.length) return null;
    return {
      id,
      objectType,
      role,
      intents,
      target: cloneSecondary(spec.target),
      refreshOnSelect: Boolean(spec.refreshOnSelect || spec.refresh_on_select),
      handler: typeof handler === "function" ? handler : null,
    };
  }

  function matchingResponders(event) {
    const objectType = interactionObjectType(event.subject);
    if (!objectType) return [];
    const values = readDeclaredResponders()
      .map((spec) => normalizeResponder(spec, null))
      .filter(Boolean);
    responderHandlers.forEach((registered) => {
      const declaredIndex = values.findIndex((value) => value.id === registered.id);
      if (declaredIndex >= 0) values[declaredIndex] = registered;
      else values.push(registered);
    });
    return values.filter((responder) => {
      if (responder.objectType !== objectType || !responder.intents.includes(event.intent)) {
        return false;
      }
      if (event.targetId && responder.id !== event.targetId) return false;
      if (event.targetRole && responder.role !== event.targetRole) return false;
      return true;
    });
  }

  function routeInteraction(event) {
    const responders = matchingResponders(event);
    if (!responders.length) return false;
    if (responders.length > 1) {
      interactionDiagnostic(
        "responder_target_ambiguous",
        `intent \`${event.intent}\` 匹配到 ${responders.length} 个同优先级 Responder，已安静停止路由`,
        event,
      );
      return false;
    }
    const responder = responders[0];
    const response = { event: cloneSecondary(event), responder: cloneSecondary(responder) };
    delete response.responder.handler;
    if (responder.handler) responder.handler(cloneSecondary(event), response.responder);
    root.dispatchEvent(new CustomEvent(INTERACTION_RESPONSE_EVENT, { detail: response }));
    return true;
  }

  function normalizeInteractionEvent(intentOrEvent, input = {}) {
    const raw =
      intentOrEvent && typeof intentOrEvent === "object"
        ? intentOrEvent
        : { ...input, intent: intentOrEvent };
    const intent = nonEmpty(raw.intent);
    if (!INTERACTION_INTENTS.has(intent)) return null;
    let subject = raw.subject;
    if (subject?.kind === "object_focus") {
      const focus = normalizeFocus(subject);
      subject = focus ? { kind: "object_focus", focus } : null;
    } else if (subject?.kind === "object_set") {
      const set = normalizeObjectSet(subject);
      subject = set ? { kind: "object_set", set } : null;
    } else if (intent === "explain_metric" || intent === "filter_query") {
      const set = normalizeObjectSet(raw);
      subject = set ? { kind: "object_set", set } : null;
    } else {
      const focus = normalizeFocus(raw);
      subject = focus ? { kind: "object_focus", focus } : null;
    }
    return {
      schemaVersion: INTERACTION_SCHEMA,
      intent,
      ...(subject ? { subject } : {}),
      ...(nonEmpty(raw.source) ? { source: nonEmpty(raw.source) } : {}),
      ...(nonEmpty(raw.targetId || raw.target_id)
        ? { targetId: nonEmpty(raw.targetId || raw.target_id) }
        : {}),
      ...(nonEmpty(raw.targetRole || raw.target_role)
        ? { targetRole: nonEmpty(raw.targetRole || raw.target_role) }
        : {}),
    };
  }

  function dispatchInteraction(intentOrEvent, input = {}) {
    const event = normalizeInteractionEvent(intentOrEvent, input);
    if (!event) return false;
    if (event.intent === "select" && event.subject?.kind === "object_focus") {
      const focus = event.subject.focus;
      select({
        descriptors: focus.objects,
        primaryObjectId: focus.primaryObjectId,
        source: event.source || "interaction",
        mode: "replace",
      });
      root.dispatchEvent(
        new CustomEvent(SELECT_EVENT, {
          detail: {
            descriptors: focus.objects,
            primaryObjectId: focus.primaryObjectId,
            source: event.source || "interaction",
            mode: "replace",
            __meiInteractionBridge: true,
          },
        }),
      );
    }
    if (
      (event.intent === "explain_metric" || event.intent === "filter_query") &&
      event.subject?.kind === "object_set"
    ) {
      objectSet = cloneSecondary(event.subject.set);
    }
    root.dispatchEvent(new CustomEvent(INTERACTION_EVENT, { detail: cloneSecondary(event) }));
    interactionSubscribers.forEach((subscriber) => subscriber(cloneSecondary(event)));
    routeInteraction(event);
    return cloneSecondary(event);
  }

  function dispatchMany(intents, detail = {}) {
    return (Array.isArray(intents) ? intents : [intents])
      .map((intent) => dispatchInteraction(intent, detail))
      .filter(Boolean);
  }

  function readDataJson(value) {
    const text = nonEmpty(value);
    if (!text) return undefined;
    try {
      return JSON.parse(text);
    } catch (_) {
      return text;
    }
  }

  function interactionDetailFromElement(element) {
    const data = element?.dataset || {};
    const detail = {
      source: data.meiInteractionSource || data.meiSource || "data-attribute",
      targetId: data.meiResponderId,
      targetRole: data.meiResponderRole,
      objectType: data.meiObjectType,
      objectId: data.meiObjectId,
      objectKey: readDataJson(data.meiObjectKey),
      entityId: readDataJson(data.meiEntityId),
      metric: data.meiMetric,
      query: readDataJson(data.meiQuery),
      sourceRef: readDataJson(data.meiSourceRef),
    };
    return Object.fromEntries(
      Object.entries(detail).filter(([, value]) => value !== undefined && value !== ""),
    );
  }

  function onInteractionClick(event) {
    const target = event?.target?.closest?.("[data-mei-intent]");
    if (!target) return;
    const intents = nonEmpty(target.dataset?.meiIntent)
      .split(/[,\s]+/u)
      .filter(Boolean);
    dispatchMany(intents, interactionDetailFromElement(target));
  }

  function registerResponder(spec, handler) {
    const responder = normalizeResponder(spec, handler);
    if (!responder) return () => {};
    responderHandlers.set(responder.id, responder);
    return () => responderHandlers.delete(responder.id);
  }

  const interactionApi = {
    intents: Object.freeze([...INTERACTION_INTENTS]),
    dispatch: dispatchInteraction,
    dispatchMany,
    subscribe(listener) {
      if (typeof listener !== "function") return () => {};
      interactionSubscribers.add(listener);
      return () => interactionSubscribers.delete(listener);
    },
    registerResponder,
    getState() {
      return {
        focus: snapshot(),
        objectSet: cloneSecondary(objectSet),
      };
    },
  };

  root.addEventListener(INTERACTION_REQUEST_EVENT, (event) => {
    const detail = event?.detail || {};
    if (Array.isArray(detail.intents)) dispatchMany(detail.intents, detail);
    else dispatchInteraction(detail);
  });
  if (typeof document !== "undefined") {
    document.addEventListener?.("click", onInteractionClick);
  }
  root.MeiInteraction = interactionApi;
  boot.interactionRuntime = interactionApi;
})();
