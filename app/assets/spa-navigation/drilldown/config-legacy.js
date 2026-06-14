  function resolveLegacySceneProjectionConfig(detail) {
    const metricId = String(detail?.metric_id || "").trim();
    const popup =
      detail?.popup && typeof detail.popup === "object" && !Array.isArray(detail.popup) ? detail.popup : {};
    const boardFields = resolveBoardLinkFields(popup, detail?.scene_local_nav_by_target);
    const projectionSlots = normalizeProjectionSlots(
      popup?.projection_slots || popup?.projectionSlots,
    );
    if (projectionSlots.length) {
      return resolveProjectionSlotsDrilldownConfig(detail, popup, boardFields, projectionSlots);
    }
    const runtime = runtimeDrilldownConfig(detail);
    if (!Object.keys(runtime).length) {
      return disabledDrilldownConfig(
        "missing_analysis_contract",
        `scene projection requires analysis_contract; metric \`${metricId || "unknown"}\` is missing runtime analysis_contract`,
      );
    }
    const analysisLink =
      detail?.analysis_link && typeof detail.analysis_link === "object" ? detail.analysis_link : {};
    const boardSceneId = nonEmptyString(
      detail?.board_scene_id,
      boardFields?.sceneId,
      popup?.scene_id,
      popup?.sceneId,
    );
    const hostSceneId = nonEmptyString(
      detail?.host_scene_id,
      detail?.dataset_scene_id,
      detail?.scene_id !== boardSceneId ? detail?.scene_id : "",
      runtime?.scene_id,
      runtime?.sceneId,
      resolveDrilldownSceneId(detail, runtime),
    );
    const sceneId = hostSceneId;
    const queryStateId = nonEmptyString(
      detail?.query_state_id,
      detail?.queryStateId,
      runtime?.query_state_id,
      runtime?.queryStateId,
    );
    const runtimeEnabled = boolValue(
      detail?.analysis_enabled,
      runtime?.enabled,
    );
    const explainKind = nonEmptyString(
      detail?.analysis_kind,
      detail?.explain_kind,
      runtime?.kind,
      runtime?.explain_kind,
    );
    const explainMetrics = normalizeExplainMetrics(
      detail?.explain_metrics,
      runtime?.explain_metrics,
      runtime?.explainMetrics,
      runtime?.blocks,
    );
    let detailFields = cloneArray(detail?.explain_detail_fields);
    if (!detailFields.length) detailFields = cloneArray(runtime?.detail_fields);
    if (!detailFields.length) detailFields = cloneArray(runtime?.detailFields);
    let columns = [];
    if (!columns.length) columns = cloneArray(runtime?.columns);
    if (!columns.length) columns = cloneArray(runtime?.detail_fields);
    if (!columns.length) columns = cloneArray(runtime?.detailFields);
    if (!columns.length) columns = cloneArray(detailFields);
    let headers = [];
    if (!headers.length) headers = cloneArray(runtime?.headers);
    let basisRefs = cloneArray(detail?.explain_basis_refs);
    if (!basisRefs.length) basisRefs = cloneArray(runtime?.basis_refs);
    if (!basisRefs.length) basisRefs = cloneArray(runtime?.basisRefs);
    let recommendedDimensions = cloneArray(detail?.explain_recommended_dimensions);
    if (!recommendedDimensions.length) recommendedDimensions = cloneArray(runtime?.recommended_dimensions);
    if (!recommendedDimensions.length) recommendedDimensions = cloneArray(runtime?.recommendedDimensions);
    const ratioNumerator = nonEmptyString(
      runtime?.ratio_numerator,
      runtime?.ratioNumerator,
    );
    const ratioDenominator = nonEmptyString(
      runtime?.ratio_denominator,
      runtime?.ratioDenominator,
    );
    const ratioFormula = nonEmptyString(
      runtime?.ratio_formula,
      runtime?.ratioFormula,
    );
    const tableMetricId = nonEmptyString(
      runtime?.table_metric_id,
      runtime?.tableMetricId,
      detail?.table_metric_id,
    );
    const datasetId = resolveDrilldownDatasetId(detail, {
      sceneId,
      hostSceneId,
      boardSceneId,
      tableMetricId,
      datasetId: nonEmptyString(runtime?.dataset_id, runtime?.datasetId),
    });
    const layoutPreset = nonEmptyString(
      runtime?.layout_preset,
      runtime?.layoutPreset,
    );
    const defaultSceneBindings = sceneBindingDefaults(
      boardSceneId,
      sceneDrilldownContextMap(detail, "scene_bindings_by_id"),
      sceneDrilldownContextMap(detail, "scene_examples_by_id"),
      sceneDrilldownAssemblyById(detail),
    );
    const tabMetrics = normalizeTabMetricOverrides(
      defaultSceneBindings,
      popup?.entry_overrides,
      popup?.bindings,
      popup?.entryOverrides,
      panelPopupSlotSources(popup),
      popup?.metrics,
      detail?.analysis_tab_metrics,
      runtime?.analysis_tab_metrics,
      runtime?.tab_metrics,
      runtime?.tabMetrics,
    );
    const panelPopup = Boolean(boardFields?.panelPopup) || isPanelPopupConfig(popup);
    const boardLink = Boolean(boardFields?.boardLink);
    const panelTemplate = panelPopup
      ? normalizePanelTemplateId(nonEmptyString(popup?.template, boardFields?.legacyTemplate))
      : "";
    const boardSceneFile = nonEmptyString(
      detail?.board_scene_file,
      boardFields?.sceneFile,
      popup?.scene_file,
      popup?.sceneFile,
    );
    const sceneAssembly = sceneProjectionAssembly(
      boardSceneId,
      sceneDrilldownAssemblyById(detail),
    );
    const sceneLocalNav =
      boardFields?.localNav ||
      normalizeSceneLocalNav(popup?.local_nav || popup?.localNav) ||
      normalizeSceneLocalNav(sceneAssembly?.local_nav || sceneAssembly?.localNav) ||
      resolveSceneLocalNav(boardSceneFile, detail?.scene_local_nav_by_target) ||
      null;
    const sceneShell = resolveSceneShell(sceneAssembly);
    const projection = normalizeProjection(
      nonEmptyString(detail?.projection, popup?.projection, boardFields?.projection, "overlay"),
    );
    const hasDetail = Boolean(
      tableMetricId ||
        columns.length ||
        detailFields.length ||
        nonEmptyString(
          detail?.explain_detail_dataset,
          runtime?.dataset_id,
          runtime?.datasetId,
        ),
    );
    const tabs = resolveDrilldownTabs({
      detail,
      runtime,
      explainKind,
      hasDetail,
      localNav: sceneLocalNav,
    });
    const ratioNote = buildRatioExplainNote({
      numerator: ratioNumerator,
      denominator: ratioDenominator,
      formula: ratioFormula,
    });
    const structuredBoard = Boolean(sceneShell?.layoutMode) && sceneShell.layoutMode !== "generic_tabs";
    const overlaySize = resolveDrilldownOverlaySize({ popup, boardFields, structuredBoard, sceneShell });
    const legacyParams = boardFields?.params || normalizeSceneParams(popup?.params);
    const legacyFilterSchema = normalizeAnalyticsFilterSchema(popup?.filter_schema || popup?.filterSchema);
    return {
      enabled:
        (boardLink && Boolean(boardSceneId)) ||
        (panelPopup && Boolean(boardSceneId) && Boolean(panelTemplate)) ||
        popup?.mode === "popup" ||
        (runtimeEnabled !== false && Boolean(hostSceneId || boardSceneId)),
      genericDrilldown: !structuredBoard,
      structuredBoard,
      sceneShell,
      hasChartZone: false,
      hasRowPreviewZone: false,
      overlaySize,
      sceneId,
      hostSceneId,
      hostSceneFile: nonEmptyString(detail?.host_scene_file, detail?.scene_path),
      queryStateId,
      params: legacyParams,
      filterSchema: legacyFilterSchema,
      rowsetDatasetId: nonEmptyString(
        legacyFilterSchema.rowsetDatasetId,
        sceneParamRowsetDatasetId(legacyParams),
      ),
      boardSceneId,
      boardLink,
      boardSceneFile,
      sceneLocalNav,
      projection,
      panelPopup,
      panelTemplate,
      panelTitle: nonEmptyString(popup?.title),
      title: nonEmptyString(
        popup?.title,
        detail?.explain_title,
        runtime?.title,
        detail?.label,
        metricId,
        "指标明细",
      ),
      note: nonEmptyString(
        runtime?.note,
        detail?.explain_note,
        detail?.analysis_note,
        ratioNote,
      ),
      tableMetricId,
      datasetId,
      columns,
      headers,
      detailFields,
      basisRefs,
      recommendedDimensions,
      ratioParts: {
        numerator: ratioNumerator,
        denominator: ratioDenominator,
        formula: ratioFormula,
      },
      compositionBy: cloneArray(detail?.explain_composition_by).length
        ? cloneArray(detail?.explain_composition_by)
        : cloneArray(runtime?.composition_by).length
          ? cloneArray(runtime?.composition_by)
          : cloneArray(recommendedDimensions),
      trendField: nonEmptyString(runtime?.trend_field, runtime?.trendField, detail?.explain_trend_field),
      trendGrain: nonEmptyString(runtime?.trend_grain, runtime?.trendGrain, detail?.explain_trend_grain, "month"),
      layoutPreset,
      explainKind,
      explainMetrics: explainMetrics.byId,
      explainMetricOrder: explainMetrics.order,
      tabs,
      tabMetrics,
      link: {
        mode: nonEmptyString(analysisLink.mode),
        template: nonEmptyString(analysisLink.template),
        entry: nonEmptyString(analysisLink.entry),
        defaultFocus: nonEmptyString(analysisLink.default_focus, analysisLink.defaultFocus),
      },
      popup: {
        mode: nonEmptyString(
          popup?.mode,
          boardLink ? "board_link" : panelPopup ? "popup_panel" : "popup",
        ),
        template: nonEmptyString(panelTemplate, popup?.template, popup?.legacy_template),
        entry: nonEmptyString(
          popup?.entry,
          popup?.entry_tab,
          popup?.entryTab,
          popup?.focus,
          boardFields?.entry,
        ),
        focus: nonEmptyString(popup?.entry, popup?.focus, popup?.entry_tab, popup?.entryTab, boardFields?.entry),
        scene_file: boardSceneFile,
        scene_id: boardSceneId,
        scene: boardFields?.sceneRef || popup?.scene || null,
        projection,
        local_nav: sceneLocalNav,
        params: boardFields?.params || normalizeSceneParams(popup?.params),
        entry_overrides: panelPopupSlotSources(popup),
        slots: panelPopupSlotSources(popup),
      },
      chartKind: nonEmptyString(runtime?.chart_kind, runtime?.chartKind),
      mapping: runtime?.mapping && typeof runtime.mapping === "object" ? runtime.mapping : null,
      pageSize:
        positiveInt(
          runtime?.page_size,
          runtime?.pageSize,
          8,
        ) || 8,
      cellPreviewMaxChars:
        positiveInt(
          runtime?.cell_preview_max_chars,
          runtime?.cellPreviewMaxChars,
        ) > 0
          ? positiveInt(
              runtime?.cell_preview_max_chars,
              runtime?.cellPreviewMaxChars,
            )
          : 0,
      columnMinWidth:
        positiveInt(
          runtime?.column_min_width,
          runtime?.columnMinWidth,
        ) > 0
          ? positiveInt(
              runtime?.column_min_width,
              runtime?.columnMinWidth,
            )
          : 0,
    };
  }

