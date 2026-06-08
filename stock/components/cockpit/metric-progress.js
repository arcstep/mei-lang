import { parseProps } from "./shared.js";

function parsePercent(raw) {
  const text = String(raw ?? "").trim();
  if (!text) return 0;
  const numeric = Number(text.replace(/%/g, "").trim());
  if (!Number.isFinite(numeric)) return 0;
  if (text.includes("%")) return Math.max(0, Math.min(1, numeric / 100));
  if (numeric <= 1) return Math.max(0, Math.min(1, numeric));
  return Math.max(0, Math.min(1, numeric / 100));
}

function progressShell(props) {
  const shell = props.progress_shell ?? props.progressShell ?? {};
  return {
    insetX: String(shell.inset_x ?? shell.insetX ?? "14px"),
    extendX: String(shell.extend_x ?? shell.extendX ?? "0px"),
    height: String(shell.height ?? "14px"),
    radius: String(shell.border_radius ?? shell.borderRadius ?? "2px"),
    fill: String(shell.fill ?? shell.background ?? "#C9E9F8"),
  };
}

class MeiCockpitMetricProgress extends HTMLElement {
  connectedCallback() {
    const props = parseProps(this);
    const percent = parsePercent(props.value ?? props.percent);
    const shell = progressShell(props);

    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: flex;
          width: 100%;
          height: 100%;
          min-width: 0;
          min-height: 0;
          align-items: center;
          justify-content: center;
          box-sizing: border-box;
        }
        .wrap {
          width: calc(100% + ${shell.extendX} + ${shell.extendX});
          max-width: none;
          height: 100%;
          margin: 0 -${shell.extendX};
          padding: 0 ${shell.insetX};
          box-sizing: border-box;
          display: flex;
          align-items: center;
          justify-content: stretch;
        }
        .track {
          width: 100%;
          height: ${shell.height};
          background: rgba(201, 233, 248, 0.18);
          border-radius: ${shell.radius};
          overflow: hidden;
        }
        .fill {
          width: ${Math.round(percent * 10000) / 100}%;
          height: 100%;
          background: ${shell.fill};
          border-radius: ${shell.radius};
        }
      </style>
      <div class="wrap">
        <div class="track">
          <div class="fill"></div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-metric-progress")) {
  customElements.define("mei-cockpit-metric-progress", MeiCockpitMetricProgress);
}
