/**
 * Thunder 地图同步：按 selectedEventId + playbackAt 过滤同心环与分型闪点；
 * 采集设备层按 status 降透、按事件代表站高亮；要素点击打开 T2。
 */
import { parseProps } from "../cockpit/shared.js";
import { MEI_MAP_SELECTION } from "../gis/layer-spec.js";
import { getThunderStore, hhmmToMinutes, subscribeThunderState } from "./event-bus.js";
import { openThunderT2 } from "./t2-open.js";

const RING_LAYER = "station-warning-rings";
const RING_SITE_LAYER = "station-warning-sites";
const LIGHTNING_LAYER = "lightning-points";
/** 分类型采集设备层（与 gis-map MAP_SPEC 对齐） */
const SITE_LAYERS = [
  "sites-lld",
  "sites-efield",
  "sites-optical",
  "sites-tlci",
  "sites-weather",
];

function findMapHost() {
  return document.querySelector("mei-map-maplibre");
}

function waitForMap(host, tries = 40) {
  return new Promise((resolve) => {
    const tick = (left) => {
      if (host?.map && host._layerRegistry) {
        resolve(host);
        return;
      }
      if (left <= 0) {
        resolve(host || null);
        return;
      }
      requestAnimationFrame(() => tick(left - 1));
    };
    tick(tries);
  });
}

function levelRank(level) {
  const text = String(level || "");
  if (text.includes("红")) return 3;
  if (text.includes("橙")) return 2;
  if (text.includes("黄")) return 1;
  return 0;
}

function ringFilter(eventId, level) {
  const rank = levelRank(level);
  const zones =
    rank >= 3 ? ["yellow", "orange", "red"] : rank >= 2 ? ["yellow", "orange"] : ["yellow"];
  const zoneMatch =
    zones.length === 1
      ? ["==", ["get", "zone"], zones[0]]
      : ["any", ...zones.map((zone) => ["==", ["get", "zone"], zone])];
  return [
    "all",
    ["==", ["get", "event_id"], eventId],
    ["==", ["get", "kind"], "ring"],
    zoneMatch,
  ];
}

function siteFilter(eventId) {
  return [
    "all",
    ["==", ["get", "event_id"], eventId],
    ["==", ["get", "kind"], "station"],
  ];
}

function lightningFilter(eventId, playbackAtMin) {
  return [
    "all",
    ["==", ["get", "event_id"], eventId],
    ["<=", ["get", "at_min"], playbackAtMin],
  ];
}

function ringLineWidth(level) {
  const rank = levelRank(level);
  return [
    "match",
    ["get", "zone"],
    "red",
    rank >= 3 ? 3.2 : 1.2,
    "orange",
    rank === 2 ? 3.0 : 1.4,
    "yellow",
    rank === 1 ? 2.8 : 1.2,
    1.2,
  ];
}

/** 采集设备：异常站降透；选中事件代表站加大半径与描边 */
function sitesPaint(focusSiteId) {
  const focus = String(focusSiteId || "").trim();
  const isFocus = focus
    ? ["==", ["get", "site_id"], focus]
    : false;
  return {
    "circle-opacity": [
      "case",
      ["==", ["get", "status"], "degraded"],
      0.55,
      1,
    ],
    "circle-radius": isFocus
      ? ["case", isFocus, 9, 6]
      : 6,
    "circle-stroke-width": isFocus
      ? ["case", isFocus, 3, 1.5]
      : 1.5,
    "circle-stroke-color": isFocus
      ? ["case", isFocus, "#e0f2fe", "#0c2848"]
      : "#0c2848",
  };
}

class MeiThunderMapSync extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `<style>:host{display:none!important;}</style>`;
    this._unsub = subscribeThunderState((detail) => this.apply(detail));
    this._onMapSelect = (event) => this.onMapSelection(event?.detail);
    window.addEventListener(MEI_MAP_SELECTION, this._onMapSelect);
    this._retry = setInterval(() => this.apply(getThunderStore()), 800);
    setTimeout(() => this.apply(getThunderStore()), 200);
  }

  disconnectedCallback() {
    if (typeof this._unsub === "function") {
      this._unsub();
      this._unsub = null;
    }
    if (this._onMapSelect) {
      window.removeEventListener(MEI_MAP_SELECTION, this._onMapSelect);
      this._onMapSelect = null;
    }
    if (this._retry) {
      clearInterval(this._retry);
      this._retry = null;
    }
  }

  onMapSelection(detail) {
    const layerId = String(detail?.layerId || "").trim();
    if (!layerId) return;
    if (layerId === LIGHTNING_LAYER) {
      openThunderT2("lightning", {
        host: this,
        filters: detail?.properties?.id ? { id: String(detail.properties.id) } : undefined,
      });
      return;
    }
    if (SITE_LAYERS.includes(layerId)) {
      const siteId = String(detail?.properties?.site_id || detail?.code || "").trim();
      openThunderT2("collection", {
        host: this,
        filters: siteId ? { site_id: siteId } : undefined,
      });
      return;
    }
    if (layerId === RING_LAYER || layerId === RING_SITE_LAYER) {
      openThunderT2("lifecycle", { host: this });
    }
  }

  async apply(state) {
    const eventId = String(state?.eventId || "").trim();
    if (!eventId) return;
    const playbackAt = String(state?.playbackAt || "").trim();
    const playbackAtMin = Number(state?.playbackAtMin) || hhmmToMinutes(playbackAt);
    const level = String(state?.level || state?.event?.level || "").trim();
    const focusSiteId = String(state?.event?.site?.site_id || "").trim();
    const host = await waitForMap(findMapHost());
    if (!host || typeof host.setLayerFeatureFilter !== "function") return;

    host.setLayerFeatureFilter(RING_LAYER, ringFilter(eventId, level));
    host.setLayerFeatureFilter(RING_SITE_LAYER, siteFilter(eventId));
    host.setLayerFeatureFilter(LIGHTNING_LAYER, lightningFilter(eventId, playbackAtMin));
    if (typeof host.setLayerPaint === "function") {
      host.setLayerPaint(RING_LAYER, { "line-width": ringLineWidth(level) });
      const paint = sitesPaint(focusSiteId);
      for (const layerId of SITE_LAYERS) {
        host.setLayerPaint(layerId, paint);
      }
    }
  }
}

if (!customElements.get("mei-thunder-map-sync")) {
  customElements.define("mei-thunder-map-sync", MeiThunderMapSync);
}
