import "./chart-donut.js";
import { escapeAttr, parseProps } from "./labor-shared.js";

function donutProps(title, dataset, mapping) {
  return JSON.stringify({
    title,
    dataset,
    mapping,
  });
}

class MeiCockpitLaborDonutRow extends HTMLElement {
  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    const p = parseProps(this);
    const left = donutProps(
      p.leftTitle || "代发人次结构",
      p.donutVisit,
      {
        label: [{ field: "label", name: "类别" }],
        y: [{ field: "value", name: "数值" }],
      },
    );
    const right = donutProps(
      p.rightTitle || "代发标准结构",
      p.donutStd,
      {
        label: [{ field: "label", name: "类别" }],
        y: [{ field: "value", name: "数值" }],
      },
    );
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .donut-row {
          display: flex; flex-wrap: wrap; gap: 12px; justify-content: center; align-items: stretch;
        }
        .donut-slot {
          flex: 1 1 220px; min-width: 200px;
          max-width: 360px;
        }
        .donut-slot mei-chart-donut { display: block; }
        .cap {
          margin-top: 6px; text-align: center; font-size: 12px; color: #94a3b8; line-height: 1.45;
        }
        .cap strong { color: #e0f2fe; }
      </style>
      <div class="donut-row">
        <div class="donut-slot">
          <mei-chart-donut data-props="${escapeAttr(left)}"></mei-chart-donut>
          <div class="cap"><strong>代发人次</strong>累计 / 本年 / 7月（示意切片）</div>
        </div>
        <div class="donut-slot">
          <mei-chart-donut data-props="${escapeAttr(right)}"></mei-chart-donut>
          <div class="cap"><strong>平均代发标准</strong>元/人次结构示意</div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-donut-row")) {
  customElements.define("mei-cockpit-labor-donut-row", MeiCockpitLaborDonutRow);
}
