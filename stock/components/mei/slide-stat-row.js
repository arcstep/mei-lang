/**
 * mei.slide-stat-row — 2–4 static KPI tiles for slides.
 * Props: items / stats / metrics as [{label,value,hint?}] or label_n / value_n pairs.
 */
import { parseProps } from "../dataset/runtime-query.js";

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function asItemList(raw) {
  if (Array.isArray(raw)) return raw;
  if (raw && typeof raw === "object") {
    if (Array.isArray(raw.items)) return raw.items;
    if (Array.isArray(raw.rows)) return raw.rows;
  }
  return null;
}

function normalizeItem(entry) {
  if (entry == null) return null;
  if (typeof entry === "string" || typeof entry === "number") {
    return { label: "", value: String(entry), hint: "" };
  }
  if (typeof entry !== "object") return null;
  const label = entry.label ?? entry.title ?? entry.name ?? "";
  const value = entry.value ?? entry.metric ?? entry.count ?? "";
  const hint = entry.hint ?? entry.unit ?? entry.subtitle ?? "";
  if (!String(label).trim() && !String(value).trim()) return null;
  return {
    label: String(label ?? "").trim(),
    value: String(value ?? "").trim(),
    hint: String(hint ?? "").trim(),
  };
}

function resolveItems(props) {
  const fromList =
    asItemList(props.items) ||
    asItemList(props.stats) ||
    asItemList(props.metrics) ||
    asItemList(props.data);
  if (fromList) {
    return fromList.map(normalizeItem).filter(Boolean).slice(0, 4);
  }
  const paired = [];
  for (let index = 1; index <= 4; index += 1) {
    const label = props[`label_${index}`] ?? props[`label${index}`];
    const value = props[`value_${index}`] ?? props[`value${index}`];
    const hint = props[`hint_${index}`] ?? props[`hint${index}`] ?? "";
    if (label == null && value == null) continue;
    const item = normalizeItem({ label, value, hint });
    if (item) paired.push(item);
  }
  return paired.slice(0, 4);
}

const STYLE = `
  :host {
    display: block;
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
  }
  .row {
    display: grid;
    grid-template-columns: repeat(var(--mei-stat-cols, 3), minmax(0, 1fr));
    gap: clamp(0.75rem, 1.4vw, 1.25rem);
    width: 100%;
    min-width: 0;
  }
  .stat {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.35rem;
    min-width: 0;
    padding: 0.95rem 1.05rem;
    border-radius: 14px;
    border: 1px solid rgba(148, 163, 184, 0.28);
    background: rgba(15, 23, 42, 0.72);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
  }
  .value {
    font-family: "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Noto Sans SC", system-ui, sans-serif;
    font-size: clamp(1.7rem, 2.6vw, 2.35rem);
    font-weight: 700;
    line-height: 1.15;
    letter-spacing: 0.01em;
    color: var(--mei-slide-claim, #f8fafc);
    word-break: break-word;
  }
  .label {
    font-family: "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Noto Sans SC", system-ui, sans-serif;
    font-size: clamp(0.95rem, 1.25vw, 1.15rem);
    font-weight: 500;
    line-height: 1.35;
    color: var(--mei-slide-muted, #cbd5e1);
  }
  .hint {
    font-size: clamp(0.8rem, 1.05vw, 0.95rem);
    font-weight: 400;
    color: rgba(148, 163, 184, 0.95);
  }
  .empty {
    color: var(--mei-slide-muted, #94a3b8);
    font-size: 1rem;
  }
`;

class MeiSlideStatRow extends HTMLElement {
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
    const items = resolveItems(props);
    const cols = Math.max(2, Math.min(4, items.length || 2));
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const body = items.length
      ? items
          .map(
            (item) => `
        <div class="stat">
          <div class="value">${escapeHtml(item.value)}</div>
          <div class="label">${escapeHtml(item.label)}</div>
          ${item.hint ? `<div class="hint">${escapeHtml(item.hint)}</div>` : ""}
        </div>`
          )
          .join("")
      : `<div class="empty">暂无指标</div>`;
    this.shadowRoot.innerHTML = `
      <style>${STYLE}</style>
      <div class="row" style="--mei-stat-cols: ${cols}" role="group">
        ${body}
      </div>
    `;
  }
}

if (!customElements.get("mei-slide-stat-row")) {
  customElements.define("mei-slide-stat-row", MeiSlideStatRow);
}
