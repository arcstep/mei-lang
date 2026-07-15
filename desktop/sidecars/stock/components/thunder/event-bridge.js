/**
 * Thunder 事件桥：加载 event-catalog，监听清单双击，广播当前事件包。
 * 隐藏宿主；同页 playback / summary / charts / map-sync 订阅 mei:thunder-event-state。
 */
import { parseProps } from "../cockpit/shared.js";
import {
  THUNDER_EVENT_ACTIVATE,
  activateThunderEvent,
  findEvent,
  getThunderStore,
  resolveDefaultEvent,
} from "./event-bus.js";

const DEFAULT_CATALOG_URL = "/workspace-app-assets/thunder/prototype/event-catalog.json";

class MeiThunderEventBridge extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `<style>:host{display:none!important;}</style>`;
    this._onActivate = (event) => this.handleActivate(event);
    window.addEventListener(THUNDER_EVENT_ACTIVATE, this._onActivate);
    this.loadCatalog();
  }

  disconnectedCallback() {
    if (this._onActivate) {
      window.removeEventListener(THUNDER_EVENT_ACTIVATE, this._onActivate);
      this._onActivate = null;
    }
  }

  catalogUrl() {
    return String(
      this._props?.catalogUrl ||
        this._props?.catalog_url ||
        DEFAULT_CATALOG_URL,
    ).trim();
  }

  async loadCatalog() {
    const url = this.catalogUrl();
    const store = getThunderStore();
    try {
      const res = await fetch(url, { cache: "no-store" });
      if (!res.ok) throw new Error(`catalog ${res.status}`);
      const catalog = await res.json();
      store.catalog = catalog;
      store.catalogUrl = url;
      const current = findEvent(catalog, store.eventId) || resolveDefaultEvent(catalog);
      if (current) {
        activateThunderEvent(current, { source: "bridge-boot" });
      }
    } catch (error) {
      console.warn("[thunder.event-bridge] catalog load failed", error);
    }
  }

  handleActivate(event) {
    const detail = event?.detail || {};
    const eventId = String(detail.eventId || detail.row?.id || "").trim();
    if (!eventId) return;
    const store = getThunderStore();
    const fromCatalog = findEvent(store.catalog, eventId);
    if (fromCatalog) {
      activateThunderEvent(fromCatalog, { source: detail.reason || "activate" });
      return;
    }
    // catalog 未就绪时用行数据占位，等 catalog 再纠正
    activateThunderEvent(
      {
        id: eventId,
        title: detail.row?.title || eventId,
        listTitle: detail.row?.title || eventId,
        status: detail.row?.status || "archived",
        level: detail.row?.level || "",
        slices: [],
        defaultSlice: "",
        sliceLevels: {},
        charts: { lifecycle: [], efield: [], frequency: [] },
        lightning: [],
      },
      { source: "activate-row" },
    );
  }
}

if (!customElements.get("mei-thunder-event-bridge")) {
  customElements.define("mei-thunder-event-bridge", MeiThunderEventBridge);
}
