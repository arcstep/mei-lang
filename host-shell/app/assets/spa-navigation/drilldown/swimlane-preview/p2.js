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
    const maxChars = Math.max(
      12,
      Number(mapping?.fact_truncate_chars || mapping?.factTruncateChars || 28) || 28,
    );
    metrics.forEach((spec) => {
      const label = String(spec?.label || spec?.field || "").trim();
      if (!label) return;
      const kind = String(spec?.kind || "").trim();
      const isText = kind === "text" || kind === "fact";
      const card = document.createElement("div");
      card.className = isText
        ? "access-drilldown-typical-case-metric access-drilldown-typical-case-metric--text"
        : "access-drilldown-typical-case-metric";
      const labelEl = document.createElement("div");
      labelEl.className = "access-drilldown-typical-case-metric-label";
      labelEl.textContent = label;
      const valueEl = document.createElement("div");
      valueEl.className = "access-drilldown-typical-case-metric-value";
      if (isText) {
        const full = resolveCaseDetailFieldValue(row, spec) || "—";
        const chars = [...full];
        const needsTruncate = full !== "—" && chars.length > maxChars;
        if (needsTruncate) {
          valueEl.classList.add("is-truncated");
          valueEl.textContent = `${chars.slice(0, maxChars).join("")}…`;
          valueEl.title = "点击查看全文";
          valueEl.setAttribute("role", "button");
          valueEl.tabIndex = 0;
          const open = (event) => {
            event?.preventDefault?.();
            event?.stopPropagation?.();
            openCaseCardFactPopover(valueEl, full, label);
          };
          valueEl.addEventListener("click", open);
          valueEl.addEventListener("keydown", (event) => {
            if (event.key === "Enter" || event.key === " ") open(event);
          });
        } else {
          valueEl.textContent = full;
        }
      } else {
        valueEl.textContent = formatTypicalCaseMetricValue(row, spec);
      }
      card.appendChild(labelEl);
      card.appendChild(valueEl);
      metricsRoot.appendChild(card);
    });
    if (metricsRoot.childElementCount) {
      metricsRoot.style.setProperty(
        "--case-metric-count",
        String(metricsRoot.childElementCount),
      );
      panel.appendChild(metricsRoot);
    }
  }

  function applyCaseDetailWarningTone(panel, row) {
    if (!(panel instanceof HTMLElement)) return;
    const tone = resolveWarningLevelTone(
      resolveCaseDetailFieldValue(row, { field: "预警等级" }),
    );
    if (tone !== "default") {
      panel.dataset.warningLevel = tone;
    } else {
      panel.removeAttribute("data-warning-level");
    }
  }

  function appendTypicalCaseStatsSection(panel, row, mapping, { wrapBand = false, config = null, factsOutsideBand = false } = {}) {
    if (!mappingHasTypicalCaseStats(mapping)) return;
    const bandTarget = (() => {
      if (!wrapBand) return panel;
      const band = document.createElement("div");
      band.className = "access-drilldown-case-detail-stats-band";
      const hideBandLabel =
        mapping?.hide_stats_band_label === true || mapping?.hideStatsBandLabel === true;
      if (hideBandLabel) {
        band.dataset.statsBandLabelHidden = "true";
      } else {
        const bandLabel = String(
          mapping?.stats_band_label ||
            mapping?.statsBandLabel ||
            mapping?.card_badge ||
            mapping?.cardBadge ||
            "实时预警",
        ).trim();
        if (bandLabel) band.dataset.statsBandLabel = bandLabel;
      }
      panel.appendChild(band);
      return band;
    })();
    appendTypicalCaseTagRow(bandTarget, row, mapping, config);
    if (!factsOutsideBand) {
      appendTypicalCaseFacts(bandTarget, row, mapping);
    }
    appendTypicalCaseStatusRow(bandTarget, row, mapping, config);
    appendTypicalCaseMetricsRow(bandTarget, row, mapping);
    if (factsOutsideBand) {
      appendTypicalCaseFacts(panel, row, mapping);
    }
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
    const enrichedRow = enrichCaseDetailRow(row, detail, config);
    const panel = document.createElement("div");
    panel.className = "access-drilldown-typical-case-panel";
    applyCaseDetailWarningTone(panel, enrichedRow);
    if (mappingShowsHeader(mapping)) {
      appendCaseDetailHeader(panel, enrichedRow, mapping, detail);
    }
    if (mappingShowsSummary(mapping)) {
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
    }
    appendTypicalCaseStatsSection(panel, enrichedRow, mapping, { config });
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
    const enrichedRow = enrichCaseDetailRow(row, detail, config);
    const panel = document.createElement("div");
    panel.className = "access-drilldown-case-detail-panel";
    const previewMode = String(mapping?.preview_mode || mapping?.previewMode || "").trim();
    // 纯行级表单：跳过案例卡头/统计带/meta，只渲染 label-value 表单。
    if (previewMode === "row_form") {
      host.classList.remove("access-drilldown-case-detail-host--hybrid");
      host.classList.add("access-drilldown-case-detail-host--row-form");
      appendRowFormFields(panel, enrichedRow, mapping);
      host.appendChild(panel);
      return;
    }
    const hybridStats = mappingHasTypicalCaseStats(mapping);
    if (hybridStats) {
      host.classList.add("access-drilldown-case-detail-host--hybrid");
    }
    applyCaseDetailWarningTone(panel, enrichedRow);

    const top = document.createElement("div");
    top.className = "access-drilldown-case-detail-top";
    if (mappingShowsHeader(mapping)) {
      appendCaseDetailHeader(top, enrichedRow, mapping, detail);
    }
    if (mappingShowsSummary(mapping)) {
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
      top.appendChild(summaryBlock);
    }
    const factsOutsideBand =
      mapping?.facts_outside_band === true ||
      mapping?.factsOutsideBand === true ||
      cloneArray(mapping?.facts).length > 0;
    appendTypicalCaseStatsSection(top, enrichedRow, mapping, {
      wrapBand: hybridStats,
      config,
      factsOutsideBand,
    });
    if (mappingShowsMeta(mapping)) {
      appendCaseDetailMetaRow(top, enrichedRow, mapping);
    }
    if (mappingWantsRowForm(mapping)) {
      appendRowFormFields(top, enrichedRow, mapping);
    }
    panel.appendChild(top);

    const columnsRoot = document.createElement("div");
    columnsRoot.className = "access-drilldown-case-detail-columns";
    const columns = cloneArray(mapping?.columns);
    const laneCount = columns.length > 0 ? columns.length : 3;
    columnsRoot.style.setProperty("--case-detail-lane-count", String(laneCount));
    columnsRoot.dataset.laneCount = String(laneCount);
    columns.forEach((column) => {
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
        appendCaseDetailSection(columnEl, section, enrichedRow, mapping, config);
      });
      if (columnEl.childElementCount) {
        columnsRoot.appendChild(columnEl);
      }
    });
    if (columnsRoot.childElementCount) {
      panel.appendChild(columnsRoot);
    } else {
      // 无泳道时顶区占满，避免空行留白。
      panel.style.gridTemplateRows = "minmax(0, 1fr)";
    }
    host.appendChild(panel);
    loadCaseCardDrilldownMeta();
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
