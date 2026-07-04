/**
 * Unified review projection depth consumer (dim / manifest overlay).
 */
(function initProjectionDepth(global) {
  "use strict";

  const REVIEW_PROJECTION_MAX_DEPTH = {
    plane: 0,
    plane_region: 1,
    plane_region_section: 2,
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
    const roleDepth = { plane: 0, region: 1, section: 2, slot: 3, content: 3 };
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
    const projection = normalizeReviewProjection(
      opts.reviewProjection ||
        root.getAttribute("data-review-projection") ||
        readReviewProjectionFromUrl(),
    );
    const maxDepth = REVIEW_PROJECTION_MAX_DEPTH[projection];
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

  function applyLayoutBudgetManifest(doc) {
    const root = doc || document;
    const manifest = global.__mei?.layout_budget_manifest;
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
        node.style.gridTemplateRows = contentRows.map((row) => `${row}px`).join(" ");
        node.dataset.manifestContentRows = contentRows.join(",");
      }
      const contentGap = entry.content_gap ?? entry.contentGap;
      if (contentGap != null && contentGap !== "") {
        node.style.rowGap = `${contentGap}px`;
        node.dataset.manifestContentGap = String(contentGap);
      }
    });
  }

  function applyProjectionDepth(root, options) {
    applyReviewProjectionChrome(root, options);
    applyLayoutBudgetManifest(root?.ownerDocument || document);
  }

  global.MeiProjectionDepth = {
    applyReviewProjectionChrome,
    applyLayoutBudgetManifest,
    applyProjectionDepth,
    normalizeReviewProjection,
    readReviewProjectionFromUrl,
    elementReviewDepth,
    REVIEW_PROJECTION_MAX_DEPTH,
  };
})(window);
