/**
 * Thunder 指标依据：通报四件套 + 闪电次数 + |E|峰值（带阈值语义）。
 * Fill-down + 主题字号（metric_label / metric_value / metric_unit）。
 * 主要依据 / 建议措施：统一长文截断（单行省略 + … + floating popover）。
 */
import { parseProps, escapeHtml, escapeAttr } from "../cockpit/shared.js";
import { COCKPIT_TYPE, cockpitCssVars } from "../cockpit/tokens.js";
import { color } from "../mei/theme-style.js";
import {
  bindOverflowTextExpand,
  closeOverflowTextPopover,
  formatOverflowTextShellHtml,
  overflowTextShellStyleBlock,
  scheduleOverflowTextSync,
} from "../mei/overflow-text.js";
import { getThunderStore, levelTone, subscribeThunderState } from "./event-bus.js";
import { openThunderT2 } from "./t2-open.js";
import { EFIELD_ABS_HINT, LIGHTNING_FREQ_HINT, eAbsPeak, levelLabelColor } from "./thresholds.js";

function ePeakTone(absE) {
  if (!(absE > 0)) return {};
  if (absE >= 9) return { fg: levelLabelColor("红") };
  if (absE >= 7) return { fg: levelLabelColor("橙") };
  if (absE >= 3) return { fg: levelLabelColor("黄") };
  return { fg: color("text_value") };
}

function card(label, value, unit = "", options = {}) {
  const tone = options.tone || {};
  const title = options.title || String(value || "");
  const valueStyle = tone.fg ? `color:${tone.fg};` : "";
  const overflow = options.overflow === true;
  const overflowKey = String(options.overflowKey || label || "").trim();
  const display = String(value ?? "—");
  const board = String(options.board || "").trim();
  const valueHtml = overflow
    ? formatOverflowTextShellHtml(display, {
        key: overflowKey,
        expandLabel: "查看全文",
        className: "value",
      })
    : `<span class="value" style="${valueStyle}">${escapeHtml(display)}</span>`;
  const hostTitle = overflow ? "" : ` title="${escapeAttr(title)}"`;
  const boardAttr = board ? ` data-t2-board="${escapeAttr(board)}"` : "";
  const clickableClass = board ? " is-clickable" : "";
  return `
    <div class="card${clickableClass}"${hostTitle}${boardAttr}>
      <div class="label">${escapeHtml(label)}</div>
      <div class="value-row">
        ${valueHtml}
        ${unit ? `<span class="unit">${escapeHtml(unit)}</span>` : ""}
      </div>
    </div>`;
}

class MeiThunderEventSummary extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.style.display = "block";
    this.style.width = "100%";
    this.style.height = "100%";
    this.style.minHeight = "0";
    this.style.overflow = "hidden";
    this.style.boxSizing = "border-box";
    this._unsub = subscribeThunderState((detail) => this.render(detail));
    this.render(getThunderStore());
  }

  disconnectedCallback() {
    if (typeof this._unsub === "function") {
      this._unsub();
      this._unsub = null;
    }
    if (typeof this._unbindOverflow === "function") {
      this._unbindOverflow();
      this._unbindOverflow = null;
    }
    closeOverflowTextPopover(this);
  }

  render(state) {
    const event = state?.event || null;
    const level = String(state?.level || event?.level || "—");
    const tone = levelTone(level);
    const empty = !event;
    if (typeof this._unbindOverflow === "function") {
      this._unbindOverflow();
      this._unbindOverflow = null;
    } else {
      closeOverflowTextPopover(this);
    }

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          height: 100%;
          min-height: 0;
          max-height: 100%;
          overflow: hidden;
          box-sizing: border-box;
          ${cockpitCssVars()}
          font-family: var(--cockpit-font-family-ui);
        }
        .grid {
          display: grid;
          grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
          grid-template-rows: minmax(0, 1fr) minmax(0, 1fr) minmax(0, 1fr);
          gap: 6px;
          width: 100%;
          height: 100%;
          min-height: 0;
          max-height: 100%;
          box-sizing: border-box;
        }
        .card {
          display: flex;
          flex-direction: column;
          justify-content: center;
          align-items: center;
          gap: 4px;
          min-width: 0;
          min-height: 0;
          max-height: 100%;
          padding: 6px 8px;
          border-radius: 4px;
          background: rgba(10, 40, 78, 0.72);
          border: 1px solid rgba(56, 160, 240, 0.28);
          box-sizing: border-box;
          text-align: center;
          overflow: hidden;
        }
        .card.is-clickable {
          cursor: pointer;
        }
        .card.is-clickable:hover {
          border-color: rgba(56, 189, 248, 0.7);
          background: rgba(14, 52, 96, 0.88);
        }
        .label {
          flex: 0 0 auto;
          font-size: ${COCKPIT_TYPE.metricLabel};
          line-height: 1.2;
          color: ${color("text_muted")};
          letter-spacing: 0.02em;
          max-width: 100%;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .value-row {
          display: flex;
          align-items: baseline;
          justify-content: center;
          gap: 4px;
          width: 100%;
          max-width: 100%;
          min-width: 0;
          min-height: 0;
        }
        .value-row .mei-overflow-shell {
          flex: 1 1 auto;
          width: 100%;
          max-width: 100%;
        }
        .value {
          font-size: ${COCKPIT_TYPE.metricValue};
          font-weight: 700;
          color: ${color("text_value")};
          line-height: 1.2;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          max-width: 100%;
        }
        .unit {
          font-size: ${COCKPIT_TYPE.metricUnit};
          color: ${color("text_unit")};
          flex: 0 0 auto;
        }
        .empty {
          grid-column: 1 / -1;
          grid-row: 1 / -1;
          display: flex;
          align-items: center;
          justify-content: center;
          color: ${color("text_muted")};
          font-size: ${COCKPIT_TYPE.metricLabel};
          border: 1px dashed rgba(100, 116, 139, 0.45);
          border-radius: 4px;
          min-height: 0;
        }
        ${overflowTextShellStyleBlock()}
      </style>
      <div class="grid">
        ${
          empty
            ? `<div class="empty">暂未发现进行中预警</div>`
            : [
                card("预警级别", level, "", { tone, title: level, board: "lifecycle" }),
                card("预计有效", event.valid_until || "—", "", {
                  title: event.valid_until,
                  board: "lifecycle",
                }),
                card("主要依据", event.basis || "—", "", {
                  overflow: true,
                  overflowKey: "主要依据",
                  title: event.basis,
                  board: "lifecycle",
                }),
                card("建议措施", event.advice || "—", "", {
                  overflow: true,
                  overflowKey: "建议措施",
                  title: event.advice,
                  board: "lifecycle",
                }),
                card("事件闪电", event.lightning_count ?? "—", "次", {
                  title: `事件累计闪电 ${event.lightning_count ?? "—"} 次（定位仪；${LIGHTNING_FREQ_HINT}）`,
                  board: "lightning",
                }),
                (() => {
                  const peak = eAbsPeak(event);
                  const display = peak == null ? "—" : String(peak);
                  return card("|E|峰值", display, "kV/m", {
                    tone: ePeakTone(peak),
                    title: peak == null
                      ? EFIELD_ABS_HINT
                      : `|E|峰值 ${peak} kV/m（${EFIELD_ABS_HINT}；原文 −3/−7/−9）`,
                    board: "efield",
                  });
                })(),
              ].join("")
        }
      </div>
    `;
    this._unbindOverflow = bindOverflowTextExpand(this, this.shadowRoot, {
      titleForKey: (key) => key || "详细内容",
    });
    scheduleOverflowTextSync(this, this.shadowRoot);
    this.bindT2Clicks();
  }

  bindT2Clicks() {
    if (!this.shadowRoot) return;
    this.shadowRoot.querySelectorAll("[data-t2-board]").forEach((el) => {
      el.addEventListener("click", (event) => {
        if (event.target?.closest?.(".mei-overflow-expand")) return;
        const board = el.getAttribute("data-t2-board");
        if (!board) return;
        openThunderT2(board, { host: this });
      });
    });
  }
}

if (!customElements.get("mei-thunder-event-summary")) {
  customElements.define("mei-thunder-event-summary", MeiThunderEventSummary);
}
