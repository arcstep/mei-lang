/**
 * 驾驶舱表格组件（cockpit.data-table → mei-cockpit-data-table）。
 */
import { escapeAttr, escapeHtml, parseProps, rowsOf } from "./shared.js";
import { COCKPIT_FONT, COCKPIT_TYPE, parseThemeFontPx, cockpitCssVars, themeShadow } from "./tokens.js";
import { color } from "../mei/theme-style.js";
import {
  deferUntilDisplayed,
  shouldReactToPreviewUpdated,
  fetchDatasetRows,
  formatRuntimeQueryUserMessage,
  getQueryState,
  mergeFilters,
  queryStateIdOf,
  resolveDatasetQueryCapability,
  resolveRuntimeDataRef,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  subscribeQueryState,
} from "../dataset/runtime-query.js";
import { applyTableQueryResult } from "../dataset/table-runtime/query.js";
import {
  bindCellPreviewClick,
  bindRelativeTimeTicker,
  cellPopoverStyleBlock,
  cellTableChromeStyleBlock,
  cellValue,
  closeCellPopover,
  openCellPopover,
  renderFormattedCellHtml,
  resolveCellPopoverVariant,
  scheduleOverflowPreviewSync,
  stopRelativeTimeTicker,
} from "../dataset/table-runtime/cells.js";
import {
  activeTableColumnState,
  activeTableFilters,
  activeTableSort,
  resolveColumnStateConfig,
  resolveSortConfig,
  sameColumnState,
  sameFilters,
  sameSort,
} from "../dataset/table-runtime/state.js";
import {
  buildColumnTemplate,
  buildExplicitColumnTemplate,
  formatCellDisplay,
  inferColumnWidthsFromSample,
  inlineStyleForColumn,
  isTagLikeColumnKey,
  isWarningLevelBlocksColumn,
  resolveColumnDescriptors,
  resolveToneToken,
} from "../dataset/table-runtime/format.js";
import {
  renderWarningLevelBlocksHtml,
  warningLevelBlocksCss,
} from "../mei/warning-level.js";
import { formatTableRowCountLabel } from "../dataset/table-runtime/footer.js";
import {
  buildTableRowDrilldownDetail,
  emitTableRowDrilldown,
  emitObjectFieldOpen,
  resolveObjectFieldLinks,
  resolveObjectFieldTargets,
  tableDrilldownMeta,
  tableRowSelectionMode,
  emitTableRowSelect,
} from "./drilldown-meta.js";

function resolveTableSpec(props) {
  const keys = Array.isArray(props.columns)
    ? props.columns.map(String)
    : Array.isArray(props.headers)
      ? props.headers.map(String)
      : [];
  const headers = Array.isArray(props.headers)
    ? props.headers.map(String)
    : keys.slice();
  return { keys, headers };
}

const LAYOUT_PRESETS = {
  alerts: "2.8fr 1.5fr 2.2fr 1.6fr 1fr",
  warnings: "0.55fr 1.25fr 1.15fr 0.65fr 0.85fr 0.75fr",
  drilldown_warnings: "0.95fr 1.15fr 1.55fr 1.1fr 0.95fr",
  drilldown_issues: "1.05fr 1.2fr 1.6fr 1.1fr 0.9fr 0.9fr",
  drilldown_models: "1fr 1fr 1.5fr 0.95fr 0.8fr",
  drilldown_matters: "1.2fr 1.4fr 1.1fr 1.45fr 1.2fr",
  cases: "1.35fr 0.65fr",
  default: "",
};

function cellToneClass(layoutPreset, colKey, raw) {
  const text = String(raw ?? "").trim();
  if (layoutPreset !== "warnings") {
    return "";
  }
  if (colKey === "level" || colKey === "预警等级" || colKey === "风险等级" || colKey === "级别") {
    if (text.includes("蓝")) return "tone-blue";
    if (text.includes("黄")) return "tone-yellow";
    if (text.includes("橙")) return "tone-orange";
    if (text.includes("红")) return "tone-red";
  }
  if (colKey === "status" || colKey === "当前状态") {
    if (text.includes("办")) return "tone-orange";
  }
  return "";
}

function isTagField(colKey) {
  return isTagLikeColumnKey(colKey);
}

function resolveTagToneClass(colKey, raw) {
  const field = String(colKey || "").trim();
  const text = String(raw ?? "").trim();
  // 类别/类型标签：只保留胶囊外形，不按 value 上色（避免与红/黄/蓝业务色板混淆）。
  if (/类型|类别/.test(field)) return "";
  if (!text) return "tone-slate";
  if (text.includes("红")) return "tone-red";
  if (text.includes("橙")) return "tone-orange";
  if (text.includes("黄")) return "tone-yellow";
  if (text.includes("蓝")) return "tone-blue";
  if (/(在办|待办|处理中|核查中|整改中)/.test(text)) return "tone-orange";
  if (/(已办|已结|完成|通过|正常|是)/.test(text)) return "tone-green";
  if (/(否|未|无|待完善|\/)/.test(text)) return "tone-slate";
  if (/个案/.test(text)) return "tone-violet";
  if (/趋势/.test(text)) return "tone-cyan";
  return "tone-slate";
}

function formatCellValue(raw, descriptor, layoutPreset) {
  if (
    layoutPreset === "warnings" &&
    (descriptor?.key === "count" || descriptor?.key === "预警件数") &&
    (raw == null || raw === "")
  ) {
    return "1";
  }
  if (
    layoutPreset === "warnings" &&
    (descriptor?.key === "status" || descriptor?.key === "当前状态") &&
    (raw == null || raw === "")
  ) {
    return "待办";
  }
  return formatCellDisplay(raw, descriptor);
}

function rowsFromMetricShape(metric) {
  if (!metric || typeof metric !== "object") return [];
  if (metric.shape === "dataframe" && Array.isArray(metric.value)) {
    return metric.value;
  }
  return rowsOf(metric);
}

function resolvePageSize(props) {
  const raw = props?.pageSize ?? props?.page_size ?? 0;
  const size = Number(raw);
  return Number.isFinite(size) && size > 0 ? Math.floor(size) : 0;
}

function paginationEnabled(props) {
  const pageSize = resolvePageSize(props);
  if (pageSize <= 0) return false;
  if (props?.pagination === false || props?.pagination === "false") return false;
  return true;
}

function resolvePaginationMode(props) {
  const raw = String(props?.paginationMode ?? props?.pagination_mode ?? "").trim().toLowerCase();
  const wantsClient = raw === "client" || raw === "local";
  const wantsServer = raw === "server" || raw === "remote";
  // 轮播需在本地分页上切页；显式 client 时不得因 embedded+metric 降为 server。
  if (carouselEnabled(props) && wantsClient) {
    return "client";
  }
  if (wantsServer) return "server";
  if (wantsClient) {
    if (props?.embedded === true && resolveRuntimeMetricRef(props)) {
      return "server";
    }
    return "client";
  }
  if (props?.embedded === true && resolveRuntimeMetricRef(props)) {
    return "server";
  }
  return "server";
}

function carouselEnabled(props) {
  return props?.carousel === true || props?.carousel === "true";
}

function resolveCarouselIntervalMs(props) {
  const raw = Number(props?.carouselIntervalMs ?? props?.carousel_interval_ms ?? 5000);
  if (!Number.isFinite(raw)) return 5000;
  return Math.max(2000, Math.floor(raw));
}

function carouselShowsPager(props) {
  return props?.carouselShowPager === true || props?.carousel_show_pager === "true";
}

function carouselShowsHint(props) {
  if (props?.carouselHint === false || props?.carousel_hint === "false") return false;
  if (!carouselEnabled(props)) return false;
  if (props?.carouselHint === true || props?.carousel_hint === "true") return true;
  return !carouselShowsPager(props);
}

const CAROUSEL_RING_RADIUS = 8;
const CAROUSEL_RING_C = 2 * Math.PI * CAROUSEL_RING_RADIUS;

function renderCarouselHintHtml(page, totalPages, intervalMs, epoch) {
  const dots = Array.from({ length: totalPages }, (_, index) => {
    const pageNo = index + 1;
    const active = pageNo === page;
    return `<button type="button" class="carousel-dot${active ? " is-active" : ""}" data-carousel-page="${pageNo}" aria-label="切换到第 ${pageNo} 页" aria-current="${active ? "true" : "false"}"></button>`;
  }).join("");
  return `
    <div class="carousel-hint" role="navigation" aria-label="轮播第 ${page} 页，共 ${totalPages} 页">
      <div class="carousel-dots">${dots}</div>
      <span class="carousel-page-label">
        <span class="carousel-page-current" data-epoch="${epoch}">${page}</span><span class="carousel-page-sep">/</span><span class="carousel-page-total">${totalPages}</span>
      </span>
      <div class="carousel-timer" style="--carousel-ms:${intervalMs}ms;--carousel-c:${CAROUSEL_RING_C}" data-epoch="${epoch}" title="自动切页倒计时">
        <svg class="carousel-ring" viewBox="0 0 20 20" width="18" height="18" aria-hidden="true">
          <circle class="carousel-ring-track" cx="10" cy="10" r="${CAROUSEL_RING_RADIUS}" />
          <circle class="carousel-ring-progress" cx="10" cy="10" r="${CAROUSEL_RING_RADIUS}" />
        </svg>
      </div>
    </div>`;
}

function shouldRenderPager(props, paging) {
  if (!paging) return false;
  if (carouselEnabled(props) && !carouselShowsPager(props)) return false;
  return true;
}

function countTemplateTracks(template) {
  const raw = String(template || "").trim();
  if (!raw) return 0;
  const repeat = raw.match(/^repeat\(\s*(\d+)\s*,/i);
  if (repeat) {
    return Number(repeat[1]) || 0;
  }
  let depth = 0;
  let tracks = 0;
  let hasToken = false;
  for (let i = 0; i < raw.length; i += 1) {
    const ch = raw[i];
    if (ch === "(") {
      depth += 1;
      hasToken = true;
    } else if (ch === ")") {
      depth = Math.max(0, depth - 1);
    } else if (/\s/.test(ch) && depth === 0) {
      if (hasToken) {
        tracks += 1;
        hasToken = false;
      }
    } else if (depth === 0) {
      hasToken = true;
    }
  }
  if (hasToken) {
    tracks += 1;
  }
  return tracks;
}

function resolveColumnMinWidth(props) {
  const raw = Number(props?.columnMinWidth ?? props?.column_min_width);
  if (Number.isFinite(raw) && raw > 0) {
    return Math.floor(raw);
  }
  if (tableScrollXEnabled(props)) {
    return 88;
  }
  return props?.embedded === true || props?.embedded === "true" ? 170 : 120;
}

function parseCssPx(raw, fallback) {
  return parseThemeFontPx(raw, fallback);
}

function resolveSampleMeasureFonts(host, { embedded = false, compactEmbedded = false } = {}) {
  const style =
    typeof window !== "undefined" && host instanceof Element ? window.getComputedStyle(host) : null;
  const fontFamily =
    String(
      style?.getPropertyValue("--mei-font-family-ui") ||
        style?.fontFamily ||
        COCKPIT_FONT.uiFamily
    ).trim() || COCKPIT_FONT.uiFamily;
  const tableHeadPx = parseCssPx(
    style?.getPropertyValue("--mei-table-head-font-size"),
    parseCssPx(style?.getPropertyValue("--mei-font-2"), 18),
  );
  const tableBodyPx = parseCssPx(
    style?.getPropertyValue("--mei-table-body-font-size"),
    parseCssPx(style?.getPropertyValue("--mei-font-2"), 18),
  );
  const bodyFontPx = compactEmbedded
    ? parseCssPx(style?.getPropertyValue("--mei-font-2"), 18)
    : embedded
      ? tableBodyPx
      : tableBodyPx;
  return {
    bodyFont: `400 ${bodyFontPx}px ${fontFamily}`,
    labelFont: `600 ${tableHeadPx}px ${fontFamily}`,
    charPx: Math.max(7, bodyFontPx * 0.9),
  };
}

function tableScrollXEnabled(props) {
  return props?.tableScrollX === true || props?.table_scroll_x === true;
}

function sumDescriptorColumnWidths(descriptors, columnMinWidth) {
  return (Array.isArray(descriptors) ? descriptors : []).reduce((acc, descriptor) => {
    const fixed = Number(descriptor?.layoutFixedWidth);
    if (Number.isFinite(fixed) && fixed > 0) return acc + fixed;
    const min = Number(descriptor?.layoutMinWidth ?? descriptor?.minWidth);
    if (Number.isFinite(min) && min > 0) return acc + min;
    return acc + columnMinWidth;
  }, 0);
}

function templateUsesExplicitPixelTracks(template) {
  const raw = String(template || "").trim();
  if (!raw || /\b1fr\b/.test(raw) || /^repeat\(\s*\d+/i.test(raw)) {
    return false;
  }
  return /\d+px/.test(raw);
}

function resolveTableGridSizing(props, descriptors, colTemplateValue, columnMinWidth) {
  const embedded = props?.embedded === true || props?.embedded === "true";
  const scrollX = tableScrollXEnabled(props);
  const template = String(colTemplateValue || "").trim();
  const explicitPx = templateUsesExplicitPixelTracks(template);
  if (scrollX || explicitPx) {
    const sum = sumDescriptorColumnWidths(descriptors, columnMinWidth);
    const px = Math.max(sum, (descriptors?.length || 0) * columnMinWidth);
    if (px > 0) {
      return { width: `max(100%, ${px}px)`, minWidth: `${px}px` };
    }
  }
  const flexible =
    embedded ||
    !template ||
    /\b1fr\b/.test(template) ||
    /^repeat\(\s*\d+/i.test(template);
  if (flexible) {
    return { width: "100%", minWidth: "0" };
  }
  const px = Math.max(
    sumDescriptorColumnWidths(descriptors, columnMinWidth),
    (descriptors?.length || 0) * columnMinWidth,
  );
  return { width: `max(100%, ${px}px)`, minWidth: `${px}px` };
}

function resolveColumnTemplate(props, keys, descriptors) {
  const explicit = String(props?.columnTemplate ?? props?.column_template ?? "").trim();
  const count = Array.isArray(keys) ? keys.length : 0;
  const minWidth = resolveColumnMinWidth(props);
  if (explicit) {
    const tracks = countTemplateTracks(explicit);
    if (tracks === 0 || tracks === count) return explicit;
  }
  const sampledTemplate = buildExplicitColumnTemplate(descriptors);
  if (sampledTemplate) {
    return sampledTemplate;
  }
  // Never use per-row max-content: .thead and each .tr are independent grids, so
  // max-content makes header tracks follow labels while body tracks follow cells
  // (visible as "表头与数据错位", often with … sitting under the next header).
  if (tableScrollXEnabled(props) && count > 0) {
    const scrollTemplate = buildColumnTemplate(descriptors, minWidth, { shrinkFit: false });
    if (scrollTemplate) return scrollTemplate;
    return `repeat(${count}, minmax(${minWidth}px, 1fr))`;
  }
  const preset = LAYOUT_PRESETS[String(props?.layoutPreset ?? "").trim()] || "";
  if (preset) {
    const tracks = countTemplateTracks(preset);
    if (tracks === 0 || tracks === count) return preset;
  }
  const embedded = props?.embedded === true || props?.embedded === "true";
  const descriptorTemplate = buildColumnTemplate(descriptors, minWidth, {
    shrinkFit: embedded && !tableScrollXEnabled(props),
  });
  if (descriptorTemplate) {
    return descriptorTemplate;
  }
  if (count > 0) {
    return `repeat(${count}, minmax(${minWidth}px, 1fr))`;
  }
  return "";
}

function renderCellContentHtml(descriptor, raw, rowIndex, textMap, props, displayOverride, toneClass = "", objectLinkTargets = []) {
  const format = descriptor?.format || {};
  const formatType = String(format?.type || descriptor?.type || "").trim().toLowerCase();
  if (formatType === "action") {
    const label = String(format.label || descriptor.label || descriptor.key || "操作").trim();
    const disabled = format.disabled !== false && format.interactive !== true;
    const disabledAttr = disabled ? ' disabled aria-disabled="true"' : "";
    return `<button type="button" class="cell-action-link${disabled ? " is-disabled" : ""}" data-action-column="${escapeAttr(
      descriptor.key,
    )}"${disabledAttr}>${escapeHtml(label)}</button>`;
  }
  if (isWarningLevelBlocksColumn(descriptor)) {
    const previewKey = `${rowIndex}::${descriptor.key}`;
    const previewAttrs = ` data-cell-preview-key="${escapeAttr(previewKey)}" data-r="${rowIndex}" data-c="${escapeAttr(
      descriptor.key
    )}"`;
    return `<span class="cell-inner is-warning-level"${previewAttrs}>${renderWarningLevelBlocksHtml(raw, props?.__host, descriptor)}</span>`;
  }
  const cell = renderFormattedCellHtml(raw, descriptor, rowIndex, textMap, props, displayOverride);
  const previewKey = `${rowIndex}::${descriptor.key}`;
  const previewAttrs = ` data-cell-preview-key="${escapeAttr(previewKey)}" data-r="${rowIndex}" data-c="${escapeAttr(
    descriptor.key
  )}"`;
  if (Array.isArray(objectLinkTargets) && objectLinkTargets.length > 0) {
    const tip = objectLinkTargets.map((target) => target.label || target.objectType).join(" / ");
    // 外层保留 data-cell-preview-key，供溢出「…」全文预览；内层按钮只负责打开智能对象。
    return `<span class="cell-object-link-host${cell.tipClass}"${previewAttrs} data-object-field="${escapeAttr(
      descriptor.key,
    )}"><button type="button" class="cell-object-link" title="${escapeAttr(
      tip || "打开智能对象",
    )}" data-object-field="${escapeAttr(descriptor.key)}" data-r="${rowIndex}" data-c="${escapeAttr(
      descriptor.key,
    )}">${cell.html}</button></span>`;
  }
  if (descriptor?.tag || isTagField(descriptor?.key)) {
    const tone = toneClass || resolveTagToneClass(descriptor?.key, displayOverride);
    return `<span class="cell-tag ${tone}${cell.tipClass}"${cell.titleAttr}${previewAttrs}>${cell.html}</span>`;
  }
  return `<span class="cell-inner${cell.tipClass}"${cell.titleAttr}${previewAttrs}>${cell.html}</span>`;
}

function eventComposedPath(event) {
  return typeof event?.composedPath === "function" ? event.composedPath() : [];
}

function findTableRowInEventPath(event, rowClass) {
  return (
    eventComposedPath(event).find(
      (node) =>
        node instanceof HTMLElement &&
        node.classList.contains("tr") &&
        node.classList.contains(rowClass),
    ) || null
  );
}

function tableRowActivationMode(props) {
  const raw = String(
    props?.row_activation_mode ?? props?.rowActivationMode ?? "",
  )
    .trim()
    .toLowerCase();
  return raw === "dblclick" ? "dblclick" : "click";
}

function rowStatusIsActive(row) {
  if (!row || typeof row !== "object") return false;
  const status = String(row.status ?? row.event_status ?? "").trim().toLowerCase();
  if (status === "active" || status === "live" || status === "进行中") return true;
  if (row.active === true || row.is_active === true || row.isActive === true) return true;
  return false;
}

function resolveDefaultSelectedRowIndex(rows, props) {
  const list = Array.isArray(rows) ? rows : [];
  if (list.length === 0) return -1;
  const preferredId = String(
    props?.default_selected_row_id ??
      props?.defaultSelectedRowId ??
      props?.selected_row_id ??
      props?.selectedRowId ??
      "",
  ).trim();
  if (preferredId) {
    const byId = list.findIndex((row) => String(row?.id ?? "").trim() === preferredId);
    if (byId >= 0) return byId;
  }
  const activeIdx = list.findIndex((row) => rowStatusIsActive(row));
  if (activeIdx >= 0) return activeIdx;
  return 0;
}

function eventPathIntersectsSelector(event, selector) {
  return eventComposedPath(event).some(
    (node) => node instanceof HTMLElement && Boolean(node.closest?.(selector)),
  );
}

/**
 * 驾驶舱表格（cockpit.data-table → mei-cockpit-data-table）。
 * 支持静态 rows 或 dataframe 指标运行时查询（props.dataset 带 __mei_runtime_ref）。
 */
export class MeiCockpitDataTable extends HTMLElement {
  connectedCallback() {
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    this._deferUntilVisibleCleanup = deferUntilDisplayed(this, () => {
      this._deferUntilVisibleCleanup = null;
      this.bootstrap();
    });
  }

  disconnectedCallback() {
    this.stopCarousel();
    if (this._carouselHoverTarget) {
      this._carouselHoverTarget.removeEventListener("mouseenter", this._onCarouselPause);
      this._carouselHoverTarget.removeEventListener("mouseleave", this._onCarouselResume);
      this._carouselHoverTarget = null;
      this._carouselHoverBound = false;
    }
    if (this._fetchAbort) {
      this._fetchAbort.abort();
      this._fetchAbort = null;
    }
    this.closeCellPopover();
    stopRelativeTimeTicker(this);
    if (typeof this._relativeTimeCleanup === "function") {
      this._relativeTimeCleanup();
      this._relativeTimeCleanup = null;
    }
    if (typeof this._deferUntilVisibleCleanup === "function") {
      this._deferUntilVisibleCleanup();
      this._deferUntilVisibleCleanup = null;
    }
    if (typeof this._onPreviewUpdated === "function") {
      window.removeEventListener("meilang:preview-updated", this._onPreviewUpdated);
      this._onPreviewUpdated = null;
    }
    if (typeof this._unsubscribeQueryState === "function") {
      this._unsubscribeQueryState();
      this._unsubscribeQueryState = null;
    }
    if (typeof this._cellPreviewCleanup === "function") {
      this._cellPreviewCleanup();
      this._cellPreviewCleanup = null;
    }
  }

  bootstrap() {
    this._props = parseProps(this);
    this._fetchAbort = new AbortController();
    this._pageSize = resolvePageSize(this._props);
    this._paging = paginationEnabled(this._props);
    this._pagingMode = resolvePaginationMode(this._props);
    this._allRows = [];
    this._lastFetchSignature = "";
    this._carouselEpoch = 0;
    this._carouselPageTurn = false;
    this._queryStateId = queryStateIdOf(this._props);
    if (tableRowSelectionMode(this._props) === "single") {
      if (!Number.isFinite(this._selectedRowIndex) || this._selectedRowIndex < 0) {
        const initialRows = rowsFromMetricShape(this._props.dataset);
        const preferred = resolveDefaultSelectedRowIndex(initialRows, this._props);
        this._selectedRowIndex = preferred >= 0 ? preferred : 0;
      }
    }
    this._sharedFilters = getQueryState(this._queryStateId).filters || {};
    this._sharedSearch = String(getQueryState(this._queryStateId).search || "").trim();
    this._state = {
      loading: false,
      error: "",
      rows: rowsFromMetricShape(this._props.dataset),
      page: 1,
      total: 0,
      hasMore: false,
      sort: resolveSortConfig(this._props),
      columnState: resolveColumnStateConfig(this._props),
    };
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this._onPreviewUpdated = (event) => {
      if (!shouldReactToPreviewUpdated(event, this)) {
        return;
      }
      this._props = parseProps(this);
      this._pageSize = resolvePageSize(this._props);
      this._paging = paginationEnabled(this._props);
      this._pagingMode = resolvePaginationMode(this._props);
      this._lastFetchSignature = "";
      this._state.page = 1;
      this._state.sort = resolveSortConfig(this._props);
      this._state.columnState = resolveColumnStateConfig(this._props);
      this.refresh();
    };
    window.addEventListener("meilang:preview-updated", this._onPreviewUpdated);
    if (this._queryStateId) {
      this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (nextState) => {
        const nextFilters = mergeFilters(nextState?.filters);
        const nextSearch = String(nextState?.search || "").trim();
        const nextSort = Array.isArray(nextState?.sort) ? nextState.sort : [];
        const nextColumnState = nextState?.column_state || nextState?.columnState || null;
        const filtersChanged = !sameFilters(nextFilters, this._sharedFilters);
        const searchChanged = nextSearch !== this._sharedSearch;
        const sortChanged = !sameSort(
          nextSort,
          activeTableSort(this._props, this._queryStateId, this._state.sort)
        );
        const columnStateChanged = !sameColumnState(
          nextColumnState,
          activeTableColumnState(this._props, this._queryStateId, this._state.columnState)
        );
        if (!filtersChanged && !searchChanged && !sortChanged && !columnStateChanged) {
          return;
        }
        this._sharedFilters = nextFilters;
        this._sharedSearch = nextSearch;
        this._state.page = 1;
        if (filtersChanged || searchChanged || sortChanged) {
          this.refresh();
        } else {
          this.render();
        }
      });
    }
    if (!this._pagerBound) {
      this._pagerBound = true;
      this.addEventListener("click", (event) => this.onPagerClick(event));
    }
    if (!this._rowDrilldownBound) {
      this._rowDrilldownBound = true;
      this.addEventListener("click", (event) => this.onRowDrilldownClick(event));
    }
    if (!this._rowSelectBound) {
      this._rowSelectBound = true;
      this.addEventListener("click", (event) => this.onRowSelectClick(event));
    }
    if (!this._rowActivateBound) {
      this._rowActivateBound = true;
      this.addEventListener("dblclick", (event) => this.onRowActivateDblclick(event));
    }
    this.bindCarouselHover();
    this.render();
    this.refresh();
  }

  /**
   * Invoked by preview-materializer `applyPropsToHost` after eval rebinds
   * `data-props`. Thin-shell F5 often binds eval in the same turn as mount,
   * before `deferUntilDisplayed` → `bootstrap()`; the old early-return left
   * tables permanently empty (no dataset query).
   */
  _bind() {
    if (!this._state) {
      if (typeof this._deferUntilVisibleCleanup === "function") {
        this._deferUntilVisibleCleanup();
        this._deferUntilVisibleCleanup = null;
      }
      this.bootstrap();
      return;
    }
    this._props = parseProps(this);
    this._pageSize = resolvePageSize(this._props);
    this._paging = paginationEnabled(this._props);
    this._pagingMode = resolvePaginationMode(this._props);
    this._lastFetchSignature = "";
    this._state.page = 1;
    this._state.sort = resolveSortConfig(this._props);
    this._state.columnState = resolveColumnStateConfig(this._props);
    this.refresh();
  }

  bindCarouselHover() {
    const pauseOnHover =
      this._props?.carouselPauseOnHover !== false &&
      this._props?.carousel_pause_on_hover !== "false";
    if (!carouselEnabled(this._props) || !pauseOnHover) return;
    if (!this._onCarouselPause) {
      this._onCarouselPause = () => {
        this.stopCarousel();
        this.shadowRoot?.querySelector(".table-wrap")?.classList.add("carousel-paused");
      };
      this._onCarouselResume = () => {
        this.shadowRoot?.querySelector(".table-wrap")?.classList.remove("carousel-paused");
        this.startCarousel();
      };
    }
    const wrap = this.shadowRoot?.querySelector(".table-wrap");
    if (!wrap || wrap === this._carouselHoverTarget) return;
    if (this._carouselHoverTarget) {
      this._carouselHoverTarget.removeEventListener("mouseenter", this._onCarouselPause);
      this._carouselHoverTarget.removeEventListener("mouseleave", this._onCarouselResume);
    }
    wrap.addEventListener("mouseenter", this._onCarouselPause);
    wrap.addEventListener("mouseleave", this._onCarouselResume);
    this._carouselHoverTarget = wrap;
    this._carouselHoverBound = true;
  }

  startCarousel() {
    this.stopCarousel();
    const p = this._props || {};
    if (!carouselEnabled(p) || !this._paging || this._pagingMode !== "client") return;
    const totalPages =
      this._state.total > 0
        ? Math.max(1, Math.ceil(this._state.total / (this._pageSize || 1)))
        : 1;
    if (totalPages <= 1) return;
    const interval = resolveCarouselIntervalMs(p);
    this._carouselTimer = setInterval(() => {
      if (this._state.loading) return;
      if (this._state.hasMore) {
        this._state.page += 1;
      } else {
        this._state.page = 1;
      }
      this._carouselEpoch += 1;
      this._carouselPageTurn = true;
      this.applyPagedRows(this._allRows);
      this.render();
      this._carouselPageTurn = false;
    }, interval);
  }

  stopCarousel() {
    if (this._carouselTimer) {
      clearInterval(this._carouselTimer);
      this._carouselTimer = null;
    }
  }

  onPagerClick(event) {
    if (this._state.loading) return;
    const path = event.composedPath();
    const carouselPageRaw = path.find(
      (node) => node instanceof HTMLElement && node.dataset?.carouselPage,
    )?.dataset?.carouselPage;
    if (carouselPageRaw != null && this._paging && this._pagingMode === "client") {
      const nextPage = Number(carouselPageRaw);
      const totalPages =
        this._state.total > 0
          ? Math.max(1, Math.ceil(this._state.total / (this._pageSize || 1)))
          : 1;
      if (
        Number.isFinite(nextPage) &&
        nextPage >= 1 &&
        nextPage <= totalPages &&
        nextPage !== this._state.page
      ) {
        this._state.page = nextPage;
        this._carouselEpoch += 1;
        this._carouselPageTurn = true;
        this.applyPagedRows(this._allRows);
        this.render();
        this._carouselPageTurn = false;
        this.startCarousel();
      }
      return;
    }
    const action = path.find(
      (node) => node instanceof HTMLElement && node.dataset?.pagerAction,
    )?.dataset?.pagerAction;
    if (!action || !this._paging) return;
    if (action === "prev" && this._state.page > 1) {
      this._state.page -= 1;
      if (this._pagingMode === "client") {
        this._carouselEpoch += 1;
        this.applyPagedRows(this._allRows);
        this.render();
        this.startCarousel();
      } else {
        this.refresh();
      }
    }
    if (action === "next" && this._state.hasMore) {
      this._state.page += 1;
      if (this._pagingMode === "client") {
        this._carouselEpoch += 1;
        this.applyPagedRows(this._allRows);
        this.render();
        this.startCarousel();
      } else {
        this.refresh();
      }
    }
  }

  applyPagedRows(allRows) {
    const rows = Array.isArray(allRows) ? allRows : [];
    this._state.total = rows.length;
    const start = (this._state.page - 1) * this._pageSize;
    this._state.rows = this._paging ? rows.slice(start, start + this._pageSize) : rows;
    this._state.hasMore = this._paging && start + this._pageSize < this._state.total;
  }

  tableFetchSignature(queryFilters, querySort, queryColumnState, wantsSummary) {
    const metricRef = resolveRuntimeMetricRef(this._props);
    const dataRef = resolveRuntimeDataRef(this._props);
    return JSON.stringify({
      scene: this._props?._mei?.active_scene_id || "",
      target: this._props?._mei?.active_target_file || "",
      compileEpoch: this._props?._mei?.compile_epoch || "",
      metric: metricRef,
      data: dataRef,
      page: this._state.page,
      paging: this._paging,
      pagingMode: this._pagingMode,
      pageSize: this._pageSize,
      filters: queryFilters,
      sort: querySort,
      columnState: queryColumnState,
      summary: wantsSummary,
      queryStateId: this._queryStateId,
    });
  }

  async refresh() {
    const metricRef = resolveRuntimeMetricRef(this._props);
    const dataRef = resolveRuntimeDataRef(this._props);
    if (!metricRef && !dataRef) {
      this._allRows = rowsFromMetricShape(this._props.dataset);
      if (this._paging) {
        this.applyPagedRows(this._allRows);
      } else {
        this._state.rows = this._allRows;
      }
      this.render();
      this.startCarousel();
      this.maybeAutoSelectPreviewRow();
      return;
    }
    const datasetQueryCapability = resolveDatasetQueryCapability(this._props);
    if (!datasetQueryCapability.enabled) {
      this._state.error =
        datasetQueryCapability.reason ||
        "shared runtime dataset query capability is unavailable";
      this._state.loading = false;
      this._allRows = rowsFromMetricShape(this._props.dataset);
      if (this._paging) {
        this.applyPagedRows(this._allRows);
      } else {
        this._state.rows = this._allRows;
      }
      this.render();
      this.startCarousel();
      this.maybeAutoSelectPreviewRow();
      return;
    }
    const queryFilters = activeTableFilters(this._props, this._queryStateId);
    const querySort = activeTableSort(this._props, this._queryStateId, this._state.sort);
    const queryColumnState = activeTableColumnState(this._props, this._queryStateId, this._state.columnState);
    const wantsSummary = false;
    const fetchSignature = this.tableFetchSignature(
      queryFilters,
      querySort,
      queryColumnState,
      wantsSummary
    );
    if (
      fetchSignature === this._lastFetchSignature &&
      !this._state.loading &&
      !this._state.error &&
      Array.isArray(this._allRows) &&
      this._allRows.length > 0
    ) {
      if (this._paging) {
        this.applyPagedRows(this._allRows);
      } else {
        this._state.rows = this._allRows;
      }
      this.render();
      this.startCarousel();
      this.maybeAutoSelectPreviewRow();
      return;
    }
    this._lastFetchSignature = fetchSignature;
    this._state.loading = true;
    this._state.error = "";
    this.render();
    try {
      let result = null;
      if (this._paging && this._pagingMode === "client") {
        result = await fetchDatasetRows(this._props, {
          full: true,
          page: 1,
          pageSize: 0,
          filters: queryFilters,
          sort: querySort,
          columnState: queryColumnState,
          summary: wantsSummary,
          signal: this._fetchAbort?.signal,
          meta: runtimeCallerMeta(this, "mei-cockpit-data-table"),
        });
        this._state = applyTableQueryResult(this._state, result, { pagingMode: "client" });
        this._allRows = Array.isArray(this._state.rows) ? this._state.rows : [];
        this.applyPagedRows(this._allRows);
      } else {
        result = await fetchDatasetRows(this._props, {
          full: !this._paging,
          page: this._paging ? this._state.page : 1,
          pageSize: this._paging ? this._pageSize : 0,
          filters: queryFilters,
          sort: querySort,
          columnState: queryColumnState,
          summary: wantsSummary,
          signal: this._fetchAbort?.signal,
          meta: runtimeCallerMeta(this, "mei-cockpit-data-table"),
        });
        this._state = applyTableQueryResult(this._state, result, { pagingMode: this._pagingMode });
        this._allRows = Array.isArray(this._state.rows) ? this._state.rows : [];
      }
      if (Array.isArray(result?.column_meta) && result.column_meta.length > 0 && !this._props.headers) {
        this._columnMeta = result.column_meta;
      }
    } catch (error) {
      if (error?.name === "AbortError") {
        return;
      }
      this._state.error = formatRuntimeQueryUserMessage(
        error?.message || error || "runtime query failed"
      );
      this._allRows = rowsFromMetricShape(this._props.dataset);
      if (this._paging) {
        this.applyPagedRows(this._allRows);
      } else {
        this._state.rows = this._allRows;
      }
    } finally {
      this._state.loading = false;
      if (
        carouselEnabled(this._props) &&
        this._paging &&
        this._pagingMode === "client" &&
        this._allRows.length > (this._pageSize || 1)
      ) {
        this._carouselEpoch = (this._carouselEpoch || 0) + 1;
      }
      this.render();
      this.startCarousel();
      this.maybeAutoSelectPreviewRow();
    }
  }

  closeCellPopover() {
    closeCellPopover(this, this.shadowRoot);
  }

  openCellPopover(fullText, anchor, options = {}) {
    const layout = options.layout || "anchored";
    openCellPopover(this, this.shadowRoot, fullText, anchor, {
      topOffset: 8,
      focusOnOpen: layout === "modal",
      variant: options.variant || resolveCellPopoverVariant(this._props || {}),
      layout,
      title: options.title || "详细内容",
      subtitle: options.subtitle || "",
    });
  }

  onRowDrilldownClick(event) {
    if (event?.target?.closest?.(".cell-expand-btn, .cell-more, .cell-preview-trigger")) {
      return;
    }
    const fieldLink = eventComposedPath(event).find(
      (node) => node instanceof HTMLElement && node.classList.contains("cell-object-link"),
    );
    if (fieldLink instanceof HTMLElement) {
      event.preventDefault();
      event.stopPropagation();
      // Prevent later host listeners (row select → render) from wiping the chooser.
      event.stopImmediatePropagation();
      this.onObjectFieldLinkClick(event, fieldLink);
      return;
    }
    const rowEl = findTableRowInEventPath(event, "drilldown-row");
    if (!(rowEl instanceof HTMLElement)) {
      return;
    }
    if (eventPathIntersectsSelector(event, ".pager, .carousel-timer, .cell-preview-trigger, .cell-expand-btn, .cell-more, .cell-object-link, .object-field-chooser")) {
      return;
    }
    const index = Number(rowEl.dataset.rowIndex);
    const rows = Array.isArray(this._state?.rows) ? this._state.rows : [];
    const row = Number.isFinite(index) && index >= 0 ? rows[index] : null;
    if (!row) {
      return;
    }
    const meta = tableDrilldownMeta(this._props || {});
    const detail = buildTableRowDrilldownDetail(meta, row, this._props || {});
    if (!detail) {
      return;
    }
    event.preventDefault();
    emitTableRowDrilldown(this, detail);
  }

  onObjectFieldLinkClick(event, linkEl) {
    const columnKey = String(linkEl?.dataset?.objectField || linkEl?.dataset?.c || "").trim();
    const rowIndex = Number(linkEl?.dataset?.r);
    const rows = Array.isArray(this._state?.rows) ? this._state.rows : [];
    const row = Number.isFinite(rowIndex) && rowIndex >= 0 ? rows[rowIndex] : null;
    if (!row || !columnKey) return;
    const targets = resolveObjectFieldTargets(this._props || {}, row, columnKey);
    if (!targets.length) return;
    if (targets.length === 1) {
      emitObjectFieldOpen(this, targets[0], row, this._props || {});
      return;
    }
    this.openObjectFieldChooser(linkEl, targets, row);
  }

  closeObjectFieldChooser() {
    const existing = this.shadowRoot?.querySelector?.(".object-field-chooser");
    if (existing) existing.remove();
    if (typeof this._objectFieldChooserCleanup === "function") {
      this._objectFieldChooserCleanup();
      this._objectFieldChooserCleanup = null;
    }
  }

  openObjectFieldChooser(anchor, targets, row) {
    this.closeObjectFieldChooser();
    if (!this.shadowRoot || !anchor) return;
    const menu = document.createElement("div");
    menu.className = "object-field-chooser";
    menu.setAttribute("role", "menu");
    menu.innerHTML = [
      `<div class="object-field-chooser-title">选择智能对象</div>`,
      ...targets.map(
        (target, index) =>
          `<button type="button" class="object-field-chooser-item" data-target-index="${index}" role="menuitem">${escapeHtml(
            target.label || `${target.objectType} · ${target.objectKey}`,
          )}</button>`,
      ),
    ].join("");
    // Fixed to viewport so overflow:hidden on .table-wrap cannot clip the menu.
    this.shadowRoot.appendChild(menu);
    const rect = anchor.getBoundingClientRect();
    menu.style.position = "fixed";
    menu.style.left = `${Math.max(8, Math.min(rect.left, window.innerWidth - 240))}px`;
    menu.style.top = `${Math.min(rect.bottom + 4, window.innerHeight - 8)}px`;

    const onPick = (event) => {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      const button = event.target?.closest?.(".object-field-chooser-item");
      if (!(button instanceof HTMLElement)) return;
      const index = Number(button.dataset.targetIndex);
      const target = targets[index];
      this.closeObjectFieldChooser();
      if (target) emitObjectFieldOpen(this, target, row, this._props || {});
    };
    menu.addEventListener("click", onPick);
    const onDoc = (event) => {
      const path = typeof event.composedPath === "function" ? event.composedPath() : [];
      if (path.includes(menu) || path.includes(anchor)) return;
      this.closeObjectFieldChooser();
    };
    window.setTimeout(() => {
      document.addEventListener("mousedown", onDoc, true);
      this._objectFieldChooserCleanup = () => {
        document.removeEventListener("mousedown", onDoc, true);
        menu.removeEventListener("click", onPick);
      };
    }, 0);
  }

  emitRowActivation(row, rowIndex, reason = "select") {
    if (!row) return;
    emitTableRowSelect(this, {
      row,
      rowIndex,
      query_state_id: String(this._queryStateId || "").trim(),
      activation: reason,
    });
    const eventId = String(row.id ?? row.event_id ?? "").trim();
    if (eventId && typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent("mei:thunder-event-activate", {
          bubbles: true,
          composed: true,
          detail: {
            eventId,
            row,
            rowIndex,
            reason,
            source: "cockpit.data-table",
          },
        }),
      );
    }
  }

  maybeAutoSelectPreviewRow() {
    const selectionMode = tableRowSelectionMode(this._props || {});
    if (selectionMode !== "single") {
      return;
    }
    if (this._state?.loading) {
      return;
    }
    const pageRows = Array.isArray(this._state?.rows) ? this._state.rows : [];
    const allRows = Array.isArray(this._allRows) && this._allRows.length > 0 ? this._allRows : pageRows;
    const autoSelectDefault =
      this._props?.autoSelectDefaultRow === true ||
      this._props?.auto_select_default_row === true ||
      this._props?.autoSelectFirstRow === true ||
      this._props?.auto_select_first_row === true ||
      this._props?.autoSelectSingleRow === true ||
      this._props?.auto_select_single_row === true;
    if (!autoSelectDefault) {
      return;
    }
    if (allRows.length === 0) {
      return;
    }
    const preferred = resolveDefaultSelectedRowIndex(allRows, this._props);
    if (preferred < 0) {
      return;
    }
    const row = allRows[preferred];
    const signature = JSON.stringify({
      queryStateId: this._queryStateId || "",
      id: row?.id ?? preferred,
    });
    if (this._autoSelectSignature === signature) {
      return;
    }
    this._autoSelectSignature = signature;
    // 分页时选中索引相对当前页；默认事件通常在首页。
    const pageIndex = pageRows.findIndex((item) => item === row || String(item?.id ?? "") === String(row?.id ?? ""));
    this._selectedRowIndex = pageIndex >= 0 ? pageIndex : preferred;
    this.render();
    this.emitRowActivation(row, preferred, "auto");
  }

  onRowSelectClick(event) {
    const selectionMode = tableRowSelectionMode(this._props || {});
    if (selectionMode !== "single") {
      return;
    }
    if (tableRowActivationMode(this._props || {}) === "dblclick") {
      // 双击激活模式：单击不切换选中，也不广播。
      return;
    }
    const rowEl = findTableRowInEventPath(event, "selectable-row");
    if (!(rowEl instanceof HTMLElement)) {
      return;
    }
    if (eventPathIntersectsSelector(event, ".pager, .carousel-timer, .cell-preview-trigger, .cell-object-link, .object-field-chooser")) {
      return;
    }
    const index = Number(rowEl.dataset.rowIndex);
    const rows = Array.isArray(this._state?.rows) ? this._state.rows : [];
    const row = Number.isFinite(index) && index >= 0 ? rows[index] : null;
    if (!row) {
      return;
    }
    this._selectedRowIndex = index;
    this.render();
    this.emitRowActivation(row, index, "click");
  }

  onRowActivateDblclick(event) {
    const selectionMode = tableRowSelectionMode(this._props || {});
    if (selectionMode !== "single") {
      return;
    }
    if (tableRowActivationMode(this._props || {}) !== "dblclick") {
      return;
    }
    const rowEl = findTableRowInEventPath(event, "selectable-row");
    if (!(rowEl instanceof HTMLElement)) {
      return;
    }
    if (eventPathIntersectsSelector(event, ".pager, .carousel-timer, .cell-preview-trigger, .cell-object-link, .object-field-chooser")) {
      return;
    }
    const index = Number(rowEl.dataset.rowIndex);
    const rows = Array.isArray(this._state?.rows) ? this._state.rows : [];
    const row = Number.isFinite(index) && index >= 0 ? rows[index] : null;
    if (!row) {
      return;
    }
    event.preventDefault();
    this._selectedRowIndex = index;
    this.render();
    this.emitRowActivation(row, index, "dblclick");
  }

  bindCellPreviewEvents() {
    if (typeof this._cellPreviewCleanup === "function") {
      this._cellPreviewCleanup();
    }
    this._cellPreviewCleanup = bindCellPreviewClick(
      this.shadowRoot,
      this._cellTextMap,
      (full, anchor, opts) => this.openCellPopover(full, anchor, opts),
      { getVariant: () => resolveCellPopoverVariant(this._props || {}) }
    );
    if (typeof this._relativeTimeCleanup === "function") {
      this._relativeTimeCleanup();
    }
    this._relativeTimeCleanup = bindRelativeTimeTicker(
      this,
      this.shadowRoot,
      this._visibleDescriptors || [],
      this._props || {}
    );
    scheduleOverflowPreviewSync(this, this.shadowRoot, this._cellTextMap, this._props || {});
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(() => this._warnIfColumnTracksDiverge());
    } else {
      this._warnIfColumnTracksDiverge();
    }
  }

  _warnIfColumnTracksDiverge() {
    const root = this.shadowRoot;
    if (!root || typeof getComputedStyle !== "function") return;
    const thead = root.querySelector(".thead");
    const tr = root.querySelector(".tr");
    if (!thead || !tr) return;
    const headGrid = getComputedStyle(thead).gridTemplateColumns;
    const rowGrid = getComputedStyle(tr).gridTemplateColumns;
    if (!headGrid || !rowGrid || headGrid === rowGrid) {
      this._columnTrackWarnKey = "";
      return;
    }
    const thCount = thead.children.length;
    const tdCount = tr.children.length;
    const warnKey = `${thCount}|${tdCount}|${headGrid}|${rowGrid}`;
    if (this._columnTrackWarnKey === warnKey) return;
    this._columnTrackWarnKey = warnKey;
    const msg =
      thCount !== tdCount
        ? `[mei-cockpit-data-table] 表头列数(${thCount})与表体列数(${tdCount})不一致`
        : `[mei-cockpit-data-table] 表头与表体列宽轨不一致（thead/tbody 独立 grid + 非共享轨）。head=${headGrid}; row=${rowGrid}`;
    console.warn(msg);
  }

  render() {
    this.closeCellPopover();
    this.closeObjectFieldChooser();
    const p = this._props || {};
    let { keys, headers } = resolveTableSpec(p);
    const rows = this._state?.rows || [];
    const embedded = p.embedded === true || p.embedded === "true";
    const scrollX = tableScrollXEnabled(p);
    const compactEmbedded =
      embedded && (p.compactEmbedded === true || p.compact_embedded === true);
    const rowMinHeight = compactEmbedded ? 34 : embedded ? 42 : 32;
    const headMinHeight = compactEmbedded ? 34 : embedded ? 42 : 32;
    const cellPadX = compactEmbedded ? 10 : embedded ? 16 : 14;
    const columnMinWidth = resolveColumnMinWidth(p);
    const popoverVariant = resolveCellPopoverVariant(p);
    if (keys.length === 0 && rows.length > 0) {
      keys = Object.keys(rows[0]);
      headers = keys.slice();
    }
    const descriptors = resolveColumnDescriptors({
      columns: keys,
      headers,
      columnMeta: this._columnMeta || this._state?.columnMeta || [],
      columnState: activeTableColumnState(this._props, this._queryStateId, this._state.columnState),
      columnFormats: p.columnFormats ?? p.column_formats,
      columnRules: p.columnRules ?? p.column_rules,
    });
    const sampleRows = (this._allRows?.length ? this._allRows : rows).slice(
      0,
      Math.max(1, Number(p.columnWidthSampleSize ?? p.column_width_sample_size) || 100),
    );
    const shouldSampleWidths =
      (p.fitColumnsFromSample === true ||
        p.fit_columns_from_sample === true ||
        p.autoFitColumns === true ||
        p.auto_fit_columns === true) &&
      sampleRows.length > 0;
    const measureFonts = resolveSampleMeasureFonts(this, { embedded, compactEmbedded });
    const layoutDescriptors = shouldSampleWidths
      ? inferColumnWidthsFromSample(sampleRows, descriptors, {
          sampleLimit: Math.max(1, Number(p.columnWidthSampleSize ?? p.column_width_sample_size) || 100),
          charPx: measureFonts.charPx,
          font: measureFonts.bodyFont,
          labelFont: measureFonts.labelFont,
          cellPaddingPx: embedded ? 30 : 24,
          minVisibleChars: Number(p.cellOverflowMinChars ?? p.cell_overflow_min_chars) || 10,
        })
      : descriptors;
    // cases 末列（处理结果 ID）：强制左对齐，覆盖 number meta / defaultAlignForType("right")
    // 带来的 inline text-align:right（否则 .align-accent 被内联样式压过）。
    if (String(p.layoutPreset ?? "") === "cases" && layoutDescriptors.length > 0) {
      const last = layoutDescriptors[layoutDescriptors.length - 1];
      last.align = "left";
      last.headerAlign = "left";
    }
    this._visibleDescriptors = layoutDescriptors;
    const visibleKeys = layoutDescriptors.map((descriptor) => descriptor.key);
    const colTemplateValue = resolveColumnTemplate(p, visibleKeys, layoutDescriptors);
    const maxHeight = embedded
      ? "100%"
      : Number(p.maxHeight) > 0
        ? `${Number(p.maxHeight)}px`
        : "173px";
    const colTemplate = colTemplateValue ? `grid-template-columns: ${colTemplateValue};` : "";
    const gridSizing = resolveTableGridSizing(p, layoutDescriptors, colTemplateValue, columnMinWidth);
    // cases 末列：强调色 + 左对齐，避免长短 ID 右齐后左缘参差。
    const lastColAccent = p.layoutPreset === "cases";

    this._cellTextMap = new Map();
    const headCells = layoutDescriptors
      .map(
        (descriptor) =>
          `<span class="th-cell" style="${escapeAttr(inlineStyleForColumn(descriptor, "header"))}" title="${escapeAttr(
            descriptor.label
          )}">${escapeHtml(descriptor.label)}</span>`
      )
      .join("");
    const layoutKey = String(p.layoutPreset ?? "");
    const fieldLinks = resolveObjectFieldLinks(p);
    const hasObjectFieldLinks = Object.keys(fieldLinks).some(
      (key) => Array.isArray(fieldLinks[key]) && fieldLinks[key].length > 0,
    );
    const locatorHint =
      p?.object_locator ||
      p?.objectLocator ||
      p?.capabilities?.object_locator ||
      p?.capabilities?.objectLocator ||
      p?.row_drilldown?.object_locator ||
      p?.rowDrilldown?.object_locator ||
      p?.row_drilldown?.objectLocator ||
      p?.rowDrilldown?.objectLocator;
    const hasObjectLocator = Boolean(
      locatorHint &&
        typeof locatorHint === "object" &&
        String(locatorHint.object_type || locatorHint.objectType || "").trim(),
    );
    // Field-level object links own navigation; never treat the whole row as a link.
    const drilldownEnabled =
      Boolean(tableDrilldownMeta(p)) && !hasObjectFieldLinks && !hasObjectLocator;
    const selectionMode = tableRowSelectionMode(p);
    const selectableRows = selectionMode === "single";
    const body = rows
      .map((row, ri) => {
        const cells = layoutDescriptors
          .map((descriptor, i) => {
            const raw = cellValue(row, descriptor.key, i);
            const formatted = String(formatCellValue(raw, descriptor, layoutKey) ?? "");
            const sharedTone = resolveToneToken(raw, descriptor);
            const tone = sharedTone ? `tone-${sharedTone}` : cellToneClass(layoutKey, descriptor.key, formatted);
            const tagTone = sharedTone ? `tone-${sharedTone}` : resolveTagToneClass(descriptor.key, formatted);
            const objectLinkTargets = Array.isArray(fieldLinks[descriptor.key])
              ? resolveObjectFieldTargets(p, row, descriptor.key)
              : [];
            const cls = [
              "td-cell",
              lastColAccent && i === layoutDescriptors.length - 1 ? "align-accent" : "",
              objectLinkTargets.length ? "" : tone,
              objectLinkTargets.length ? "has-object-link" : "",
            ]
              .filter(Boolean)
              .join(" ");
            return `<span class="${cls}" style="${escapeAttr(inlineStyleForColumn(descriptor, "cell"))}">${renderCellContentHtml(
              descriptor,
              raw,
              ri,
              this._cellTextMap,
              { ...p, __host: this },
              formatted,
              objectLinkTargets.length ? "" : tagTone,
              objectLinkTargets,
            )}</span>`;
          })
          .join("");
        const activeEvent = rowStatusIsActive(row);
        const rowClass = [
          "tr",
          ri % 2 === 1 ? "zebra" : "",
          drilldownEnabled ? "drilldown-row" : "",
          selectableRows ? "selectable-row" : "",
          selectableRows && ri === this._selectedRowIndex ? "is-selected" : "",
          activeEvent ? "is-active-event" : "",
        ]
          .filter(Boolean)
          .join(" ");
        return `<div class="${rowClass}" data-row-index="${ri}" role="${drilldownEnabled || selectableRows ? "button" : "row"}" tabindex="${drilldownEnabled || selectableRows ? "0" : "-1"}">${cells}</div>`;
      })
      .join("");
    const emptyHint =
      !this._state?.loading && rows.length === 0
        ? `<div class="empty">${escapeHtml(this._state?.error || "暂无数据")}</div>`
        : "";
    const paging = this._paging;
    const showPager = shouldRenderPager(p, paging);
    const page = Math.max(1, Number(this._state?.page) || 1);
    const rowCount = Math.max(0, Number(this._state?.total) || rows.length || 0);
    const pageSize = this._pageSize || 1;
    const carouselPaging = paging && carouselEnabled(p);
    const totalPages =
      rowCount > 0 && (showPager || carouselPaging)
        ? Math.max(1, Math.ceil(rowCount / pageSize))
        : 1;
    const showCarouselHint =
      carouselShowsHint(p) && totalPages > 1 && !this._state?.loading;
    const rowCountLabel = escapeHtml(formatTableRowCountLabel(rowCount));
    const carouselEpoch = this._carouselEpoch || 0;
    const footerHtml = `
        <div class="table-footer${showCarouselHint ? " has-carousel-hint" : ""}">
          <span class="row-total">${rowCountLabel}</span>
          ${
            showPager
              ? `<div class="pager">
          <button type="button" class="pager-btn" data-pager-action="prev" ${this._state?.loading || page <= 1 ? "disabled" : ""}>上一页</button>
          <span class="pager-info">${page} / ${totalPages}</span>
          <button type="button" class="pager-btn" data-pager-action="next" ${this._state?.loading || !this._state?.hasMore ? "disabled" : ""}>下一页</button>
        </div>`
              : ""
          }
          ${
            showCarouselHint
              ? renderCarouselHintHtml(
                  page,
                  totalPages,
                  resolveCarouselIntervalMs(p),
                  carouselEpoch
                )
              : ""
          }
        </div>`;
    const tbodyClass = [
      this._carouselPageTurn ? "page-turn" : "",
    ]
      .filter(Boolean)
      .join(" ");

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          position: relative;
          z-index: 1;
          width: 100%;
          min-width: 0;
          ${embedded ? "height:100%;min-height:0;" : ""}
          ${cockpitCssVars()}
        }
        .table-wrap {
          ${embedded ? "height:100%;max-height:none;" : `max-height:${maxHeight};`}
          overflow: hidden;
          border-radius: 0;
          font-family: ${COCKPIT_FONT.uiFamily};
          display: flex;
          flex-direction: column;
          min-height: 0;
        }
        .table-scroll {
          flex: 1 1 auto;
          min-height: 0;
          min-width: 0;
          overflow: auto;
          overflow-x: auto;
          overflow-y: auto;
          overscroll-behavior: contain;
        }
        .tbody {
          min-height: 0;
          overflow: visible;
          transition: opacity 220ms ease;
        }
        .table-wrap.carousel-active .tbody {
          opacity: 1;
        }
        .tbody.page-turn {
          animation: carousel-body-turn 340ms cubic-bezier(0.22, 1, 0.36, 1);
        }
        @keyframes carousel-body-turn {
          0% { opacity: 0.45; transform: translateY(8px); }
          100% { opacity: 1; transform: translateY(0); }
        }
        .table-canvas {
          width: ${gridSizing.width};
          min-width: ${gridSizing.minWidth};
        }
        .thead {
          display: grid;
          ${colTemplate}
          width: 100%;
          min-height: ${headMinHeight}px;
          align-items: center;
          padding: 0;
          column-gap: 0;
          background: ${color("table_head_bg")};
          border-bottom: 1px solid ${color("table_head_border")};
          position: sticky;
          top: 0;
          z-index: 1;
        }
        .th-cell {
          display: flex;
          align-items: center;
          box-sizing: border-box;
          padding: 0 ${cellPadX}px;
          font-size: ${compactEmbedded ? "var(--cockpit-font-label)" : COCKPIT_TYPE.tableHead};
          color: ${color("table_btn_fg")};
          font-weight: 600;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .tr {
          display: grid;
          ${colTemplate}
          width: 100%;
          min-height: ${rowMinHeight}px;
          align-items: center;
          padding: 0;
          column-gap: 0;
          border-bottom: 1px solid ${color("table_row_border")};
          transition: background 140ms ease, box-shadow 140ms ease, transform 140ms ease;
        }
        .tr.zebra { background: ${color("table_row_zebra")}; }
        .tr.drilldown-row,
        .tr.selectable-row { cursor: pointer; }
        .tr.is-selected {
          background: ${color("table_row_hover")};
          box-shadow: ${themeShadow("table_row_hover", "inset 0 0 0 1px rgba(56, 189, 248, 0.55)")};
        }
        /* 进行中：事件名/时间红字；级别 tag 仍走 tone-*，不被 inherit 冲掉 */
        .tr.is-active-event .td-cell {
          color: ${color("tone_red")};
          font-weight: 600;
        }
        .tr.is-active-event .td-cell .cell-inner {
          color: inherit;
          font-weight: 600;
        }
        .tr.is-active-event .td-cell .cell-tag {
          font-weight: 600;
        }
        /* Only interactive rows show hover affordance (not whole-row when field links own nav). */
        .tr.drilldown-row:hover,
        .tr.selectable-row:hover {
          background: ${color("table_row_selected")};
          box-shadow: ${themeShadow("table_row_selected", "inset 0 0 0 1px rgba(125, 211, 252, 0.2)")};
          transform: translateY(-1px);
        }
        .tr:last-child { border-bottom: none; }
        .td-cell {
          display: flex;
          align-items: center;
          box-sizing: border-box;
          padding: 0 ${cellPadX}px;
          min-width: 0;
          font-size: ${compactEmbedded ? "var(--cockpit-font-label)" : COCKPIT_TYPE.tableHead};
          color: ${color("text_body")};
          line-height: 1.35;
          overflow: hidden;
          transition: color 120ms ease;
        }
        .tr.drilldown-row:hover .td-cell,
        .tr.selectable-row:hover .td-cell {
          color: ${color("text_primary")};
        }
        .align-right { text-align: right; color: ${color("text_accent")}; }
        .align-accent { text-align: left; justify-content: flex-start; color: ${color("text_accent")}; }
        .tone-blue { color: ${color("tone_blue")}; }
        .tone-yellow { color: ${color("tone_yellow")}; }
        .tone-red { color: ${color("tone_red")}; }
        .tone-orange { color: ${color("tone_orange")}; }
        .tone-green { color: ${color("tone_green")}; }
        .tone-slate { color: ${color("tone_slate")}; }
        .tone-cyan { color: ${color("tone_cyan")}; }
        .tone-violet { color: ${color("tone_violet")}; }
        .cell-tag {
          display: inline-flex;
          align-items: center;
          max-width: 100%;
          min-width: 0;
          padding: 2px 10px;
          border-radius: 999px;
          border: 1px solid ${color("chart_5")};
          background: transparent;
          color: inherit;
          box-shadow: none;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          vertical-align: middle;
        }
        /* 类别/类型标签保持正文色；其它 tag 若带 tone-* 仅改文字色，边线仍用图表主色 */
        .cell-tag.tone-blue,
        .cell-tag.tone-yellow,
        .cell-tag.tone-red,
        .cell-tag.tone-orange,
        .cell-tag.tone-green,
        .cell-tag.tone-slate,
        .cell-tag.tone-cyan,
        .cell-tag.tone-violet {
          border-color: ${color("chart_5")};
        }
        .cell-action-link {
          display: inline-flex;
          align-items: center;
          padding: 0;
          border: 0;
          background: transparent;
          color: ${color("text_accent")};
          font: inherit;
          line-height: 1.35;
          cursor: pointer;
          text-decoration: underline;
          text-underline-offset: 2px;
        }
        .cell-action-link.is-disabled,
        .cell-action-link:disabled {
          opacity: 0.72;
          cursor: default;
          text-decoration: none;
        }
        .cell-object-link-host {
          display: inline-flex;
          align-items: center;
          gap: 4px;
          max-width: 100%;
          min-width: 0;
        }
        .cell-object-link {
          display: inline;
          max-width: 100%;
          padding: 0;
          border: 0;
          background: transparent;
          color: ${color("text_accent")};
          font: inherit;
          line-height: 1.35;
          cursor: pointer;
          text-align: inherit;
          text-decoration: underline;
          text-underline-offset: 2px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .td-cell.has-object-link {
          color: inherit;
        }
        .cell-object-link:hover,
        .tr:hover .cell-object-link,
        .tr.drilldown-row:hover .cell-object-link,
        .tr.selectable-row:hover .cell-object-link {
          color: ${color("text_accent")};
          filter: brightness(1.12);
        }
        .object-field-chooser {
          position: fixed;
          z-index: 12000;
          min-width: 220px;
          max-width: min(360px, 90vw);
          padding: 8px;
          border-radius: 8px;
          border: 1px solid rgba(125, 211, 252, 0.35);
          background: rgba(15, 23, 42, 0.96);
          box-shadow: 0 10px 28px rgba(2, 6, 23, 0.45);
          display: grid;
          gap: 4px;
        }
        .object-field-chooser-title {
          font-size: 12px;
          color: rgba(226, 232, 240, 0.78);
          padding: 2px 6px 6px;
        }
        .object-field-chooser-item {
          appearance: none;
          border: 0;
          border-radius: 6px;
          background: transparent;
          color: #e2e8f0;
          text-align: left;
          padding: 8px 10px;
          font: inherit;
          cursor: pointer;
        }
        .object-field-chooser-item:hover,
        .object-field-chooser-item:focus-visible {
          background: rgba(56, 189, 248, 0.16);
          outline: none;
        }
        .empty {
          padding: 24px 10px;
          text-align: center;
          color: ${color("text_muted")};
          font-size: ${COCKPIT_TYPE.tableHead};
        }
        .table-footer {
          flex: 0 0 auto;
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 12px;
          min-height: ${embedded ? "44px" : "36px"};
          padding: ${embedded ? "8px 14px 6px" : "6px 12px 4px"};
          border-top: 1px solid ${color("table_footer_border")};
          background: ${color("table_footer_bg")};
        }
        .row-total {
          flex: 0 0 auto;
          font-size: ${COCKPIT_TYPE.tableBody};
          color: ${color("text_muted")};
          white-space: nowrap;
        }
        .table-footer .pager {
          flex: 1 1 auto;
          display: flex;
          align-items: center;
          justify-content: flex-end;
          gap: 12px;
          min-height: 0;
          padding: 0;
          border-top: none;
          background: transparent;
        }
        .pager-btn {
          border: 1px solid ${color("table_btn_border")};
          background: ${color("table_btn_bg")};
          color: ${color("table_btn_fg")};
          font-size: ${COCKPIT_TYPE.tableBody};
          line-height: 1.2;
          padding: ${embedded ? "5px 12px" : "3px 10px"};
          border-radius: 4px;
          cursor: pointer;
        }
        .pager-btn:hover:not(:disabled) {
          border-color: ${color("table_btn_hover_border")};
          color: ${color("table_btn_hover_fg")};
        }
        .pager-btn:disabled {
          opacity: 0.45;
          cursor: not-allowed;
        }
        .pager-info {
          min-width: 3.5rem;
          text-align: center;
          font-size: ${COCKPIT_TYPE.tableBody};
          color: ${color("text_muted")};
        }
        .table-footer.has-carousel-hint {
          justify-content: space-between;
        }
        .carousel-hint {
          flex: 0 0 auto;
          display: inline-flex;
          align-items: center;
          gap: 10px;
          margin-left: auto;
          padding: 2px 4px;
        }
        .carousel-dots {
          display: inline-flex;
          align-items: center;
          gap: 5px;
        }
        .carousel-dot {
          width: 6px;
          height: 6px;
          padding: 0;
          border: 0;
          border-radius: 50%;
          background: rgba(125, 211, 252, 0.22);
          cursor: pointer;
          transition:
            transform 280ms cubic-bezier(0.34, 1.4, 0.64, 1),
            background 220ms ease,
            box-shadow 220ms ease;
        }
        .carousel-dot:hover {
          background: rgba(125, 211, 252, 0.45);
        }
        .carousel-dot.is-active {
          background: ${color("tone_blue")};
          transform: scale(1.4);
          box-shadow: ${themeShadow("scrollbar_thumb", "0 0 8px rgba(56, 189, 248, 0.5)")};
          cursor: default;
        }
        .carousel-page-label {
          display: inline-flex;
          align-items: baseline;
          gap: 2px;
          font-size: ${COCKPIT_TYPE.tableBody};
          color: ${color("text_muted")};
          font-variant-numeric: tabular-nums;
          letter-spacing: 0.02em;
        }
        .carousel-page-current {
          display: inline-block;
          min-width: 0.65em;
          text-align: center;
          color: ${color("table_btn_hover_fg")};
          font-weight: 600;
          animation: carousel-page-bump 380ms cubic-bezier(0.34, 1.4, 0.64, 1);
        }
        .carousel-page-sep {
          opacity: 0.55;
          padding: 0 1px;
        }
        .carousel-page-total {
          color: ${color("table_btn_fg")};
          font-weight: 500;
        }
        @keyframes carousel-page-bump {
          0% { transform: scale(0.82); opacity: 0.55; }
          55% { transform: scale(1.14); opacity: 1; }
          100% { transform: scale(1); opacity: 1; }
        }
        .carousel-timer {
          display: inline-flex;
          align-items: center;
          justify-content: center;
          width: 20px;
          height: 20px;
          flex: 0 0 auto;
        }
        .carousel-ring {
          display: block;
        }
        .carousel-ring-track {
          fill: none;
          stroke: rgba(125, 211, 252, 0.16);
          stroke-width: 2;
        }
        .carousel-ring-progress {
          fill: none;
          stroke: ${color("tone_blue")};
          stroke-width: 2;
          stroke-linecap: round;
          transform: rotate(-90deg);
          transform-origin: 50% 50%;
          stroke-dasharray: var(--carousel-c);
          stroke-dashoffset: 0;
          animation: carousel-ring-countdown var(--carousel-ms) linear forwards;
        }
        .table-wrap.carousel-paused .carousel-ring-progress {
          animation-play-state: paused;
        }
        @keyframes carousel-ring-countdown {
          from { stroke-dashoffset: 0; }
          to { stroke-dashoffset: var(--carousel-c); }
        }
        ${cellTableChromeStyleBlock()}
        ${cellPopoverStyleBlock(popoverVariant)}
        ${warningLevelBlocksCss()}
        /* 本地强化：色块宽高比 ≤ 2:1；三连 0 间距无圆角（防 ES module 缓存） */
        .mei-warning-level-item {
          height: 28px !important;
          width: 48px !important;
          max-width: 48px !important;
          flex: 0 0 48px !important;
        }
        .mei-warning-level-blocks {
          justify-content: center !important;
          width: auto !important;
        }
        .mei-warning-level-blocks.is-multi {
          gap: 0 !important;
        }
        .mei-warning-level-blocks.is-multi .mei-warning-level-item {
          border-radius: 0 !important;
        }
        .mei-warning-level-blocks.is-multi .mei-warning-level-item + .mei-warning-level-item {
          margin-left: 0 !important;
          border-left-width: 0 !important;
        }
      </style>
      <div class="table-wrap${carouselEnabled(p) ? " carousel-active" : ""}">
        <div class="table-scroll">
          <div class="table-canvas">
            <div class="thead">${headCells}</div>
            <div class="tbody${tbodyClass ? ` ${tbodyClass}` : ""}">${this._state?.loading ? `<div class="empty">加载中…</div>` : body || emptyHint}</div>
          </div>
        </div>
        ${footerHtml}
      </div>
    `;
    this.bindCellPreviewEvents();
    this.bindCarouselHover();
  }
}

if (!customElements.get("mei-cockpit-data-table")) {
  customElements.define("mei-cockpit-data-table", MeiCockpitDataTable);
}

