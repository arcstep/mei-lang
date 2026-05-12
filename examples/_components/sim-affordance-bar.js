import { escapeHtml, getRuntimeStore, parseProps } from "./sim-runtime.js";

class MeiSimAffordanceBar extends HTMLElement {
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
    if (!this.shadowRoot) {
      return;
    }
    const view = this.snapshot?.sceneView || {};
    const loading = !!this.snapshot?.loading;
    const actions = (view.available_actions || []).filter((action) => action !== "tick");
    const labelFor = (action) => {
      if (action === "start") return view.start_label || "开始";
      if (action === "pause") return "暂停时钟";
      if (action === "resume") return "恢复时钟";
      if (action === "rate_half") return "0.5x";
      if (action === "rate_normal") return "1x";
      if (action === "rate_double") return "2x";
      if (action === "restart") return "重新开始";
      return action;
    };
    const error = this.snapshot?.error
      ? `<p class="error">${escapeHtml(this.snapshot.error)}</p>`
      : "";
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .bar { display: grid; gap: 8px; border: 1px solid rgba(148,163,184,.2); border-radius: 12px; padding: 10px; background: rgba(15,23,42,.76); color: #e2e8f0; }
        .meta { display: flex; flex-wrap: wrap; gap: 10px; color: #94a3b8; font-size: 12px; }
        .actions { display: flex; flex-wrap: wrap; gap: 8px; }
        .action-btn { appearance: none; border: 1px solid rgba(96,165,250,.3); background: rgba(30,41,59,.9); color: #e2e8f0; border-radius: 10px; padding: 7px 10px; cursor: pointer; }
        .action-btn[disabled] { opacity: .55; cursor: not-allowed; }
        .error { margin: 0; color: #fca5a5; font-size: 12px; }
      </style>
      <section class="bar">
        <div class="meta">
          <span>phase=${escapeHtml(view.phase || "ready")}</span>
          <span>countdown=${escapeHtml(String(view.countdown ?? 0))}</span>
          <span>time=${escapeHtml(String(view.current_time ?? 0))} ${escapeHtml(view.time_unit || "second")}</span>
          <span>rate=${escapeHtml(String(view.time_rate ?? 1))}x</span>
          <span>inventory=${escapeHtml((view.inventory || []).join(", ") || "-")}</span>
        </div>
        ${error}
        <div class="actions">
          ${actions
            .map(
              (action) =>
                `<button class="action-btn" data-intent="${escapeHtml(action)}" ${loading ? "disabled" : ""}>${escapeHtml(labelFor(action))}</button>`,
            )
            .join("")}
        </div>
      </section>
    `;
    this.shadowRoot.querySelectorAll("[data-intent]").forEach((button) => {
      button.addEventListener("click", () => {
        this.store.sendIntent({ kind: button.dataset.intent });
      });
    });
  }
}

if (!customElements.get("mei-sim-affordance-bar")) {
  customElements.define("mei-sim-affordance-bar", MeiSimAffordanceBar);
}
