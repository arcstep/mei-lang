import { cockpitAsset, escapeHtml, parseProps } from "./shared.js";
import {
  COCKPIT_COLOR,
  COCKPIT_FONT,
  COCKPIT_LAYOUT,
  COCKPIT_PANEL_TITLE_CARET,
  COCKPIT_SHADOW,
  COCKPIT_SPACING,
  COCKPIT_TYPE,
  cockpitCssVars,
} from "./tokens.js";

class MeiCockpitPanelTitle extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const p = parseProps(this);
    const title = p.title || "板块标题";
    const size = p.size === "compact" ? "compact" : "wide";
    const bgKey = size === "compact" ? "title_bg_s" : "title_bg_l";
    const bg = cockpitAsset(p, bgKey);
    const caretSrc =
      cockpitAsset(p, "caret_left") || cockpitAsset(p, "caret_right");
    const showCarets =
      (p.showCarets === true || p.showCarets === "true") && !!caretSrc;
    const height =
      size === "compact" ? COCKPIT_LAYOUT.panelTitleCompact : COCKPIT_LAYOUT.panelTitleWide;
    const maxWidth = Number(p.maxWidth) > 0 ? Number(p.maxWidth) : COCKPIT_LAYOUT.panelWidth;
    const fontSize =
      size === "compact" ? COCKPIT_TYPE.panelTitleCompact : COCKPIT_TYPE.panelTitle;
    const letterSpacing =
      size === "compact"
        ? COCKPIT_TYPE.panelTitleLetterSpacing
        : maxWidth >= 800
          ? COCKPIT_TYPE.panelTitleLetterSpacingWide
          : COCKPIT_TYPE.panelTitleLetterSpacing;
    const caretPos = COCKPIT_PANEL_TITLE_CARET[size] || COCKPIT_PANEL_TITLE_CARET.compact;
    const caretTop =
      size === "compact"
        ? "calc(50% + 1px)"
        : "calc(50% + 2px)";

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          position: relative;
          z-index: 1;
          width: 100%;
          min-width: 0;
          max-width: 100%;
          box-sizing: border-box;
          ${cockpitCssVars()}
        }
        .wrap {
          position: relative;
          width: 100%;
          height: ${height}px;
          min-height: ${height}px;
          max-height: ${height}px;
          overflow: hidden;
          background-repeat: no-repeat;
          background-position: center;
          background-size: 100% 100%;
        }
        .caret {
          position: absolute;
          top: ${caretTop};
          z-index: 2;
          width: 14px;
          height: 24px;
          pointer-events: none;
        }
        .caret img {
          width: 100%;
          height: 100%;
          display: block;
        }
        .caret--left-slot {
          left: ${(caretPos.left * 100).toFixed(1)}%;
          transform: translate(-50%, -50%) scaleX(-1);
        }
        .caret--right-slot {
          left: ${(caretPos.right * 100).toFixed(1)}%;
          transform: translate(-50%, -50%) scaleX(-1);
        }
        .title {
          position: relative;
          z-index: 3;
          display: flex;
          align-items: center;
          justify-content: center;
          height: 100%;
          padding: var(--cockpit-panel-pad);
          margin: 0;
          box-sizing: border-box;
          font-family: var(--mei-panel-head-font-family, ${COCKPIT_FONT.headerFamily});
          font-size: var(--mei-panel-head-font-size, ${fontSize}px);
          font-weight: var(--mei-panel-head-font-weight, 700);
          letter-spacing: var(--mei-panel-head-letter-spacing, ${letterSpacing});
          line-height: var(--mei-panel-head-line-height, 1);
          color: var(--mei-panel-head-color, var(--cockpit-color-panel, ${COCKPIT_COLOR.panelTitle}));
          text-align: var(--mei-panel-head-text-align, center);
          text-shadow: ${COCKPIT_SHADOW.panelTitle};
          max-width: 100%;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
      </style>
      <div class="wrap">
        ${
          showCarets
            ? `<div class="caret caret--left-slot" aria-hidden="true"><img src="${caretSrc}" alt="" /></div>
        <div class="caret caret--right-slot" aria-hidden="true"><img src="${caretSrc}" alt="" /></div>`
            : ""
        }
        <h3 class="title">${escapeHtml(title)}</h3>
      </div>
    `;
    const wrap = this.shadowRoot.querySelector(".wrap");
    if (wrap && bg) {
      wrap.style.backgroundImage = `url("${String(bg).replace(/"/g, '\\"')}")`;
    }
  }
}

if (!customElements.get("mei-cockpit-panel-title")) {
  customElements.define("mei-cockpit-panel-title", MeiCockpitPanelTitle);
}
