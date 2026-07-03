import { escapeHtml, parseProps } from "./shared.js";

const TAG = "mei-cockpit-scene-action-bar";

function dispatchPresentationAction(action) {
  if (!action || typeof action !== "object") {
    return false;
  }
  const dispatch =
    window.MeiPresentation?.dispatch ||
    window.__meiLangBoot?.dispatchPresentationAction;
  if (typeof dispatch !== "function") {
    return false;
  }
  return dispatch(action);
}

class MeiCockpitSceneActionBar extends HTMLElement {
  connectedCallback() {
    this.props = parseProps(this);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
  }

  render() {
    const actions = Array.isArray(this.props?.actions) ? this.props.actions : [];
    const buttons = actions
      .map((entry, index) => {
        const label = escapeHtml(String(entry?.label || entry?.text || `操作 ${index + 1}`));
        return `<button type="button" class="action-btn" data-action-index="${index}">${label}</button>`;
      })
      .join("");
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
        }
        .bar {
          display: flex;
          flex-wrap: wrap;
          gap: 8px;
          padding: 8px 0 0;
        }
        .action-btn {
          flex: 1 1 auto;
          min-width: 0;
          padding: 7px 10px;
          border-radius: 8px;
          border: 1px solid rgba(148, 163, 184, 0.35);
          background: rgba(15, 40, 72, 0.82);
          color: #e2e8f0;
          font-size: 12px;
          line-height: 1.3;
          cursor: pointer;
        }
        .action-btn:hover {
          border-color: rgba(250, 204, 21, 0.55);
          color: #fef08a;
        }
      </style>
      <div class="bar">${buttons}</div>
    `;
    this.shadowRoot.querySelectorAll("button[data-action-index]").forEach((button) => {
      button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        const index = Number(button.getAttribute("data-action-index"));
        const entry = actions[index];
        const action = entry?.presentationAction || entry?.presentation_action;
        dispatchPresentationAction(action);
      });
    });
  }
}

if (!customElements.get(TAG)) {
  customElements.define(TAG, MeiCockpitSceneActionBar);
}
