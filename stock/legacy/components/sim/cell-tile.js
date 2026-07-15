import { escapeHtml, parseProps } from "./runtime.js";

class MeiSimCellTile extends HTMLElement {
  connectedCallback() {
    this.props = parseProps(this);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
  }

  render() {
    const cell = this.props.cell || {};
    const meta = [
      cell.hazard_state ? `hazard=${cell.hazard_state}` : null,
      cell.flammable === true ? "flammable" : null,
      cell.flammable === false ? "nonflammable" : null,
      ...(cell.tags || []).map((tag) => `#${tag}`),
    ]
      .filter(Boolean)
      .join(" · ");
    const timer =
      typeof cell.hazard_timer_seconds === "number"
        ? `<span class="timer">${escapeHtml(String(cell.hazard_timer_seconds))}</span>`
        : "";
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .tile { border: 1px solid rgba(148,163,184,.2); border-radius: 12px; padding: 10px; background: rgba(2,6,23,.34); color: #e2e8f0; display: grid; gap: 8px; }
        .head { display: flex; justify-content: space-between; align-items: center; font-size: 12px; color: #dbeafe; }
        .meta { font-size: 11px; color: #94a3b8; }
        .timer { min-width: 20px; text-align: center; border-radius: 999px; background: rgba(14,116,144,.35); padding: 2px 6px; }
      </style>
      <section class="tile">
        <div class="head">
          <span>${escapeHtml(cell.id || "cell")}</span>
          ${timer}
        </div>
        <div class="meta">${escapeHtml(meta || "-")}</div>
      </section>
    `;
  }
}

if (!customElements.get("mei-sim-cell-tile")) {
  customElements.define("mei-sim-cell-tile", MeiSimCellTile);
}
