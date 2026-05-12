import { escapeHtml, getRuntimeStore, parseProps } from "./sim-runtime.js";

class MeiSimOutcomeHud extends HTMLElement {
  connectedCallback() {
    this.props = parseProps(this);
    this.store = getRuntimeStore(this.props);
    this.unsubscribe = this.store.subscribe((snapshot) => {
      this.snapshot = snapshot;
      this.render();
    });
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
  }

  disconnectedCallback() {
    if (this.unsubscribe) {
      this.unsubscribe();
      this.unsubscribe = null;
    }
  }

  render() {
    const view = this.snapshot?.sceneView || {};
    const phase = view.outcome_state || view.phase || "ready";
    const reason = view.outcome_message || view.reason || "";
    const className =
      phase === "success"
        ? "success"
        : phase === "fail"
          ? "fail"
          : phase === "running"
            ? "running"
            : "ready";
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .hud { border: 1px solid rgba(148,163,184,.2); border-radius: 12px; padding: 10px; background: rgba(15,23,42,.76); color: #e2e8f0; display: grid; gap: 6px; }
        .phase { font-size: 12px; letter-spacing: .04em; text-transform: uppercase; font-weight: 700; }
        .msg { font-size: 13px; color: #cbd5e1; min-height: 18px; }
        .ready .phase { color: #93c5fd; }
        .running .phase { color: #60a5fa; }
        .success .phase { color: #4ade80; }
        .fail .phase { color: #f87171; }
      </style>
      <section class="hud ${className}">
        <div class="phase">${escapeHtml(phase)}</div>
        <div class="msg">${escapeHtml(reason || view.summary || "-")}</div>
      </section>
    `;
  }
}

if (!customElements.get("mei-sim-outcome-hud")) {
  customElements.define("mei-sim-outcome-hud", MeiSimOutcomeHud);
}
