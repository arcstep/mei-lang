/**
 * Thunder 回看条：级别徽章（左）+ 右侧上下叠：事件名 / 5 分钟切片轴。
 * 主题字号：metric_value（级别）/ metric_label（事件）/ chart_label（切片）。
 */
import { parseProps, escapeHtml, escapeAttr } from "../cockpit/shared.js";
import { COCKPIT_TYPE, cockpitCssVars } from "../cockpit/tokens.js";
import { color } from "../mei/theme-style.js";
import {
  activateThunderEvent,
  getThunderStore,
  levelTone,
  listTitleOf,
  resolveDefaultEvent,
  selectThunderSlice,
  subscribeThunderState,
} from "./event-bus.js";

const DEFAULT_CATALOG_URL = "/workspace-app-assets/thunder/prototype/event-catalog.json";

class MeiThunderPlaybackStrip extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.style.display = "block";
    this.style.width = "100%";
    this.style.height = "100%";
    this.style.minHeight = "0";
    this.style.overflow = "hidden";
    this.style.boxSizing = "border-box";
    this._unsub = subscribeThunderState((detail) => this.render(detail));
    this.shadowRoot.addEventListener("click", (event) => this.onClick(event));
    this.render(getThunderStore());
    this.ensureCatalogBoot();
  }

  async ensureCatalogBoot() {
    const store = getThunderStore();
    if (store.event || store.catalog) return;
    try {
      const res = await fetch(DEFAULT_CATALOG_URL, { cache: "no-store" });
      if (!res.ok) return;
      const catalog = await res.json();
      store.catalog = catalog;
      const current = resolveDefaultEvent(catalog);
      if (current) activateThunderEvent(current, { source: "playback-boot" });
    } catch (_) {
      /* bridge 会再试 */
    }
  }

  disconnectedCallback() {
    if (typeof this._unsub === "function") {
      this._unsub();
      this._unsub = null;
    }
  }

  onClick(event) {
    const btn = event
      .composedPath()
      .find((node) => node instanceof HTMLElement && node.dataset?.slice);
    if (!btn) return;
    selectThunderSlice(btn.dataset.slice, { source: "playback-strip" });
  }

  render(state) {
    const event = state?.event || getThunderStore().event;
    const playbackAt = String(state?.playbackAt || event?.defaultSlice || "").trim();
    const level = String(state?.level || event?.level || "—").trim() || "—";
    const tone = levelTone(level);
    const title = listTitleOf(event) || "暂未发现进行中预警";
    const slices = Array.isArray(event?.slices) ? event.slices : [];
    const nodes = slices
      .map((slice) => {
        const active = slice === playbackAt;
        const sliceLevel = event?.sliceLevels?.[slice] || level;
        return `<button type="button" class="slice${active ? " is-active" : ""}" data-slice="${escapeAttr(
          slice,
        )}" title="${escapeAttr(`${slice} · ${sliceLevel}`)}"><span class="dot"></span><span class="label">${escapeHtml(
          slice,
        )}</span></button>`;
      })
      .join("");

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          height: 100%;
          min-height: 0;
          max-height: 100%;
          overflow: hidden;
          box-sizing: border-box;
          ${cockpitCssVars()}
          font-family: var(--cockpit-font-family-ui);
        }
        .wrap {
          display: grid;
          grid-template-columns: auto minmax(0, 1fr);
          grid-template-rows: auto minmax(0, 1fr);
          grid-template-areas:
            "level meta"
            "level axis";
          gap: 6px 8px;
          align-items: stretch;
          width: 100%;
          height: 100%;
          min-height: 0;
          box-sizing: border-box;
        }
        .level, .event, .axis {
          display: flex;
          align-items: center;
          border-radius: 4px;
          box-sizing: border-box;
          min-width: 0;
          min-height: 0;
        }
        .level {
          grid-area: level;
          justify-content: center;
          align-self: stretch;
          padding: 8px 14px;
          font-size: ${COCKPIT_TYPE.metricValue};
          font-weight: 700;
          line-height: 1.2;
          color: ${tone.fg};
          background: ${tone.bg};
          border: 1px solid ${tone.border};
        }
        .event {
          grid-area: meta;
          padding: 4px 10px;
          font-size: ${COCKPIT_TYPE.metricLabel};
          font-weight: 600;
          line-height: 1.25;
          color: ${color("text_value")};
          background: rgba(10, 40, 78, 0.82);
          border: 1px solid rgba(56, 160, 240, 0.45);
          overflow: hidden;
          white-space: nowrap;
          text-overflow: ellipsis;
        }
        .axis {
          grid-area: axis;
          gap: 2px;
          padding: 4px 8px;
          overflow-x: auto;
          overflow-y: hidden;
          background: rgba(10, 40, 78, 0.72);
          border: 1px solid rgba(56, 160, 240, 0.28);
        }
        .slice {
          flex: 0 0 auto;
          display: inline-flex;
          flex-direction: column;
          align-items: center;
          gap: 3px;
          min-width: 44px;
          padding: 2px 4px;
          border: 0;
          background: transparent;
          color: ${color("text_muted")};
          cursor: pointer;
          font: inherit;
        }
        .slice .dot {
          width: 10px;
          height: 10px;
          border-radius: 50%;
          background: rgba(125, 211, 252, 0.28);
          box-shadow: 0 0 0 1px rgba(56, 160, 240, 0.35);
        }
        .slice .label {
          font-size: ${COCKPIT_TYPE.chartLabel};
          line-height: 1;
          font-variant-numeric: tabular-nums;
          letter-spacing: 0.02em;
          transform: scale(0.9);
          transform-origin: center top;
        }
        .slice.is-active {
          color: ${color("text_body")};
          font-weight: 600;
        }
        .slice.is-active .dot {
          background: #38bdf8;
          box-shadow: 0 0 0 2px rgba(56, 189, 248, 0.45);
          transform: scale(1.15);
        }
        .slice:hover .dot {
          background: #7dd3fc;
        }
        .empty {
          color: ${color("text_muted")};
          font-size: ${COCKPIT_TYPE.chartLabel};
          padding: 0 8px;
        }
      </style>
      <div class="wrap">
        <div class="level">${escapeHtml(level)}</div>
        <div class="event" title="${escapeAttr(title)}">${escapeHtml(title)}</div>
        <div class="axis" role="listbox" aria-label="5分钟切片">
          ${nodes || `<span class="empty">无切片</span>`}
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-thunder-playback-strip")) {
  customElements.define("mei-thunder-playback-strip", MeiThunderPlaybackStrip);
}
