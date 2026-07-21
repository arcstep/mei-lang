/**
 * mei.slide-callout — presentation action / conclusion bar.
 * Static props only (content / title / tone); no eval dependency.
 */
import { parseProps } from "../dataset/runtime-query.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function resolveText(props) {
  const candidates = [props.content, props.title, props.text, props.label];
  for (const value of candidates) {
    if (value != null && String(value).trim()) {
      return String(value).trim();
    }
  }
  return "";
}

function resolveTone(props) {
  const raw = String(props.tone || props.variant || "accent").trim().toLowerCase();
  if (raw === "warn" || raw === "warning") return "warn";
  if (raw === "muted" || raw === "soft") return "muted";
  return "accent";
}

const STYLE = `
  :host {
    display: block;
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
  }
  .callout {
    display: flex;
    align-items: flex-start;
    gap: 0.85rem;
    width: 100%;
    box-sizing: border-box;
    padding: 0.85rem 1.15rem;
    border-radius: 12px;
    border: 1px solid rgba(125, 211, 252, 0.35);
    background: linear-gradient(135deg, rgba(14, 116, 144, 0.35), rgba(15, 23, 42, 0.72));
    box-shadow: 0 10px 28px rgba(2, 6, 23, 0.35);
    color: var(--mei-slide-claim, #f8fafc);
  }
  .callout[data-tone="warn"] {
    border-color: rgba(251, 191, 36, 0.45);
    background: linear-gradient(135deg, rgba(180, 83, 9, 0.38), rgba(15, 23, 42, 0.72));
  }
  .callout[data-tone="muted"] {
    border-color: rgba(148, 163, 184, 0.35);
    background: rgba(30, 41, 59, 0.72);
  }
  .mark {
    flex: 0 0 auto;
    margin-top: 0.2em;
    width: 0.55rem;
    height: 0.55rem;
    border-radius: 999px;
    background: var(--mei-slide-accent, #7dd3fc);
    box-shadow: 0 0 0 4px rgba(125, 211, 252, 0.18);
  }
  .callout[data-tone="warn"] .mark {
    background: #fbbf24;
    box-shadow: 0 0 0 4px rgba(251, 191, 36, 0.18);
  }
  .callout[data-tone="muted"] .mark {
    background: #94a3b8;
    box-shadow: 0 0 0 4px rgba(148, 163, 184, 0.16);
  }
  .body {
    flex: 1 1 auto;
    min-width: 0;
    font-family: "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Noto Sans SC", system-ui, sans-serif;
    font-size: clamp(1.35rem, 1.9vw, 1.7rem);
    font-weight: 650;
    line-height: 1.4;
    letter-spacing: 0.01em;
  }
`;

class MeiSlideCallout extends HTMLElement {
  static get observedAttributes() {
    return ["data-props"];
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (name !== "data-props" || oldValue === newValue || !this.isConnected) return;
    this.render();
  }

  connectedCallback() {
    this.render();
  }

  render() {
    const props = parseProps(this);
    const text = resolveText(props);
    const tone = resolveTone(props);
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.shadowRoot.innerHTML = `
      <style>${STYLE}</style>
      <div class="callout" data-tone="${escapeHtml(tone)}" role="note">
        <span class="mark" aria-hidden="true"></span>
        <div class="body">${escapeHtml(text)}</div>
      </div>
    `;
  }
}

if (!customElements.get("mei-slide-callout")) {
  customElements.define("mei-slide-callout", MeiSlideCallout);
}
