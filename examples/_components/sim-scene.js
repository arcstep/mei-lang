class MeiSimScene extends HTMLElement {
  connectedCallback() {
    const props = parseProps(this);
    const contract = props.scene_contract || props;
    this.attachShadow({ mode: "open" });
    const scene = contract.scene || {};
    const world = contract.world || {};
    const entities = world.entities || [];
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .wrap { display: grid; gap: 12px; padding: 16px; border-radius: 16px; background: rgba(15,23,42,.78); border: 1px solid rgba(148,163,184,.18); color: #e2e8f0; }
        h3 { margin: 0; color: #f8fafc; }
        p, li { color: #cbd5e1; }
      </style>
      <section class="wrap">
        <h3>${escapeHtml(scene.id || "scene")}</h3>
        <p>${escapeHtml(scene.summary || "运行态组件将在后续阶段接入。")}</p>
        <ul>
          <li>goal: ${escapeHtml(scene.goal || "未声明")}</li>
          <li>entities: ${entities.length}</li>
          <li>start_label: ${escapeHtml(scene.start_label || "开始")}</li>
        </ul>
      </section>
    `;
  }
}

function parseProps(element) {
  try {
    return JSON.parse(element.dataset.props || "{}");
  } catch {
    return {};
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

customElements.define("mei-sim-scene", MeiSimScene);
