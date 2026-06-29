import { escapeHtml, parseProps } from "./shared.js";
import {
  POPUP_OPEN_EVENT_NAME,
  ANALYSIS_OPEN_EVENT_NAME,
  DRILLDOWN_EVENT_NAME,
  popupConfigOf,
  sceneDrilldownContextValue,
  tableDrilldownMeta,
} from "./drilldown-meta.js";

const TAG = "mei-cockpit-opinion-panel";

function actionDrilldownProps(props) {
  const p = props || {};
  const patch = p.action_metric_patch ?? p.actionMetricPatch ?? {};
  return {
    ...p,
    content: p.action_metric ?? p.action_content ?? p.actionMetric ?? p.actionContent,
    metric_role: "value",
    metric_patch: {
      value: String(p.action_text ?? p.actionText ?? "详细介绍"),
      ...patch,
    },
    popup: p.popup ?? p.action_popup ?? p.actionPopup,
  };
}

function hasAction(props) {
  const p = props || {};
  return Boolean(
    String(p.action_text ?? p.actionText ?? "").trim() ||
      p.action_metric ||
      p.action_content ||
      p.actionMetric ||
      p.popup ||
      p.action_popup,
  );
}

function renderBodyHtml(body, format) {
  const text = String(body ?? "").trim();
  if (!text) {
    return "";
  }
  const mode = String(format ?? "plain").trim().toLowerCase();
  if (mode === "html") {
    return text;
  }
  return escapeHtml(text);
}

class MeiCockpitOpinionPanel extends HTMLElement {
  constructor() {
    super();
    this._actionClick = this._actionClick.bind(this);
    this._actionKey = this._actionKey.bind(this);
  }

  connectedCallback() {
    this.props = parseProps(this);
    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }
    this.render();
  }

  disconnectedCallback() {
    const action = this.shadowRoot?.querySelector(".action");
    action?.removeEventListener("click", this._actionClick);
    action?.removeEventListener("keydown", this._actionKey);
  }

  _emitPopup(detail) {
    if (!detail) {
      return;
    }
    this.dispatchEvent(
      new CustomEvent(DRILLDOWN_EVENT_NAME, {
        bubbles: true,
        composed: true,
        detail,
      }),
    );
    this.dispatchEvent(
      new CustomEvent(ANALYSIS_OPEN_EVENT_NAME, {
        bubbles: true,
        composed: true,
        detail,
      }),
    );
    this.dispatchEvent(
      new CustomEvent(POPUP_OPEN_EVENT_NAME, {
        bubbles: true,
        composed: true,
        detail,
      }),
    );
  }

  _actionClick(event) {
    event.preventDefault();
    event.stopPropagation();
    const props = actionDrilldownProps(this.props);
    let meta = tableDrilldownMeta(props);
    if (!meta) {
      const popup = popupConfigOf(props);
      const boardSceneId = String(
        popup?.scene_id ?? popup?.sceneId ?? popup?.scene?.scene_id ?? "",
      ).trim();
      if (!popup || !boardSceneId) {
        return;
      }
      const panelId =
        this.closest?.("[data-mei-panel-id]")?.getAttribute?.("data-mei-panel-id") || "";
      meta = {
        popup,
        board_scene_id: boardSceneId,
        board_scene_file: String(popup.scene_file ?? popup.sceneFile ?? "").trim(),
        projection: String(popup.projection || "overlay").trim() || "overlay",
        metric_id: String(
          props?.action_metric?.__mei_runtime_ref?.metric_id ??
            props?.content?.__mei_runtime_ref?.metric_id ??
            "",
        ).trim(),
        dataset_id: String(
          props?.action_metric?.__mei_runtime_ref?.dataset_id ??
            props?.content?.__mei_runtime_ref?.dataset_id ??
            "",
        ).trim(),
        panel_id: panelId,
        host_scene_id: String(props?._mei?.active_scene_id || "").trim(),
        host_scene_file: String(props?._mei?.active_target_file || "").trim(),
        scene_projection_assembly_by_id: sceneDrilldownContextValue(
          props,
          "scene_projection_assembly_by_id",
        ),
        scene_local_nav_by_target: sceneDrilldownContextValue(
          props,
          "scene_local_nav_by_target",
        ),
      };
    }
    const panelId =
      this.closest?.("[data-mei-panel-id]")?.getAttribute?.("data-mei-panel-id") || "";
    this._emitPopup({ ...meta, panel_id: panelId });
  }

  _actionKey(event) {
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }
    event.preventDefault();
    this._actionClick(event);
  }

  render() {
    const p = this.props || {};
    const title = String(p.title ?? "观点").trim();
    const body = String(p.body ?? p.content ?? "").trim();
    const bodyFormat = String(p.body_format ?? p.bodyFormat ?? "plain").trim().toLowerCase();
    const pointId = String(p.point_id ?? p.pointId ?? "").trim();
    const emphasis = p.emphasis === true || p.emphasis === "true";
    const showAction = hasAction(p);
    const actionText = String(p.action_text ?? p.actionText ?? "详细介绍").trim();
    const actionInteractive =
      Boolean(tableDrilldownMeta(actionDrilldownProps(p))) ||
      Boolean(
        popupConfigOf(actionDrilldownProps(p))?.scene_id ||
          popupConfigOf(actionDrilldownProps(p))?.sceneId,
      );

    const gridRows = showAction ? "auto 1fr auto" : "auto 1fr";
    const gridAreas = showAction
      ? '"title" "body" "action"'
      : '"title" "body"';

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          height: 100%;
          min-height: 0;
          box-sizing: border-box;
        }
        .shell {
          display: grid;
          grid-template-rows: ${gridRows};
          grid-template-areas: ${gridAreas};
          width: 100%;
          height: 100%;
          min-height: 0;
          border: 1px solid rgba(56, 160, 240, 0.35);
          border-radius: 4px;
          background: rgba(14, 52, 96, 0.65);
          box-sizing: border-box;
          overflow: hidden;
          ${emphasis ? "box-shadow: 0 0 0 1px rgba(56,160,240,0.25) inset;" : ""}
        }
        .title {
          grid-area: title;
          padding: 10px 12px 6px;
          text-align: center;
          color: #a8c8e6;
          font: 600 16px/1.35 system-ui, sans-serif;
        }
        .point {
          display: inline-block;
          margin-right: 6px;
          color: #7dd3fc;
        }
        .body {
          grid-area: body;
          padding: 6px 12px 8px;
          text-align: left;
          color: #e2e8f0;
          font: 400 16px/1.55 system-ui, sans-serif;
          white-space: ${bodyFormat === "html" ? "normal" : "pre-wrap"};
          overflow: auto;
          min-height: 0;
        }
        .body :where(p) {
          margin: 0 0 0.5em;
        }
        .body :where(p:last-child) {
          margin-bottom: 0;
        }
        .action {
          grid-area: action;
          padding: 4px 12px 10px;
          text-align: left;
          color: #7dd3fc;
          font: 500 14px/1.4 system-ui, sans-serif;
          ${actionInteractive ? "cursor: pointer; text-decoration: underline; text-underline-offset: 3px;" : ""}
        }
        .action:hover {
          color: #bae6fd;
        }
        .action:focus-visible {
          outline: 1px solid rgba(125, 211, 252, 0.9);
          outline-offset: 2px;
          border-radius: 4px;
        }
      </style>
      <article class="shell">
        <header class="title">
          ${pointId ? `<span class="point">${escapeHtml(pointId)}</span>` : ""}${escapeHtml(title)}
        </header>
        <div class="body">${renderBodyHtml(body, bodyFormat)}</div>
        ${
          showAction
            ? `<div class="action" ${actionInteractive ? 'tabindex="0" role="button"' : ""}>${escapeHtml(actionText)}</div>`
            : ""
        }
      </article>
    `;

    if (showAction && actionInteractive) {
      const action = this.shadowRoot.querySelector(".action");
      action?.addEventListener("click", this._actionClick);
      action?.addEventListener("keydown", this._actionKey);
    }
  }
}

if (!customElements.get(TAG)) {
  customElements.define(TAG, MeiCockpitOpinionPanel);
}

export { MeiCockpitOpinionPanel };
