import { escapeHtml, formatNowParts, parseProps } from "./shared.js";

class MeiCockpitHeaderClock extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.render();
    this._clockTimer = setInterval(() => this.updateClock(), 1000);
  }

  disconnectedCallback() {
    if (this._clockTimer) {
      clearInterval(this._clockTimer);
      this._clockTimer = null;
    }
  }

  updateClock() {
    const now = formatNowParts();
    const timeEl = this.shadowRoot?.querySelector(".time");
    const weekEl = this.shadowRoot?.querySelector(".week");
    const dateEl = this.shadowRoot?.querySelector(".date");
    if (!timeEl || !weekEl || !dateEl) return;
    timeEl.textContent = now.time;
    weekEl.textContent = now.weekday;
    dateEl.textContent = now.date;
  }

  render() {
    const p = parseProps(this);
    const now = formatNowParts();
    const week = p.weekday || now.weekday;
    const date = p.date || now.date;
    const time = p.time || now.time;
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; justify-self: end; align-self: center; min-width: 0; }
        .wrap {
          display: inline-flex;
          flex-wrap: nowrap;
          align-items: center;
          justify-content: flex-end;
          gap: 12px;
          min-width: 0;
          color: #e3f4fc;
        }
        .time {
          font-family: "DIN Alternate", "DINPro", "Barlow Condensed", Arial, sans-serif;
          font-size: 32px;
          font-weight: 700;
          line-height: 32px;
          letter-spacing: 1px;
          font-variant-numeric: tabular-nums;
          flex-shrink: 0;
        }
        .week, .date {
          font-size: 12px;
          line-height: 1;
          letter-spacing: 1px;
          white-space: nowrap;
        }
        .date { opacity: 0.6; }
      </style>
      <div class="wrap">
        <div class="time">${escapeHtml(time)}</div>
        <div class="week">${escapeHtml(week)}</div>
        <div class="date">${escapeHtml(date)}</div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-header-clock")) {
  customElements.define("mei-cockpit-header-clock", MeiCockpitHeaderClock);
}
