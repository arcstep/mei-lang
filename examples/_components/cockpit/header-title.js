import { escapeHtml, COCKPIT_FIGMA_ASSETS, parseProps } from "./shared.js";

const A = COCKPIT_FIGMA_ASSETS;

function numOr(value, fallback) {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
}

class MeiCockpitHeaderTitle extends HTMLElement {
  constructor() {
    super();
    this._ro = null;
  }

  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.render();
  }

  disconnectedCallback() {
    this._ro?.disconnect();
    this._ro = null;
  }

  setupObserver() {
    this._ro?.disconnect();
    const title = this.shadowRoot?.querySelector(".title");
    const wrap = this.shadowRoot?.querySelector(".wrap");
    if (!title || !wrap) return;
    this._ro = new ResizeObserver(() => this.syncBandWidth());
    this._ro.observe(title);
    this._ro.observe(wrap);
    this._ro.observe(this);
  }

  syncBandWidth() {
    const title = this.shadowRoot?.querySelector(".title");
    const wrap = this.shadowRoot?.querySelector(".wrap");
    const core = this.shadowRoot?.querySelector(".capStretch");
    const shadow = this.shadowRoot?.querySelector(".bandShadow");
    const overlay = this.shadowRoot?.querySelector(".bandOverlay");
    if (!title || !wrap || !core || !shadow || !overlay) return;
    const p = this.props || {};
    const fixedWidth = Math.max(0, numOr(p.titleBandWidth, 0));
    const minWidth = Math.max(320, numOr(p.titleBandMinWidth, 598));
    const stripPad = Math.max(80, numOr(p.stripPad, 208));
    const titleWidth = Math.ceil(Math.max(title.scrollWidth, title.getBoundingClientRect().width));
    const measured = titleWidth + stripPad;
    const nextWidth = fixedWidth > 0 ? Math.max(minWidth, fixedWidth) : Math.max(minWidth, measured);
    core.style.width = `${nextWidth}px`;
    shadow.style.width = `${Math.max(0, nextWidth - 22)}px`;
    overlay.style.width = `${nextWidth + 11}px`;
    wrap.style.setProperty("--band-width", `${nextWidth}px`);
  }

  render() {
    this.props = parseProps(this);
    const p = this.props;
    const title = p.title || "这是标题可视化大屏";
    const stripPad = Math.max(80, numOr(p.stripPad, 208));
    this._ro?.disconnect();
    this._ro = null;
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          justify-self: center;
          align-self: start;
          min-width: 0;
          overflow: visible;
          color: var(--mei-color-text-primary, #d8f0ff);
        }
        .wrap {
          display: inline-block;
          width: fit-content;
          max-width: 100%;
          position: relative;
          min-width: 0;
          --band-width: 598px;
        }
        .band {
          position: relative;
          width: max-content;
          min-width: var(--band-width);
          height: 98px;
        }
        .bandShadow {
          position: absolute;
          left: 50%;
          bottom: -1px;
          transform: translateX(-50%);
          width: calc(var(--band-width) - 22px);
          height: 68px;
          opacity: 0.66;
          object-fit: fill;
          pointer-events: none;
        }
        .bandOverlay {
          position: absolute;
          left: 50%;
          top: 1px;
          transform: translateX(-50%);
          width: calc(var(--band-width) + 11px);
          height: 90.8731px;
          object-fit: fill;
          pointer-events: none;
          opacity: 0.82;
        }
        .goldBar {
          position: absolute;
          left: 50%;
          bottom: 11px;
          transform: translateX(-50%);
          width: clamp(120px, calc(var(--band-width) * 0.24), 160px);
          height: 3px;
          border-radius: 999px;
          background:
            linear-gradient(90deg, rgba(253, 189, 0, 0) 0%, rgba(253, 189, 0, 0.92) 20%, rgba(255, 218, 102, 0.98) 50%, rgba(253, 189, 0, 0.92) 80%, rgba(253, 189, 0, 0) 100%);
          box-shadow:
            0 0 6px rgba(253, 189, 0, 0.42),
            0 0 14px rgba(253, 189, 0, 0.16);
          pointer-events: none;
          z-index: 2;
        }
        .capRow {
          position: absolute;
          left: 50%;
          top: 2px;
          transform: translateX(-50%);
          display: block;
          pointer-events: none;
          z-index: 3;
        }
        .capStretch {
          height: 80px;
          position: relative;
          min-width: 320px;
        }
        .cap {
          position: absolute;
          inset: 0;
          width: 100%;
          height: 100%;
          object-fit: fill;
          display: block;
        }
        .titleRow {
          position: absolute;
          left: 50%;
          top: 11px;
          transform: translateX(-50%);
          display: flex;
          flex-direction: row;
          align-items: center;
          justify-content: center;
          gap: 14px;
          width: max-content;
          max-width: calc(var(--band-width) - 20px);
          box-sizing: border-box;
          padding: 0 ${Math.round(stripPad / 4)}px;
          background: transparent;
          z-index: 4;
        }
        .tri {
          width: 24.7665px;
          height: 24.7639px;
          flex: 0 0 auto;
          display: block;
          margin-top: 10px;
        }
        .title {
          margin: 0;
          background: transparent;
          text-align: center;
          font-family: "YouSheBiaoTiHei", "YouShe BiaoTiHei", "DIN Alternate", "Microsoft YaHei", sans-serif;
          font-size: var(--mei-font-4, 40px);
          font-weight: 400;
          letter-spacing: 2.5px;
          color: var(--mei-color-text-primary, #d8f0ff);
          text-shadow: 0 20px 30px #0091ff, 0 0 4px #0d74c2;
          line-height: calc(var(--mei-font-4, 40px) * 1.3);
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          max-width: 100%;
        }
        @media (max-width: 900px) {
          .band {
            min-width: 0;
            width: 100%;
            height: auto;
            padding-top: 10px;
          }
          .bandShadow,
          .bandOverlay,
          .capRow {
            display: none;
          }
          .titleRow {
            position: relative;
            left: auto;
            top: auto;
            transform: none;
            padding: 0 20px;
            flex-wrap: wrap;
            max-width: 100%;
          }
          .title { white-space: normal; }
        }
      </style>
      <div class="wrap">
        <div class="band">
          <img class="bandShadow" src="${A}/labor-hdr-title-shadow.png" alt="" aria-hidden="true" />
          <div class="goldBar" aria-hidden="true"></div>
          <div class="capRow" aria-hidden="true">
            <div class="capStretch">
              <img class="cap" src="${A}/labor-hdr-center.svg" alt="" />
            </div>
          </div>
          <img class="bandOverlay" src="${A}/labor-hdr-center-overlay.svg" alt="" aria-hidden="true" />
          <div class="titleRow">
            <img class="tri" src="${A}/labor-hdr-tri-left.svg" alt="" />
            <h1 class="title">${escapeHtml(title)}</h1>
            <img class="tri" src="${A}/labor-hdr-tri-right.svg" alt="" />
          </div>
        </div>
      </div>
    `;
    queueMicrotask(() => {
      this.setupObserver();
      this.syncBandWidth();
    });
  }
}

if (!customElements.get("mei-cockpit-header-title")) {
  customElements.define("mei-cockpit-header-title", MeiCockpitHeaderTitle);
}
