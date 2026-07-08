/**
 * Unified review projection depth consumer (dim / manifest overlay).
 */
(function initProjectionDepth(global) {
  "use strict";

  const REVIEW_PROJECTION_MAX_DEPTH = {
    plane: 0,
    plane_region: 1,
    plane_region_section: 2,
    plane_region_section_slot: 3,
    static_full: 99,
    live_full: 99,
    static: 99,
    live: 99,
  };

  function normalizeReviewProjection(value) {
    return String(value || "")
      .trim()
      .toLowerCase()
      .replace(/-/g, "_");
  }

  function readReviewProjectionFromUrl() {
    try {
      return String(
        new URL(global.location.href).searchParams.get("review_projection") || "",
      ).trim();
    } catch (_) {
      return "";
    }
  }

  function elementReviewDepth(el) {
    if (global.MeiStructureAnchor?.elementReviewDepth) {
      return global.MeiStructureAnchor.elementReviewDepth(el);
    }
    if (!(el instanceof HTMLElement)) return 99;
    const role = String(el.getAttribute("data-mei-ui-role") || "")
      .trim()
      .toLowerCase();
    const roleDepth = { plane: 0, region: 1, section: 2, slot: 3, content: 4 };
    if (role && Object.prototype.hasOwnProperty.call(roleDepth, role)) {
      return roleDepth[role];
    }
    if (el.hasAttribute("data-mei-panel-id")) return 1;
    if (el.hasAttribute("data-preview-scope")) return 2;
    if (el.hasAttribute("data-mei-use-key") || el.hasAttribute("data-build-node")) return 3;
    return 99;
  }

  function applyReviewProjectionChrome(root, options) {
    if (!(root instanceof HTMLElement)) return;
    const opts = options || {};
    if (opts.verifyOnly === true) {
      return verifyComposeProjection(root, opts);
    }
    const surface = String(
      global.document?.body?.getAttribute("data-surface") ||
        global.document?.body?.getAttribute("data-mei-view") ||
        "",
    )
      .trim()
      .toLowerCase();
    if (surface === "layout") {
      root.removeAttribute("data-review-projection-active");
      root
        .querySelectorAll(".build-review-projection-dim, .mei-review-projection-dim")
        .forEach((el) => {
          el.classList.remove("build-review-projection-dim", "mei-review-projection-dim");
          if (el instanceof HTMLElement) el.style.removeProperty("pointer-events");
        });
      return;
    }
    const projection = normalizeReviewProjection(
      opts.reviewProjection ||
        root.getAttribute("data-review-projection") ||
        readReviewProjectionFromUrl(),
    );
    const maxDepth = REVIEW_PROJECTION_MAX_DEPTH[projection];
    root.querySelectorAll(".mei-compose-hidden, [hidden].mei-compose-hidden").forEach((el) => {
      if (!(el instanceof HTMLElement)) return;
      el.classList.remove("mei-compose-hidden");
      el.removeAttribute("hidden");
    });
    root.querySelectorAll(".build-review-projection-dim, .mei-review-projection-dim").forEach((el) => {
      el.classList.remove("build-review-projection-dim", "mei-review-projection-dim");
      if (el instanceof HTMLElement) el.style.removeProperty("pointer-events");
    });
    if (maxDepth == null || maxDepth >= 99) {
      root.removeAttribute("data-review-projection-active");
      return;
    }
    root.setAttribute("data-review-projection-active", projection || "static_full");
    root
      .querySelectorAll(
        "[data-mei-ui-role], [data-mei-panel-id], [data-preview-scope], [data-mei-use-key], [data-build-node], .preview-projection-skeleton",
      )
      .forEach((el) => {
        if (!(el instanceof HTMLElement)) return;
        if (el.classList.contains("preview-projection-skeleton")) return;
        const depth = elementReviewDepth(el);
        if (depth > maxDepth) {
          el.classList.add("build-review-projection-dim", "mei-review-projection-dim");
          el.style.pointerEvents = "none";
        }
      });
  }

  function applyGridBudgetToNode(node, entry) {
    if (!(node instanceof HTMLElement) || !entry || typeof entry !== "object") return;
    const cols = entry.grid_template_columns ?? entry.gridTemplateColumns;
    const rows = entry.grid_template_rows ?? entry.gridTemplateRows;
    const areas = entry.grid_template_areas ?? entry.gridTemplateAreas;
    const gap = entry.gap ?? entry.content_gap ?? entry.contentGap;
    const slotAreas = entry.slot_areas ?? entry.slotAreas;
    const hasGrid =
      cols ||
      rows ||
      areas ||
      (Array.isArray(entry.content_rows ?? entry.contentRows) &&
        (entry.content_rows ?? entry.contentRows).length > 0) ||
      (Array.isArray(entry.section_rows ?? entry.sectionRows) &&
        (entry.section_rows ?? entry.sectionRows).length > 0);
    if (!hasGrid && !(Array.isArray(slotAreas) && slotAreas.length > 0)) return;
    node.style.display = "grid";
    node.style.minHeight = "0";
    node.style.minWidth = "0";
    if (cols) {
      node.style.gridTemplateColumns = String(cols);
      node.dataset.manifestGridColumns = String(cols);
    }
    if (rows) {
      node.style.gridTemplateRows = String(rows);
      node.dataset.manifestGridRows = String(rows);
    }
    if (areas) {
      node.style.gridTemplateAreas = String(areas);
      node.dataset.manifestGridAreas = String(areas);
    }
    if (gap != null && gap !== "") {
      const gapText = String(gap).endsWith("px") ? String(gap) : `${gap}px`;
      node.style.gap = gapText;
      node.dataset.manifestGap = String(gap);
    }
    if (Array.isArray(slotAreas) && slotAreas.length > 0) {
      const scope = String(node.getAttribute("data-preview-scope") || "").trim();
      slotAreas.forEach((areaName) => {
        const area = String(areaName || "").trim();
        if (!area) return;
        const child =
          (scope
            ? node.querySelector(`[data-preview-scope="${CSS.escape(`${scope}/${area}`)}"]`)
            : null) ||
          [...node.children].find((el) => {
            if (!(el instanceof HTMLElement)) return false;
            const childScope = String(el.getAttribute("data-preview-scope") || "");
            return childScope === `${scope}/${area}` || childScope.endsWith(`/${area}`);
          });
        if (child instanceof HTMLElement) {
          child.style.gridArea = area;
          child.dataset.manifestGridArea = area;
        }
      });
      node.dataset.manifestSlotAreas = slotAreas.join(",");
    }
  }

  function applyLayoutBudgetManifest(doc) {
    const root = doc || document;
    const manifest = globalThis.__mei?.layout_budget_manifest;
    if (!manifest?.entries || typeof manifest.entries !== "object") return;
    Object.entries(manifest.entries).forEach(([scope, entry]) => {
      if (!entry || typeof entry !== "object") return;
      const node = root.querySelector(`[data-preview-scope="${CSS.escape(scope)}"]`);
      if (!(node instanceof HTMLElement)) return;
      const slotHeight = entry.slot_height_px ?? entry.slotHeightPx;
      if (slotHeight != null) {
        node.style.setProperty("--mei-slot-height", `${slotHeight}px`);
        node.dataset.manifestSlotHeight = String(slotHeight);
      }
      const paddingProfile = entry.padding_profile ?? entry.paddingProfile;
      if (paddingProfile) {
        node.dataset.manifestPaddingProfile = String(paddingProfile);
      }
      const contentRows = entry.content_rows ?? entry.contentRows;
      if (Array.isArray(contentRows) && contentRows.length > 0) {
        node.style.display = "grid";
        const total = contentRows.reduce((sum, row) => sum + Number(row), 0);
        if (total > 0) {
          node.style.gridTemplateRows = contentRows
            .map((row) => `${(Number(row) / total) * 100}fr`)
            .join(" ");
        } else {
          node.style.gridTemplateRows = contentRows.map((row) => `${row}px`).join(" ");
        }
        node.dataset.manifestContentRows = contentRows.join(",");
      }
      const contentGap = entry.content_gap ?? entry.contentGap;
      if (contentGap != null && contentGap !== "") {
        node.style.rowGap = `${contentGap}px`;
        node.dataset.manifestContentGap = String(contentGap);
      }
      const sectionRows = entry.section_rows ?? entry.sectionRows;
      const manifestGridRows = entry.grid_template_rows ?? entry.gridTemplateRows;
      if (
        Array.isArray(sectionRows) &&
        sectionRows.length > 0 &&
        !manifestGridRows
      ) {
        node.style.display = "grid";
        node.style.gridTemplateRows = sectionRows.map((row) => String(row)).join(" ");
        node.dataset.manifestSectionRows = sectionRows.join(",");
      }
      applyGridBudgetToNode(node, entry);
    });
  }

  function verifyComposeProjection(root, options) {
    if (!(root instanceof HTMLElement)) return { ok: true, skipped: true };
    const opts = options || {};
    const expected = normalizeReviewProjection(
      opts.reviewProjection || readReviewProjectionFromUrl() || "live_full",
    );
    const composed = normalizeReviewProjection(
      root.getAttribute("data-compose-projection") || "",
    );
    const active = normalizeReviewProjection(
      root.getAttribute("data-review-projection-active") || "",
    );
    if (!composed) {
      return { ok: false, reason: "missing_data_compose_projection" };
    }
    const projectionOk = composed === expected;
    const chromeOk = !active || active === composed;
    return {
      ok: projectionOk && chromeOk,
      expected,
      composed,
      active: active || null,
    };
  }

  function applyProjectionDepth(root, options) {
    const opts = options || {};
    if (root instanceof HTMLElement && root.getAttribute("data-compose-projection")) {
      const verified = verifyComposeProjection(root, opts);
      if (verified.ok) {
        applyLayoutBudgetManifest(root?.ownerDocument || document);
        return verified;
      }
    }
    applyReviewProjectionChrome(root, opts);
    applyLayoutBudgetManifest(root?.ownerDocument || document);
    return verifyComposeProjection(root, opts);
  }

  global.MeiProjectionDepth = {
    applyReviewProjectionChrome,
    applyLayoutBudgetManifest,
    applyGridBudgetToNode,
    applyProjectionDepth,
    verifyComposeProjection,
    normalizeReviewProjection,
    readReviewProjectionFromUrl,
    elementReviewDepth,
    REVIEW_PROJECTION_MAX_DEPTH,
  };
})(window);
