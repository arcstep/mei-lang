    canvasWidth,
    designHeight,
    fluidHeight,
    contentMaxWidth,
  ) {
    const extent = measureStageContentExtent(stage, contentMaxWidth);
    return {
      width: Math.max(canvasWidth, extent.width),
      extentWidth: extent.width,
      height: Math.max(fluidHeight ? extent.height : designHeight, extent.height),
      extentHeight: extent.height,
    };
  }

  function nodeAuditLabel(node) {
    if (!node) return "unknown";
    const id = node.id ? `#${node.id}` : "";
    if (id) return id;
    const area =
      node.getAttribute("data-area") || node.getAttribute("area") || node.dataset?.area;
    if (area) return `${node.tagName.toLowerCase()}[area=${area}]`;
    const klass = String(node.className || "")
      .split(/\s+/)
      .map((item) => item.trim())
      .filter(Boolean)[0];
    if (klass) return `${node.tagName.toLowerCase()}.${klass}`;
    return node.tagName.toLowerCase();
  }

  function closestPanelId(node) {
    const panel = node?.closest?.(".preview-card[data-mei-panel-id]");
    const id = String(panel?.getAttribute?.("data-mei-panel-id") || "").trim();
    return id || null;
  }

  function closestPanelLabel(node) {
    const panel = node?.closest?.(".preview-card[data-mei-panel-id]");
    return panel ? nodeAuditLabel(panel) : nodeAuditLabel(node);
  }

  function parsePropsPayload(node) {
    const raw = String(node?.getAttribute?.("data-props") || "").trim();
    if (!raw) return {};
    try {
      return JSON.parse(raw);
    } catch (_) {
      return {};
    }
  }

  function metricRoleNodes(card) {
    return Array.from(card.querySelectorAll("[data-props]"))
      .map((node) => ({ node, props: parsePropsPayload(node) }))
      .filter((entry) => String(entry.props?.metric_role || "").trim());
  }

  function widthBucket(width) {
    return Math.round(Number(width || 0) / 16) * 16;
  }

  function buildRuntimeEvalReport(diagnostics) {
    const items = Array.isArray(diagnostics) ? diagnostics : [];
    const severityWeight = {
      error: 100,
      warning: 40,
      info: 10,
    };
    const metrics = {
      total: items.length,
      errors: 0,
      warnings: 0,
      infos: 0,
      countsByCode: {},
    };
    const panelScores = new Map();
    let score = 0;
    items.forEach((diag) => {
      const severity = String(diag?.severity || "info").trim().toLowerCase();
      if (severity === "error") metrics.errors += 1;
      else if (severity === "warning") metrics.warnings += 1;
      else metrics.infos += 1;
      const weight = severityWeight[severity] || 0;
      score += weight;
      const code = String(diag?.code || "layout_eval_runtime_unknown").trim();
      metrics.countsByCode[code] = (metrics.countsByCode[code] || 0) + 1;
      const panelId = String(diag?.panelId || "").trim();
      if (panelId) {
        panelScores.set(panelId, (panelScores.get(panelId) || 0) + weight);
      }
    });
    const worstPanels = Array.from(panelScores.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([panelId, panelScore]) => ({
        panelId,
        score: panelScore,
      }));
    return {
      score,
      blocking: metrics.errors > 0,
      worstPanels,
      metrics,
    };
  }

  function detectDegenerateBoxes(stage, limit = 8) {
    const hits = [];
    stage
      .querySelectorAll(
        ".preview-card, .preview-panel-body, .panel-body-cell, .panel-head-cell, .panel-heading",
      )
      .forEach((node) => {
        if (hits.length >= limit) return;
        if (node.offsetWidth > 0 && node.offsetHeight > 0) return;
        hits.push({
          panelId: closestPanelId(node),
          label: nodeAuditLabel(node),
        });
      });
    return hits;
  }

  function detectClippedNodes(stage, limit = 8) {
    const hits = [];
    stage
      .querySelectorAll(
        ".preview-card, .preview-panel-body, .panel-body-cell, .panel-head-cell, .panel-heading",
      )
      .forEach((node) => {
        if (hits.length >= limit) return;
        const style = getComputedStyle(node);
        const clipsY = /hidden|clip/.test(style.overflowY || style.overflow || "");
        const clipsX = /hidden|clip/.test(style.overflowX || style.overflow || "");
        if (!clipsY && !clipsX) return;
        const nodeRect = node.getBoundingClientRect();
        let xOverflow = false;
        let yOverflow = false;
        node.querySelectorAll(":scope > *, :scope > * *").forEach((child) => {
          if (xOverflow && yOverflow) return;
          if (!(child instanceof Element)) return;
          const rect = child.getBoundingClientRect();
          if (rect.width < 1 || rect.height < 1) return;
          if (clipsX && (rect.left < nodeRect.left - 1 || rect.right > nodeRect.right + 1)) {
            xOverflow = true;
          }
          if (clipsY && (rect.top < nodeRect.top - 1 || rect.bottom > nodeRect.bottom + 1)) {
            yOverflow = true;
          }
        });
        if ((clipsY && yOverflow) || (clipsX && xOverflow)) {
          hits.push({
            panelId: closestPanelId(node),
            label: nodeAuditLabel(node),
          });
        }
      });
    return hits;
  }

  function detectHeadMetricSpacing(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".preview-card").forEach((panel) => {
      if (hits.length >= limit) return;
      const head = panel.querySelector(":scope > .panel-head-cell");
      const body = panel.querySelector(":scope > .panel-body-cell");
      if (!head || !body) return;
      const first = body.querySelector(
        ":scope > .preview-card, :scope > .component-card, :scope > .mei-text, :scope > [data-metric-role]",
      );
      if (!first) return;
      const gap = first.getBoundingClientRect().top - body.getBoundingClientRect().top;
      if (gap > 28) {
        hits.push({
          panelId: String(panel.getAttribute("data-mei-panel-id") || "").trim() || null,
          label: nodeAuditLabel(panel),
          gap,
        });
      }
    });
    return hits;
  }

  function detectBottomClipRisk(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".preview-card").forEach((panel) => {
      if (hits.length >= limit) return;
      const body = panel.querySelector(":scope > .panel-body-cell");
      if (!body) return;
      const panelStyle = getComputedStyle(panel);
      const bodyStyle = getComputedStyle(body);
      const panelClips = /hidden|clip/.test(panelStyle.overflowY || panelStyle.overflow || "");
      const bodyClips = /hidden|clip/.test(bodyStyle.overflowY || bodyStyle.overflow || "");
      if (!panelClips && !bodyClips) return;
      const bodyRect = body.getBoundingClientRect();
      let maxBottom = bodyRect.top;
      body
        .querySelectorAll(":scope > .preview-card, :scope > .component-card, :scope > *")
        .forEach((node) => {
          const rect = node.getBoundingClientRect();
          if (rect.height < 1) return;
          if (rect.bottom > maxBottom) maxBottom = rect.bottom;
        });
      if (maxBottom > bodyRect.bottom + 1) {
        hits.push({
          panelId: String(panel.getAttribute("data-mei-panel-id") || "").trim() || null,
          label: nodeAuditLabel(panel),
          overflowPx: maxBottom - bodyRect.bottom,
        });
      }
    });
    return hits;
  }

  function rectsOverlap(a, b) {
    return (
      a.left < b.right - 1 &&
      a.right > b.left + 1 &&
      a.top < b.bottom - 1 &&
      a.bottom > b.top + 1
    );
  }

  function rectOverlapX(a, b) {
    return Math.min(a.right, b.right) - Math.max(a.left, b.left);
  }

  function rectOverlapY(a, b) {
    return Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
  }

  function detectCardOverlap(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".panel-body-cell").forEach((body) => {
      if (hits.length >= limit) return;
      const groupPanelId = closestPanelId(body);
      const groupLabel = closestPanelLabel(body);
      const cards = Array.from(
        body.querySelectorAll(':scope > .preview-card[data-mei-panel-id]'),
      );
      if (cards.length < 2) return;
      for (let i = 0; i < cards.length; i += 1) {
        if (hits.length >= limit) break;
        const a = cards[i].getBoundingClientRect();
        if (a.width < 1 || a.height < 1) continue;
        for (let j = i + 1; j < cards.length; j += 1) {
          if (hits.length >= limit) break;
          const b = cards[j].getBoundingClientRect();
          if (b.width < 1 || b.height < 1) continue;
          if (rectsOverlap(a, b)) {
            hits.push({
              panelId: groupPanelId,
              label: groupLabel,
              pair: [nodeAuditLabel(cards[i]), nodeAuditLabel(cards[j])],
            });
          }
        }
      }
    });
    return hits;
  }

  function detectCardGapBudget(stage, limit = 6) {
    const hits = [];
    stage.querySelectorAll(".panel-body-cell").forEach((body) => {
      if (hits.length >= limit) return;
      const groupPanelId = closestPanelId(body);
      const groupLabel = closestPanelLabel(body);
      const cards = Array.from(
        body.querySelectorAll(':scope > .preview-card[data-mei-panel-id]'),
      );
      if (cards.length < 2) return;
      const isMetricGroup = cards.every((card) => metricRoleNodes(card).length > 0);
      const maxHorizontalGap = isMetricGroup ? 32 : 12;
      const maxVerticalGap = isMetricGroup ? 24 : 16;
      const maxPadding = isMetricGroup ? 64 : 24;
      const bodyRect = body.getBoundingClientRect();
      const horizontal = cards
        .map((node) => node.getBoundingClientRect())
        .filter((rect) => rect.width > 1 && rect.height > 1)
        .sort((a, b) => a.left - b.left);
      if (horizontal.length >= 2) {
        const pair = horizontal.find((rect, idx) => {
          if (idx === 0) return false;
          return rectOverlapY(horizontal[idx - 1], rect) > 8;
        });
        const pairIdx = pair ? horizontal.indexOf(pair) : -1;
        const prev = pairIdx > 0 ? horizontal[pairIdx - 1] : null;
        const gap = prev && pair ? pair.left - prev.right : null;
        if (gap != null && (gap < 4 || gap > maxHorizontalGap)) {
          hits.push({
            panelId: groupPanelId,
            label: groupLabel,
            axis: "horizontal",
            gap,
          });
        }
      }
      const vertical = cards
        .map((node) => node.getBoundingClientRect())
        .filter((rect) => rect.width > 1 && rect.height > 1)
        .sort((a, b) => a.top - b.top);
      if (vertical.length >= 2) {
        const pair = vertical.find((rect, idx) => {
          if (idx === 0) return false;
          return rectOverlapX(vertical[idx - 1], rect) > 8;
        });
        const pairIdx = pair ? vertical.indexOf(pair) : -1;
        const prev = pairIdx > 0 ? vertical[pairIdx - 1] : null;
        const gap = prev && pair ? pair.top - prev.bottom : null;
        if (gap != null && (gap < 4 || gap > maxVerticalGap)) {
          hits.push({
            panelId: groupPanelId,
            label: groupLabel,
            axis: "vertical",
            gap,
          });
        }
      }
      const leftPad = Math.max(0, horizontal[0]?.left - bodyRect.left || 0);
      const rightPad = Math.max(
        0,
        bodyRect.right - (horizontal[horizontal.length - 1]?.right || bodyRect.right),
      );
      const topPad = Math.max(0, vertical[0]?.top - bodyRect.top || 0);
      const bottomPad = Math.max(
        0,
        bodyRect.bottom - (vertical[vertical.length - 1]?.bottom || bodyRect.bottom),
      );
      const maxPad = Math.max(leftPad, rightPad, topPad, bottomPad);
      if (maxPad > maxPadding) {
        hits.push({
          panelId: groupPanelId,
          label: groupLabel,
          axis: "padding",
          gap: maxPad,
        });
      }
      const horizontalPadDelta = Math.abs(leftPad - rightPad);
      if (horizontalPadDelta > 12) {
        hits.push({
          panelId: groupPanelId,
          label: groupLabel,
          axis: "padding_symmetry",
          gap: horizontalPadDelta,
          leftPad,
          rightPad,
        });
