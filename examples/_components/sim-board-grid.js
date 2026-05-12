import { escapeHtml, getRuntimeStore, parseProps } from "./sim-runtime.js";

function cellClass(cell) {
  const classes = ["cell"];
  if (cell.hazard_state) {
    classes.push(`hazard-${cell.hazard_state}`);
  }
  if (cell.key_target) {
    classes.push("key-target");
  }
  return classes.join(" ");
}

class MeiSimBoardGrid extends HTMLElement {
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
    const contract = this.snapshot?.contract || {};
    const topologyCols = Number(contract?.world?.topology?.cols || 1);
    const cols = Math.max(1, topologyCols);
    const loading = !!this.snapshot?.loading;
    const running = view.phase === "running";
    const cells = (view.cells || [])
      .map((cell) => {
        const timerBadge =
          typeof cell.hazard_timer_seconds === "number" && cell.hazard_timer_seconds >= 0
            ? `<span class="timer">${escapeHtml(String(cell.hazard_timer_seconds))}</span>`
            : "";
        const entities = (cell.entities || [])
          .map((entity) => {
            const status = entity.status ? ` · ${entity.status}` : "";
            return `<button class="entity-btn" data-target="${escapeHtml(entity.id)}" ${loading || !running ? "disabled" : ""}>${escapeHtml((entity.label || entity.id) + status)}</button>`;
          })
          .join("");
        const target = cell.interaction_target || "";
        const meta = [
          cell.hazard_state ? `hazard=${cell.hazard_state}` : null,
          cell.flammable === true ? "flammable" : null,
          cell.flammable === false ? "nonflammable" : null,
          ...(cell.tags || []).map((tag) => `#${tag}`),
        ]
          .filter(Boolean)
          .join(" · ");
        return `
          <div class="${cellClass(cell)}">
            <button class="cell-hit" data-target="${escapeHtml(target)}" ${loading || !running || !cell.clickable ? "disabled" : ""}>
              <span class="id">${escapeHtml(cell.id)}</span>
              ${timerBadge}
            </button>
            <div class="meta">${escapeHtml(meta || "-")}</div>
            <div class="entities">${entities || `<span class="empty">empty</span>`}</div>
          </div>
        `;
      })
      .join("");

    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .board { display: grid; gap: 10px; grid-template-columns: repeat(${cols}, minmax(0, 1fr)); }
        .cell { border: 1px solid rgba(148,163,184,.18); border-radius: 12px; padding: 8px; background: rgba(2,6,23,.34); display: grid; gap: 8px; }
        .key-target { box-shadow: inset 0 0 0 1px rgba(59,130,246,.4); }
        .cell-hit { appearance: none; display: flex; justify-content: space-between; align-items: center; border: 1px solid rgba(96,165,250,.3); background: rgba(30,41,59,.9); color: #e2e8f0; border-radius: 8px; padding: 6px 8px; cursor: pointer; }
        .cell-hit[disabled] { opacity: .55; cursor: not-allowed; }
        .id { font-size: 12px; color: #dbeafe; }
        .timer { min-width: 22px; text-align: center; border-radius: 999px; background: rgba(14,116,144,.35); font-size: 12px; padding: 2px 6px; }
        .meta { font-size: 11px; color: #94a3b8; min-height: 14px; }
        .entities { display: grid; gap: 6px; }
        .entity-btn { appearance: none; border: 1px solid rgba(96,165,250,.3); background: rgba(15,23,42,.92); color: #e2e8f0; border-radius: 8px; padding: 6px 8px; cursor: pointer; text-align: left; }
        .entity-btn[disabled] { opacity: .55; cursor: not-allowed; }
        .empty { font-size: 12px; color: #64748b; }
        .hazard-smoke { border-color: rgba(245,158,11,.35); background: rgba(120,53,15,.22); }
        .hazard-burning { border-color: rgba(239,68,68,.45); background: rgba(127,29,29,.28); }
        .hazard-engulfed, .hazard-big { border-color: rgba(220,38,38,.55); background: rgba(69,10,10,.36); }
        .hazard-out { border-color: rgba(74,222,128,.35); background: rgba(20,83,45,.25); }
      </style>
      <section class="board">
        ${cells}
      </section>
    `;
    this.shadowRoot.querySelectorAll("[data-target]").forEach((button) => {
      button.addEventListener("click", () => {
        const target = button.dataset.target;
        if (!target) {
          return;
        }
        this.store.sendIntent({ kind: "click", target });
      });
    });
  }
}

if (!customElements.get("mei-sim-board-grid")) {
  customElements.define("mei-sim-board-grid", MeiSimBoardGrid);
}
