import { escapeHtml, COCKPIT_FIGMA_ASSETS, parseProps } from "./shared.js";

const A = COCKPIT_FIGMA_ASSETS;

class MeiCockpitSectionTitle extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const p = parseProps(this);
    const title = p.title || "这是板块标题";
    const flair = p.flair != false;
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          min-width: 0;
        }
        .panel-hd {
          display: flex;
          align-items: center;
          gap: 10px;
          width: 100%;
          min-width: 0;
          padding: 14px 16px 12px 18px;
          position: relative;
          background: linear-gradient(180deg, rgba(0, 20, 45, 0.55) 0%, rgba(0, 12, 28, 0.15) 100%);
          border-bottom: 1px solid rgba(0, 180, 255, 0.22);
          box-shadow:
            inset 0 1px 0 rgba(120, 220, 255, 0.12),
            0 0 22px rgba(0, 140, 255, 0.12);
        }
        .panel-hd::after {
          content: "";
          position: absolute;
          left: 0;
          right: 0;
          bottom: 0;
          height: 1px;
          background: linear-gradient(90deg, transparent, rgba(0, 200, 255, 0.35), transparent);
          opacity: 0.9;
        }
        .hd-accent {
          width: 78px;
          height: 3px;
          border-radius: 2px;
          flex: 0 0 auto;
          background: linear-gradient(90deg, #22d3ee, #38bdf8 55%, rgba(56, 189, 248, 0.05));
          box-shadow: 0 0 14px rgba(0, 180, 255, 0.65), 0 0 4px rgba(0, 120, 255, 0.9);
        }
        .hd-tri {
          width: auto;
          height: 9px;
          flex: 0 0 auto;
          opacity: 0.92;
          filter: drop-shadow(0 0 5px rgba(255, 214, 102, 0.45));
        }
        .hd-title {
          flex: 1;
          min-width: 0;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          font-size: 15px;
          font-weight: 700;
          color: #ecfeff;
          letter-spacing: 0.12em;
          text-shadow: 0 0 12px rgba(0, 140, 255, 0.35);
        }
        .hd-dots {
          display: flex;
          gap: 5px;
          flex: 0 0 auto;
          align-items: center;
        }
        .hd-dots span {
          width: 3px;
          height: 9px;
          border-radius: 1px;
          background: linear-gradient(180deg, #5eead4, #0ea5e9);
          box-shadow: 0 0 8px rgba(34, 211, 238, 0.55);
          opacity: 0.9;
        }
      </style>
      <div class="panel-hd">
        <div class="hd-accent"></div>
        ${flair ? `<img class="hd-tri" src="${A}/labor-hdr-tri-left.svg" alt="" />` : ""}
        <div class="hd-title">${escapeHtml(title)}</div>
        ${flair ? `<img class="hd-tri" src="${A}/labor-hdr-tri-right.svg" alt="" />` : ""}
        <div class="hd-dots"><span></span><span></span><span></span></div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-section-title")) {
  customElements.define("mei-cockpit-section-title", MeiCockpitSectionTitle);
}
