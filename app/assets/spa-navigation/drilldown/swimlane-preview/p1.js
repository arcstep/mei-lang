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

  function isDocumentPreview(config) {
    const mapping = resolveListPreviewMapping(config);
    return String(mapping?.preview_mode || mapping?.previewMode || "").trim() === "document_preview";
  }

  function isVideoSubtitleCockpitPreview(config) {
    const mapping = resolveListPreviewMapping(config);
    return (
      String(mapping?.preview_mode || mapping?.previewMode || "").trim() === "video_subtitle_cockpit"
    );
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

  function applyExternalCaseDetailRowEnricher(row, detail) {
    if (!row || typeof row !== "object") {
      return row;
    }
    const enricher =
      typeof window !== "undefined" ? window.__meiCaseDetailRowEnricher : null;
    if (typeof enricher !== "function") {
      return row;
    }
    const enriched = enricher(row, detail);
    return enriched && typeof enriched === "object" ? enriched : row;
  }

  function enrichCaseDetailRow(row, detail) {
    if (!row || typeof row !== "object") return row;
    const enriched = { ...row };
    const filters =
      detail?.drilldown_filters && typeof detail.drilldown_filters === "object"
        ? detail.drilldown_filters
        : detail?.default_filters && typeof detail.default_filters === "object"
          ? detail.default_filters
          : {};
    const title = String(detail?.label ?? detail?.desc ?? "").trim();
    if (title && !String(enriched.title ?? enriched.label ?? "").trim()) {
      enriched.title = title;
    }
    Object.entries(filters).forEach(([key, value]) => {
      const text = String(value ?? "").trim();
      if (text) {
        enriched[key] = text;
      }
    });
    return applyExternalCaseDetailRowEnricher(enriched, detail);
  }

  function mappingShowsHeader(mapping) {
    return mapping?.show_header !== false && mapping?.showHeader !== false;
  }

  function mappingShowsSummary(mapping) {
    return mapping?.show_summary !== false && mapping?.showSummary !== false;
  }

  function mappingShowsMeta(mapping) {
    return mapping?.show_meta !== false && mapping?.showMeta !== false;
  }

  function mappingHasTypicalCaseStats(mapping) {
    return (
      cloneArray(mapping?.tags).length > 0 ||
      cloneArray(mapping?.facts).length > 0 ||
      cloneArray(mapping?.status_flags || mapping?.statusFlags).length > 0 ||
      cloneArray(mapping?.metrics).length > 0
    );
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
    const raw = resolveCaseDetailFieldValue(row, spec);
    if (!raw) return "—";
    const numeric = Number(raw.replace(/,/g, ""));
    const unit = String(spec?.unit || "").trim();
    if (Number.isFinite(numeric)) {
      const formatted = Number.isInteger(numeric) ? String(numeric) : numeric.toFixed(2).replace(/\.?0+$/, "");
      return unit ? `${formatted}${unit}` : formatted;
    }
    return unit ? `${raw}${unit}` : raw;
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
