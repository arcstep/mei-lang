class MeiCockpitRatioSummary extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.render();
  }

  render() {
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          color: var(--mei-color-text-primary, #e0f2fe);
          font-size: var(--mei-font-2, 14px);
          font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif;
        }
        .wrap { padding: 6px 12px 14px; }
        .stat-grid {
          display: grid;
          grid-template-columns: 1fr 1fr 1fr;
          gap: 8px;
          font-size: var(--mei-font-1, 12px);
          color: var(--mei-color-text-muted, #94a3b8);
          margin-bottom: 10px;
        }
        .stat-grid strong { display: block; font-size: var(--mei-font-4, 20px); color: var(--mei-color-text-accent, #fde68a); margin-top: 2px; }
        .foot-metrics {
          display: grid;
          grid-template-columns: repeat(3, 1fr);
          gap: 8px;
          font-size: var(--mei-font-1, 11px);
          color: var(--mei-color-text-muted, #94a3b8);
        }
        .foot-metrics div { text-align: center; padding: 6px; border-radius: 4px; background: rgba(15,23,42,.5); }
        .foot-metrics strong { display: block; color: var(--mei-color-text-accent, #fde68a); margin-top: 4px; font-size: var(--mei-font-2, 13px); }
      </style>
      <div class="wrap">
        <div class="stat-grid">
          <div>代发人数 · 累计<strong>3452人</strong></div>
          <div>本年<strong>3452人</strong></div>
          <div>7月<strong>340人</strong></div>
        </div>
        <div class="foot-metrics">
          <div>累计占比<strong>34.52%</strong></div>
          <div>本年占比<strong>34.52%</strong></div>
          <div>7月占比<strong>34.52%</strong></div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-ratio-summary")) {
  customElements.define("mei-cockpit-ratio-summary", MeiCockpitRatioSummary);
}
