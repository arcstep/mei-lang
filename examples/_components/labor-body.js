import { escapeAttr, parseProps } from "./labor-shared.js";
import "./labor-section-title.js";
import "./labor-subcontract-table.js";
import "./labor-ranking-list.js";
import "./labor-person-cards.js";
import "./labor-kpi-strip.js";
import "./labor-donut-row.js";

class MeiCockpitLaborBody extends HTMLElement {
  connectedCallback() {
    this.props = parseProps(this);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
  }

  render() {
    const p = this.props || {};
    const subProps = escapeAttr(JSON.stringify({ dataset: p.subcontract }));
    const rankL = escapeAttr(
      JSON.stringify({
        dataset: p.rankingLeft,
        headers: ["字段名称一", "字段名称一", "字段名称一"],
      }),
    );
    const rankR = escapeAttr(
      JSON.stringify({
        dataset: p.rankingRight,
        headers: ["字段名称一", "字段名称一"],
      }),
    );
    const personsP = escapeAttr(JSON.stringify({ dataset: p.persons, monthLabel: "7月" }));
    const kpiP = escapeAttr(JSON.stringify({ kpis: p.kpis }));
    const donutP = escapeAttr(
      JSON.stringify({
        donutVisit: p.donutVisit,
        donutStd: p.donutStd,
      }),
    );

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          min-height: 100%;
          font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif;
        }
        .labor-root {
          min-height: 100%;
          background: transparent;
          border: none;
          border-radius: 0;
          overflow: hidden;
          color: #e0f2fe;
        }
        .labor-body {
          display: grid;
          grid-template-columns: minmax(260px, 28%) minmax(320px, 1.6fr) minmax(260px, 28%);
          gap: 12px;
          padding: 12px 14px 16px;
          align-items: start;
        }
        @media (max-width: 1100px) {
          .labor-body { grid-template-columns: 1fr; }
        }
        .labor-col { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
        .labor-panel {
          position: relative;
          border: none;
          border-radius: 0;
          background: transparent;
          box-shadow: none;
          overflow: hidden;
        }
        .labor-panel::before {
          content: "";
          position: absolute;
          inset: 0;
          z-index: 0;
          pointer-events: none;
          background: url("/workspace-components/labor-figma/labor-panel-bg.png") center / 100% 100% no-repeat;
          filter: saturate(1.05);
        }
        .labor-panel > * {
          position: relative;
          z-index: 1;
        }
        .labor-panel-bd { padding: 6px 12px 14px; }
        .center-stack { display: flex; flex-direction: column; gap: 12px; }
        .center-mid {
          display: grid;
          grid-template-columns: 1fr 1fr;
          gap: 10px;
        }
        @media (max-width: 900px) {
          .center-mid { grid-template-columns: 1fr; }
        }
        .stat-card {
          border-radius: 4px;
          padding: 10px 12px;
          border: 1px solid rgba(56,189,248,.15);
          background: rgba(8,47,73,.4);
        }
        .stat-card h4 { margin: 0 0 8px; font-size: 13px; color: #bae6fd; font-weight: 600; }
        .stat-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; font-size: 12px; color: #94a3b8; }
        .stat-grid.tri { grid-template-columns: 1fr 1fr 1fr; }
        .stat-grid strong { display: block; font-size: 20px; color: #fde68a; margin-top: 2px; }
        .digits { display: flex; gap: 6px; margin-top: 10px; flex-wrap: wrap; }
        .digit {
          min-width: 34px;
          text-align: center;
          padding: 6px 4px;
          border-radius: 4px;
          font-size: 18px;
          font-weight: 800;
          color: #f0f9ff;
          background: rgba(15,23,42,.85);
          border: 1px solid rgba(56,189,248,.2);
        }
        .foot-metrics {
          display: grid;
          grid-template-columns: repeat(3, 1fr);
          gap: 8px;
          font-size: 11px;
          color: #94a3b8;
        }
        .foot-metrics div { text-align: center; padding: 6px; border-radius: 4px; background: rgba(15,23,42,.5); }
        .foot-metrics strong { display: block; color: #fde68a; margin-top: 4px; font-size: 13px; }
      </style>
      <div class="labor-root">
        <div class="labor-body">
          <div class="labor-col labor-col-left">
            <section class="labor-panel">
              <mei-cockpit-labor-section-title data-props="${escapeAttr(JSON.stringify({ title: "这是板块标题" }))}"></mei-cockpit-labor-section-title>
              <div class="labor-panel-bd">
                <mei-cockpit-labor-subcontract-table data-props="${subProps}"></mei-cockpit-labor-subcontract-table>
              </div>
            </section>
            <section class="labor-panel">
              <mei-cockpit-labor-section-title data-props="${escapeAttr(JSON.stringify({ title: "这是板块标题" }))}"></mei-cockpit-labor-section-title>
              <div class="labor-panel-bd">
                <mei-cockpit-labor-ranking-list data-props="${rankL}"></mei-cockpit-labor-ranking-list>
              </div>
            </section>
          </div>
          <div class="labor-col labor-col-center">
            <div class="center-stack">
              <mei-cockpit-labor-kpi-strip data-props="${kpiP}"></mei-cockpit-labor-kpi-strip>
              <div class="center-mid">
                <div class="stat-card">
                  <h4>本年工人劳动合同签订人数</h4>
                  <div class="stat-grid">
                    <div>累计代发农民工工资金额：<strong>258.12</strong><span style="font-size:11px;color:#64748b"> 万元</span></div>
                    <div>累计代发农民工工资金额：<strong>258.12</strong><span style="font-size:11px;color:#64748b"> 万元</span></div>
                  </div>
                  <div class="digits"><span class="digit">2</span><span class="digit">3</span><span class="digit">5</span><span class="digit">1</span></div>
                </div>
                <div class="stat-card">
                  <h4>本年工人劳动合同签订人数</h4>
                  <div class="stat-grid">
                    <div>累计代发农民工工资金额：<strong>258.12</strong><span style="font-size:11px;color:#64748b"> 万元</span></div>
                    <div>累计代发农民工工资金额：<strong>258.12</strong><span style="font-size:11px;color:#64748b"> 万元</span></div>
                  </div>
                  <div class="digits"><span class="digit">3</span><span class="digit">5</span><span class="digit">5</span><span class="digit">8</span></div>
                </div>
              </div>
              <section class="labor-panel">
                <mei-cockpit-labor-section-title data-props="${escapeAttr(JSON.stringify({ title: "这是板块标题" }))}"></mei-cockpit-labor-section-title>
                <div class="labor-panel-bd">
                  <mei-cockpit-labor-donut-row data-props="${donutP}"></mei-cockpit-labor-donut-row>
                  <div class="foot-metrics" style="margin-top:10px">
                    <div>累计<strong>3542人次</strong></div>
                    <div>本年<strong>3542人次</strong></div>
                    <div>7月<strong>340人次</strong></div>
                  </div>
                </div>
              </section>
              <section class="labor-panel">
                <mei-cockpit-labor-section-title data-props="${escapeAttr(JSON.stringify({ title: "这是板块标题" }))}"></mei-cockpit-labor-section-title>
                <div class="labor-panel-bd">
                  <div class="stat-grid tri" style="margin-bottom:10px">
                    <div>代发人数 · 累计<strong>3452人</strong></div>
                    <div>本年<strong>3452人</strong></div>
                    <div>7月<strong>340人</strong></div>
                  </div>
                  <div class="foot-metrics">
                    <div>累计占比<strong>34.52%</strong></div>
                    <div>本年占比<strong>34.52%</strong></div>
                    <div>7月占比<strong>34.52%</strong></div>
                  </div>
                </div>
              </section>
            </div>
          </div>
          <div class="labor-col labor-col-right">
            <section class="labor-panel">
              <mei-cockpit-labor-section-title data-props="${escapeAttr(JSON.stringify({ title: "这是板块标题" }))}"></mei-cockpit-labor-section-title>
              <div class="labor-panel-bd">
                <mei-cockpit-labor-person-cards data-props="${personsP}"></mei-cockpit-labor-person-cards>
              </div>
            </section>
            <section class="labor-panel">
              <mei-cockpit-labor-section-title data-props="${escapeAttr(JSON.stringify({ title: "这是板块标题" }))}"></mei-cockpit-labor-section-title>
              <div class="labor-panel-bd">
                <mei-cockpit-labor-ranking-list data-props="${rankR}"></mei-cockpit-labor-ranking-list>
              </div>
            </section>
          </div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-cockpit-labor-body")) {
  customElements.define("mei-cockpit-labor-body", MeiCockpitLaborBody);
}
