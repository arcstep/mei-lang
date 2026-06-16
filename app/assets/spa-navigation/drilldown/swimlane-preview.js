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

  function isCaseDetailCardPreview(config) {
    const mapping = resolveListPreviewMapping(config);
    return String(mapping?.preview_mode || mapping?.previewMode || "").trim() === "case_detail_card";
  }

  function isTypicalCaseCardPreview(config) {
    const mapping = resolveListPreviewMapping(config);
    return String(mapping?.preview_mode || mapping?.previewMode || "").trim() === "typical_case_card";
  }

  function isSheetDetailCardPreview(config) {
    return isCaseDetailCardPreview(config) || isTypicalCaseCardPreview(config);
  }

  function isPreviewOnlyMapping(config) {
    const mapping = resolveListPreviewMapping(config);
    return Boolean(mapping?.preview_only || mapping?.previewOnly);
  }

  function resolveCaseDetailFieldValue(row, spec) {
    if (!row || typeof row !== "object" || !spec || typeof spec !== "object") return "";
    const primary = String(spec.field || spec.column || "").trim();
    const fallbacks = cloneArray(spec.fallback_fields || spec.fallbackFields)
      .map((entry) => String(entry || "").trim())
      .filter(Boolean);
    for (const key of [primary, ...fallbacks]) {
      if (!key) continue;
      const value = row[key];
      if (value != null && String(value).trim()) {
        return String(value).trim();
      }
    }
    return "";
  }

  function splitMechanismDocuments(text) {
    const raw = String(text || "").trim();
    if (!raw || raw === "—" || raw === "-" || raw === "无") return [];
    return raw
      .split(/[、,，;；]/)
      .map((entry) => entry.trim().replace(/^[《]+|[》]+$/g, "").trim())
      .filter(Boolean)
      .map((entry) => (entry.startsWith("《") ? entry : `《${entry}》`));
  }

  function enrichCaseDetailRow(row, detail) {
    if (!row || typeof row !== "object") return row;
    const enriched = { ...row };
    const title = String(detail?.label ?? detail?.desc ?? "").trim();
    if (title && !String(enriched.案例名称 ?? "").trim()) {
      enriched.案例名称 = title;
    }
    const resultId = String(
      detail?.drilldown_filters?.处理结果ID ??
        detail?.default_filters?.处理结果ID ??
        detail?.value ??
        detail?.处理结果ID ??
        "",
    ).trim();
    if (resultId) {
      enriched.处理结果ID = resultId;
    }
    return enriched;
  }

  function resolveCaseDetailSituationText(row, mapping) {
    const situationField = String(mapping?.situation_field || mapping?.situationField || "").trim();
    if (situationField) {
      const value = resolveCaseDetailFieldValue(row, {
        field: situationField,
        fallback_fields: mapping?.situation_fallback_fields || mapping?.situationFallbackFields,
      });
      if (value) return value;
    }
    return resolveCaseDetailFieldValue(row, {
      field: String(mapping?.summary_field || mapping?.summaryField || "基本情况").trim(),
      fallback_fields: mapping?.summary_fallback_fields || mapping?.summaryFallbackFields,
    });
  }

  function appendCaseDetailMetaItem(parent, label, value) {
    const item = document.createElement("div");
    item.className = "access-drilldown-case-detail-meta-item";
    const labelEl = document.createElement("span");
    labelEl.className = "access-drilldown-case-detail-meta-label";
    labelEl.textContent = `${label}：`;
    const valueEl = document.createElement("span");
    valueEl.className = "access-drilldown-case-detail-meta-value";
    valueEl.textContent = value || "—";
    item.appendChild(labelEl);
    item.appendChild(valueEl);
    parent.appendChild(item);
  }

  function appendCaseDetailMetaRow(panel, row, mapping) {
    const specs = [
      ...cloneArray(mapping?.meta_left || mapping?.metaLeft),
      ...cloneArray(mapping?.meta_right || mapping?.metaRight),
    ];
    if (!specs.length) return;
    const meta = document.createElement("div");
    meta.className = "access-drilldown-case-detail-meta";
    specs.forEach((spec) => {
      const label = String(spec?.label || spec?.field || "").trim();
      if (!label) return;
      appendCaseDetailMetaItem(meta, label, resolveCaseDetailFieldValue(row, spec));
    });
    if (meta.childElementCount) panel.appendChild(meta);
  }

  function appendCaseDetailSection(block, section, row, mapping) {
    const label = String(section?.label || "").trim();
    const kind = String(section?.kind || "").trim();
    let value = resolveCaseDetailFieldValue(row, section);
    if (kind === "situation") {
      value = resolveCaseDetailSituationText(row, mapping);
    }
    if (!label) return;
    const sectionEl = document.createElement("div");
    sectionEl.className = "access-drilldown-case-detail-section";
    if (kind === "id") {
      const idRow = document.createElement("div");
      idRow.className = "access-drilldown-case-detail-id-row";
      const idLabel = document.createElement("span");
      idLabel.className = "access-drilldown-case-detail-id-label";
      idLabel.textContent = label;
      const idValue = document.createElement("span");
      idValue.className = "access-drilldown-case-detail-id-chip";
      idValue.textContent = value || "—";
      idRow.appendChild(idLabel);
      idRow.appendChild(idValue);
      sectionEl.appendChild(idRow);
      block.appendChild(sectionEl);
      return;
    }
    const labelEl = document.createElement("div");
    labelEl.className = "access-drilldown-case-detail-section-label";
    labelEl.textContent = label;
    sectionEl.appendChild(labelEl);
    const body = document.createElement("div");
    body.className = "access-drilldown-case-detail-section-body";
    if ((label === "健全机制" || label === "制度文件") && value) {
      const list = document.createElement("ul");
      list.className = "access-drilldown-case-detail-mechanism-list";
      splitMechanismDocuments(value).forEach((doc) => {
        const li = document.createElement("li");
        li.textContent = doc;
        list.appendChild(li);
      });
      if (!list.childElementCount) {
        body.textContent = value;
      } else {
        body.appendChild(list);
      }
    } else {
      body.textContent = value || "—";
    }
    sectionEl.appendChild(body);
    block.appendChild(sectionEl);
  }

  function resolveCaseDetailHeaderSubtitle(row, detail) {
    return nonEmptyString(
      detail?.value,
      resolveCaseDetailFieldValue(row, { field: "处理结果ID" }),
      resolveCaseDetailFieldValue(row, { field: "预警ID" }),
    );
  }

  function appendCaseDetailHeader(panel, row, mapping, detail = null) {
    const badge = String(mapping?.card_badge || mapping?.cardBadge || "典型案例").trim();
    const title = resolveCaseDetailFieldValue(row, {
      field: String(mapping?.title_field || mapping?.titleField || "案例名称").trim(),
      fallback_fields: mapping?.title_fallback_fields || mapping?.titleFallbackFields,
    });
    const header = document.createElement("div");
    header.className = "access-drilldown-case-detail-header";
    const main = document.createElement("div");
    main.className = "access-drilldown-case-detail-header-main";
    const badgeEl = document.createElement("div");
    badgeEl.className = "access-drilldown-case-detail-badge";
    badgeEl.textContent = badge;
    main.appendChild(badgeEl);
    const titleEl = document.createElement("h2");
    titleEl.className = "access-drilldown-case-detail-title";
    titleEl.textContent = title || "典型案例详情";
    main.appendChild(titleEl);
    const subtitleId = resolveCaseDetailHeaderSubtitle(row, detail);
    if (subtitleId) {
      const sub = document.createElement("span");
      sub.className = "access-drilldown-case-detail-subtitle";
      sub.textContent = subtitleId;
      main.appendChild(sub);
    }
    header.appendChild(main);
    panel.appendChild(header);
  }

  function resolveWarningLevelTone(value) {
    const text = String(value ?? "").trim();
    if (text.includes("红")) return "red";
    if (text.includes("黄")) return "yellow";
    if (text.includes("蓝")) return "blue";
    return "default";
  }

  function formatTypicalCaseMetricValue(row, spec) {
    const field = String(spec?.field || "").trim();
    if (!field) return "—";
    const raw = row?.[field];
    if (raw == null || raw === "") return "—";
    const text = String(raw).trim();
    const numeric = Number(text.replace(/,/g, ""));
    const unit = String(spec?.unit || "").trim();
    if (Number.isFinite(numeric)) {
      const formatted = Number.isInteger(numeric) ? String(numeric) : numeric.toFixed(2).replace(/\.?0+$/, "");
      return unit ? `${formatted}${unit}` : formatted;
    }
    return unit ? `${text}${unit}` : text;
  }

  function appendTypicalCaseTagRow(panel, row, mapping) {
    const tags = cloneArray(mapping?.tags);
    if (!tags.length) return;
    const tagRow = document.createElement("div");
    tagRow.className = "access-drilldown-typical-case-tags";
    tags.forEach((spec) => {
      const label = String(spec?.label || spec?.field || "").trim();
      const field = String(spec?.field || "").trim();
      if (!label || !field) return;
      const value = resolveCaseDetailFieldValue(row, spec);
      if (!value) return;
      const tag = document.createElement("span");
      const kind = String(spec?.kind || "").trim();
      tag.className =
        kind === "warning_level"
          ? `access-drilldown-typical-case-tag access-drilldown-typical-case-tag--warning access-drilldown-typical-case-tag--${resolveWarningLevelTone(value)}`
          : "access-drilldown-typical-case-tag";
      tag.textContent = `${label}：${value}`;
      tagRow.appendChild(tag);
    });
    if (tagRow.childElementCount) panel.appendChild(tagRow);
  }

  function appendTypicalCaseFacts(panel, row, mapping) {
    const facts = cloneArray(mapping?.facts);
    if (!facts.length) return;
    const factsRoot = document.createElement("div");
    factsRoot.className = "access-drilldown-typical-case-facts";
    facts.forEach((spec) => {
      const label = String(spec?.label || spec?.field || "").trim();
      if (!label) return;
      const item = document.createElement("div");
      item.className = "access-drilldown-typical-case-fact";
      const labelEl = document.createElement("div");
      labelEl.className = "access-drilldown-typical-case-fact-label";
      labelEl.textContent = label;
      const valueEl = document.createElement("div");
      valueEl.className = "access-drilldown-typical-case-fact-value";
      valueEl.textContent = resolveCaseDetailFieldValue(row, spec) || "—";
      item.appendChild(labelEl);
      item.appendChild(valueEl);
      factsRoot.appendChild(item);
    });
    if (factsRoot.childElementCount) panel.appendChild(factsRoot);
  }

  function appendTypicalCaseStatusRow(panel, row, mapping) {
    const flags = cloneArray(mapping?.status_flags || mapping?.statusFlags);
    if (!flags.length) return;
    const statusRoot = document.createElement("div");
    statusRoot.className = "access-drilldown-typical-case-status";
    const title = document.createElement("div");
    title.className = "access-drilldown-typical-case-status-title";
    title.textContent = "办理状态";
    statusRoot.appendChild(title);
    const pills = document.createElement("div");
    pills.className = "access-drilldown-typical-case-status-pills";
    flags.forEach((spec) => {
      const label = String(spec?.label || spec?.field || "").trim();
      const field = String(spec?.field || "").trim();
      if (!label || !field) return;
      const active = isTruthyFlag(row?.[field]);
      const pill = document.createElement("span");
      pill.className = `access-drilldown-typical-case-status-pill${active ? " access-drilldown-typical-case-status-pill--on" : ""}`;
      pill.textContent = label;
      pills.appendChild(pill);
    });
    statusRoot.appendChild(pills);
    panel.appendChild(statusRoot);
  }

  function appendTypicalCaseMetricsRow(panel, row, mapping) {
    const metrics = cloneArray(mapping?.metrics);
    if (!metrics.length) return;
    const metricsRoot = document.createElement("div");
    metricsRoot.className = "access-drilldown-typical-case-metrics";
    metrics.forEach((spec) => {
      const label = String(spec?.label || spec?.field || "").trim();
      if (!label) return;
      const card = document.createElement("div");
      card.className = "access-drilldown-typical-case-metric";
      const labelEl = document.createElement("div");
      labelEl.className = "access-drilldown-typical-case-metric-label";
      labelEl.textContent = label;
      const valueEl = document.createElement("div");
      valueEl.className = "access-drilldown-typical-case-metric-value";
      valueEl.textContent = formatTypicalCaseMetricValue(row, spec);
      card.appendChild(labelEl);
      card.appendChild(valueEl);
      metricsRoot.appendChild(card);
    });
    if (metricsRoot.childElementCount) panel.appendChild(metricsRoot);
  }

  function renderSheetDetailCardPanel(host, row, config, detail) {
    if (isTypicalCaseCardPreview(config)) {
      renderTypicalCaseCardPanel(host, row, config, detail);
      return;
    }
    renderCaseDetailCardPanel(host, row, config, detail);
  }

  function renderTypicalCaseCardPanel(host, row, config, detail) {
    if (!(host instanceof HTMLElement)) return;
    host.replaceChildren();
    host.classList.remove("access-drilldown-case-detail-host");
    host.classList.add("access-drilldown-typical-case-host");
    if (!row || typeof row !== "object") {
      const empty = document.createElement("div");
      empty.className = "access-drilldown-list-preview-empty";
      empty.textContent = "正在加载典型案例…";
      host.appendChild(empty);
      return;
    }
    const mapping = resolveListPreviewMapping(config);
    if (!mapping) {
      renderListPreviewItemPanel(host, row, config);
      return;
    }
    const enrichedRow = enrichCaseDetailRow(row, detail);
    const panel = document.createElement("div");
    panel.className = "access-drilldown-typical-case-panel";
    appendCaseDetailHeader(panel, enrichedRow, mapping, detail);
    appendTypicalCaseTagRow(panel, enrichedRow, mapping);
    const summary = resolveCaseDetailFieldValue(enrichedRow, {
      field: String(mapping?.summary_field || mapping?.summaryField || "基本情况").trim(),
      fallback_fields: mapping?.summary_fallback_fields || mapping?.summaryFallbackFields,
    });
    const summaryBlock = document.createElement("div");
    summaryBlock.className = "access-drilldown-typical-case-summary";
    const summaryLabel = document.createElement("div");
    summaryLabel.className = "access-drilldown-typical-case-summary-label";
    summaryLabel.textContent = String(mapping?.summary_label || mapping?.summaryLabel || "基本情况").trim();
    const summaryText = document.createElement("div");
    summaryText.className = "access-drilldown-typical-case-summary-text";
    summaryText.textContent = summary || "—";
    summaryBlock.appendChild(summaryLabel);
    summaryBlock.appendChild(summaryText);
    panel.appendChild(summaryBlock);
    appendTypicalCaseFacts(panel, enrichedRow, mapping);
    appendTypicalCaseStatusRow(panel, enrichedRow, mapping);
    appendTypicalCaseMetricsRow(panel, enrichedRow, mapping);
    host.appendChild(panel);
  }

  function renderCaseDetailCardPanel(host, row, config, detail) {
    if (!(host instanceof HTMLElement)) return;
    host.replaceChildren();
    host.classList.remove("access-drilldown-typical-case-host");
    host.classList.add("access-drilldown-case-detail-host");
    if (!row || typeof row !== "object") {
      const empty = document.createElement("div");
      empty.className = "access-drilldown-list-preview-empty";
      empty.textContent = "正在加载案例详情…";
      host.appendChild(empty);
      return;
    }
    const mapping = resolveListPreviewMapping(config);
    if (!mapping) {
      renderListPreviewItemPanel(host, row, config);
      return;
    }
    const enrichedRow = enrichCaseDetailRow(row, detail);
    const panel = document.createElement("div");
    panel.className = "access-drilldown-case-detail-panel";
    appendCaseDetailHeader(panel, enrichedRow, mapping, detail);
    const summary = resolveCaseDetailFieldValue(enrichedRow, {
      field: String(mapping?.summary_field || mapping?.summaryField || "基本情况").trim(),
      fallback_fields: mapping?.summary_fallback_fields || mapping?.summaryFallbackFields,
    });
    const summaryBlock = document.createElement("div");
    summaryBlock.className = "access-drilldown-case-detail-summary";
    const summaryLabel = document.createElement("div");
    summaryLabel.className = "access-drilldown-case-detail-summary-label";
    summaryLabel.textContent = String(mapping?.summary_label || mapping?.summaryLabel || "基本情况").trim();
    const summaryText = document.createElement("div");
    summaryText.className = "access-drilldown-case-detail-summary-text";
    summaryText.textContent = summary || "—";
    summaryBlock.appendChild(summaryLabel);
    summaryBlock.appendChild(summaryText);
    panel.appendChild(summaryBlock);
    appendCaseDetailMetaRow(panel, enrichedRow, mapping);
    const columnsRoot = document.createElement("div");
    columnsRoot.className = "access-drilldown-case-detail-columns";
    cloneArray(mapping?.columns).forEach((column) => {
      const columnEl = document.createElement("div");
      columnEl.className = "access-drilldown-case-detail-column";
      const columnId = String(column?.id || "").trim();
      if (columnId) {
        columnEl.dataset.caseDetailColumn = columnId;
      }
      const columnTitle = String(column?.title || column?.label || "").trim();
      if (columnTitle) {
        const titleEl = document.createElement("div");
        titleEl.className = "access-drilldown-case-detail-column-title";
        titleEl.textContent = columnTitle;
        columnEl.appendChild(titleEl);
      }
      cloneArray(column?.sections).forEach((section) => {
        appendCaseDetailSection(columnEl, section, enrichedRow, mapping);
      });
      if (columnEl.childElementCount) {
        columnsRoot.appendChild(columnEl);
      }
    });
    if (columnsRoot.childElementCount) {
      panel.appendChild(columnsRoot);
    }
    host.appendChild(panel);
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
