import { escapeHtml, formatNowParts, LABOR_FIGMA_ASSETS, parseProps } from "./labor-shared.js";

const A = LABOR_FIGMA_ASSETS;

function numOr(v, d) {
  const n = Number(v);
  return Number.isFinite(n) ? n : d;
}

class MeiCockpitLaborHeader extends HTMLElement {
  constructor() {
    super();
    this._stripGid = 0;
    this._stripRo = null;
  }

  connectedCallback() {
    this.attachShadow({ mode: "open" });
    this.render();
    this._clockTimer = setInterval(() => this.updateClock(), 1000);
  }

  disconnectedCallback() {
    if (this._clockTimer) {
      clearInterval(this._clockTimer);
      this._clockTimer = null;
    }
    this._stripRo?.disconnect();
    this._stripRo = null;
  }

  updateClock() {
    const el = this.shadowRoot?.querySelector(".labor-clock-time");
    const elD = this.shadowRoot?.querySelector(".labor-clock-date");
    const elW = this.shadowRoot?.querySelector(".labor-clock-week");
    if (!el || !elD || !elW) return;
    const p = formatNowParts();
    el.textContent = p.time;
    elD.textContent = p.date;
    elW.textContent = p.weekday;
  }

  setupStripObserver() {
    this._stripRo?.disconnect();
    const grid = this.shadowRoot?.querySelector(".hdr-grid");
    const mid = this.shadowRoot?.querySelector(".hdr-mid");
    if (!grid || !mid) return;
    this._stripRo = new ResizeObserver(() => this.paintStripBackground());
    this._stripRo.observe(grid);
    this._stripRo.observe(mid);
    this._stripRo.observe(this);
  }

  /**
   * 无文字顶栏矢量底：左右压暗翼 + 底边高光，中间为「标题带」透明区。
   * 透明区宽度 = max(titleBandMinWidth, 实测 .hdr-mid + stripPad | titleBandWidth)。
   * 原 labor-header-strip.png 已弃用（含栅格化文字且不可调宽）。
   */
  paintStripBackground() {
    const bg = this.shadowRoot?.querySelector(".hdr-bg");
    const grid = this.shadowRoot?.querySelector(".hdr-grid");
    const mid = this.shadowRoot?.querySelector(".hdr-mid");
    if (!bg || !grid || !mid) return;
    const gw = Math.max(1, Math.floor(grid.getBoundingClientRect().width));
    const gr = grid.getBoundingClientRect();
    const midRect = mid.getBoundingClientRect();
    const pad = numOr(this.props?.stripPad, 36);
    const minMid = numOr(this.props?.titleBandMinWidth, 240);
    const fixed = numOr(this.props?.titleBandWidth, 0);
    let mw = Math.floor(midRect.width + pad);
    if (fixed > 0) mw = Math.max(minMid, Math.floor(fixed));
    else mw = Math.max(minMid, mw);
    mw = Math.min(mw, gw - 16);
    const midCx = midRect.left + midRect.width / 2;
    let ml = Math.floor(midCx - gr.left - mw / 2);
    ml = Math.max(8, Math.min(gw - mw - 8, ml));
    const mrEdge = ml + mw;
    const gid = ++this._stripGid;
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0 0 ${gw} 80" preserveAspectRatio="none" role="presentation" aria-hidden="true">
<defs>
  <linearGradient id="sb${gid}_bg" x1="0" y1="0" x2="0" y2="1" gradientUnits="userSpaceOnUse">
    <stop stop-color="#0a2848"/><stop offset="0.55" stop-color="#051a30"/><stop offset="1" stop-color="#020814"/>
  </linearGradient>
  <linearGradient id="sb${gid}_line" x1="0" y1="0" x2="1" y2="0" gradientUnits="objectBoundingBox">
    <stop stop-color="#00a8ff" stop-opacity="0"/><stop offset="0.5" stop-color="#5ed4ff" stop-opacity="0.42"/><stop offset="1" stop-color="#00a8ff" stop-opacity="0"/>
  </linearGradient>
  <linearGradient id="sb${gid}_side" x1="0" y1="0" x2="0" y2="1" gradientUnits="objectBoundingBox">
    <stop stop-color="#010c18" stop-opacity="0.72"/><stop offset="1" stop-color="#010c18" stop-opacity="0.05"/>
  </linearGradient>
</defs>
<rect x="0" y="0" width="${gw}" height="80" fill="url(#sb${gid}_bg)"/>
<rect x="0" y="78" width="${gw}" height="2" fill="url(#sb${gid}_line)"/>
<path d="M0 0 L${ml + 28} 0 L${ml} 80 L0 80 Z" fill="url(#sb${gid}_side)" stroke="rgba(80,200,255,0.14)" stroke-width="0.5"/>
<path d="M${mrEdge} 0 L${gw} 0 L${gw} 80 L${mrEdge - 28} 80 Z" fill="url(#sb${gid}_side)" stroke="rgba(80,200,255,0.14)" stroke-width="0.5"/>
</svg>`;
    bg.innerHTML = svg;
  }

  render() {
    this.props = parseProps(this);
    const p = this.props;
    this._stripRo?.disconnect();
    this._stripRo = null;
    const title = p.title || "这是标题可视化大屏";
    const clock = formatNowParts();
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .hdr {
          position: relative;
          min-height: 80px;
          height: 80px;
          overflow: hidden;
          color: #e3f4fc;
          display: flex;
          flex-direction: column;
        }
        .hdr-bg {
          position: absolute;
          inset: 0;
          z-index: 0;
          pointer-events: none;
          background: #030d18;
        }
        .hdr-grid {
          position: relative;
          z-index: 1;
          flex: 1;
          display: grid;
          grid-template-columns: minmax(140px, 18%) minmax(0, 1fr) minmax(140px, 20%);
          align-items: center;
          padding: 0 10px 0 6px;
          box-sizing: border-box;
          min-height: 0;
        }
        .hdr-left {
          display: flex;
          align-items: center;
          gap: 10px;
          padding-left: 6px;
          min-width: 0;
        }
        .hdr-wic {
          width: 46px;
          height: 46px;
          flex: 0 0 auto;
          object-fit: contain;
          filter: drop-shadow(0 0 8px rgba(0, 145, 255, 0.45));
        }
        .hdr-wline1 {
          font-family: "DIN Alternate", "DINPro", "Barlow Condensed", "Arial Narrow", Arial, sans-serif;
          font-size: 20px;
          font-weight: 700;
          line-height: 1.1;
          letter-spacing: 1px;
        }
        .hdr-wline2 {
          font-size: 12px;
          line-height: 1.2;
          opacity: 0.6;
          letter-spacing: 1px;
        }
        .hdr-center {
          display: flex;
          justify-content: center;
          align-items: center;
          min-width: 0;
          padding: 0 6px;
        }
        .hdr-mid {
          display: inline-flex;
          flex-direction: column;
          align-items: stretch;
          width: fit-content;
          max-width: 100%;
        }
        .hdr-capRow {
          display: flex;
          flex-direction: row;
          align-items: flex-end;
          justify-content: center;
          width: 100%;
          pointer-events: none;
        }
        .hdr-capRow .wing {
          height: 58px;
          width: auto;
          flex: 0 0 auto;
          display: block;
          margin: 0 -1px;
        }
        .hdr-capStretch {
          flex: 1 1 auto;
          min-width: 120px;
          height: 76px;
          align-self: flex-end;
          position: relative;
        }
        .hdr-capStretch .cap {
          position: absolute;
          inset: 0;
          width: 100%;
          height: 100%;
          object-fit: fill;
          display: block;
        }
        .hdr-titleRow {
          display: flex;
          flex-direction: row;
          align-items: center;
          justify-content: center;
          gap: 10px;
          width: 100%;
          box-sizing: border-box;
          padding: 0 6px;
          margin-top: -42px;
          position: relative;
          z-index: 1;
        }
        .hdr-tri {
          width: auto;
          height: 10px;
          flex: 0 0 auto;
          opacity: 0.95;
          filter: drop-shadow(0 0 6px rgba(255, 214, 102, 0.55));
        }
        .hdr-title {
          margin: 0;
          text-align: center;
          font-size: clamp(20px, 2vw, 40px);
          font-weight: 700;
          letter-spacing: 0.12em;
          color: #d8f0ff;
          text-shadow: 0 20px 30px #0091ff, 0 0 4px #0d74c2;
          line-height: 1.1;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          max-width: 100%;
        }
        .hdr-right {
          display: flex;
          flex-direction: row;
          flex-wrap: nowrap;
          align-items: center;
          justify-content: flex-end;
          gap: 12px;
          padding-right: 6px;
          min-width: 0;
        }
        .labor-clock-time {
          font-family: "DIN Alternate", "DINPro", "Barlow Condensed", Arial, sans-serif;
          font-size: 32px;
          font-weight: 700;
          line-height: 32px;
          letter-spacing: 1px;
          font-variant-numeric: tabular-nums;
          flex-shrink: 0;
        }
        .labor-clock-week {
          font-size: 12px;
          line-height: 1;
          letter-spacing: 1px;
          white-space: nowrap;
        }
        .labor-clock-date {
          font-size: 12px;
          line-height: 1;
          letter-spacing: 1px;
          opacity: 0.6;
          white-space: nowrap;
        }
        @media (max-width: 900px) {
          .hdr { height: auto; min-height: 0; }
          .hdr-grid {
            grid-template-columns: 1fr;
            gap: 10px;
            padding: 12px 10px 14px;
            text-align: center;
          }
          .hdr-left { justify-content: center; padding-left: 0; }
          .hdr-right { align-items: center; justify-content: center; padding-right: 0; flex-wrap: nowrap; gap: 8px; }
          .labor-clock-time { font-size: 22px; line-height: 1; }
          .hdr-title { white-space: normal; }
          .hdr-titleRow { margin-top: 6px; flex-wrap: wrap; }
          .hdr-capRow { display: none; }
        }
      </style>
      <div class="hdr">
        <div class="hdr-bg" aria-hidden="true"></div>
        <div class="hdr-grid">
          <div class="hdr-left">
            <img class="hdr-wic" src="${A}/labor-weather-icon.svg" alt="" />
            <div>
              <div class="hdr-wline1">${escapeHtml(p.temp || "28°C")}</div>
              <div class="hdr-wline2">${escapeHtml(p.sky || "多云")}</div>
            </div>
          </div>
          <div class="hdr-center">
            <div class="hdr-mid">
              <div class="hdr-capRow" aria-hidden="true">
                <img class="wing wing-l" src="${A}/labor-hdr-wing-left.svg" alt="" />
                <div class="hdr-capStretch">
                  <img class="cap" src="${A}/labor-hdr-center.svg" alt="" />
                </div>
                <img class="wing wing-r" src="${A}/labor-hdr-wing-right.svg" alt="" />
              </div>
              <div class="hdr-titleRow">
                <img class="hdr-tri" src="${A}/labor-hdr-tri-left.svg" alt="" />
                <h1 class="hdr-title">${escapeHtml(title)}</h1>
                <img class="hdr-tri" src="${A}/labor-hdr-tri-right.svg" alt="" />
              </div>
            </div>
          </div>
          <div class="hdr-right">
            <div class="labor-clock-time">${escapeHtml(clock.time)}</div>
            <div class="labor-clock-week">${escapeHtml(clock.weekday)}</div>
            <div class="labor-clock-date">${escapeHtml(clock.date)}</div>
          </div>
        </div>
      </div>
    `;
    queueMicrotask(() => {
      this.setupStripObserver();
      this.paintStripBackground();
    });
  }
}

if (!customElements.get("mei-cockpit-labor-header")) {
  customElements.define("mei-cockpit-labor-header", MeiCockpitLaborHeader);
}
