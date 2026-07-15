/**
 * Thunder T2 打开辅助：从 JS 组件派发 mei:scene-open / mei:metric-drilldown，
 * 对齐 link_decl 打开的 page_instance（analytics_frame）。
 * eventId 优先取当前 selectedEventId，回落 catalog 默认 / 260709-01。
 */
import {
  ANALYSIS_OPEN_EVENT_NAME,
  DRILLDOWN_EVENT_NAME,
  POPUP_OPEN_EVENT_NAME,
  SCENE_OPEN_EVENT_NAME,
} from "../cockpit/drilldown-meta.js";
import { getThunderStore } from "./event-bus.js";

/** catalog 事件 id → T2 fixture 目录键（P0 多数事件共用默认 EVT 明细） */
export const EVENT_FIXTURE_KEY = {
  "260709-01": "EVT-20260709-01",
  "260708-17": "EVT-20260709-01",
  "260707-09": "EVT-20260709-01",
};

export const T2_BOARDS = {
  lifecycle: {
    sceneId: "warning_lifecycle_page",
    title: "预警生命周期",
    metricId: "lifecycle_node_count",
    rowsetDatasetId: "lifecycle_nodes",
    needsEvent: true,
  },
  collection: {
    sceneId: "collection_health_page",
    title: "采集健康与来源档案",
    metricId: "collection_site_count",
    rowsetDatasetId: "collection_sites",
    needsEvent: false,
  },
  volume: {
    sceneId: "collection_volume_page",
    title: "采集量构成",
    metricId: "collection_volume_count",
    rowsetDatasetId: "collection_volume_rows",
    needsEvent: false,
  },
  lightning: {
    sceneId: "lightning_in_event_page",
    title: "事件内闪电",
    metricId: "lightning_in_event_count",
    rowsetDatasetId: "lightning_events",
    needsEvent: true,
  },
  efield: {
    sceneId: "efield_in_event_page",
    title: "电场采样",
    metricId: "efield_sample_count",
    rowsetDatasetId: "efield_samples",
    needsEvent: true,
  },
  optical: {
    sceneId: "optical_in_event_page",
    title: "光学帧旁证",
    metricId: "optical_frame_count",
    rowsetDatasetId: "optical_frames",
    needsEvent: true,
  },
};

function nonEmpty(...values) {
  for (const value of values) {
    const text = String(value || "").trim();
    if (text) return text;
  }
  return "";
}

function drilldownContext() {
  if (typeof window === "undefined") return null;
  if (window.__meiSceneDrilldownContext && typeof window.__meiSceneDrilldownContext === "object") {
    return window.__meiSceneDrilldownContext;
  }
  const script = document.getElementById("mei-scene-drilldown-context");
  const raw = String(script?.textContent || "").trim();
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      window.__meiSceneDrilldownContext = parsed;
      return parsed;
    }
  } catch (_) {
    /* ignore */
  }
  return null;
}

function assemblyForScene(sceneId) {
  const ctx = drilldownContext();
  const byId = ctx?.scene_projection_assembly_by_id;
  if (!byId || typeof byId !== "object") return null;
  return byId[sceneId] || null;
}

export function resolveThunderEventId(explicit) {
  const store = getThunderStore();
  return nonEmpty(
    explicit,
    store?.eventId,
    store?.catalog?.defaultEventId,
    "260709-01",
  );
}

export function resolveFixtureKey(eventId) {
  const id = resolveThunderEventId(eventId);
  return EVENT_FIXTURE_KEY[id] || "EVT-20260709-01";
}

/**
 * @param {keyof typeof T2_BOARDS | string} boardKey
 * @param {{ eventId?: string, host?: EventTarget, title?: string, filters?: object }} [options]
 */
export function openThunderT2(boardKey, options = {}) {
  const board = T2_BOARDS[boardKey];
  if (!board) {
    console.warn("[thunder.t2-open] unknown board", boardKey);
    return false;
  }
  const sceneId = board.sceneId;
  const assembly = assemblyForScene(sceneId);
  const sceneFile = nonEmpty(
    assembly?.target_file,
    assembly?.targetFile,
    assembly?.scene_file,
    assembly?.sceneFile,
  );
  if (!sceneFile) {
    console.warn("[thunder.t2-open] missing assembly target_file for", sceneId);
    return false;
  }
  const eventId = board.needsEvent ? resolveThunderEventId(options.eventId) : "";
  const params = {
    rowset_dataset_id: board.rowsetDatasetId,
    ...(eventId ? { eventId } : {}),
  };
  const popup = {
    kind: "scene_open",
    mode: "popup",
    type: "popup",
    projection: "overlay",
    overlay_size: "large",
    overlay_workspace: {
      host: "t2",
      tab_policy: "append",
      layout: "single",
      size: "large",
      close: "tab_then_stack",
    },
    scene_id: sceneId,
    scene_file: sceneFile,
    title: nonEmpty(options.title, board.title, sceneId),
    params,
    target: {
      kind: "board",
      scene_id: sceneId,
      scene_file: sceneFile,
    },
    presentation: {
      kind: "overlay_board",
      projection: "overlay",
      type: "popup",
      overlay_size: "large",
    },
  };
  const detail = {
    kind: "scene_open",
    popup,
    board_scene_id: sceneId,
    board_scene_file: sceneFile,
    projection: "overlay",
    metric_id: board.metricId,
    dataset_id: board.rowsetDatasetId,
    label: popup.title,
    title: popup.title,
    params,
    ...(options.filters && typeof options.filters === "object"
      ? { drilldown_filters: options.filters, default_filters: options.filters }
      : {}),
    scene_projection_assembly_by_id: drilldownContext()?.scene_projection_assembly_by_id || null,
    scene_bindings_by_id: drilldownContext()?.scene_bindings_by_id || null,
    scene_local_nav_by_target: drilldownContext()?.scene_local_nav_by_target || null,
  };
  const host = options.host || (typeof window !== "undefined" ? window : null);
  if (!host || typeof host.dispatchEvent !== "function") return false;
  for (const name of [
    SCENE_OPEN_EVENT_NAME,
    DRILLDOWN_EVENT_NAME,
    ANALYSIS_OPEN_EVENT_NAME,
    POPUP_OPEN_EVENT_NAME,
  ]) {
    host.dispatchEvent(
      new CustomEvent(name, {
        bubbles: true,
        composed: true,
        detail: { ...detail },
      }),
    );
  }
  return true;
}
