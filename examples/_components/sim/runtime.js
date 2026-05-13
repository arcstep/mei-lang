const RUNTIME_STORES = new Map();

export function parseProps(element) {
  try {
    return JSON.parse(element.dataset.props || "{}");
  } catch {
    return {};
  }
}

export function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function resolveContract(props) {
  return props.scene || props.scene_contract || props || {};
}

function storeKey(host, contract) {
  const sceneId = contract?.scene?.id || "scene";
  const stepApi = host?.step_api || "local";
  return `${stepApi}::${sceneId}`;
}

function normalizeActions(view, state, contract) {
  if (Array.isArray(view?.available_actions) && view.available_actions.length > 0) {
    return view.available_actions;
  }
  const phase = state?.phase || view?.phase || contract?.scene?.state?.phase || "ready";
  const paused = !!state?.clock?.paused;
  if (phase === "ready") {
    return ["start"];
  }
  if (phase === "running") {
    return ["tick", "rate_half", "rate_normal", "rate_double", paused ? "resume" : "pause"];
  }
  return ["restart"];
}

function interactionTargetForCell(contract, cellId) {
  const interactions = contract?.flow?.interactions || [];
  const prefixed = `cell:${cellId}`;
  const rule = interactions.find((entry) => entry.target === prefixed || entry.target === cellId);
  return rule ? rule.target : null;
}

function keyTargetForCell(contract, cellId) {
  const interactions = contract?.flow?.interactions || [];
  const prefixed = `cell:${cellId}`;
  return interactions.some((entry) => {
    if (!(entry.target === prefixed || entry.target === cellId)) {
      return false;
    }
    return entry.require?.type === "has";
  });
}

function entityInteractionTarget(contract, entityId) {
  const interactions = contract?.flow?.interactions || [];
  const rule = interactions.find((entry) => entry.target === entityId);
  return rule ? rule.target : null;
}

function interactionAvailable(rule, state) {
  if (!rule?.require) {
    return true;
  }
  if (rule.require.type === "has") {
    return (state?.inventory || []).includes(rule.require.value);
  }
  return true;
}

function timerRemainingForCell(state, cellId) {
  const subjectRef = `cell:${cellId}`;
  const timers = state?.subject_timers || [];
  const now = Number(state?.clock?.current_time || 0);
  const remaining = timers
    .filter((timer) => timer.subject_ref === subjectRef)
    .map((timer) => Math.max(0, Number(timer.due_at || 0) - now));
  if (remaining.length === 0) {
    return null;
  }
  return Math.min(...remaining);
}

function fallbackSceneView(contract, state) {
  const scene = contract.scene || {};
  const world = contract.world || {};
  const entities = (world.entities || []).map((entity) => {
    const inInventory = (state?.inventory || []).includes(entity.id);
    const entityRule = (contract.flow?.interactions || []).find((entry) => entry.target === entity.id);
    const interactionTarget = entityRule ? entityRule.target : null;
    return {
      id: entity.id,
      kind: entity.kind,
      label: entity.label,
      slot: inInventory ? null : state?.placements?.[entity.id] || null,
      status: state?.statuses?.[entity.id] || entity.status || null,
      interaction_target: interactionTarget,
      clickable: Boolean(interactionTarget) && interactionAvailable(entityRule, state),
      in_inventory: inInventory,
      flags: {},
    };
  });

  const rows = Number(world.topology?.rows || 0);
  const cols = Number(world.topology?.cols || 0);
  const declaredCells = new Map((world.topology?.cells || []).map((cell) => [cell.id, cell]));
  const cells = [];

  for (let row = 1; row <= rows; row += 1) {
    for (let col = 1; col <= cols; col += 1) {
      const id = `r${row}c${col}`;
      const declared = declaredCells.get(id) || {};
      const statusKey = `cell:${id}`;
      const runtimeHazard = state?.statuses?.[statusKey] || null;
      const timerRemaining = timerRemainingForCell(state, id);
      const cellRule = (contract.flow?.interactions || []).find((entry) => entry.target === statusKey || entry.target === id);
      const interactionTarget = cellRule ? cellRule.target : null;
      cells.push({
        id,
        surface_kind: declared.surface_kind || null,
        flammable: typeof declared.flammable === "boolean" ? declared.flammable : null,
        walkable: typeof declared.walkable === "boolean" ? declared.walkable : null,
        occupiable: typeof declared.occupiable === "boolean" ? declared.occupiable : null,
        hazard_state: runtimeHazard || declared.hazard_state || null,
        hazard_timer_remaining: timerRemaining,
        hazard_timer_seconds: timerRemaining === null ? null : Math.ceil(timerRemaining),
        interaction_target: interactionTarget,
        clickable: Boolean(interactionTarget) && interactionAvailable(cellRule, state),
        key_target: keyTargetForCell(contract, id),
        tags: declared.tags || [],
        entities: entities.filter((entity) => entity.slot === id && !entity.in_inventory),
      });
    }
  }

  const view = {
    scene_id: scene.id || "scene",
    goal: scene.goal || null,
    profile: scene.profile || null,
    summary: scene.summary || null,
    phase: state?.phase || scene.state?.phase || "ready",
    result: state?.result || "ready",
    reason: state?.reason || null,
    outcome_state: state?.phase || scene.state?.phase || "ready",
    outcome_message: state?.reason || null,
    countdown: state?.countdown ?? scene.state?.countdown ?? 0,
    current_time: state?.clock?.current_time ?? 0,
    time_unit: state?.clock?.time_unit || "second",
    clock_paused: !!state?.clock?.paused,
    time_rate: state?.clock?.rate ?? 1,
    inventory: state?.inventory || [],
    entities,
    cells,
    subject_timers: state?.subject_timers || [],
    available_actions: [],
    start_label: contract.flow?.start?.action_label || "开始",
  };
  view.available_actions = normalizeActions(view, state, contract);
  return view;
}

function createRuntimeStore(host, contract) {
  let currentHost = host || {};
  let currentContract = contract || {};
  let runtimeState = null;
  let sceneView = null;
  let error = null;
  let loading = false;
  let autoTickHandle = null;
  let synced = false;
  const listeners = new Set();

  const clearAutoTick = () => {
    if (autoTickHandle !== null) {
      clearTimeout(autoTickHandle);
      autoTickHandle = null;
    }
  };

  const snapshot = () => {
    const projected = sceneView || fallbackSceneView(currentContract, runtimeState);
    projected.available_actions = normalizeActions(projected, runtimeState, currentContract);
    return {
      host: currentHost,
      contract: currentContract,
      runtimeState,
      sceneView: projected,
      loading,
      error,
    };
  };

  const scheduleAutoTick = (snap) => {
    clearAutoTick();
    if (!currentHost.step_api || snap.loading || snap.error) {
      return;
    }
    if (!snap.sceneView || snap.sceneView.phase !== "running" || snap.sceneView.clock_paused) {
      return;
    }
    autoTickHandle = setTimeout(() => {
      autoTickHandle = null;
      sendIntent({ kind: "tick" });
    }, 1000);
  };

  const notify = () => {
    const snap = snapshot();
    listeners.forEach((listener) => listener(snap));
    scheduleAutoTick(snap);
  };

  const sendIntent = async (intent) => {
    if (!currentHost.step_api) {
      error = "缺少 step_api，无法进入运行态。";
      notify();
      return;
    }
    loading = true;
    error = null;
    notify();
    try {
      const payload = { intent };
      if (runtimeState) {
        payload.state = runtimeState;
      }
      const response = await fetch(currentHost.step_api, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!response.ok) {
        throw new Error(`step 请求失败: ${response.status}`);
      }
      const data = await response.json();
      runtimeState = data.state || null;
      sceneView = data.scene_view || null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
      notify();
    }
  };

  const ensureSync = () => {
    if (synced) {
      return;
    }
    synced = true;
    if (currentHost.step_api) {
      sendIntent({ kind: "sync" });
    } else {
      notify();
    }
  };

  const subscribe = (listener) => {
    listeners.add(listener);
    listener(snapshot());
    ensureSync();
    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) {
        clearAutoTick();
      }
    };
  };

  const updateContext = (hostUpdate, contractUpdate) => {
    currentHost = hostUpdate || {};
    currentContract = contractUpdate || {};
    notify();
    ensureSync();
  };

  return {
    subscribe,
    sendIntent,
    updateContext,
    snapshot,
  };
}

export function getRuntimeStore(props) {
  const contract = resolveContract(props);
  const host = props._mei || {};
  const key = storeKey(host, contract);
  if (!RUNTIME_STORES.has(key)) {
    RUNTIME_STORES.set(key, createRuntimeStore(host, contract));
  }
  const store = RUNTIME_STORES.get(key);
  store.updateContext(host, contract);
  return store;
}
