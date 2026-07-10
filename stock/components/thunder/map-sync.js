/**
 * Thunder 地图同步：按 selectedEventId + playbackAt 过滤同心环与分型闪点。
 */
import { parseProps } from "../cockpit/shared.js";
import { getThunderStore, hhmmToMinutes, subscribeThunderState } from "./event-bus.js";

const RING_LAYER = "station-warning-rings";
const RING_SITE_LAYER = "station-warning-sites";
const LIGHTNING_LAYER = "lightning-points";

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
  // 当前级别对应环加粗：红>橙>黄
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

class MeiThunderMapSync extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `<style>:host{display:none!important;}</style>`;
    this._unsub = subscribeThunderState((detail) => this.apply(detail));
    this._retry = setInterval(() => this.apply(getThunderStore()), 800);
    setTimeout(() => this.apply(getThunderStore()), 200);
  }

  disconnectedCallback() {
    if (typeof this._unsub === "function") {
      this._unsub();
      this._unsub = null;
    }
    if (this._retry) {
      clearInterval(this._retry);
      this._retry = null;
    }
  }

  async apply(state) {
    const eventId = String(state?.eventId || "").trim();
    if (!eventId) return;
    const playbackAt = String(state?.playbackAt || "").trim();
    const playbackAtMin = Number(state?.playbackAtMin) || hhmmToMinutes(playbackAt);
    const level = String(state?.level || state?.event?.level || "").trim();
    const host = await waitForMap(findMapHost());
    if (!host || typeof host.setLayerFeatureFilter !== "function") return;

    host.setLayerFeatureFilter(RING_LAYER, ringFilter(eventId, level));
    host.setLayerFeatureFilter(RING_SITE_LAYER, siteFilter(eventId));
    host.setLayerFeatureFilter(LIGHTNING_LAYER, lightningFilter(eventId, playbackAtMin));
    if (typeof host.setLayerPaint === "function") {
      host.setLayerPaint(RING_LAYER, { "line-width": ringLineWidth(level) });
    }
  }
}

if (!customElements.get("mei-thunder-map-sync")) {
  customElements.define("mei-thunder-map-sync", MeiThunderMapSync);
}
