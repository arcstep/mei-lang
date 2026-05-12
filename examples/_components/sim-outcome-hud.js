import { escapeHtml, getRuntimeStore, parseProps } from "./sim-runtime.js";

function runningHint(view) {
  const cells = view.cells || [];
  const targetCell = cells.find((cell) => cell.key_target) || cells.find((cell) => cell.clickable);
  const inventory = view.inventory || [];
  if (!targetCell) {
    return view.summary || "场景运行中";
  }
  if (!inventory.includes("extinguisher_1")) {
    return "先点击“拾取 灭火器”，把工具收入库存。";
  }
  if (targetCell.hazard_state === "smoke") {
    const seconds = targetCell.hazard_timer_seconds;
    return `目标格 ${targetCell.id} 正在冒烟${typeof seconds === "number" ? `，约 ${seconds}s 后会起火。` : "。"} `;
  }
  if (targetCell.hazard_state === "burning") {
    const seconds = targetCell.hazard_timer_seconds;
    return `目标格 ${targetCell.id} 已是小火，请立刻点击该格扑灭${typeof seconds === "number" ? `，否则约 ${seconds}s 后会升级为大火。` : "。"} `;
  }
  if (targetCell.hazard_state === "out") {
    return `目标格 ${targetCell.id} 已扑灭，演练成功。`;
  }
  return view.summary || "场景运行中";
}

function displayMessage(view, phase, reason) {
  if (phase === "ready") {
    return "点击开始后，先拾取灭火器，再处置目标格。";
  }
  if (phase === "running") {
    return runningHint(view);
  }
  return reason || view.summary || "-";
}

class MeiSimOutcomeHud extends HTMLElement {
  connectedCallback() {
    this.props = parseProps(this);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.store = getRuntimeStore(this.props);
    this.unsubscribe = this.store.subscribe((snapshot) => {
      this.snapshot = snapshot;
      this.render();
    });
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
    const phase = view.outcome_state || view.phase || "ready";
    const reason = view.outcome_message || view.reason || "";
    const message = displayMessage(view, phase, reason);
    const className =
      phase === "success"
        ? "success"
        : phase === "fail"
          ? "fail"
          : phase === "running"
            ? "running"
            : "ready";
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .hud { border: 1px solid rgba(148,163,184,.2); border-radius: 12px; padding: 10px; background: rgba(15,23,42,.76); color: #e2e8f0; display: grid; gap: 6px; }
        .phase { font-size: 12px; letter-spacing: .04em; text-transform: uppercase; font-weight: 700; }
        .msg { font-size: 13px; color: #cbd5e1; min-height: 18px; }
        .ready .phase { color: #93c5fd; }
        .running .phase { color: #60a5fa; }
        .success .phase { color: #4ade80; }
        .fail .phase { color: #f87171; }
      </style>
      <section class="hud ${className}">
        <div class="phase">${escapeHtml(phase)}</div>
        <div class="msg">${escapeHtml(message)}</div>
      </section>
    `;
  }
}

if (!customElements.get("mei-sim-outcome-hud")) {
  customElements.define("mei-sim-outcome-hud", MeiSimOutcomeHud);
}
