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
    const mode = String(mapping?.preview_mode || mapping?.previewMode || "").trim();
    return mode === "case_detail_card" || mode === "row_form";
  }

  function mappingWantsRowForm(mapping) {
    if (!mapping || typeof mapping !== "object") return false;
    const mode = String(mapping.preview_mode || mapping.previewMode || "").trim();
    return (
      mode === "row_form" ||
      mapping.auto_fields === true ||
      mapping.autoFields === true ||
      mapping.form_fields === "all" ||
      mapping.formFields === "all"
    );
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
    if (!Boolean(mapping?.preview_only || mapping?.previewOnly)) return false;
    // 左清单 + 右预览：preview_only 只表示右侧是纯 PDF，不能把整板折成独立预览。
    if (String(config?.sceneLocalNav?.kind || config?.scene_local_nav?.kind || "").trim() === "list_preview_drilldown_page") {
      return false;
    }
    if (nonEmptyString(config?.rowPreviewSourceZoneId, config?.row_preview_source_zone_id)) {
      return false;
    }
    return true;
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

  function resolveCaseDetailObjectType(detail, config) {
    const nav =
      config?.sceneLocalNav && typeof config.sceneLocalNav === "object"
        ? config.sceneLocalNav
        : config?.scene_local_nav && typeof config.scene_local_nav === "object"
          ? config.scene_local_nav
          : {};
    const locator =
      (nav.object_locator && typeof nav.object_locator === "object" && !Array.isArray(nav.object_locator)
        ? nav.object_locator
        : null) ||
      (nav.objectLocator && typeof nav.objectLocator === "object" && !Array.isArray(nav.objectLocator)
        ? nav.objectLocator
        : null) ||
      (config?.object_locator && typeof config.object_locator === "object" && !Array.isArray(config.object_locator)
        ? config.object_locator
        : null) ||
      (config?.objectLocator && typeof config.objectLocator === "object" && !Array.isArray(config.objectLocator)
        ? config.objectLocator
        : null);
    return nonEmptyString(
      locator?.object_type,
      locator?.objectType,
      nav.object_type,
      nav.objectType,
      config?.object_type,
      config?.objectType,
      detail?.object_type,
      detail?.objectType,
    );
  }

  function isWarningObjectType(objectType) {
    const type = String(objectType || "").trim();
    if (!type) return false;
    return type === "Warning" || type.endsWith(".Warning");
  }

  /**
   * 办理状态（是否待办/在办/已办）只服务 Warning 案例卡。
   * row_form 通用属性表单、以及 AlertModel/Matter 等非 Warning 对象不得注入。
   */
  function shouldDeriveWarningHandlingStatusFlags(detail, config) {
    const mapping = resolveListPreviewMapping(config);
    const mode = String(mapping?.preview_mode || mapping?.previewMode || "").trim();
    if (mode === "row_form") return false;
    const objectType = resolveCaseDetailObjectType(detail, config);
    if (objectType && !isWarningObjectType(objectType)) return false;
    return true;
  }

  function enrichCaseDetailRow(row, detail, config) {
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
      if (!text) return;
      enriched[key] = text;
      // Identity filters must win over a mismatched preview row (scalar-rowset often ignores them).
      if (key === "warningId" || key === "预警ID") {
        enriched.warningId = text;
        enriched["预警ID"] = text;
      }
      if (key === "resultId" || key === "处理结果ID") {
        enriched.resultId = text;
        enriched["处理结果ID"] = text;
      }
      if (key === "matterId" || key === "序号") {
        enriched.matterId = text;
        enriched["序号"] = text;
      }
      if (key === "matter" || key === "风险事项" || key === "监督事项") {
        enriched.matter = text;
        enriched["风险事项"] = text;
        enriched["监督事项"] = text;
      }
      if (key === "modelId" || key === "模型ID") {
        // Excel 浮点整型常变成 2025001.0 / number；详情卡统一成无小数文本。
        const normalized = (() => {
          if (typeof value === "number" && Number.isFinite(value) && Math.abs(value % 1) < Number.EPSILON) {
            return String(Math.trunc(value));
          }
          const raw = String(value ?? "").trim();
          return /^-?\d+\.0+$/.test(raw) ? raw.replace(/\.0+$/, "") : raw;
        })();
        if (!normalized) return;
        enriched.modelId = normalized;
        enriched["模型ID"] = normalized;
        return;
      }
      if (key === "mechanismName" || key === "机制名称" || key === "健全机制") {
        // 过滤值可能是顿号多值；只取首个可匹配的机制名称，避免整串无法命中 CSV 行。
        const parts = String(text || "")
          .split(/[、,，;；]/)
          .map((entry) =>
            String(entry || "")
              .trim()
              .replace(/^[《]+|[》]+$/g, "")
              .trim(),
          )
          .filter(Boolean);
        const normalized = parts[0] || String(text).replace(/^[《]+|[》]+$/g, "").trim();
        if (!normalized) return;
        enriched.mechanismName = normalized;
        enriched["机制名称"] = normalized;
        enriched["健全机制"] = normalized;
        return;
      }
    });
    if (shouldDeriveWarningHandlingStatusFlags(detail, config)) {
      deriveWarningHandlingStatusFlags(enriched);
    }
    return applyExternalCaseDetailRowEnricher(enriched, detail);
  }

  function caseDetailFieldPresent(value) {
    const text = String(value ?? "").trim();
    return Boolean(text) && text !== "—" && text !== "-" && text !== "－";
  }

  /** 与 issue-handling 指标一致：跟踪ID+承办部门+办结时间 → 已办 / 在办 / 待办（仅 Warning 案例卡） */
  function deriveWarningHandlingStatusFlags(row) {
    if (!row || typeof row !== "object") return row;
    const hasExplicit =
      caseDetailFieldPresent(row["是否待办"]) ||
      caseDetailFieldPresent(row["是否在办"]) ||
      caseDetailFieldPresent(row["是否已办"]);
    if (hasExplicit) return row;
    const tracking = caseDetailFieldPresent(row["问题跟踪ID"]);
    const dept = caseDetailFieldPresent(row["承办部门"]);
    const closed = caseDetailFieldPresent(row["办结时间"]);
    let pending = "否";
    let inProgress = "否";
    let done = "否";
    if (tracking && dept && closed) {
      done = "是";
    } else if (tracking && dept) {
      inProgress = "是";
    } else {
      pending = "是";
    }
    row["是否待办"] = pending;
    row["是否在办"] = inProgress;
    row["是否已办"] = done;
    return row;
  }

  function caseCardObjectProps(config) {
    const nav =
      config?.sceneLocalNav && typeof config.sceneLocalNav === "object" ? config.sceneLocalNav : {};
    const locator =
      (nav.object_locator && typeof nav.object_locator === "object" && !Array.isArray(nav.object_locator)
        ? nav.object_locator
        : null) ||
      (nav.objectLocator && typeof nav.objectLocator === "object" && !Array.isArray(nav.objectLocator)
        ? nav.objectLocator
        : null) ||
      (config?.object_locator && typeof config.object_locator === "object" && !Array.isArray(config.object_locator)
        ? config.object_locator
        : null) ||
      (config?.objectLocator && typeof config.objectLocator === "object" && !Array.isArray(config.objectLocator)
        ? config.objectLocator
        : null);
    const objectType = nonEmptyString(
      locator?.object_type,
      locator?.objectType,
      nav.object_type,
      nav.objectType,
      config?.object_type,
      config?.objectType,
    );
    const identityField = nonEmptyString(
      locator?.identity_field,
      locator?.identityField,
      nav.identity_field,
      nav.identityField,
      config?.identity_field,
      config?.identityField,
    );
    const resolved =
      objectType || identityField
        ? {
            ...(locator && typeof locator === "object" ? locator : {}),
            ...(objectType ? { object_type: objectType, objectType } : {}),
            ...(identityField ? { identity_field: identityField, identityField } : {}),
          }
        : {
            object_type: "zhifa.Warning",
            objectType: "zhifa.Warning",
            identity_field: "预警ID",
            identityField: "预警ID",
          };
    return {
      object_locator: resolved,
      objectLocator: resolved,
    };
  }

  async function loadCaseCardDrilldownMeta() {
    if (typeof window !== "undefined" && window.MeiDrilldownMeta) {
      return window.MeiDrilldownMeta;
    }
    try {
      const mod = await import("/workspace-components/cockpit/drilldown-meta.js");
      const api = {
        resolveObjectFieldTargets: mod.resolveObjectFieldTargets,
        emitObjectFieldOpen: mod.emitObjectFieldOpen,
        resolveObjectFieldLinks: mod.resolveObjectFieldLinks,
        splitMultiObjectKeys: mod.splitMultiObjectKeys,
      };
      if (typeof window !== "undefined") {
        window.MeiDrilldownMeta = api;
      }
      return api;
    } catch (_error) {
      return null;
    }
  }

  function filterCaseCardObjectTargets(targets, spec) {
    const allowed = cloneArray(spec?.object_types || spec?.objectTypes)
      .map((type) => String(type || "").trim())
      .filter(Boolean);
    let list = Array.isArray(targets) ? targets : [];
    if (allowed.length) {
      list = list.filter((target) =>
        allowed.includes(String(target?.objectType || target?.object_type || "").trim()),
      );
    }
    if (window.MeiDrilldownMeta?.preferUniqueObjectTargets) {
      return window.MeiDrilldownMeta.preferUniqueObjectTargets(list, allowed);
    }
    return list;
  }

  function createCaseCardObjectLinkButton(text) {
    const link = document.createElement("button");
    link.type = "button";
    link.className = "access-drilldown-case-object-link";
    link.textContent = String(text ?? "");
    return link;
  }

  function bindCaseCardObjectOpen(el, host, row, field, spec, config) {
    if (!(el instanceof HTMLElement) || !field) return;
    el.classList.add("is-object-link");
    el.setAttribute("role", "button");
    el.tabIndex = 0;
    el.title = el.title || "打开智能对象";
    const open = async (event) => {
      event?.preventDefault?.();
      event?.stopPropagation?.();
      const meta = await loadCaseCardDrilldownMeta();
      if (!meta?.resolveObjectFieldTargets || !meta?.emitObjectFieldOpen) return;
      const fieldCandidates = [
        field,
        ...cloneArray(spec?.fallback_fields || spec?.fallbackFields),
      ]
        .map((entry) => String(entry || "").trim())
        .filter(Boolean);
      const uniqueFields = [...new Set(fieldCandidates)];
      let props = caseCardObjectProps(config);
      let targets = [];
      const tryResolve = (nextProps) => {
        for (const candidate of uniqueFields) {
          const resolved = filterCaseCardObjectTargets(
            meta.resolveObjectFieldTargets(nextProps, row, candidate),
            spec,
          );
          if (resolved.length) return resolved;
        }
        return [];
      };
      targets = tryResolve(props);
      // 典型案例等页缺 object_locator 时会回落到 Warning，关联预警ID/健全机制会空；按字段再试 IssueResult。
      if (!targets.length) {
        const retryTypes = ["zhifa.IssueResult", "zhifa.Warning", "zhifa.MechanismDocument"];
        for (const type of retryTypes) {
          const locator = {
            object_type: type,
            objectType: type,
            ...(type === "zhifa.IssueResult"
              ? {
                  identity_field: "处理结果ID-问题跟踪ID",
                  identityField: "处理结果ID-问题跟踪ID",
                }
              : type === "zhifa.MechanismDocument"
                ? { identity_field: "机制名称", identityField: "机制名称" }
                : { identity_field: "预警ID", identityField: "预警ID" }),
          };
          const retryProps = { object_locator: locator, objectLocator: locator };
          targets = tryResolve(retryProps);
          if (targets.length) {
            props = retryProps;
            break;
          }
        }
      }
      // 多值 ID 芯片：仅打开当前 chip 对应 identity
      const chipKey = String(el.dataset?.objectKey || "").trim();
      if (chipKey) {
        const normalizeKey = (raw) =>
          String(raw ?? "")
            .trim()
            .replace(/^[《]+|[》]+$/g, "")
            .trim();
        const chipNorm = normalizeKey(chipKey);
        const filtered = targets.filter((target) => {
          const key = String(target?.objectKey || target?.object_key || "").trim();
          return (
            key === chipKey ||
            normalizeKey(key) === chipNorm ||
            (chipNorm && key.startsWith(`${chipNorm}-`))
          );
        });
        if (filtered.length) {
          targets = filtered;
        } else if (chipNorm) {
          const compositeField = String(row?.["处理结果ID-问题跟踪ID"] ?? "").trim();
          const compositeKeys = meta.splitMultiObjectKeys
            ? meta.splitMultiObjectKeys(compositeField)
            : compositeField
                .split(/[\n\r\s、，,;；]+/)
                .map((part) => String(part || "").trim())
                .filter(Boolean);
          const matchedComposites = compositeKeys.filter(
            (key) => key === chipNorm || key.startsWith(`${chipNorm}-`),
          );
          if (matchedComposites.length) {
            const template =
              targets[0] ||
              ({
                role: "self",
                objectType: "zhifa.IssueResult",
                keyMode: "identity",
                filterKey: "resultId",
                hasDetail: true,
              });
            targets = matchedComposites.map((objectKey) => ({
              ...template,
              objectType: "zhifa.IssueResult",
              objectKey,
              object_key: objectKey,
              label: objectKey,
              filterKey: "resultId",
              filter_key: "resultId",
            }));
          } else if (targets.length) {
            // 多值顿号拆分后仍对不上时：用当前 chip 身份覆盖模板 target（健全机制常见）
            const template = targets[0];
            targets = [
              {
                ...template,
                objectKey: chipNorm,
                object_key: chipNorm,
                label: chipNorm,
                filterKey: nonEmptyString(
                  template?.filterKey,
                  template?.filter_key,
                  "mechanismName",
                ),
                filter_key: nonEmptyString(
                  template?.filterKey,
                  template?.filter_key,
                  "mechanismName",
                ),
              },
            ];
          }
        }
        if (!targets.length && chipNorm && (field === "健全机制" || field === "机制名称")) {
          // 完全解析失败时仍尝试打开机制 PDF 详情
          const locator = {
            object_type: "zhifa.MechanismDocument",
            objectType: "zhifa.MechanismDocument",
            identity_field: "机制名称",
            identityField: "机制名称",
          };
          props = { object_locator: locator, objectLocator: locator };
          const links = meta.resolveObjectFieldLinks?.(props) || {};
          const specs = Array.isArray(links["机制名称"])
            ? links["机制名称"]
            : Array.isArray(links["健全机制"])
              ? links["健全机制"]
              : [];
          const spec0 = specs[0] || {};
          const openPopup =
            (spec0.openPopup && typeof spec0.openPopup === "object" ? spec0.openPopup : null) ||
            (spec0.open_popup && typeof spec0.open_popup === "object" ? spec0.open_popup : null) ||
            {
              kind: "scene_open",
              mode: "popup",
              type: "popup",
              projection: "overlay",
              overlay_size: "large",
              scene_id: "mechanism_document_detail_page",
              scene_file:
                "src/scene/home/t1/region-right-rail/section-effect/plane-mechanism-document-detail.mei",
              page_scene_id: "mechanism_document_detail_page",
              page_scene_file:
                "src/scene/home/t1/region-right-rail/section-effect/plane-mechanism-document-detail.mei",
              params: {
                metric: {
                  __ref: "metric_ref",
                  __args: {
                    arg0: "mechanism_document_detail",
                    bundle: "metrics/effectiveness.bundle.mei",
                  },
                },
                rowset_dataset_id: "mechanism_documents",
              },
            };
          targets = [
            {
              role: "relation",
              objectType: "zhifa.MechanismDocument",
              objectKey: chipNorm,
              keyMode: "identity",
              filterKey: "mechanismName",
              filter_key: "mechanismName",
              hasDetail: true,
              openPopup,
              detailPage:
                spec0.detailPage ||
                spec0.detail_page ||
                "zhifa/home/t1/region-right-rail/section-effect/plane-mechanism-document-detail",
              label: chipNorm,
            },
          ];
        } else {
          targets = [];
        }
      }
      if (!targets.length) return;
      const emitHost = host instanceof HTMLElement ? host : el;
      if (targets.length === 1) {
        meta.emitObjectFieldOpen(emitHost, targets[0], row, props);
        return;
      }
      openCaseCardObjectChooser(el, targets, (target) => {
        meta.emitObjectFieldOpen(emitHost, target, row, props);
      }, field);
    };
    el.addEventListener("click", open);
    el.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") open(event);
    });
  }

  function openCaseCardObjectChooser(anchor, targets, onPick, fieldLabel = "") {
    const existing = document.querySelector(".access-drilldown-object-chooser");
    if (existing) existing.remove();
    const menu = document.createElement("div");
    menu.className = "access-drilldown-object-chooser";
    menu.setAttribute("role", "menu");
    const title = document.createElement("div");
    title.className = "access-drilldown-object-chooser-title";
    const fieldName = String(fieldLabel || "").trim();
    title.textContent = fieldName ? `选择${fieldName}` : "选择智能对象";
    menu.appendChild(title);
    targets.forEach((target) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "access-drilldown-object-chooser-item";
      button.setAttribute("role", "menuitem");
      button.textContent = String(
        target?.label || target?.objectKey || target?.object_key || "",
      ).trim();
      button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        menu.remove();
        onPick?.(target);
      });
      menu.appendChild(button);
    });
    document.body.appendChild(menu);
    const rect = anchor.getBoundingClientRect();
    menu.style.position = "fixed";
    menu.style.left = `${Math.max(8, Math.min(rect.left, window.innerWidth - 260))}px`;
    menu.style.top = `${Math.min(rect.bottom + 4, window.innerHeight - 8)}px`;
    const close = (event) => {
      if (menu.contains(event.target) || anchor.contains?.(event.target)) return;
      menu.remove();
      document.removeEventListener("mousedown", close, true);
    };
    setTimeout(() => document.addEventListener("mousedown", close, true), 0);
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

  /** 与明细表一致：风险等级三色块 / 预警等级单色块 */
  function appendWarningLevelBlocks(valueEl, fieldName, rawValue) {
    const colors = {
      红: "var(--mei-color-warning_level_red, #E53935)",
      黄: "var(--mei-color-warning_level_yellow, #FFB300)",
      蓝: "var(--mei-color-warning_level_blue, #1E88E5)",
      灰: "var(--mei-color-warning_level_grey, #90A4AE)",
    };
    const order = ["红", "黄", "蓝"];
    const text = String(rawValue ?? "").trim();
    const active = new Set(order.filter((key) => text.includes(key)));
    const multi = fieldName === "风险等级";
    const root = document.createElement("span");
    root.className = `mei-warning-level-blocks ${multi ? "is-multi" : "is-single"}`;
    root.title = text;
    if (multi) {
      order.forEach((key) => {
        const on = active.has(key);
        const item = document.createElement("span");
        item.className = `mei-warning-level-item${on ? " is-on" : " is-off"}`;
        item.style.background = on ? colors[key] : "transparent";
        item.style.borderColor = on ? colors[key] : colors.灰;
        const label = document.createElement("span");
        label.className = "mei-warning-level-label";
        label.textContent = key;
        item.appendChild(label);
        root.appendChild(item);
      });
    } else {
      const top = order.find((key) => active.has(key)) || "";
      const on = Boolean(top);
      const item = document.createElement("span");
      item.className = `mei-warning-level-item${on ? " is-on" : " is-off"}`;
      item.style.background = on ? colors[top] : "transparent";
      item.style.borderColor = on ? colors[top] : colors.灰;
      const label = document.createElement("span");
      label.className = "mei-warning-level-label";
      label.textContent = on ? top : "";
      item.appendChild(label);
      root.appendChild(item);
    }
    valueEl.appendChild(root);
  }

  function mappingAllowsAutoFields(mapping) {
    if (!mapping || typeof mapping !== "object") return true;
    // 定制卡可显式关闭：仅展示 field_order。
    if (mapping.auto_fields === false || mapping.autoFields === false) return false;
    return true;
  }

  function isLongRowFormField(name, value) {
    if (value.length >= 28) return true;
    return /依据|规则|描述|问题|表现|政策|数据|情况|说明|附件/.test(String(name || ""));
  }

  /** 表单风格：按 field_order 展开行字段；排除注入噪音键，避免英文 title/matter 污染。 */
  function appendRowFormFields(panel, row, mapping) {
    if (!row || typeof row !== "object" || Array.isArray(row)) return;
    const exclude = new Set(
      [
        "title",
        "label",
        "matter",
        "matterId",
        "监督事项",
        "warningId",
        "resultId",
        "modelId",
        ...cloneArray(mapping?.exclude_fields || mapping?.excludeFields),
      ]
        .map((name) => String(name || "").trim())
        .filter(Boolean),
    );
    const preferred = cloneArray(mapping?.field_order || mapping?.fieldOrder)
      .map((name) => String(name || "").trim())
      .filter(Boolean);
    const labelMap =
      mapping?.field_labels && typeof mapping.field_labels === "object"
        ? mapping.field_labels
        : mapping?.fieldLabels && typeof mapping.fieldLabels === "object"
          ? mapping.fieldLabels
          : {};
    const seen = new Set();
    const keys = [];
    preferred.forEach((key) => {
      if (!Object.prototype.hasOwnProperty.call(row, key) || seen.has(key)) return;
      seen.add(key);
      keys.push(key);
    });
    // 通用默认：无 field_order → 全字段；有 field_order 且 auto_fields≠false → 顺序优先后补齐。
    // 定制卡：auto_fields=false + field_order → 仅展示指定列。
    if (!preferred.length || mappingAllowsAutoFields(mapping)) {
      Object.keys(row).forEach((key) => {
        if (seen.has(key)) return;
        seen.add(key);
        keys.push(key);
      });
    }
    const form = document.createElement("div");
    form.className = "access-drilldown-row-form";
    form.setAttribute("role", "list");
    keys.forEach((key) => {
      const name = String(key || "").trim();
      if (!name || name.startsWith("__") || exclude.has(name)) return;
      // 过滤英文/内部别名，行级详情只展示业务中文列。
      if (/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) return;
      const raw = row[name];
      if (raw != null && typeof raw === "object") return;
      let value = String(raw ?? "").trim();
      // 模型/预警等 ID：去掉 Excel 浮点旁路留下的 ".0"
      if ((name.endsWith("ID") || name.endsWith("Id") || name === "序号") && /^-?\d+\.0+$/.test(value)) {
        value = value.replace(/\.0+$/, "");
      } else if (typeof raw === "number" && Number.isFinite(raw) && Math.abs(raw % 1) < Number.EPSILON) {
        value = String(Math.trunc(raw));
      }
      const item = document.createElement("div");
      item.className = "access-drilldown-row-form-item";
      item.setAttribute("role", "listitem");
      if (isLongRowFormField(name, value)) {
        item.classList.add("access-drilldown-row-form-item--long");
      }
      const labelEl = document.createElement("div");
      labelEl.className = "access-drilldown-row-form-label";
      labelEl.textContent = String(labelMap[name] || name).trim() || name;
      const valueEl = document.createElement("div");
      valueEl.className = "access-drilldown-row-form-value";
      if (name === "风险等级" || name === "预警等级") {
        valueEl.classList.add("access-drilldown-row-form-value--level");
        appendWarningLevelBlocks(valueEl, name, value);
      } else {
        valueEl.textContent = value || "—";
      }
      item.appendChild(labelEl);
      item.appendChild(valueEl);
      form.appendChild(item);
    });
    if (form.childElementCount) panel.appendChild(form);
  }

  function appendCaseDetailSection(block, section, row, mapping, config = null) {
    const label = String(section?.label || "").trim();
    const kind = String(section?.kind || "").trim();
    const field = String(section?.field || "").trim();
    const hideLabel = section?.hide_label === true || section?.hideLabel === true;
    let value = resolveCaseDetailFieldValue(row, section);
    if (kind === "situation") {
      value = resolveCaseDetailSituationText(row, mapping);
    }
    if (!label && !hideLabel && kind !== "situation" && !field) return;
    const sectionEl = document.createElement("div");
    sectionEl.className = "access-drilldown-case-detail-section";
    if (kind === "id") {
      const idRow = document.createElement("div");
      idRow.className = "access-drilldown-case-detail-id-row";
      const idLabel = document.createElement("span");
      idLabel.className = "access-drilldown-case-detail-id-label";
      idLabel.textContent = label;
      idRow.appendChild(idLabel);
      const rawKeys = (() => {
        if (window.MeiDrilldownMeta?.splitMultiObjectKeys) {
          return window.MeiDrilldownMeta.splitMultiObjectKeys(value);
        }
        return String(value ?? "")
          .split(/[\n\r\s、，,;；]+/)
          .map((part) => String(part || "").trim().replace(/^\d+\.\s*/, ""))
          .filter((part) => part && part !== "-" && part !== "—" && part !== "－" && part !== "——");
      })();
      // Never fall back to raw cell text: blank sentinels like `——` must stay non-links.
      const keys = rawKeys;
      if (!keys.length) {
        const empty = document.createElement("span");
        empty.className = "access-drilldown-case-detail-id-chip";
        empty.textContent = "—";
        idRow.appendChild(empty);
      } else {
        const wantsLink = section?.object_link === true || section?.objectLink === true;
        keys.forEach((key) => {
          if (wantsLink && field) {
            const idValue = createCaseCardObjectLinkButton(key);
            idValue.dataset.objectKey = key;
            bindCaseCardObjectOpen(idValue, block, row, field, section, config);
            idRow.appendChild(idValue);
            return;
          }
          const idValue = document.createElement("span");
          idValue.className = "access-drilldown-case-detail-id-chip";
          idValue.textContent = key;
          idRow.appendChild(idValue);
        });
      }
      sectionEl.appendChild(idRow);
      block.appendChild(sectionEl);
      return;
    }
    if (label && !hideLabel) {
      const labelEl = document.createElement("div");
      labelEl.className = "access-drilldown-case-detail-section-label";
      labelEl.textContent = label;
      sectionEl.appendChild(labelEl);
    }
    const body = document.createElement("div");
    body.className = "access-drilldown-case-detail-section-body";
    if ((label === "健全机制" || label === "制度文件" || field === "健全机制") && value) {
      const list = document.createElement("ul");
      list.className = "access-drilldown-case-detail-mechanism-list";
      const wantsLink = section?.object_link === true || section?.objectLink === true;
      splitMechanismDocuments(value).forEach((doc) => {
        const li = document.createElement("li");
        if (wantsLink && field) {
          const link = createCaseCardObjectLinkButton(doc);
          // 机制名称 identity 通常不含书名号；展示可带《》，匹配时剥离
          link.dataset.objectKey = String(doc || "")
            .trim()
            .replace(/^[《]+|[》]+$/g, "")
            .trim();
          bindCaseCardObjectOpen(link, block, row, field, section, config);
          li.appendChild(link);
        } else {
          li.textContent = doc;
        }
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
    // 标题已是身份 ID 时不再重复副标题
    if (subtitleId && subtitleId !== title) {
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

  function appendTypicalCaseTagRow(panel, row, mapping, config = null) {
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
      if (kind === "warning_level") {
        tag.className =
          "access-drilldown-typical-case-tag access-drilldown-typical-case-tag--warning-level";
        const labelEl = document.createElement("span");
        labelEl.className = "access-drilldown-typical-case-tag-label";
        labelEl.textContent = `${label}：`;
        tag.appendChild(labelEl);
        appendWarningLevelBlocks(tag, field, value);
      } else if (spec?.object_link === true || spec?.objectLink === true) {
        // 与明细表 cell-object-link 一致：标签普通文本 + accent 下划线值链接
        tag.className =
          "access-drilldown-typical-case-tag access-drilldown-typical-case-tag--with-object-link";
        const labelEl = document.createElement("span");
        labelEl.className = "access-drilldown-typical-case-tag-label";
        labelEl.textContent = `${label}：`;
        const link = createCaseCardObjectLinkButton(value);
        bindCaseCardObjectOpen(link, panel, row, field, spec, config);
        tag.appendChild(labelEl);
        tag.appendChild(link);
      } else {
        tag.className = "access-drilldown-typical-case-tag";
        tag.textContent = `${label}：${value}`;
      }
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
      const full = resolveCaseDetailFieldValue(row, spec) || "—";
      const item = document.createElement("div");
      item.className = "access-drilldown-typical-case-fact";
      item.style.flex = "0 0 auto";
      const labelEl = document.createElement("div");
      labelEl.className = "access-drilldown-typical-case-fact-label";
      labelEl.textContent = label;
      const valueEl = document.createElement("div");
      valueEl.className = "access-drilldown-typical-case-fact-value";
      valueEl.textContent = full;
      item.appendChild(labelEl);
      item.appendChild(valueEl);
      factsRoot.appendChild(item);
    });
    if (factsRoot.childElementCount) panel.appendChild(factsRoot);
  }

  function resolveVerifiedStatusPill(row, spec) {
    const field = String(spec?.field || "是否查实").trim();
    const raw = String(row?.[field] ?? "").trim();
    if (!raw || raw === "—" || raw === "-" || raw === "－") {
      return null;
    }
    const countField = String(spec?.count_field || spec?.countField || "查实条数").trim();
    const countRaw = String(row?.[countField] ?? "").trim();
    const countNum = Number(String(countRaw).replace(/,/g, ""));
    const countText =
      Number.isFinite(countNum) && countNum > 0
        ? String(Math.trunc(countNum))
        : countRaw && countRaw !== "—"
          ? countRaw
          : "";
    if (raw === "否" || raw === "0" || raw.includes("否")) {
      return { label: "未查实", active: false };
    }
    if (raw.includes("是") || (Number.isFinite(countNum) && countNum > 0)) {
      return { label: countText ? `查实${countText}条` : "查实", active: true };
    }
    return null;
  }

  function appendTypicalCaseStatusRow(panel, row, mapping, config = null) {
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
      const kind = String(spec?.kind || "").trim();
      if (kind === "id_chip" || kind === "id") {
        const label = String(spec?.label || spec?.field || "").trim();
        const field = String(spec?.field || "").trim();
        if (!label || !field) return;
        const value = resolveCaseDetailFieldValue(row, spec);
        const rawKeys = (() => {
          if (window.MeiDrilldownMeta?.splitMultiObjectKeys) {
            return window.MeiDrilldownMeta.splitMultiObjectKeys(value);
          }
          return String(value ?? "")
            .split(/[\n\r\s、，,;；]+/)
            .map((part) => String(part || "").trim().replace(/^\d+\.\s*/, ""))
            .filter((part) => part && part !== "-" && part !== "—" && part !== "－" && part !== "——");
        })();
        // Never fall back to raw cell text: blank sentinels like `——` must stay non-links.
        const keys = rawKeys;
        if (!keys.length) return;
        const wantsLink = spec?.object_link === true || spec?.objectLink === true;
        keys.forEach((key) => {
          if (wantsLink) {
            // 可点 ID（如办理结果ID）：只展示 ID 值，与明细表 object-link 一致，不重复字段标签。
            const link = createCaseCardObjectLinkButton(key);
            link.dataset.objectKey = key;
            bindCaseCardObjectOpen(link, panel, row, field, spec, config);
            pills.appendChild(link);
            return;
          }
          const pill = document.createElement("span");
          pill.className =
            "access-drilldown-typical-case-status-pill access-drilldown-typical-case-status-pill--id";
          pill.textContent = `${label} ${key}`;
          pills.appendChild(pill);
        });
        return;
      }
      if (kind === "verified_count" || kind === "verified") {
        const resolved = resolveVerifiedStatusPill(row, spec);
        if (!resolved) return;
        const pill = document.createElement("span");
        pill.className = `access-drilldown-typical-case-status-pill${
          resolved.active ? " access-drilldown-typical-case-status-pill--on" : ""
        }`;
        pill.textContent = resolved.label;
        pills.appendChild(pill);
        return;
      }
      const label = String(spec?.label || spec?.field || "").trim();
