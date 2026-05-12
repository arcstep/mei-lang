import { getRuntimeStore, parseProps } from "./sim-runtime.js";

class MeiSimStepBridge extends HTMLElement {
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
    const visible = this.props.visible === true;
    const message = this.snapshot?.error || "";
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: ${visible ? "block" : "none"}; }
        .bridge { font-size: 12px; color: #fca5a5; }
      </style>
      <div class="bridge">${message}</div>
    `;
  }
}

if (!customElements.get("mei-sim-step-bridge")) {
  customElements.define("mei-sim-step-bridge", MeiSimStepBridge);
}
