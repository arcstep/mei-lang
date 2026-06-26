      }
    });
    return hits;
  }

  function detectMetricVerticalBandDrift(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".preview-card[data-mei-panel-id]").forEach((card) => {
      if (hits.length >= limit) return;
      const rect = card.getBoundingClientRect();
      if (rect.width < 8 || rect.height < 24) return;
      const roles = metricRoleNodes(card);
      const labelNode = roles.find((entry) => entry.props.metric_role === "label");
      const valueNode = roles.find((entry) => entry.props.metric_role === "value");
      if (!labelNode || !valueNode) return;
      const labelRect = labelNode.node.getBoundingClientRect();
      const valueRect = valueNode.node.getBoundingClientRect();
      const midY = rect.top + rect.height * 0.5;
      const labelCenterY = (labelRect.top + labelRect.bottom) / 2;
      const valueBottomY = valueRect.bottom;
      const panelId = closestPanelId(card);
      const label = closestPanelLabel(card);
      if (labelCenterY > midY + 6) {
        hits.push({
          panelId,
          label,
          role: "label",
          spread: labelCenterY - midY,
          detail: "label_below_midline",
        });
        return;
      }
      if (valueBottomY < midY - 4) {
        hits.push({
          panelId,
          label,
          role: "value",
          spread: midY - valueBottomY,
          detail: "value_above_midline",
        });
      }
    });
    return hits;
  }

  function detectMetricAlignmentDrift(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".panel-body-cell").forEach((body) => {
      if (hits.length >= limit) return;
      const groupPanelId = closestPanelId(body);
      const groupLabel = closestPanelLabel(body);
      const cards = Array.from(
        body.querySelectorAll(':scope > .preview-card[data-mei-panel-id]'),
      )
        .map((card) => {
          const rect = card.getBoundingClientRect();
          const roles = metricRoleNodes(card);
          const slotRects = {
            label: roles
              .filter((entry) => entry.props.metric_role === "label")
              .map((entry) => entry.node.getBoundingClientRect().left - rect.left),
            value: roles
              .filter((entry) => entry.props.metric_role === "value")
              .map((entry) => entry.node.getBoundingClientRect().right - rect.left),
            unit: roles
              .filter((entry) => entry.props.metric_role === "unit")
              .map((entry) => entry.node.getBoundingClientRect().right - rect.left),
          };
          return { rect, slotRects };
        })
        .filter((entry) => entry.rect.width > 1 && entry.rect.height > 1);
      if (cards.length < 2) return;
      const buckets = new Map();
      cards.forEach((card) => {
        const bucket = widthBucket(card.rect.width);
        if (!buckets.has(bucket)) buckets.set(bucket, []);
        buckets.get(bucket).push(card);
      });
      buckets.forEach((bucketCards) => {
        if (bucketCards.length < 2 || hits.length >= limit) return;
        const labelLefts = bucketCards.flatMap((item) => item.slotRects.label);
        const valueRights = bucketCards.flatMap((item) => item.slotRects.value);
        const unitRights = bucketCards.flatMap((item) => item.slotRects.unit);
        const spreads = [];
        if (labelLefts.length >= 2) {
          spreads.push({ role: "label", spread: Math.max(...labelLefts) - Math.min(...labelLefts) });
        }
        if (valueRights.length >= 2) {
          spreads.push({ role: "value", spread: Math.max(...valueRights) - Math.min(...valueRights) });
        }
        if (unitRights.length >= 2) {
          spreads.push({ role: "unit", spread: Math.max(...unitRights) - Math.min(...unitRights) });
        }
        const worst = spreads.sort((a, b) => b.spread - a.spread)[0];
        if (worst && worst.spread > 8) {
          hits.push({
            panelId: groupPanelId,
            label: groupLabel,
            role: worst.role,
            spread: worst.spread,
          });
        }
      });
    });
    return hits;
  }

  const LAYOUT_AUDIT_EVENT = "mei:layout-audit";

  function layoutAuditStorageKey(sourcePath) {
    const pathname = String(window.location.pathname || "").trim();
    const source = String(sourcePath || "main.mei").trim() || "main.mei";
    return `mei:layout-audit:${pathname}:${source}`;
  }

  function persistLayoutAudit(payload) {
    try {
      const key = layoutAuditStorageKey(payload?.sourcePath);
      sessionStorage.setItem(key, JSON.stringify(payload));
    } catch (_) {}
  }

  function dispatchLayoutAudit(targetDocument, payload) {
    if (!targetDocument) return;
    targetDocument.dispatchEvent(new CustomEvent(LAYOUT_AUDIT_EVENT, { detail: payload }));
  }

  function sceneIdFromLocation() {
    try {
      const url = new URL(window.location.href);
      const scene = String(url.searchParams.get("scene") || "").trim();
      if (scene) return scene;
      const match = String(url.pathname || "").match(/\/scene\/([^/?#]+)/i);
      if (match && match[1]) {
        return decodeURIComponent(match[1]);
      }
    } catch (_) {}
    return "";
  }

  function collectPanelMeta(stage) {
    const map = new Map();
    if (!stage || !stage.querySelectorAll) return map;
    stage.querySelectorAll("[data-mei-panel-id]").forEach((panel) => {
      const panelId = String(panel.getAttribute("data-mei-panel-id") || "").trim();
      if (!panelId || map.has(panelId)) return;
      const label =
        String(panel.querySelector?.("[data-mei-panel-head]")?.getAttribute?.("aria-label") || "").trim() ||
        panelId;
      map.set(panelId, {
        panelId,
        panelLabel: label,
        componentLabel: label,
      });
    });
    return map;
  }

  function enrichLayoutDiagnostics(diagnostics, panelMeta, sceneId, targetFile) {
    const list = Array.isArray(diagnostics) ? diagnostics : [];
    return list.map((diag) => {
      const panelId = String(diag?.panelId || "").trim();
      const panel = panelId ? panelMeta.get(panelId) : null;
      const panelLabel = String(diag?.panelLabel || diag?.label || panel?.panelLabel || "").trim();
      const componentLabel = String(diag?.component_label || panelLabel || panel?.componentLabel || "").trim();
      return {
        ...diag,
        panelId: panelId || undefined,
        panelLabel: panelLabel || undefined,
        component_label: componentLabel || undefined,
        scene_id: sceneId || undefined,
        target_file: targetFile || undefined,
        source_path: String(diag?.source_path || targetFile || "").trim() || undefined,
      };
    });
  }

  function publishLayoutAudit(root, stage, diagnostics, report = {}) {
    const targetFile = String(
      root?.dataset?.targetFile || root?.dataset?.sourcePath || root?.dataset?.activeTarget || "main.mei"
    ).trim();
    const sceneId = String(root?.dataset?.sceneId || "").trim() || sceneIdFromLocation();
    const panelMeta = collectPanelMeta(stage);
    const normalizedDiagnostics = enrichLayoutDiagnostics(diagnostics, panelMeta, sceneId, targetFile);
    const normalizedWorstPanels = (Array.isArray(report?.worstPanels) ? report.worstPanels : []).map((entry) => {
      const panelId = String(entry?.panelId || "").trim();
      const panel = panelId ? panelMeta.get(panelId) : null;
      return {
        ...entry,
        panelId: panelId || entry?.panelId,
        panelLabel: String(entry?.panelLabel || panel?.panelLabel || "").trim() || undefined,
      };
    });
    const payload = {
      sourcePath: targetFile || "main.mei",
      targetFile: targetFile || "main.mei",
      sceneId: sceneId || undefined,
      diagnostics: normalizedDiagnostics,
      score: Number(report?.score || 0),
      blocking: report?.blocking === true,
      worstPanels: normalizedWorstPanels,
      metrics: report?.metrics && typeof report.metrics === "object" ? report.metrics : {},
    };
    window.__meiLastLayoutEval = payload;
    persistLayoutAudit(payload);
    dispatchLayoutAudit(document, payload);
    if (window.parent && window.parent !== window) {
      try {
        window.parent.__meiLastLayoutEval = payload;
      } catch (_) {}
      try {
        dispatchLayoutAudit(window.parent.document, payload);
      } catch (_) {}
      try {
        window.parent.postMessage({ type: LAYOUT_AUDIT_EVENT, detail: payload }, window.location.origin);
      } catch (_) {}
    }
  }

  function runLayoutAudit(
    root,
    stage,
    designWidth,
    designHeight,
    contentWidth,
    contentHeight,
    extentWidth,
  ) {
    if (!root || !stage) return;
    const diagnostics = [];
    if (extentWidth > designWidth + 1) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_canvas_overflow_x",
        message: `横向内容超出设计宽度：实测 ${Math.round(extentWidth)}px / 设计 ${Math.round(designWidth)}px`,
      });
    }
    if (contentHeight > designHeight + 1) {
      diagnostics.push({
        severity: "warning",
        code: "layout_eval_canvas_overflow_y",
        message: `纵向内容超出设计高度：实测 ${Math.round(contentHeight)}px / 设计 ${Math.round(designHeight)}px`,
      });
    }
    const clipped = detectClippedNodes(stage);
    if (clipped.length) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_clipped_content",
        panelId: clipped[0]?.panelId || null,
        message: `检测到父容器裁切风险：${clipped.map((hit) => hit.label).join("、")}`,
      });
    }
    const degenerate = detectDegenerateBoxes(stage);
    if (degenerate.length) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_degenerate_box",
        panelId: degenerate[0]?.panelId || null,
        message: `检测到零尺寸/退化盒：${degenerate.map((hit) => hit.label).join("、")}`,
      });
    }
    const spacing = detectHeadMetricSpacing(stage);
    if (spacing.length) {
      diagnostics.push({
        severity: "info",
        code: "layout_eval_head_body_spacing_loose",
        panelId: spacing[0]?.panelId || null,
        message: `检测到标题与指标区起始距离偏大：${spacing
          .map((hit) => `${hit.label}(+${Math.round(hit.gap)}px)`)
          .join("、")}`,
      });
    }
    const bottomClip = detectBottomClipRisk(stage);
    if (bottomClip.length) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_panel_bottom_clip_risk",
        panelId: bottomClip[0]?.panelId || null,
        message: `检测到 panel 底部裁切风险：${bottomClip
          .map((hit) => `${hit.label}(+${Math.round(hit.overflowPx)}px)`)
          .join("、")}`,
      });
    }
    const overlap = detectCardOverlap(stage);
    if (overlap.length) {
      diagnostics.push({
        severity: "error",
        code: "layout_eval_card_overlap",
        panelId: overlap[0]?.panelId || null,
        message: `检测到卡片重叠：${overlap
          .map((hit) => `${hit.label}(${hit.pair.join(" x ")})`)
          .join("、")}`,
      });
    }
    const gapBudget = detectCardGapBudget(stage);
    if (gapBudget.length) {
      diagnostics.push({
        severity: gapBudget.some(
          (hit) => hit.axis === "padding" || hit.axis === "padding_symmetry",
        )
          ? "warning"
          : "info",
        code: "layout_eval_card_gap_budget_runtime",
        panelId: gapBudget[0]?.panelId || null,
        message: `检测到卡组 gap/留白偏离预算：${gapBudget
          .map((hit) => {
            if (hit.axis === "padding") return `${hit.label}(四周留白≈${Math.round(hit.gap)}px)`;
            if (hit.axis === "padding_symmetry") {
              return `${hit.label}(左右留白≈${Math.round(hit.leftPad || 0)}px/${Math.round(hit.rightPad || 0)}px)`;
            }
            const axis = hit.axis === "vertical" ? "纵向" : "横向";
            return `${hit.label}(${axis} gap≈${Math.round(hit.gap)}px)`;
          })
          .join("、")}`,
      });
    }
    const alignment = detectMetricAlignmentDrift(stage);
    if (alignment.length) {
      diagnostics.push({
        severity: "warning",
        code: "layout_eval_metric_alignment_drift",
        panelId: alignment[0]?.panelId || null,
        message: `检测到指标参考线漂移：${alignment
          .map((hit) => `${hit.label}(${hit.role} 偏移≈${Math.round(hit.spread)}px)`)
          .join("、")}`,
      });
    }
    const verticalBands = detectMetricVerticalBandDrift(stage);
    if (verticalBands.length) {
      diagnostics.push({
        severity: "warning",
        code: "layout_eval_metric_vertical_band_drift",
        panelId: verticalBands[0]?.panelId || null,
        message: `检测到指标垂直分区落点偏离：${verticalBands
          .map((hit) => {
            const hint =
              hit.detail === "label_below_midline"
                ? "标题区落入下半区"
                : "数值区落在上半区";
            return `${hit.label}(${hint}，≈${Math.round(hit.spread)}px)`;
          })
          .join("、")}`,
      });
    }
    publishLayoutAudit(root, stage, diagnostics, buildRuntimeEvalReport(diagnostics));
  }
