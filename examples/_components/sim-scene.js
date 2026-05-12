class MeiSimScene extends HTMLElement {
  connectedCallback() {
    this.props = parseProps(this);
    this.contract = this.props.scene || this.props.scene_contract || this.props;
    this.host = this.props._mei || {};
    this.runtimeState = null;
    this.sceneView = null;
    this.error = null;
    this.loading = false;
    this.autoTickHandle = null;
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
    this.sendIntent({ kind: "sync" });
  }

  disconnectedCallback() {
    this.clearAutoTick();
  }

  clearAutoTick() {
    if (this.autoTickHandle !== null) {
      clearTimeout(this.autoTickHandle);
      this.autoTickHandle = null;
    }
  }

  syncAutoTick(view) {
    this.clearAutoTick();
    if (!this.host.step_api || this.loading || this.error) {
      return;
    }
    if (!view || view.phase !== "running" || view.clock_paused) {
      return;
    }
    this.autoTickHandle = setTimeout(() => {
      this.autoTickHandle = null;
      this.sendIntent({ kind: "tick" });
    }, 1000);
  }

  async sendIntent(intent) {
    if (!this.host.step_api) {
      this.error = "缺少 step_api，无法进入运行态。";
      this.render();
      return;
    }
    this.loading = true;
    this.error = null;
    this.render();
    try {
      const payload = { intent };
      if (this.runtimeState) {
        payload.state = this.runtimeState;
      }
      const response = await fetch(this.host.step_api, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!response.ok) {
        throw new Error(`step 请求失败: ${response.status}`);
      }
      const data = await response.json();
      this.runtimeState = data.state || null;
      this.sceneView = data.scene_view || null;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    } finally {
      this.loading = false;
      this.render();
    }
  }

  render() {
    const contract = this.contract || {};
    const scene = contract.scene || {};
    const world = contract.world || {};
    const topology = world.topology || {};
    const view = this.sceneView || fallbackSceneView(contract, this.runtimeState);
    const timeline = this.runtimeState?.timeline || [];
    const visibleActions = (view.available_actions || []).filter((action) => action !== "tick");
    const statusText = this.error
      ? `<p class="error">${escapeHtml(this.error)}</p>`
      : `<p>${escapeHtml(view.summary || scene.summary || "scene runtime")}</p>`;
    const actionButtons = visibleActions
      .map((action) => {
        const label =
          action === "start"
            ? view.start_label || "开始"
            : action === "pause"
                ? "暂停时钟"
                : action === "resume"
                  ? "恢复时钟"
                  : action === "rate_half"
                    ? "0.5x"
                    : action === "rate_normal"
                      ? "1x"
                      : action === "rate_double"
                        ? "2x"
              : action === "restart"
                ? "重新开始"
                : action;
        return `<button class="action-btn" data-intent="${escapeHtml(action)}" ${this.loading ? "disabled" : ""}>${escapeHtml(label)}</button>`;
      })
      .join("");
    const gridColumns = Number(topology.cols || 1);
    const cells = (view.cells || [])
      .map((cell) => {
        const cellMeta = [
          cell.hazard_state ? `hazard=${cell.hazard_state}` : null,
          cell.flammable === true ? "flammable" : null,
          cell.flammable === false ? "nonflammable" : null,
          cell.walkable === true ? "walkable" : null,
          cell.occupiable === true ? "occupiable" : null,
          ...(cell.tags || []).map((tag) => `#${tag}`),
        ].filter(Boolean);
        const cellStateClass = cell.hazard_state ? ` hazard-${cell.hazard_state}` : "";
        const entities = (cell.entities || [])
          .map((entity) => {
            const label = entity.label || entity.id;
            const status = entity.status ? ` · ${entity.status}` : "";
            return `<button class="entity-btn" data-entity-id="${escapeHtml(entity.id)}" ${this.loading || view.phase !== "running" ? "disabled" : ""}>${escapeHtml(label + status)}</button>`;
          })
          .join("");
        return `
          <div class="cell${escapeHtml(cellStateClass)}">
            <div class="cell-id">${escapeHtml(cell.id)}</div>
            <div class="cell-meta">${cellMeta.length > 0 ? escapeHtml(cellMeta.join(" · ")) : "&nbsp;"}</div>
            <div class="cell-entities">${entities || `<span class="cell-empty">empty</span>`}</div>
          </div>
        `;
      })
      .join("");
    const entityList = (view.entities || [])
      .map((entity) => {
        const flags = Object.entries(entity.flags || {})
          .map(([key, value]) => `${key}=${value}`)
          .join(", ");
        return `<li><strong>${escapeHtml(entity.label || entity.id)}</strong> · ${escapeHtml(entity.kind)} · slot=${escapeHtml(entity.slot || "-")} · status=${escapeHtml(entity.status || "-")}${flags ? ` · flags=${escapeHtml(flags)}` : ""}</li>`;
      })
      .join("");
    this.shadowRoot.innerHTML = `
      <style>
        :host { display: block; }
        .wrap { display: grid; gap: 14px; padding: 16px; border-radius: 16px; background: rgba(15,23,42,.78); border: 1px solid rgba(148,163,184,.18); color: #e2e8f0; }
        .head { display: grid; gap: 8px; }
        .meta { display: flex; flex-wrap: wrap; gap: 10px; color: #94a3b8; font-size: 12px; }
        .actions { display: flex; flex-wrap: wrap; gap: 8px; }
        .action-btn, .entity-btn { appearance: none; border: 1px solid rgba(96,165,250,.28); background: rgba(30,41,59,.86); color: #e2e8f0; padding: 8px 10px; border-radius: 10px; cursor: pointer; }
        .action-btn[disabled], .entity-btn[disabled] { opacity: .55; cursor: not-allowed; }
        .grid { display: grid; gap: 10px; }
        .cell { border: 1px solid rgba(148,163,184,.16); border-radius: 12px; padding: 10px; min-height: 84px; display: grid; gap: 8px; background: rgba(2,6,23,.32); }
        .cell-id { font-size: 11px; color: #94a3b8; }
        .cell-meta { font-size: 11px; color: #7dd3fc; min-height: 14px; }
        .cell-entities { display: flex; flex-direction: column; gap: 6px; }
        .cell-empty { color: #64748b; font-size: 12px; }
        .hazard-smoke { border-color: rgba(245,158,11,.35); background: rgba(120,53,15,.22); }
        .hazard-burning { border-color: rgba(239,68,68,.4); background: rgba(127,29,29,.28); }
        .hazard-engulfed { border-color: rgba(220,38,38,.5); background: rgba(69,10,10,.34); }
        .cols { display: grid; gap: 14px; grid-template-columns: minmax(0, 2fr) minmax(280px, 1fr); }
        .side { display: grid; gap: 12px; }
        .card { border: 1px solid rgba(148,163,184,.14); border-radius: 12px; padding: 12px; background: rgba(2,6,23,.26); }
        .card h4 { margin: 0 0 8px; color: #f8fafc; }
        ul { margin: 0; padding-left: 18px; display: grid; gap: 6px; }
        p, li { color: #cbd5e1; }
        .error { color: #fca5a5; }
      </style>
      <section class="wrap">
        <div class="head">
          <h3>${escapeHtml(view.scene_id || scene.id || "scene")}</h3>
          ${statusText}
          <div class="meta">
            <span>phase=${escapeHtml(view.phase || "ready")}</span>
            <span>countdown=${escapeHtml(String(view.countdown ?? scene.state?.countdown ?? 0))}</span>
            <span>current_time=${escapeHtml(String(view.current_time ?? 0))} ${escapeHtml(view.time_unit || "second")}</span>
            <span>paused=${escapeHtml(String(!!view.clock_paused))}</span>
            <span>rate=${escapeHtml(String(view.time_rate ?? 1))}x</span>
            <span>inventory=${escapeHtml((view.inventory || []).join(", ") || "-")}</span>
            <span>profile=${escapeHtml(view.profile || scene.profile || "scene")}</span>
          </div>
          <div class="actions">${actionButtons}</div>
        </div>
        <div class="cols">
          <div class="grid" style="grid-template-columns: repeat(${gridColumns}, minmax(0, 1fr));">
            ${cells}
          </div>
          <div class="side">
            <section class="card">
              <h4>实体状态</h4>
              <ul>${entityList}</ul>
            </section>
            <section class="card">
              <h4>时间线</h4>
              <ul>${timeline.map((item) => `<li>${escapeHtml(item)}</li>`).join("") || "<li>暂无</li>"}</ul>
            </section>
          </div>
        </div>
      </section>
    `;
    this.syncAutoTick(view);
    this.shadowRoot.querySelectorAll("[data-intent]").forEach((button) => {
      button.addEventListener("click", () => {
        this.sendIntent({ kind: button.dataset.intent });
      });
    });
    this.shadowRoot.querySelectorAll("[data-entity-id]").forEach((button) => {
      button.addEventListener("click", () => {
        this.sendIntent({
          kind: "click",
          target: button.dataset.entityId,
        });
      });
    });
  }
}

function fallbackSceneView(contract, state) {
  const scene = contract.scene || {};
  const world = contract.world || {};
  const entities = (world.entities || []).map((entity) => ({
    id: entity.id,
    kind: entity.kind,
    label: entity.label,
    slot: state?.placements?.[entity.id] || null,
    status: state?.statuses?.[entity.id] || entity.status || null,
    flags: {},
  }));
  const rows = Number(world.topology?.rows || 0);
  const cols = Number(world.topology?.cols || 0);
  const declaredCells = new Map((world.topology?.cells || []).map((cell) => [cell.id, cell]));
  const cells = [];
  for (let row = 1; row <= rows; row += 1) {
    for (let col = 1; col <= cols; col += 1) {
      const id = `r${row}c${col}`;
      const declared = declaredCells.get(id) || {};
      cells.push({
        id,
        surface_kind: declared.surface_kind || null,
        flammable: typeof declared.flammable === "boolean" ? declared.flammable : null,
        walkable: typeof declared.walkable === "boolean" ? declared.walkable : null,
        occupiable: typeof declared.occupiable === "boolean" ? declared.occupiable : null,
        hazard_state: declared.hazard_state || null,
        tags: declared.tags || [],
        entities: entities.filter((entity) => entity.slot === id),
      });
    }
  }
  return {
    scene_id: scene.id || "scene",
    goal: scene.goal || null,
    profile: scene.profile || null,
    summary: scene.summary || null,
    phase: state?.phase || scene.state?.phase || "ready",
    result: state?.result || "ready",
    reason: state?.reason || null,
    countdown: state?.countdown ?? scene.state?.countdown ?? 0,
    current_time: state?.clock?.current_time ?? 0,
    time_unit: state?.clock?.time_unit || "second",
    clock_paused: !!state?.clock?.paused,
    time_rate: state?.clock?.rate ?? 1,
    inventory: state?.inventory || [],
    entities,
    cells,
    available_actions: state?.phase && state.phase !== "ready" ? [] : ["start"],
    start_label: contract.flow?.start?.action_label || "开始",
  };
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
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

customElements.define("mei-sim-scene", MeiSimScene);
