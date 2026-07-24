/** 驾驶舱大屏标题：主题底色层 + SVG 装饰线/帽檐 + 主标题。1920×72 设计稿逻辑尺寸。 */
import { color, gradient } from "../mei/theme-style.js";
import { COCKPIT_SHADOW } from "./tokens.js";

import { cockpitAsset, escapeHtml, parseProps } from "./shared.js";

function numOr(value, fallback) {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
}

const HEADER_HEIGHT_PX = 72;
const CAP_MIN_WIDTH_PX = 633;
const CAP_HEIGHT_PX = 62;
const CAP_TOP_PX = 8;
const CAP_PAD_DEFAULT = 140;

const TITLE_FONT_FAMILY =
  '"YouSheBiaoTiHei", "YouShe BiaoTiHei", "DIN Alternate", "Microsoft YaHei", sans-serif';

function wantsTitleLayer(p, key, flag) {
  if (p[flag] === false || p[flag] === "false") return false;
  if (p[flag] === true || p[flag] === "true") return true;
  return !!cockpitAsset(p, key);
}

function resolveBandBackground(p) {
  if (
    p.bandBackground === false ||
    p.bandBackground === "false" ||
    p.band_background === false ||
    p.band_background === "false"
  ) {
    return "transparent";
  }
  const raw = p.bandBackground ?? p.band_background;
  if (raw === undefined || raw === null || raw === "") {
    return gradient("header_band_bg");
  }
  const value = String(raw).trim();
  if (!value || value === "transparent") {
    return "transparent";
  }
  // Token name (no css function / url) → gradient var
  if (!/[#(]|url\(/i.test(value) && !value.includes(" ")) {
    return gradient(value);
  }
  return value;
}

function resolveCapBackground(p) {
  if (
    p.capBackground === false ||
    p.capBackground === "false" ||
    p.cap_background === false ||
    p.cap_background === "false"
  ) {
    return "transparent";
  }
  const raw = p.capBackground ?? p.cap_background;
  if (raw === undefined || raw === null || raw === "") {
    return gradient("header_cap_bg");
  }
  const value = String(raw).trim();
  if (!value || value === "transparent") {
    return "transparent";
  }
  if (!/[#(]|url\(/i.test(value) && !value.includes(" ")) {
    return gradient(value);
  }
  return value;
}

class MeiCockpitHeaderBrand extends HTMLElement {
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
    const cap = this.shadowRoot?.querySelector(".cap");
    if (!title || !cap) return;
    this._ro = new ResizeObserver(() => this.syncCapWidth());
    this._ro.observe(title);
    this._ro.observe(this);
  }

  syncCapWidth() {
    const title = this.shadowRoot?.querySelector(".title");
    const cap = this.shadowRoot?.querySelector(".cap");
    if (!title || !cap) return;
    const p = this.props || {};
    const minWidth = Math.max(CAP_MIN_WIDTH_PX, numOr(p.capMinWidth, CAP_MIN_WIDTH_PX));
    const pad = Math.max(48, numOr(p.capPad, CAP_PAD_DEFAULT));
    const titleWidth = Math.ceil(Math.max(title.scrollWidth, title.getBoundingClientRect().width));
    const nextWidth = Math.max(minWidth, titleWidth + pad);
    cap.style.width = `${nextWidth}px`;
    this.shadowRoot.querySelector(".wrap")?.style.setProperty("--cap-width", `${nextWidth}px`);
  }

  render() {
    this.props = parseProps(this);
    const p = this.props;
    const title = p.title || "标题占位";
    const titleBg = wantsTitleLayer(p, "title_bg", "showTitleBg")
      ? cockpitAsset(p, "title_bg")
      : "";
    const titleMid = wantsTitleLayer(p, "title_mid", "showTitleCap")
      ? cockpitAsset(p, "title_mid")
      : "";
    const chromeless = !titleBg && !titleMid;
    const titleColor = p.titleColor || color("text_inverse");
    const titleLineHeight = p.titleLineHeight || "68px";
    const titleLetterSpacing =
      p.titleLetterSpacing !== undefined && p.titleLetterSpacing !== null
        ? String(p.titleLetterSpacing)
        : "0";
    const titleFontSize = p.titleFontSize || "36px";
    const bandBg = resolveBandBackground(p);
    const capBg = resolveCapBackground(p);

    this._ro?.disconnect();
    this._ro = null;
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          min-width: 0;
          overflow: visible;
        }
        .band {
          position: relative;
          width: 100%;
          height: ${HEADER_HEIGHT_PX}px;
          min-height: ${HEADER_HEIGHT_PX}px;
          max-height: ${HEADER_HEIGHT_PX}px;
          overflow: hidden;
          background: ${bandBg};
        }
        .band-bg {
          position: absolute;
          inset: 0;
          width: 100%;
          height: 100%;
          object-fit: fill;
          object-position: center bottom;
          pointer-events: none;
          z-index: 1;
        }
        .wrap {
          position: absolute;
          left: 50%;
          top: 0;
          transform: translateX(-50%);
          width: var(--cap-width, ${CAP_MIN_WIDTH_PX}px);
          height: 100%;
          max-width: calc(100% - 32px);
          --cap-width: ${CAP_MIN_WIDTH_PX}px;
          z-index: 2;
        }
        .cap {
          position: absolute;
          left: 50%;
          top: ${CAP_TOP_PX}px;
          transform: translateX(-50%);
          width: var(--cap-width);
          height: ${CAP_HEIGHT_PX}px;
        }
        .cap-fill {
          position: absolute;
          inset: 0;
          background: ${capBg};
          /* 近似「大标题-中心」主体梯形：上宽下窄；翼线留在未裁切的 SVG 层 */
          clip-path: polygon(8.2% 0%, 92.6% 0%, 84.5% 100%, 16.3% 100%);
          pointer-events: none;
        }
        .cap img {
          position: absolute;
          inset: 0;
          width: 100%;
          height: 100%;
          object-fit: fill;
          display: block;
          pointer-events: none;
        }
        .title-row {
          position: absolute;
          left: 50%;
          top: 50%;
          transform: translate(-50%, -50%);
          z-index: 3;
          display: flex;
          align-items: center;
          justify-content: center;
          max-width: calc(var(--cap-width) + 48px);
          padding: 0 12px;
          box-sizing: border-box;
          pointer-events: none;
        }
        .title-row--solo {
          position: static;
          transform: none;
          width: 100%;
          max-width: none;
          height: 100%;
          min-height: ${HEADER_HEIGHT_PX}px;
        }
        .title {
          margin: 0;
          text-align: center;
          font-family: ${TITLE_FONT_FAMILY};
          font-size: ${titleFontSize};
          font-weight: 400;
          letter-spacing: ${titleLetterSpacing};
          line-height: ${titleLineHeight};
          max-height: 68px;
          color: ${titleColor};
          text-shadow: ${COCKPIT_SHADOW.headerTitle};
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        @media (max-width: 900px) {
          .title { white-space: normal; font-size: 28px; }
        }
      </style>
      <div class="band">
        ${titleBg ? `<img class="band-bg" src="${titleBg}" alt="" />` : ""}
        ${
          chromeless
            ? `<div class="title-row title-row--solo">
            <h1 class="title">${escapeHtml(title)}</h1>
          </div>`
            : `<div class="wrap">
          <div class="cap" aria-hidden="true">
            <div class="cap-fill"></div>
            ${titleMid ? `<img src="${titleMid}" alt="" />` : ""}
          </div>
          <div class="title-row">
            <h1 class="title">${escapeHtml(title)}</h1>
          </div>
        </div>`
        }
      </div>
    `;
    queueMicrotask(() => {
      if (!chromeless) {
        this.setupObserver();
        this.syncCapWidth();
      }
    });
  }
}

if (!customElements.get("mei-cockpit-header-brand")) {
  customElements.define("mei-cockpit-header-brand", MeiCockpitHeaderBrand);
}
