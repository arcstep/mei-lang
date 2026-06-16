  function resolveListPreviewMapping(config) {
    const slot = config?.rowPreviewSlot || config?.previewSlot || {};
    const mapping = slot?.mapping;
    if (mapping && typeof mapping === "object" && !Array.isArray(mapping)) {
      return mapping;
    }
    return null;
  }

  function isSwimlanePreview(config) {
    const mapping = resolveListPreviewMapping(config);
    return (
      String(mapping?.preview_mode || mapping?.previewMode || "").trim() === "swimlane" &&
      Array.isArray(mapping?.lanes) &&
      mapping.lanes.length > 0
    );
  }

  function isTruthyFlag(value) {
    const text = String(value ?? "").trim();
    if (!text || text === "—" || text === "-" || text === "－" || text === "否" || text === "0") {
      return false;
    }
    if (text.includes("是")) return true;
    const numeric = Number(text.replace(/,/g, ""));
    return Number.isFinite(numeric) && numeric > 0;
  }

  function formatMetricValue(value, step) {
    const text = String(value ?? "").trim();
    if (!text) return "—";
    const numeric = Number(text.replace(/,/g, ""));
    if (!Number.isFinite(numeric) || numeric <= 0) return "—";
    const unit = String(step?.unit || "").trim();
    const formatted = Number.isInteger(numeric) ? String(numeric) : numeric.toFixed(2).replace(/\.?0+$/, "");
    return unit ? `${formatted}${unit}` : formatted;
  }

  function resolveSequentialStepStates(steps, row) {
    let lastActiveIndex = -1;
    steps.forEach((step, index) => {
      const field = String(step?.field || "").trim();
      if (!field) return;
      if (String(step?.kind || "flag").trim() === "metric") {
        const numeric = Number(String(row?.[field] ?? "").replace(/,/g, ""));
        if (Number.isFinite(numeric) && numeric > 0) {
          lastActiveIndex = index;
        }
        return;
      }
      if (isTruthyFlag(row?.[field])) {
        lastActiveIndex = index;
      }
    });
    return steps.map((step, index) => {
      const field = String(step?.field || "").trim();
      const kind = String(step?.kind || "flag").trim();
      if (kind === "metric") {
        const display = formatMetricValue(row?.[field], step);
        return {
          label: String(step?.label || field).trim(),
          state: display === "—" ? "pending" : "done",
          detail: display,
        };
      }
      const active = field ? isTruthyFlag(row?.[field]) : false;
      if (lastActiveIndex < 0) {
        return {
          label: String(step?.label || field).trim(),
          state: index === 0 ? "current" : "pending",
          detail: "",
        };
      }
      if (index < lastActiveIndex) {
        return { label: String(step?.label || field).trim(), state: "done", detail: "" };
      }
      if (index === lastActiveIndex) {
        return { label: String(step?.label || field).trim(), state: "current", detail: "" };
      }
      return { label: String(step?.label || field).trim(), state: "pending", detail: "" };
    });
  }

  function resolveListPreviewTitle(row, config, mapping) {
    const titleField = String(mapping?.title_field || mapping?.titleField || "").trim();
    if (titleField && row?.[titleField] != null && String(row[titleField]).trim()) {
      return String(row[titleField]).trim();
    }
    return resolveListPreviewRowTitle(row, config);
  }

  function appendSwimlaneSubtitle(panel, row, mapping) {
    const fields = cloneArray(mapping?.subtitle_fields || mapping?.subtitleFields);
    if (!fields.length) return;
    const meta = document.createElement("div");
    meta.className = "access-drilldown-swimlane-meta";
    fields.forEach((fieldName) => {
      const key = String(fieldName || "").trim();
      if (!key) return;
      const value = row?.[key];
      if (value == null || !String(value).trim()) return;
      const item = document.createElement("span");
      item.className = "access-drilldown-swimlane-meta-item";
      item.textContent = `${key}：${String(value).trim()}`;
      meta.appendChild(item);
    });
    if (meta.childElementCount) {
      panel.appendChild(meta);
    }
  }

  function appendSwimlaneContext(panel, row, mapping) {
    const contextField = String(mapping?.context_field || mapping?.contextField || "基本情况").trim();
    const value = row?.[contextField];
    if (value == null || !String(value).trim()) return;
    const block = document.createElement("div");
    block.className = "access-drilldown-swimlane-context";
    const label = document.createElement("div");
    label.className = "access-drilldown-swimlane-context-label";
    label.textContent = contextField;
    const text = document.createElement("div");
    text.className = "access-drilldown-swimlane-context-text";
    text.textContent = String(value).trim();
    block.appendChild(label);
    block.appendChild(text);
    panel.appendChild(block);
  }

  function renderSwimlaneNode(stepState) {
    const node = document.createElement("div");
    node.className = `access-drilldown-swimlane-node access-drilldown-swimlane-node--${stepState.state}`;
    const dot = document.createElement("span");
    dot.className = "access-drilldown-swimlane-node-dot";
    node.appendChild(dot);
    const label = document.createElement("span");
    label.className = "access-drilldown-swimlane-node-label";
    label.textContent = stepState.label;
    node.appendChild(label);
    if (stepState.detail && stepState.detail !== "—") {
      const detail = document.createElement("span");
      detail.className = "access-drilldown-swimlane-node-detail";
      detail.textContent = stepState.detail;
      node.appendChild(detail);
    }
    return node;
  }

  function renderSwimlanePreviewPanel(host, row, config) {
    if (!(host instanceof HTMLElement)) return;
    host.replaceChildren();
    if (!row || typeof row !== "object") {
      const empty = document.createElement("div");
      empty.className = "access-drilldown-list-preview-empty";
      empty.textContent = "点击清单中的案例查看办理泳道";
      host.appendChild(empty);
      return;
    }
    const mapping = resolveListPreviewMapping(config);
    if (!mapping) {
      renderListPreviewItemPanel(host, row, config);
      return;
    }
    const panel = document.createElement("div");
    panel.className = "access-drilldown-swimlane-panel";
    const title = document.createElement("div");
    title.className = "access-drilldown-swimlane-title";
    title.textContent = resolveListPreviewTitle(row, config, mapping);
    panel.appendChild(title);
    appendSwimlaneSubtitle(panel, row, mapping);
    appendSwimlaneContext(panel, row, mapping);

    const lanesRoot = document.createElement("div");
    lanesRoot.className = "access-drilldown-swimlane-lanes";
    cloneArray(mapping.lanes).forEach((lane) => {
      const laneEl = document.createElement("div");
      laneEl.className = "access-drilldown-swimlane-lane";
      const laneLabel = document.createElement("div");
      laneLabel.className = "access-drilldown-swimlane-lane-label";
      laneLabel.textContent = String(lane?.label || lane?.id || "流程").trim();
      laneEl.appendChild(laneLabel);
      const track = document.createElement("div");
      track.className = "access-drilldown-swimlane-track";
      const steps = cloneArray(lane?.steps);
      const stepStates = resolveSequentialStepStates(steps, row);
      stepStates.forEach((stepState, index) => {
        if (index > 0) {
          const connector = document.createElement("span");
          connector.className = "access-drilldown-swimlane-connector";
          connector.setAttribute("aria-hidden", "true");
          track.appendChild(connector);
        }
        track.appendChild(renderSwimlaneNode(stepState));
      });
      laneEl.appendChild(track);
      lanesRoot.appendChild(laneEl);
    });
    panel.appendChild(lanesRoot);
    host.appendChild(panel);
  }
