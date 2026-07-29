import {
  deferUntilDisplayed,
  elementIsDisplayed,
  shouldReactToPreviewUpdated,
  fetchDatasetRows,
  fetchPanelRuntimeMetrics,
  findRuntimeMetricInResults,
  formatRuntimeQueryDisplayMessage,
  isStaticSkeletonDisplay,
  parseProps,
  queryStateIdOf,
  resolveRuntimeDataRef,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  setQueryStateFilter,
  subscribeQueryState,
} from "../../dataset/runtime-query.js";
import {
  runtimePropsHaveRenderableRows,
  shouldApplyDatasetMetricRowsResult,
  shouldApplyMetricFallbackResult,
} from "../../dataset/metric-dataframe-authority.js";
import { createComponentTracer } from "../../perf/render-trace.js";
import {
  clampThemeFontPx,
  cockpitCssVars,
  readThemeChartCategoricalPalette,
  readThemeChartPalette,
  readThemeColor,
  readThemeTypography,
  readThemeUiFontFamily,
} from "../../cockpit/tokens.js";
import {
  isWarningLevelDimension,
  readWarningLevelColors,
  resolveWarningLevelSliceColor,
} from "../../mei/warning-level.js";
import {
  bindFloatingPopoverDrag,
  buildTextPopoverShellHtml,
  copyTextToClipboard,
  ensureFloatingTextPopoverStyles,
  mountFloatingPopoverOnBody,
  positionFloatingPopoverNearAnchor,
} from "../../mei/floating-text-popover.js";
import { ensureEChartsGlobal } from "../../vendor/runtime-libs.js";
const CARTESIAN_KINDS = new Set(["line", "area", "trend", "column", "bar", "scatter"]);
const PIE_KINDS = new Set(["pie", "donut", "rose"]);

function isAbortError(error) {
  if (!error) return false;
  if (error.name === "AbortError") return true;
  const msg = String(error.message || error || "");
  return msg.includes("aborted") || msg.includes("AbortError");
}

function pickRuntimeMetricFromResult(result, metricRef) {
  const metrics = Array.isArray(result?.metrics) ? result.metrics : [];
  if (!metricRef?.metric_id) {
    return metrics[0] || null;
  }
  return findRuntimeMetricInResults(metrics, metricRef);
}

function chartPropsNeedRuntimeFetch(props) {
  if (isStaticSkeletonDisplay(props)) {
    return false;
  }
  return Boolean(resolveRuntimeMetricRef(props) || resolveRuntimeDataRef(props));
}

function buildStaticChartRows(mapping, rowCount = 4) {
  const xField =
    mapping?.x?.[0]?.field ||
    mapping?.x?.[0]?.name ||
    "category";
  const yField =
    mapping?.y?.[0]?.field ||
    mapping?.y?.[0]?.name ||
    "value";
  const yValues = [12, 34, 56, 78, 90, 62];
  const count = Math.max(4, Math.min(6, Number(rowCount) || 4));
  return Array.from({ length: count }, (_entry, index) => ({
    [xField]: `类目${index + 1}`,
    [yField]: yValues[index] ?? 12,
  }));
}

function chartPropsHaveInitialRows(props) {
  return resolveRows(props).length > 0;
}

function isEChartsInstanceAlive(chart, chartEl) {
  if (!chart || !chartEl) return false;
  try {
    if (typeof chart.isDisposed === "function" && chart.isDisposed()) {
      return false;
    }
    const dom = typeof chart.getDom === "function" ? chart.getDom() : null;
    return Boolean(dom && dom === chartEl && dom.isConnected);
  } catch {
    return false;
  }
}

function releaseChartSurface(chartEl, chart) {
  if (chart) {
    try {
      chart.dispose();
    } catch (_) {
      /* ignore */
    }
  }
  if (chartEl) {
    chartEl.innerHTML = "";
  }
}

export function defineChartElement(tagName, chartKind, defaultTitle) {
  if (customElements.get(tagName)) {
    return;
  }
  class MeiChartElement extends HTMLElement {
    connectedCallback() {
      if (typeof this._deferUntilVisibleCleanup === "function") {
        this._deferUntilVisibleCleanup();
        this._deferUntilVisibleCleanup = null;
      }
      this._deferUntilVisibleCleanup = deferUntilDisplayed(this, () => {
        this._deferUntilVisibleCleanup = null;
        this.bootstrapChartElement();
      });
    }

    bootstrapChartElement() {
      if (!this.shadowRoot) {
        this.attachShadow({ mode: "open" });
      }
      const bootProps = Object.assign({}, parseProps(this), { __chartKind: chartKind });
      this.shadowRoot.innerHTML = chartShellHtml(defaultTitle, bootProps);
      this.chartEl = this.shadowRoot.querySelector(".chart");
      this.metaEl = this.shadowRoot.querySelector(".meta");
      this.errorEl = this.shadowRoot.querySelector(".error");
      this._props = Object.assign({}, parseProps(this), { __chartKind: chartKind });
      this._runtimeProps = null;
      this._sharedFilters = {};
      this._renderTrace = createComponentTracer(this, tagName, {
        chart_kind: chartKind,
      });
      this.refresh = () => {
        this._props = Object.assign({}, parseProps(this), { __chartKind: chartKind });
        // props 可能在 bootstrap 之后才带上 fillHeight / carousel；就地改 shell。
        this.applyFillHeightShell(this._props);
        syncChartShellTitle(this.shadowRoot, defaultTitle, this._props);
        const needsRuntime = chartPropsNeedRuntimeFetch(this._props);
        if (needsRuntime && !this._queryStateId) {
          void this.refreshRuntimeData();
          return;
        }
        this.renderChart();
        if (!this._queryStateId && needsRuntime) {
          void this.refreshRuntimeData();
        }
      };
      this._onPreviewUpdated = (event) => {
        if (!shouldReactToPreviewUpdated(event, this)) {
          return;
        }
        this.refresh();
        // SPA 切页后布局常晚于 setOption；即使 refresh 已触发，再排一轮 settle resize
        this.scheduleChartSurfaceSync({ reason: "preview-updated" });
      };
      window.addEventListener("meilang:preview-updated", this._onPreviewUpdated);
      this.resizeObserver = new ResizeObserver((entries) => {
        this.scheduleChartSurfaceSync({
          reason: "resize-observer",
          entryHeight: Math.round(entries?.[0]?.contentRect?.height || 0),
        });
      });
      this.resizeObserver.observe(this);
      if (this.chartEl instanceof HTMLElement) {
        // host 宽高未变、仅内部 .chart flex 撑开时，也必须跟着 resize
        this.resizeObserver.observe(this.chartEl);
      }
      // 从其它页切回：元素已连接但曾被隐藏，交叉可见后再对齐画布
      if (typeof IntersectionObserver === "function") {
        this._visibilityObserver = new IntersectionObserver(
          (entries) => {
            const visible = entries.some((entry) => entry.isIntersecting && entry.intersectionRatio > 0);
            if (!visible) return;
            this.scheduleChartSurfaceSync({ reason: "intersection-visible" });
          },
          { threshold: [0, 0.01, 0.1] },
        );
        this._visibilityObserver.observe(this);
      }
      this._queryStateId = queryStateIdOf(this._props);
      this._renderTrace.mark("bootstrap", {
        query_state_id: this._queryStateId || "",
        needs_runtime: chartPropsNeedRuntimeFetch(this._props),
      });
      const needsRuntime = chartPropsNeedRuntimeFetch(this._props);
      if (!needsRuntime || chartPropsHaveInitialRows(this._props)) {
        this.renderChart();
      }
      this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
        this._sharedFilters = state?.filters || {};
        this.refreshRuntimeData();
      });
      if (!this._queryStateId && needsRuntime) {
        void this.refreshRuntimeData();
      }
    }

    disconnectedCallback() {
      this.closeLabelPopover();
      this.stopCarousel();
      this.unbindCarouselHover();
      if (typeof this._deferUntilVisibleCleanup === "function") {
        this._deferUntilVisibleCleanup();
        this._deferUntilVisibleCleanup = null;
      }
      if (typeof this._onPreviewUpdated === "function") {
        window.removeEventListener("meilang:preview-updated", this._onPreviewUpdated);
      } else if (typeof this.refresh === "function") {
        window.removeEventListener("meilang:preview-updated", this.refresh);
      }
      if (typeof this._unsubscribeQueryState === "function") {
        this._unsubscribeQueryState();
      }
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
      }
      if (this._visibilityObserver) {
        this._visibilityObserver.disconnect();
        this._visibilityObserver = null;
      }
      this.cancelChartSurfaceSync();
      if (this.chart) {
        this.chart.dispose();
        this.chart = null;
      }
    }

    cancelChartSurfaceSync() {
      if (this._surfaceSyncRaf != null) {
        cancelAnimationFrame(this._surfaceSyncRaf);
        this._surfaceSyncRaf = null;
      }
      if (Array.isArray(this._surfaceSyncTimers)) {
        for (const id of this._surfaceSyncTimers) {
          window.clearTimeout(id);
        }
      }
      this._surfaceSyncTimers = [];
    }

    /** 读取画布盒；优先 client*（设计像素），避免 stage CSS scale 把 visual rect 缩窄 */
    readChartSurfaceBox() {
      const box = this.chartEl instanceof HTMLElement ? this.chartEl : this;
      const width = Math.max(
        0,
        Math.floor(box.clientWidth || this.clientWidth || 0),
      );
      const height = Math.max(
        0,
        Math.floor(box.clientHeight || this.clientHeight || 0),
      );
      return { width, height, box };
    }

    syncChartSurfaceSize(meta = {}) {
      if (!this.isConnected || !elementIsDisplayed(this)) {
        return;
      }
      const props = Object.assign({}, parseProps(this), this._runtimeProps || {}, {
        __host: this,
        __chartKind: chartKind,
      });
      if (this.chart) {
        const { width, height } = this.readChartSurfaceBox();
        if (width < 8 || height < 8) {
          this.rebindTruncatedTitles(props);
          return;
        }
        const prev = this._lastSurfaceBox || { width: 0, height: 0 };
        // 尺寸未变也允许强制对齐（SPA 切回后 canvas 常仍是旧 px）
        const force = meta.force === true;
        if (!force && prev.width === width && prev.height === height) {
          this.rebindTruncatedTitles(props);
          return;
        }
        this._lastSurfaceBox = { width, height };
        try {
          this.chart.resize({ width, height });
        } catch (_) {
          try {
            this.chart.resize();
          } catch (__) {
            /* ignore */
          }
        }
        this.rebindTruncatedTitles(props);
        return;
      }
      // ranking 的 DOM 布局（above / bar）+ fillHeight：无 ECharts 实例，需随格子尺寸重算行高。
      const rankingKind = normalizeKind(chartKind);
      const rankingLayout =
        rankingKind === "ranking-bar" ? "bar" : resolveRankingLayout(props);
      if (
        (rankingKind !== "ranking" && rankingKind !== "ranking-bar") ||
        (rankingLayout !== "above" && rankingLayout !== "bar") ||
        !rankingFillHeightEnabled(props)
      ) {
        this.rebindTruncatedTitles(props);
        return;
      }
      const nextH = Math.round(
        meta.entryHeight || this.readChartSurfaceBox().height || this.clientHeight || 0,
      );
      if (nextH <= 0 || nextH === this._rankingFillHeightPx) {
        this.rebindTruncatedTitles(props);
        return;
      }
      this._rankingFillHeightPx = nextH;
      void this.renderChart();
    }

    rebindTruncatedTitles(props = {}) {
      this.bindShellTitlePopover(props);
      const titleEl = this.chartEl?.querySelector?.(
        ".mei-rank-bar-title-text, .mei-rank-above-title",
      );
      if (titleEl instanceof HTMLElement) {
        const full = titleEl.dataset.meiFullTitle || String(titleEl.textContent || "").trim();
        this.bindTruncatedTitleClick(titleEl, full);
      }
    }

    /**
     * 布局结算后再 resize：首帧 flex/grid 常未给出最终宽高，
     * 只靠单次 resize 会把柱图锁在「挤成一团」的旧 canvas 尺寸上（F5 才恢复）。
     */
    scheduleChartSurfaceSync(meta = {}) {
      this.cancelChartSurfaceSync();
      const run = (force) => {
        this.syncChartSurfaceSize({ ...meta, force });
      };
      this._surfaceSyncRaf = requestAnimationFrame(() => {
        this._surfaceSyncRaf = requestAnimationFrame(() => {
          this._surfaceSyncRaf = null;
          run(true);
        });
      });
      this._surfaceSyncTimers = [0, 48, 160].map((ms) =>
        window.setTimeout(() => run(true), ms),
      );
    }

    applyFillHeightShell(props) {
      if (!chartFillHeightEnabled(props) || !this.shadowRoot) return;
      this.style.display = "flex";
      this.style.flexDirection = "column";
      this.style.height = "100%";
      this.style.minHeight = "0";
      this.style.alignSelf = "stretch";
      this.style.justifyContent = "flex-start";
      this.style.boxSizing = "border-box";
      const wrap = this.shadowRoot.querySelector(".wrap");
      if (wrap instanceof HTMLElement) {
        wrap.style.display = "flex";
        wrap.style.flexDirection = "column";
        wrap.style.height = "100%";
        wrap.style.minHeight = "0";
        wrap.style.flex = "1 1 0";
        wrap.style.justifyContent = "flex-start";
        wrap.style.alignItems = "stretch";
        wrap.style.boxSizing = "border-box";
        wrap.style.gridTemplateRows = "";
      }
      if (this.chartEl instanceof HTMLElement) {
        // flex-basis:0 强制画布吃满头/脚之外的剩余高度，避免上下大块留白。
        // ranking-bar：对齐校服排名图可视高度（约 5 行 + 标题），避免矮轨 section 把条压扁。
        const rankingBarFloor =
          normalizeKind(props?.__chartKind || chartKind) === "ranking-bar" ||
          resolveRankingLayout(props) === "bar";
        this.chartEl.style.minHeight = rankingBarFloor ? "200px" : "96px";
        this.chartEl.style.flex = "1 1 0";
        this.chartEl.style.height = "auto";
        this.chartEl.style.maxHeight = "none";
        this.chartEl.style.width = "100%";
        this.chartEl.style.minWidth = "0";
        this.chartEl.style.boxSizing = "border-box";
      }
      const headEl = this.shadowRoot.querySelector(".head");
      if (headEl instanceof HTMLElement) {
        headEl.style.flex = "0 0 auto";
        headEl.style.margin = "0";
        headEl.style.minHeight = "0";
        headEl.style.lineHeight = "1.15";
      }
      const hintHost = this.shadowRoot.querySelector(".carousel-hint-host");
      if (hintHost instanceof HTMLElement) {
        hintHost.style.flex = "0 0 auto";
        hintHost.style.margin = "0";
        hintHost.style.padding = "0";
      }
      const errorEl = this.shadowRoot.querySelector(".error");
      if (errorEl instanceof HTMLElement && !String(errorEl.textContent || "").trim()) {
        errorEl.style.display = "none";
      }
    }

    stopCarousel() {
      if (this._carouselTimer) {
        clearInterval(this._carouselTimer);
        this._carouselTimer = null;
      }
    }

    unbindCarouselHover() {
      if (this._carouselHoverTarget && this._onCarouselPause && this._onCarouselResume) {
        this._carouselHoverTarget.removeEventListener("mouseenter", this._onCarouselPause);
        this._carouselHoverTarget.removeEventListener("mouseleave", this._onCarouselResume);
      }
      this._carouselHoverTarget = null;
      this._carouselHoverBound = false;
    }

    bindCarouselHover(props) {
      const pauseOnHover =
        props?.carouselPauseOnHover !== false && props?.carousel_pause_on_hover !== "false";
      if (!chartCarouselEnabled(props) || !pauseOnHover) {
        this.unbindCarouselHover();
        return;
      }
      const wrap = this.shadowRoot?.querySelector(".wrap");
      if (!(wrap instanceof HTMLElement)) return;
      if (this._carouselHoverBound && this._carouselHoverTarget === wrap) return;
      this.unbindCarouselHover();
      this._onCarouselPause = () => {
        this._carouselPaused = true;
        wrap.classList.add("carousel-paused");
        this.stopCarousel();
      };
      this._onCarouselResume = () => {
        this._carouselPaused = false;
        wrap.classList.remove("carousel-paused");
        this.startCarousel(props);
      };
      wrap.addEventListener("mouseenter", this._onCarouselPause);
      wrap.addEventListener("mouseleave", this._onCarouselResume);
      this._carouselHoverTarget = wrap;
      this._carouselHoverBound = true;
    }

    startCarousel(props) {
      this.stopCarousel();
      if (!chartCarouselEnabled(props) || this._carouselPaused) return;
      const slides = Array.isArray(this._carouselSlides) ? this._carouselSlides : [];
      if (slides.length <= 1) return;
      const interval = resolveChartCarouselIntervalMs(props);
      this._carouselTimer = setInterval(() => {
        const total = Array.isArray(this._carouselSlides) ? this._carouselSlides.length : 0;
        if (total <= 1) return;
        this._carouselIndex = ((Number(this._carouselIndex) || 0) + 1) % total;
        this._carouselEpoch = (Number(this._carouselEpoch) || 0) + 1;
        void this.renderChart();
      }, interval);
    }

    onCarouselDotClick(event) {
      const pageRaw = event
        .composedPath()
        .find((node) => node instanceof HTMLElement && node.dataset?.carouselPage)
        ?.dataset?.carouselPage;
      if (pageRaw == null) return;
      const nextIndex = Number(pageRaw) - 1;
      const total = Array.isArray(this._carouselSlides) ? this._carouselSlides.length : 0;
      if (!Number.isFinite(nextIndex) || nextIndex < 0 || nextIndex >= total) return;
      if (nextIndex === (Number(this._carouselIndex) || 0)) return;
      this._carouselIndex = nextIndex;
      this._carouselEpoch = (Number(this._carouselEpoch) || 0) + 1;
      // renderChart 末尾会 sync hint 并重启自动轮播计时
      void this.renderChart();
    }

    syncCarouselHint(props) {
      const host = this.shadowRoot?.querySelector(".carousel-hint-host");
      const wrap = this.shadowRoot?.querySelector(".wrap");
      if (!(host instanceof HTMLElement)) return;
      if (!this._carouselDotBound) {
        this._carouselDotBound = true;
        host.addEventListener("click", (event) => this.onCarouselDotClick(event));
      }
      if (!chartCarouselEnabled(props) || !chartCarouselShowsHint(props)) {
        host.innerHTML = "";
        host.hidden = true;
        wrap?.classList.remove("has-carousel-hint");
        return;
      }
      const total = Array.isArray(this._carouselSlides) ? this._carouselSlides.length : 0;
      if (total <= 1) {
        host.innerHTML = "";
        host.hidden = true;
        wrap?.classList.remove("has-carousel-hint");
        return;
      }
      const page = (Number(this._carouselIndex) || 0) + 1;
      const interval = resolveChartCarouselIntervalMs(props);
      const epoch = Number(this._carouselEpoch) || 0;
      host.hidden = false;
      host.innerHTML = renderChartCarouselHintHtml(page, total, interval, epoch);
      wrap?.classList.add("has-carousel-hint");
    }

    async renderChart() {
      let props = Object.assign({}, parseProps(this), this._runtimeProps || {}, {
        __host: this,
        __chartKind: chartKind,
      });
      props = applyChartCarouselFilter(this, props);
      this.applyFillHeightShell(props);
      if (this.shadowRoot) {
        syncChartShellTitle(this.shadowRoot, defaultTitle, props);
        this.bindShellTitlePopover(props);
      }
      if (isStaticSkeletonDisplay(props)) {
        this.classList.add("mei-chart--static-skeleton");
      } else {
        this.classList.remove("mei-chart--static-skeleton");
      }
      this._renderTrace?.mark("render_start", {
        has_runtime_props: Boolean(this._runtimeProps),
      });
      const diagnostics = [];
      const model = buildChartModel(chartKind, props, diagnostics);
      this._selectionContext = resolveChartSelectionContext(chartKind, props, model);
      this.metaEl.textContent = props.__carouselActive ? "" : model.meta;
      if (diagnostics.length > 0) {
        this.errorEl.textContent = diagnostics.join(" | ");
      } else {
        this.errorEl.textContent = "";
      }
      if (
        (chartKind === "ranking" || chartKind === "ranking-bar") &&
        (model.layout === "above" || model.layout === "bar")
      ) {
        releaseChartSurface(this.chartEl, this.chart);
        this.chart = null;
        this._rankingFullLabels = Array.isArray(model.fullLabels) ? model.fullLabels : [];
        const pullUp = Math.max(0, Number(props.rankingPullUp ?? props.ranking_pull_up ?? 0));
        if (pullUp > 0) {
          this.style.overflow = "visible";
        } else {
          this.style.removeProperty("overflow");
        }
        if (model.layout === "bar") {
          renderRankingBarDom(this.chartEl, model, props, {
            onLabelClick: (fullText, event) => {
              this.openLabelPopover(fullText, event);
            },
            bindTitle: (el, fullText) => this.bindTruncatedTitleClick(el, fullText),
          });
        } else {
          renderRankingAboveDom(this.chartEl, model, props, {
            onLabelClick: (fullText, event) => {
              this.openLabelPopover(fullText, event);
            },
            bindTitle: (el, fullText) => this.bindTruncatedTitleClick(el, fullText),
          });
        }
        this.syncCarouselHint(props);
        this.stopCarousel();
        return;
      }
      try {
        const renderSeq = (this._renderSeq = (this._renderSeq || 0) + 1);
        const hadDomRanking = Boolean(
          this.chartEl?.querySelector?.(".mei-rank-above, .mei-rank-bar"),
        );
        const canReuse =
          !hadDomRanking && isEChartsInstanceAlive(this.chart, this.chartEl);
        if (!canReuse) {
          releaseChartSurface(this.chartEl, this.chart);
          this.chart = null;
        }
        this._renderTrace?.mark("echarts_load_start");
        const echarts = await ensureECharts();
        this._renderTrace?.mark("echarts_load_done");
        if (renderSeq !== this._renderSeq) {
          return;
        }
        if (!this.chart) {
          this.chart = echarts.init(this.chartEl);
        }
        if (chartKind === "ranking" || chartKind === "ranking-bar") {
          this._rankingFullLabels = Array.isArray(model.fullLabels) ? model.fullLabels : [];
          const fillHeight = rankingFillHeightEnabled(props);
          const shellH = resolveRankingShellHeight(props, model.rowCount);
          // fillHeight 时由 flex 吃满格子，勿再写死 minHeight 与壳抢尺寸
          if (!fillHeight && model.rowCount > 0 && shellH > 0) {
            this.chartEl.style.minHeight = `${shellH}px`;
            if (isRankingCompact(props) && Number(props.chartHeight) > 0) {
              this.chartEl.style.height = `${shellH}px`;
              this.chartEl.style.maxHeight = `${shellH}px`;
            }
          } else if (fillHeight) {
            this.chartEl.style.width = "100%";
            this.chartEl.style.minWidth = "0";
          } else {
            this.chartEl.style.minHeight = "";
            this.chartEl.style.height = "";
            this.chartEl.style.maxHeight = "";
          }
          this.setupRankingChartInteractions();
        } else {
          this._rankingFullLabels = null;
          this.setupSelectionInteractions(chartKind);
        }
        this.chart.setOption(model.option, true);
        this.scheduleChartSurfaceSync({ reason: "render-chart", force: true });
        this._renderTrace?.mark("render_done", {
          rows: Array.isArray(model.rows) ? model.rows.length : 0,
        });
        this.syncCarouselHint(props);
        this.bindCarouselHover(props);
        this.startCarousel(props);
      } catch (error) {
        this.errorEl.textContent = "图表引擎加载失败: " + String(error?.message || error);
        this._renderTrace?.mark("render_error", {
          message: String(error?.message || error),
        });
      }
    }

    setupRankingChartInteractions() {
      if (!this.chart) return;
      this.chart.off("click");
      this.chart.on("click", (params) => {
        let index = -1;
        if (params.componentType === "series") {
          index = Number(params.dataIndex);
        } else if (params.componentType === "yAxis") {
          index = Number.isFinite(params.dataIndex)
            ? Number(params.dataIndex)
            : (this._rankingFullLabels || []).indexOf(params.value);
        }
        const full = this._rankingFullLabels?.[index];
        if (!full) return;
        this.openLabelPopover(full, params.event?.event);
      });
    }

    setupSelectionInteractions(chartKind) {
      if (!this.chart) return;
      this.chart.off("click");
      const selection = this._selectionContext;
      if (!selection?.queryStateId || !selection?.dimension) return;
      if (normalizeKind(chartKind) === "ranking") return;
      this.chart.on("click", (params) => {
        const selectedValue = resolveChartSelectionValue(chartKind, params, selection);
        if (!selectedValue) return;
        setQueryStateFilter(selection.queryStateId, selection.dimension, selectedValue, {
          filterIntentSource: "chart_selection",
          transitionSource: "chart_selection",
          toggle: selection.toggle,
          encode: selection.filterEncode,
        });
      });
    }

    openLabelPopover(fullText, anchorEvent, options = {}) {
      this.closeLabelPopover();
      ensureFloatingTextPopoverStyles();
      const dialogTitle = String(options.dialogTitle || "完整名称").trim() || "完整名称";
      const pop = document.createElement("div");
      pop.className = "cell-pop cell-pop--large";
      pop.setAttribute("role", "dialog");
      pop.setAttribute("aria-modal", "true");
      pop.setAttribute("aria-label", dialogTitle);
      pop.innerHTML = buildTextPopoverShellHtml(
        { title: dialogTitle, subtitle: "", fullText },
        escapeHtml,
      );
      mountFloatingPopoverOnBody(pop, { width: 420 });
      this._labelPopoverEl = pop;
      const anchor = anchorEvent?.target || anchorEvent;
      positionFloatingPopoverNearAnchor(pop, anchor, {
        topOffset: 8,
        defaultWidth: 420,
      });
      this._labelPopoverDragCleanup = bindFloatingPopoverDrag(
        pop,
        pop.querySelector(".cell-pop-drag-handle"),
      );

      const onDoc = (ev) => {
        const path = ev.composedPath();
        if (path.includes(pop) || (anchor && path.includes(anchor))) return;
        this.closeLabelPopover();
      };
      setTimeout(() => document.addEventListener("pointerdown", onDoc, true), 0);
      this._labelPopoverDocCleanup = () => document.removeEventListener("pointerdown", onDoc, true);
      this._labelPopoverKeydown = (ev) => {
        if (ev.key === "Escape") {
          ev.stopPropagation();
          this.closeLabelPopover();
        }
      };
      document.addEventListener("keydown", this._labelPopoverKeydown, true);
      const close = () => this.closeLabelPopover();
      pop.querySelector(".cell-pop-close")?.addEventListener("click", close);
      pop.querySelector(".cell-pop-done")?.addEventListener("click", close);
      pop.querySelector(".cell-pop-copy")?.addEventListener("click", () => {
        copyTextToClipboard(fullText);
      });
      (pop.querySelector(".cell-pop-done") || pop.querySelector(".cell-pop-close"))?.focus();
    }

    /**
     * 标题 CSS 截断后可点击查看全文（与排名标签飘窗同壳）。
     * 布局未结算时 scrollWidth 可能不准，故 rAF 后再判定。
     */
    bindTruncatedTitleClick(el, fullText) {
      if (!(el instanceof HTMLElement)) return;
      const full = String(fullText ?? "").trim();
      if (typeof el._meiTruncatedTitleClick === "function") {
        el.removeEventListener("click", el._meiTruncatedTitleClick);
        el._meiTruncatedTitleClick = null;
      }
      el.classList.remove("is-truncated-title");
      el.style.cursor = "";
      el.removeAttribute("role");
      el.removeAttribute("tabindex");
      el.removeAttribute("aria-label");
      if (el.getAttribute("title") === el.dataset.meiFullTitle) {
        el.removeAttribute("title");
      }
      delete el.dataset.meiFullTitle;
      if (!full) return;
      el.dataset.meiFullTitle = full;
      const apply = () => {
        if (!el.isConnected || el.dataset.meiFullTitle !== full) return;
        const truncated = el.scrollWidth > el.clientWidth + 1;
        if (!truncated) {
          el.classList.remove("is-truncated-title");
          el.style.cursor = "";
          el.removeAttribute("role");
          el.removeAttribute("tabindex");
          el.removeAttribute("aria-label");
          if (el.getAttribute("title") === full) el.removeAttribute("title");
          if (typeof el._meiTruncatedTitleClick === "function") {
            el.removeEventListener("click", el._meiTruncatedTitleClick);
            el._meiTruncatedTitleClick = null;
          }
          return;
        }
        el.classList.add("is-truncated-title");
        el.style.cursor = "pointer";
        el.setAttribute("role", "button");
        el.setAttribute("tabindex", "0");
        el.setAttribute("title", full);
        el.setAttribute("aria-label", `查看完整标题：${full}`);
        if (typeof el._meiTruncatedTitleClick === "function") {
          el.removeEventListener("click", el._meiTruncatedTitleClick);
        }
        el._meiTruncatedTitleClick = (event) => {
          event.preventDefault();
          event.stopPropagation();
          this.openLabelPopover(full, event, { dialogTitle: "完整标题" });
        };
        el.addEventListener("click", el._meiTruncatedTitleClick);
      };
      requestAnimationFrame(() => requestAnimationFrame(apply));
    }

    bindShellTitlePopover(props = {}) {
      const titleEl = this.shadowRoot?.querySelector(".head .title");
      if (!(titleEl instanceof HTMLElement)) return;
      const headEl = this.shadowRoot?.querySelector(".head");
      const headVisible =
        headEl instanceof HTMLElement && getComputedStyle(headEl).display !== "none";
      if (!headVisible) return;
      const full = String(props.title ?? titleEl.textContent ?? "").trim();
      this.bindTruncatedTitleClick(titleEl, full);
    }

    closeLabelPopover() {
      if (typeof this._labelPopoverDragCleanup === "function") {
        this._labelPopoverDragCleanup();
        this._labelPopoverDragCleanup = null;
      }
      if (typeof this._labelPopoverDocCleanup === "function") {
        this._labelPopoverDocCleanup();
        this._labelPopoverDocCleanup = null;
      }
      if (typeof this._labelPopoverKeydown === "function") {
        document.removeEventListener("keydown", this._labelPopoverKeydown, true);
        this._labelPopoverKeydown = null;
      }
      if (this._labelPopoverEl) {
        this._labelPopoverEl.remove();
        this._labelPopoverEl = null;
      }
    }

    async refreshRuntimeData() {
      const props = this._props || parseProps(this);
      const metricRef = resolveRuntimeMetricRef(props);
      const dataRef = resolveRuntimeDataRef(props);
      if (!metricRef && !dataRef) {
        this._runtimeProps = null;
        this.renderChart();
        return;
      }
      // Ignore stale overlapping refreshes (bootstrap paint then empty metrics overwrite).
      const refreshGen = (this._runtimeRefreshGen = (this._runtimeRefreshGen || 0) + 1);
      try {
        this._renderTrace?.mark("runtime_query_start", {
          mode: metricRef ? "metric" : "dataset",
        });
        if (metricRef) {
          const metricId = String(metricRef?.metric_id || metricRef?.metricId || "").trim();
          const supportRole = String(props?.supportRole ?? props?.support_role ?? "").trim().toLowerCase();
          const dedicatedExplain =
            (metricId.includes("::") && !metricId.endsWith("::__scalar_rowset__")) ||
            supportRole === "composition" ||
            supportRole === "trend" ||
            supportRole === "attribution" ||
            (!metricId.includes("::") && /composition|trend|breakdown|attribution/i.test(metricId));
          const topN = Number(props?.top_n ?? props?.topN ?? 0);
          const explicitPage = Number(props?.pageSize ?? props?.page_size ?? 0);
          // 显式 pageSize 优先，避免 trend 角色默認 64 截断长表后 carouselTopN 算错学校。
          const pageSize =
            explicitPage > 0
              ? Math.floor(explicitPage)
              : dedicatedExplain
                ? topN > 0
                  ? Math.max(topN, 16)
                  : 64
                : 20;
          const rowsResult = await fetchDatasetRows(props, {
            queryStateId: this._queryStateId,
            filters: this._sharedFilters,
            page: 1,
            pageSize,
            full: false,
            meta: runtimeCallerMeta(this, tagName),
          });
          // Empty rows:[] must not short-circuit. Stale gen with good rows must still
          // upgrade an empty paint (overlapping refreshRuntimeData race).
          if (
            shouldApplyDatasetMetricRowsResult({
              refreshGen,
              currentGen: this._runtimeRefreshGen,
              rowsResult,
              runtimeProps: this._runtimeProps,
            })
          ) {
            const dataset = resolveDatasetSource(props);
            this._runtimeProps = {
              data: {
                ...dataset,
                columns: Array.isArray(rowsResult.columns) ? rowsResult.columns : dataset.columns || [],
                rows: rowsResult.rows,
              },
            };
            this._renderTrace?.mark("runtime_query_done", {
              mode: "dataset_metric",
              row_count: rowsResult.rows.length,
              client_total_ms: rowsResult?.perf?.client_total_ms ?? "",
              server_total_ms:
                rowsResult?.perf?.server_handler_total_ms ?? rowsResult?.perf?.total_ms ?? "",
              stale_applied: refreshGen !== this._runtimeRefreshGen ? 1 : 0,
            });
            await this.renderChart();
            return;
          }
          if (refreshGen !== this._runtimeRefreshGen) {
            return;
          }
          const result = await fetchPanelRuntimeMetrics(this, props, {
            filters: this._sharedFilters,
            meta: runtimeCallerMeta(this, tagName),
          });
          const metric = pickRuntimeMetricFromResult(result, metricRef);
          if (
            shouldApplyMetricFallbackResult({
              refreshGen,
              currentGen: this._runtimeRefreshGen,
              metric,
              runtimeProps: this._runtimeProps,
            })
          ) {
            this._runtimeProps = props.value?.__mei_runtime_ref
              ? { value: metric }
              : { data: metric };
          } else if (
            refreshGen === this._runtimeRefreshGen &&
            !runtimePropsHaveRenderableRows(this._runtimeProps)
          ) {
            this._runtimeProps = null;
          }
          if (refreshGen !== this._runtimeRefreshGen) {
            return;
          }
          this._renderTrace?.mark("runtime_query_done", {
            mode: "metric",
            metric_count: Array.isArray(result?.metrics) ? result.metrics.length : 0,
            client_total_ms: result?.perf?.client_total_ms ?? "",
            server_total_ms: result?.perf?.server_handler_total_ms ?? result?.perf?.total_ms ?? "",
          });
        } else {
          const result = await fetchDatasetRows(props, {
            queryStateId: this._queryStateId,
            filters: this._sharedFilters,
            page: 1,
            pageSize: Number(props?.pageSize ?? props?.page_size ?? 0) > 0
              ? Number(props?.pageSize ?? props?.page_size)
              : 20,
            full: false,
            meta: runtimeCallerMeta(this, tagName),
          });
          const dataset = resolveDatasetSource(props);
          this._runtimeProps = {
            data: {
              ...dataset,
              columns: Array.isArray(result?.columns) ? result.columns : dataset.columns || [],
              rows: Array.isArray(result?.rows) ? result.rows : [],
            },
          };
          this._renderTrace?.mark("runtime_query_done", {
            mode: "dataset",
            row_count: Array.isArray(result?.rows) ? result.rows.length : 0,
            client_total_ms: result?.perf?.client_total_ms ?? "",
            server_total_ms: result?.perf?.server_handler_total_ms ?? result?.perf?.total_ms ?? "",
          });
        }
        await this.renderChart();
      } catch (error) {
        if (isAbortError(error)) return;
        this.errorEl.textContent = formatRuntimeQueryDisplayMessage(error?.message || error);
        this._renderTrace?.mark("runtime_query_error", {
          message: String(error?.message || error),
        });
      }
    }
  }
  customElements.define(tagName, MeiChartElement);
}

function syncChartShellTitle(shadowRoot, defaultTitle, props = {}) {
  if (!shadowRoot) return;
  const titleEl = shadowRoot.querySelector(".title");
  const headEl = shadowRoot.querySelector(".head");
  if (!(titleEl instanceof HTMLElement) || !(headEl instanceof HTMLElement)) return;
  const title = String(props.title ?? defaultTitle).trim();
  titleEl.textContent = title || defaultTitle;
  // rankingLayout=above/bar 自带标题行；hide shell head to avoid duplicates.
  // titleInPlot：标题画在 ECharts 画布内；默认壳头在画布外（同园区罚金统计）。
  const rankingOwnsTitle = rankingLayoutOwnsTitle(props);
  const showHead = Boolean(title) && !rankingOwnsTitle && !titleInPlotEnabled(props);
  headEl.style.display = showHead ? "flex" : "none";
}

function rankingLayoutOwnsTitle(props = {}) {
  const kind = normalizeKind(props?.__chartKind || props?.chartKind || "");
  if (kind === "ranking-bar") return true;
  const layout = resolveRankingLayout(props);
  return layout === "above" || layout === "bar";
}

function chartFillHeightEnabled(props = {}) {
  // 轮播图必须走 fill：校名头 + 倒计时脚会叠在 chartHeight 外，固定高度必裁 X 轴。
  if (chartCarouselEnabled(props)) return true;
  return (
    props.fillHeight === true ||
    props.fillHeight === "true" ||
    props.fill_height === true ||
    props.fill_height === "true" ||
    ((props.compact === true || props.compact === "true") && !(Number(props.chartHeight) > 0))
  );
}

function chartShellHtml(defaultTitle, props = {}) {
  const compact = props.compact === true || props.compact === "true";
  const fillHeight = chartFillHeightEnabled(props);
  const chartHeight = Number(props.chartHeight) > 0 ? Number(props.chartHeight) : compact ? 64 : 260;
  const title = String(props.title ?? defaultTitle).trim();
  const rankingOwnsTitle = rankingLayoutOwnsTitle(props);
  // above/bar layout owns the title node; shell .head would duplicate it.
  // titleInPlot：标题进画布；默认壳头在画布外（对齐 cockpit.park-amount-list）。
  // 轮播：校名放 head，但用更紧凑行高，把垂直空间留给画布与 hint。
  const showHead = title.length > 0 && !rankingOwnsTitle && !titleInPlotEnabled(props);
  // 壳头在画布外时：固定 chartHeight 表示「整卡预算」，画布高度减去标题行，避免父级 overflow 裁切 X 轴。
  const headReservePx = showHead ? (compact ? 18 : 24) : 0;
  const plotHeight =
    Number(props.chartHeight) > 0 ? Math.max(40, chartHeight - headReservePx) : chartHeight;
  const chartSizeCss = fillHeight
    ? "min-height: 0; flex: 1 1 0; height: auto; max-height: none;"
    : `min-height: ${plotHeight}px; height: ${compact ? plotHeight + "px" : "auto"}; max-height: ${compact ? plotHeight + "px" : "none"};`;
  const hostHeightCss =
    !fillHeight && compact && Number(props.chartHeight) > 0
      ? `height: ${chartHeight}px; max-height: ${chartHeight}px;`
      : "";
  return `
    <style>
      :host {
        display: ${fillHeight ? "flex" : "block"};
        flex-direction: column;
        width: 100%;
        ${fillHeight ? "height: 100%; min-height: 0; align-self: stretch; justify-content: flex-start;" : hostHeightCss}
        min-width: 0;
        overflow: hidden;
        box-sizing: border-box;
        ${cockpitCssVars()}
      }
      .wrap {
        display: ${fillHeight || showHead ? "flex" : "grid"};
        ${fillHeight || showHead ? "flex-direction: column; height: 100%; min-height: 0; flex: 1 1 0; justify-content: flex-start; align-items: stretch;" : ""}
        gap: ${compact ? (showHead ? "2px" : "0") : "8px"};
        padding: ${compact ? "0" : "14px"};
        border-radius: 0;
        border: ${compact ? "none" : "1px solid rgba(148,163,184,.2)"};
        /* Chart chrome stays transparent so section/panel theme tokens show through. */
        background: transparent;
        box-sizing: border-box;
        min-height: 0;
      }
      .head {
        display: ${showHead ? "flex" : "none"};
        flex: 0 0 auto;
        justify-content: flex-start;
        gap: 6px;
        align-items: baseline;
        min-width: 0;
        margin: 0;
        padding: 0 2px;
        line-height: 1.2;
      }
      .title {
        margin: 0;
        font-size: var(--cockpit-font-chart-title);
        font-weight: 600;
        /* 与园区罚金统计 .head 一致：画布外左上、反色字 */
        color: ${compact ? "var(--mei-color-text-inverse, #f8fafc)" : "#f8fafc"};
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: 100%;
      }
      .title.is-truncated-title,
      .mei-rank-bar-title-text.is-truncated-title,
      .mei-rank-above-title.is-truncated-title {
        cursor: pointer;
      }
      .meta { font-size: var(--cockpit-font-unit); color: #94a3b8; }
      .chart {
        width: 100%;
        ${chartSizeCss}
        overflow: hidden;
        box-sizing: border-box;
      }
      .mei-rank-above {
        display: flex;
        flex-direction: column;
        height: 100%;
        min-height: 0;
        box-sizing: border-box;
        overflow: hidden;
      }
      .mei-rank-above-title {
        flex: 0 0 auto;
        margin: 0 0 2px;
        font-size: var(--cockpit-font-chart-title);
        font-weight: 600;
        color: var(--mei-color-text-inverse, #f8fafc);
        line-height: 1.2;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }
      .mei-rank-above-list {
        flex: 1 1 0;
        min-height: 0;
        display: flex;
        flex-direction: column;
        justify-content: space-between;
        gap: 1px;
      }
      .mei-rank-above-row {
        flex: 1 1 0;
        min-height: 0;
        display: flex;
        flex-direction: column;
        justify-content: center;
        gap: 1px;
        cursor: pointer;
      }
      .mei-rank-above-head {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        gap: 8px;
        min-width: 0;
      }
      .mei-rank-above-label {
        flex: 1 1 0;
        min-width: 0;
        font-size: var(--cockpit-font-label);
        line-height: 1.2;
        font-weight: 500;
        color: #e2e8f0;
        display: block;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
      }
      .mei-rank-above-value {
        flex: 0 0 auto;
        font-size: var(--cockpit-font-label);
        font-weight: 600;
        color: #7dd3fc;
      }
      .mei-rank-above-track {
        position: relative;
        width: 100%;
        height: 5px;
        border-radius: 0;
        overflow: hidden;
        background: transparent;
        border: 1px solid rgba(100, 116, 139, 0.35);
        box-sizing: border-box;
      }
      .mei-rank-above-fill {
        position: absolute;
        left: 0;
        top: 0;
        height: 100%;
        min-width: 3px;
        border-radius: 0;
        background: #38bdf8;
        box-shadow: 0 1px 5px rgba(56, 189, 248, 0.35);
        z-index: 1;
        pointer-events: none;
      }
      .mei-rank-bar {
        display: flex;
        flex-direction: column;
        height: 100%;
        min-height: 0;
        box-sizing: border-box;
        overflow: hidden;
      }
      .mei-rank-bar-title {
        flex: 0 0 auto;
        display: flex;
        align-items: baseline;
        justify-content: flex-start;
        gap: 6px;
        min-width: 0;
        margin: 0 0 2px;
        padding: 0 2px;
        line-height: 1.2;
      }
      .mei-rank-bar-title-text {
        flex: 1 1 auto;
        min-width: 0;
        margin: 0;
        font-size: var(--cockpit-font-chart-title);
        font-weight: 600;
        /* 对齐柱图壳头标题：固定反色白字 */
        color: var(--mei-color-text-inverse, #f8fafc);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }
      .mei-rank-bar-unit {
        display: none;
      }
      .mei-rank-bar-list {
        flex: 1 1 0;
        min-height: 0;
        display: flex;
        flex-direction: column;
        justify-content: space-between;
        gap: 2px;
      }
      .mei-rank-bar-row {
        position: relative;
        flex: 1 1 0;
        min-height: 0;
        display: flex;
        align-items: stretch;
        overflow: hidden;
        cursor: pointer;
        border: 1px solid rgba(100, 116, 139, 0.28);
        background: rgba(15, 23, 42, 0.22);
        box-sizing: border-box;
      }
      .mei-rank-bar-fill {
        position: absolute;
        left: 0;
        top: 0;
        bottom: 0;
        min-width: 4px;
        z-index: 0;
        pointer-events: none;
      }
      .mei-rank-bar-content {
        position: relative;
        z-index: 1;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        width: 100%;
        min-width: 0;
        padding: 0 8px;
        box-sizing: border-box;
      }
      .mei-rank-bar-label {
        flex: 1 1 0;
        min-width: 0;
        font-size: var(--cockpit-font-label);
        line-height: 1.2;
        font-weight: 600;
        color: #f8fafc;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
        /* 描边：暗/亮底都能读 */
        -webkit-text-stroke: 0.55px rgba(8, 28, 52, 0.92);
        paint-order: stroke fill;
        text-shadow:
          0 0 2px rgba(8, 28, 52, 0.75),
          0 1px 1px rgba(8, 28, 52, 0.55);
      }
      .mei-rank-bar-value {
        flex: 0 0 auto;
        font-size: var(--cockpit-font-label);
        line-height: 1.2;
        font-weight: 700;
        color: #f8fafc;
        font-variant-numeric: tabular-nums;
        text-align: right;
        white-space: nowrap;
        -webkit-text-stroke: 0.55px rgba(8, 28, 52, 0.92);
        paint-order: stroke fill;
        text-shadow:
          0 0 2px rgba(8, 28, 52, 0.75),
          0 1px 1px rgba(8, 28, 52, 0.55);
      }
      .error {
        flex: 0 0 auto;
        min-height: 0;
        font-size: var(--cockpit-font-unit);
        color: #fca5a5;
      }
      .error:empty { display: none; }
      .carousel-hint-host {
        flex: 0 0 auto;
        display: flex;
        justify-content: flex-end;
        align-items: center;
        min-height: 0;
        padding: 0 2px;
      }
      .carousel-hint-host[hidden] { display: none !important; }
      .carousel-hint {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        padding: 1px 2px;
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
        background: #38bdf8;
        transform: scale(1.4);
        box-shadow: 0 0 8px rgba(56, 189, 248, 0.5);
        cursor: default;
      }
      .carousel-page-label {
        display: inline-flex;
        align-items: baseline;
        gap: 2px;
        font-size: var(--cockpit-font-unit);
        color: #94a3b8;
        font-variant-numeric: tabular-nums;
      }
      .carousel-page-current {
        display: inline-block;
        min-width: 0.65em;
        text-align: center;
        color: #e2e8f0;
        font-weight: 600;
        animation: chart-carousel-page-bump 380ms cubic-bezier(0.34, 1.4, 0.64, 1);
      }
      .carousel-page-sep { opacity: 0.55; padding: 0 1px; }
      .carousel-page-total { color: #cbd5e1; font-weight: 500; }
      @keyframes chart-carousel-page-bump {
        0% { transform: scale(0.82); opacity: 0.55; }
        55% { transform: scale(1.14); opacity: 1; }
        100% { transform: scale(1); opacity: 1; }
      }
      .carousel-timer {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 18px;
        height: 18px;
        flex: 0 0 auto;
      }
      .carousel-ring { display: block; }
      .carousel-ring-track {
        fill: none;
        stroke: rgba(125, 211, 252, 0.16);
        stroke-width: 2;
      }
      .carousel-ring-progress {
        fill: none;
        stroke: #38bdf8;
        stroke-width: 2;
        stroke-linecap: round;
        transform: rotate(-90deg);
        transform-origin: 50% 50%;
        stroke-dasharray: var(--carousel-c);
        stroke-dashoffset: 0;
        animation: chart-carousel-ring-countdown var(--carousel-ms) linear forwards;
      }
      .wrap.carousel-paused .carousel-ring-progress {
        animation-play-state: paused;
      }
      @keyframes chart-carousel-ring-countdown {
        from { stroke-dashoffset: 0; }
        to { stroke-dashoffset: var(--carousel-c); }
      }
    </style>
    <section class="wrap">
      <div class="head">
        <h4 class="title">${escapeHtml(String(props.title ?? defaultTitle))}</h4>
        <span class="meta"></span>
      </div>
      <div class="chart"></div>
      <div class="carousel-hint-host" hidden></div>
      <div class="error"></div>
    </section>
  `;
}

async function ensureECharts() {
  return ensureEChartsGlobal();
}

function resolveTopN(props) {
  const raw = props?.top_n ?? props?.topN ?? props?.composition_top_n ?? props?.compositionTopN;
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : 0;
}

function limitCartesianRowsByTopY(rows, mapping, topN) {
  const limit = Number(topN);
  if (!Number.isFinite(limit) || limit <= 0 || !Array.isArray(rows) || rows.length === 0) {
    return rows;
  }
  const xField = mapping.x[0]?.field;
  const yFields = mapping.y.map((item) => item?.field).filter(Boolean);
  if (!xField || yFields.length === 0) {
    return rows;
  }
  const ranked = new Map();
  rows.forEach((row) => {
    const label = String(row?.[xField] ?? "").trim();
    if (!label) return;
    const total = yFields.reduce((sum, field) => {
      const value = toNumber(row?.[field]);
      return Number.isFinite(value) ? sum + value : sum;
    }, 0);
    if (!Number.isFinite(total)) return;
    ranked.set(label, (ranked.get(label) || 0) + total);
  });
  const keep = new Set(
    Array.from(ranked.entries())
      .sort((left, right) => right[1] - left[1])
      .slice(0, limit)
      .map(([label]) => label),
  );
  const categoryOrder = Array.from(ranked.entries())
    .sort((left, right) => right[1] - left[1])
    .slice(0, limit)
    .map(([label]) => label);
  const filtered = rows.filter((row) => keep.has(String(row?.[xField] ?? "").trim()));
  return reorderRowsByCategoryOrder(filtered, xField, categoryOrder);
}

function orderCartesianCategories(rows, mapping, yFields) {
  const xField = mapping.x[0]?.field;
  if (!xField || !Array.isArray(rows) || rows.length === 0) return [];
  const fields =
    Array.isArray(yFields) && yFields.length > 0
      ? yFields
      : mapping.y.map((item) => item?.field).filter(Boolean);
  if (fields.length === 0) return [];
  const totals = new Map();
  rows.forEach((row) => {
    const label = String(row?.[xField] ?? "").trim();
    if (!label) return;
    const total = fields.reduce((sum, field) => {
      const value = toNumber(row?.[field]);
      return Number.isFinite(value) ? sum + value : sum;
    }, 0);
    totals.set(label, (totals.get(label) || 0) + total);
  });
  return Array.from(totals.entries())
    .sort((left, right) => right[1] - left[1])
    .map(([label]) => label);
}

/** 办理状态等生命周期维：固定 X 轴顺序，表达连续状态（缺类目补 0）。 */
const HANDLING_STATUS_CATEGORY_ORDER = ["待办", "在办", "办结"];

function resolveFixedCategoryOrder(props, xField) {
  const field = String(xField || "").trim();
  const fromProps = props?.category_order ?? props?.categoryOrder;
  if (Array.isArray(fromProps) && fromProps.length > 0) {
    return fromProps.map((item) => String(item || "").trim()).filter(Boolean);
  }
  if (field === "办理状态" || field === "handlingStatus" || field === "status") {
    return HANDLING_STATUS_CATEGORY_ORDER.slice();
  }
  return null;
}

function reorderRowsByCategoryOrder(rows, xField, categoryOrder) {
  if (!xField || !Array.isArray(categoryOrder) || categoryOrder.length === 0) {
    return rows;
  }
  const rank = new Map(categoryOrder.map((label, index) => [label, index]));
  return [...rows].sort((left, right) => {
    const leftRank = rank.get(String(left?.[xField] ?? "").trim());
    const rightRank = rank.get(String(right?.[xField] ?? "").trim());
    if (leftRank === undefined && rightRank === undefined) return 0;
    if (leftRank === undefined) return 1;
    if (rightRank === undefined) return -1;
    return leftRank - rightRank;
  });
}

function reconcileCartesianMapping(rows, mapping) {
  const xField = mapping?.x?.[0]?.field;
  if (!Array.isArray(rows) || rows.length === 0 || !xField) {
    return mapping;
  }
  const hasXValues = rows.some((row) => {
    const value = row?.[xField];
    return value !== undefined && value !== null && String(value).trim() !== "";
  });
  if (hasXValues) {
    return mapping;
  }
  const sample = rows[0] && typeof rows[0] === "object" ? rows[0] : {};
  const fallback = Object.keys(sample).find((key) => key !== "value" && key !== xField);
  if (!fallback) {
    return mapping;
  }
  return {
    ...mapping,
    x: [{ field: fallback, name: fallback }],
    label:
      Array.isArray(mapping.label) && mapping.label.length > 0
        ? mapping.label
        : [{ field: fallback, name: fallback }],
  };
}

function buildChartModel(kind, props, diagnostics) {
  const rows = resolveRows(props);
  const columns = resolveColumns(props, rows);
  const normalized = normalizeKind(kind);
  const baseMapping = resolveMapping(props, columns);
  const mapping =
    PIE_KINDS.has(normalized) || normalized === "radar" || normalized === "boxplot"
      ? baseMapping
      : reconcileCartesianMapping(rows, baseMapping);
  const legacy = Object.assign(resolveLegacyBehavior(props), { __host: props.__host });
  const topN = resolveTopN(props);
  const fixedCategoryOrder = resolveFixedCategoryOrder(props, mapping?.x?.[0]?.field);
  const chartRows =
    topN > 0 && normalized === "column" && !fixedCategoryOrder
      ? limitCartesianRowsByTopY(rows, mapping, topN)
      : fixedCategoryOrder && mapping?.x?.[0]?.field
        ? reorderRowsByCategoryOrder(rows, mapping.x[0].field, fixedCategoryOrder)
        : rows;
  if (topN > 0 && normalized === "column" && !fixedCategoryOrder) {
    legacy.sortCategoriesByYTotal = true;
  }
  if (fixedCategoryOrder) {
    legacy.sortCategoriesByYTotal = false;
    legacy.fixedCategoryOrder = fixedCategoryOrder;
  }
  if (normalized === "ranking" || normalized === "ranking-bar") {
    const layout =
      normalized === "ranking-bar" ? "bar" : resolveRankingLayout(props);
    const compact = props.compact === true || props.compact === "true";
    if (layout === "above" || layout === "bar") {
      const { items, valueName } = buildRankingItems(chartRows, mapping, diagnostics);
      const configuredMaxChars = resolveRankingLabelMaxChars(props, layout);
      const maxChars = configuredMaxChars > 0 ? configuredMaxChars : layout === "bar" ? 16 : 20;
      return {
        kind: normalized,
        layout,
        rows: chartRows,
        mapping,
        items,
        valueName,
        theme: resolveRankingTheme(props),
        maxChars,
        meta: compact
          ? ""
          : layout === "bar"
            ? `水平排名 ${items.length} 项 · 横条底图（悬停/点击查看全文）`
            : `排名 ${items.length} 项 · 标签置顶（点击查看全文）`,
        fullLabels: items.map((item) => item.label),
        rowCount: items.length,
      };
    }
    const ranking = buildRankingSideOption(chartRows, mapping, props, diagnostics);
    return {
      kind: normalized,
      layout: "side",
      rows: chartRows,
      mapping,
      option: ranking.option,
      meta: compact ? "" : ranking.meta,
      fullLabels: ranking.fullLabels,
      rowCount: ranking.rowCount,
    };
  }
  const option = buildOption(kind, chartRows, mapping, legacy, diagnostics);
  const compact = props.compact === true || props.compact === "true";
  const dimensionCount =
    normalized === "radar"
      ? (mapping.radarDimensions?.length || mapping.y?.length || 0)
      : 0;
  // compact 驾驶舱不展示「label -> 项目数」这类调试 meta
  const meta = compact
    ? ""
    : normalized === "radar"
      ? `${mapping.label?.[0]?.name || mapping.label?.[0]?.field || "series"} · ${dimensionCount} dims · ${chartRows.length} rows`
      : `${mapping.titleLeft} -> ${mapping.titleRight}`;
  return {
    kind: normalized,
    rows: chartRows,
    mapping,
    option,
    meta,
  };
}

function firstNonEmptyString(...values) {
  for (const value of values) {
    const text = String(value || "").trim();
    if (text) return text;
  }
  return "";
}

function resolveChartSelectionContext(kind, props, model) {
  const normalizedKind = normalizeKind(kind);
  const queryStateId = queryStateIdOf(props);
  if (!queryStateId) return null;
  const mapping = model?.mapping || {};
  const selectionDimension = firstNonEmptyString(
    props.selection_dimension,
    props.selectionDimension,
    normalizedKind === "pie" || normalizedKind === "donut" || normalizedKind === "rose"
      ? mapping.label?.[0]?.field || mapping.x?.[0]?.field
      : normalizedKind === "radar" || normalizedKind === "boxplot" || normalizedKind === "ranking"
        ? mapping.label?.[0]?.field || mapping.x?.[0]?.field
        : normalizedKind === "scatter"
          ? mapping.label?.[0]?.field
          : mapping.x?.[0]?.field
  );
  if (!selectionDimension) return null;
  const filterEncode = firstNonEmptyString(
    props.selection_filter_encode,
    props.selectionFilterEncode,
    // 风险/预警等级多标签：点击扇区按「包含该等级」写入筛选，而非精确组合值
    selectionDimension === "风险等级" || selectionDimension === "预警等级"
      ? "contains_any"
      : "",
  );
  return {
    queryStateId,
    dimension: selectionDimension,
    rows: Array.isArray(model?.rows) ? model.rows : [],
    mapping,
    toggle: props.selection_toggle !== false && props.selectionToggle !== false,
    filterEncode: filterEncode || undefined,
  };
}

function resolveChartSelectionValue(kind, params, selection) {
  const normalizedKind = normalizeKind(kind);
  if (normalizedKind === "scatter") {
    const labelField = selection?.mapping?.label?.[0]?.field;
    const row = Number.isFinite(Number(params?.dataIndex)) ? selection?.rows?.[params.dataIndex] : null;
    return firstNonEmptyString(labelField ? row?.[labelField] : "", params?.name);
  }
  if (normalizedKind === "pie" || normalizedKind === "donut" || normalizedKind === "rose") {
    return firstNonEmptyString(params?.name, params?.data?.name);
  }
  if (normalizedKind === "radar" || normalizedKind === "boxplot") {
    return firstNonEmptyString(params?.name, params?.axisValueLabel, params?.axisValue);
  }
  return firstNonEmptyString(params?.name, params?.axisValueLabel, params?.axisValue, params?.data?.name);
}

const CHART_CAROUSEL_RING_RADIUS = 8;
const CHART_CAROUSEL_RING_C = 2 * Math.PI * CHART_CAROUSEL_RING_RADIUS;

function chartCarouselEnabled(props) {
  return props?.carousel === true || props?.carousel === "true";
}

function chartCarouselShowsHint(props) {
  if (props?.carouselHint === false || props?.carousel_hint === "false") return false;
  if (!chartCarouselEnabled(props)) return false;
  if (props?.carouselHint === true || props?.carousel_hint === "true") return true;
  return true;
}

function resolveChartCarouselIntervalMs(props) {
  const raw = Number(props?.carouselIntervalMs ?? props?.carousel_interval_ms ?? 5000);
  if (!Number.isFinite(raw)) return 5000;
  return Math.max(2000, Math.floor(raw));
}

function resolveChartCarouselByField(props) {
  return String(props?.carouselBy ?? props?.carousel_by ?? "").trim();
}

function resolveChartCarouselTopN(props) {
  const n = Number(props?.carouselTopN ?? props?.carousel_top_n ?? 0);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : 0;
}

function buildChartCarouselSlides(rows, byField, valueField, topN) {
  const totals = new Map();
  for (const row of Array.isArray(rows) ? rows : []) {
    const key = String(row?.[byField] ?? "").trim();
    if (!key) continue;
    const amount = toNumber(row?.[valueField]);
    const prev = totals.get(key) || 0;
    totals.set(key, prev + (Number.isFinite(amount) ? amount : 0));
  }
  let keys = [...totals.keys()].sort((a, b) => (totals.get(b) || 0) - (totals.get(a) || 0));
  if (topN > 0) keys = keys.slice(0, topN);
  return keys;
}

function withChartCarouselRows(props, filteredRows) {
  const next = Object.assign({}, props);
  if (props.data && typeof props.data === "object") {
    next.data = Object.assign({}, props.data, { rows: filteredRows });
  } else {
    next.data = { rows: filteredRows };
  }
  if (props.value && typeof props.value === "object") {
    if (Array.isArray(props.value.rows)) {
      next.value = Object.assign({}, props.value, { rows: filteredRows });
    } else if (
      props.value.shape === "dataframe" &&
      props.value.value &&
      typeof props.value.value === "object" &&
      Array.isArray(props.value.value.rows)
    ) {
      next.value = Object.assign({}, props.value, {
        value: Object.assign({}, props.value.value, { rows: filteredRows }),
      });
    }
  }
  return next;
}

function applyChartCarouselFilter(host, props) {
  if (!chartCarouselEnabled(props)) {
    host._carouselSlides = null;
    return props;
  }
  const byField = resolveChartCarouselByField(props);
  if (!byField) {
    host._carouselSlides = null;
    return props;
  }
  const rows = resolveRows(props);
  const valueField =
    props?.mapping?.y?.[0]?.field ||
    props?.mapping?.y?.[0]?.name ||
    "value";
  const topN = resolveChartCarouselTopN(props);
  const slidesSignature = JSON.stringify({
    byField,
    valueField,
    topN,
    keys: buildChartCarouselSlides(rows, byField, valueField, 0),
  });
  const slides = buildChartCarouselSlides(rows, byField, valueField, topN);
  host._carouselSlides = slides;
  if (slidesSignature !== host._carouselSlidesSignature) {
    host._carouselSlidesSignature = slidesSignature;
    host._carouselIndex = 0;
    host._carouselEpoch = (Number(host._carouselEpoch) || 0) + 1;
  }
  if (slides.length === 0) {
    return Object.assign({}, props, { __carouselActive: true });
  }
  let index = Number(host._carouselIndex) || 0;
  if (index < 0 || index >= slides.length) {
    index = 0;
    host._carouselIndex = 0;
  }
  const current = slides[index];
  const filtered = rows.filter((row) => String(row?.[byField] ?? "").trim() === current);
  const titled = Object.assign(withChartCarouselRows(props, filtered), {
    title: String(props.title || "").trim() || current,
    __carouselActive: true,
    __carouselCurrent: current,
  });
  return titled;
}

function renderChartCarouselHintHtml(page, totalPages, intervalMs, epoch) {
  const dots = Array.from({ length: totalPages }, (_, index) => {
    const pageNo = index + 1;
    const active = pageNo === page;
    return `<button type="button" class="carousel-dot${active ? " is-active" : ""}" data-carousel-page="${pageNo}" aria-label="切换到第 ${pageNo} 项" aria-current="${active ? "true" : "false"}"></button>`;
  }).join("");
  return `
    <div class="carousel-hint" role="navigation" aria-label="轮播第 ${page} 项，共 ${totalPages} 项">
      <div class="carousel-dots">${dots}</div>
      <span class="carousel-page-label">
        <span class="carousel-page-current" data-epoch="${epoch}">${page}</span><span class="carousel-page-sep">/</span><span class="carousel-page-total">${totalPages}</span>
      </span>
      <div class="carousel-timer" style="--carousel-ms:${intervalMs}ms;--carousel-c:${CHART_CAROUSEL_RING_C}" data-epoch="${epoch}" title="自动切换倒计时">
        <svg class="carousel-ring" viewBox="0 0 20 20" width="16" height="16" aria-hidden="true">
          <circle class="carousel-ring-track" cx="10" cy="10" r="${CHART_CAROUSEL_RING_RADIUS}" />
          <circle class="carousel-ring-progress" cx="10" cy="10" r="${CHART_CAROUSEL_RING_RADIUS}" />
        </svg>
      </div>
    </div>`;
}

function resolveRows(props) {
  const candidates = [props.data, props.value];
  for (const source of candidates) {
    if (!source || typeof source !== "object") continue;
    if (Array.isArray(source.rows)) return source.rows;
    if (Array.isArray(source.value)) return source.value;
    if (
      source.shape === "dataframe" &&
      source.value &&
      typeof source.value === "object" &&
      Array.isArray(source.value.rows)
    ) {
      return source.value.rows;
    }
    if (source.shape === "scalar" && source.value && typeof source.value === "object") {
      return Object.entries(source.value).map(([label, value]) => ({
        label,
        value,
      }));
    }
  }
  const dataset = props.dataset?.dataset || props.dataset || {};
  if (Array.isArray(dataset.rows)) {
    return dataset.rows;
  }
  if (isStaticSkeletonDisplay(props)) {
    return buildStaticChartRows(props.mapping);
  }
  return [];
}

function resolveColumns(props, rows) {
  const candidates = [props.data, props.value, props.dataset];
  for (const source of candidates) {
    if (!source || typeof source !== "object") continue;
    if (Array.isArray(source.schema) && source.schema.length > 0) {
      return source.schema.map((col) => col?.name).filter(Boolean);
    }
    if (Array.isArray(source.columns) && source.columns.length > 0) {
      return source.columns;
    }
    if (source.dataset && Array.isArray(source.dataset.columns)) {
      return source.dataset.columns;
    }
  }
  if (rows.length > 0 && rows[0] && typeof rows[0] === "object") {
    return Object.keys(rows[0]);
  }
  return ["label", "value"];
}

function resolveMapping(props, columns) {
  const mapping = props.mapping || {};
  const x = channelList(mapping.x, props.labelField, columns[0] || "label");
  let y = channelList(mapping.y, props.valueField, columns[1] || "value");
  if (y.length === 0 && Array.isArray(props.metrics) && props.metrics.length > 0) {
    y = props.metrics
      .map((field) => String(field || "").trim())
      .filter(Boolean)
      .map((field) => ({ field, name: field }));
  }
  // 客户端重聚合常只有 x=[{field:"label", name:"风险等级"}]；合成 label 时必须继承 name，
  // 否则预警色板检测会把 name 当成 "label" 而回退到分类色板。
  let label = channelList(mapping.label, null, "");
  if (label.length === 0 && x[0]) {
    label = [{ field: x[0].field, name: x[0].name || x[0].field }];
  } else if (label.length === 0) {
    label = channelList(null, null, x[0]?.field || "label");
  }
  const group = channelList(mapping.group, null, "");
  const color = channelList(mapping.color, null, "");
  const shape = channelList(mapping.shape, null, "");
  const size = channelList(mapping.size, null, "");
  let boxplot = channelList(mapping.boxplot, null, "");
  if (boxplot.length === 0 && Array.isArray(props.boxplot)) {
    boxplot = props.boxplot
      .map((field) => String(field || "").trim())
      .filter(Boolean)
      .map((field) => ({ field, name: field }));
  }
  const radarDimensions = Array.isArray(props.dimensions)
    ? props.dimensions.map((field) => ({ field, name: field }))
    : [];
  return {
    x,
    y,
    label,
    group,
    color,
    shape,
    size,
    boxplot,
    radarDimensions,
    titleLeft: x[0]?.name || label[0]?.name || "x",
    titleRight: y[0]?.name || "y",
  };
}

function hexToRgba(hex, alpha) {
  const raw = String(hex || "").trim();
  const m = /^#?([0-9a-f]{6})$/i.exec(raw);
  if (!m) return `rgba(16, 185, 129, ${alpha})`;
  const n = Number.parseInt(m[1], 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** palette[0]=最浅 … palette[n]=最深；按数值映射，越大越深。 */
function pickPaletteColorByValue(value, min, max, palette) {
  const colors = Array.isArray(palette) ? palette.filter(Boolean) : [];
  if (!colors.length) return undefined;
  const n = Number(value);
  if (!Number.isFinite(n)) return colors[0];
  if (!(max > min)) return colors[colors.length - 1];
  const t = Math.min(1, Math.max(0, (n - min) / (max - min)));
  const idx = Math.min(colors.length - 1, Math.round(t * (colors.length - 1)));
  return colors[idx];
}

function valueExtentFromSeriesData(data) {
  const nums = (Array.isArray(data) ? data : [])
    .map((entry) => {
      if (entry && typeof entry === "object" && !Array.isArray(entry)) {
        return Number(entry.value);
      }
      return Number(entry);
    })
    .filter((n) => Number.isFinite(n));
  if (!nums.length) return null;
  return { min: Math.min(...nums), max: Math.max(...nums) };
}

/** 柱/条：按数值上色（越大越深）。 */
function colorizeBarDataByValue(data, palette) {
  const colors = Array.isArray(palette) ? palette.filter(Boolean) : [];
  if (colors.length < 2 || !Array.isArray(data) || data.length === 0) return data;
  const extent = valueExtentFromSeriesData(data);
  if (!extent) return data;
  return data.map((entry) => {
    const value = entry && typeof entry === "object" && !Array.isArray(entry) ? entry.value : entry;
    const color = pickPaletteColorByValue(value, extent.min, extent.max, colors);
    if (entry && typeof entry === "object" && !Array.isArray(entry)) {
      return {
        ...entry,
        itemStyle: { ...(entry.itemStyle || {}), color },
      };
    }
    return { value, itemStyle: { color } };
  });
}

/** 饼/玫瑰：按扇区数值上色（越大越深）；环内百分比随扇区底色自适应字色+描边。 */
function colorizePieDataByValue(data, palette) {
  const colors = Array.isArray(palette) ? palette.filter(Boolean) : [];
  if (colors.length < 2 || !Array.isArray(data) || data.length === 0) return data;
  const extent = valueExtentFromSeriesData(data);
  if (!extent) return data;
  return data.map((entry) => {
    const color = pickPaletteColorByValue(entry?.value, extent.min, extent.max, colors);
    const labelStyle = echartsLabelOnFill(color);
    return {
      ...entry,
      itemStyle: { ...(entry?.itemStyle || {}), color },
      label: {
        ...(entry?.label && typeof entry.label === "object" ? entry.label : {}),
        ...labelStyle,
      },
    };
  });
}

/** 饼/环/玫瑰：按类目序轮转色板（同类占比不再撞成同色）。 */
function colorizePieDataByCategory(data, palette) {
  const colors = Array.isArray(palette) ? palette.filter(Boolean) : [];
  if (colors.length === 0 || !Array.isArray(data) || data.length === 0) return data;
  return data.map((entry, index) => {
    const color = colors[index % colors.length];
    const labelStyle = echartsLabelOnFill(color);
    return {
      ...entry,
      itemStyle: { ...(entry?.itemStyle || {}), color },
      label: {
        ...(entry?.label && typeof entry.label === "object" ? entry.label : {}),
        ...labelStyle,
      },
    };
  });
}

/** 风险/预警等级：扇区按业务色（多色取最高严重度；空值用灰）。 */
function colorizePieDataByWarningLevel(data, host) {
  if (!Array.isArray(data) || data.length === 0) return data;
  const colors = readWarningLevelColors(host);
  return data.map((entry) => {
    const color = resolveWarningLevelSliceColor(entry?.name, colors);
    const labelStyle = echartsLabelOnFill(color);
    return {
      ...entry,
      itemStyle: { ...(entry?.itemStyle || {}), color },
      // 黄扇区等浅色底上用深色字 + 反色描边，避免环内百分比看不清
      label: {
        ...(entry?.label && typeof entry.label === "object" ? entry.label : {}),
        ...labelStyle,
      },
    };
  });
}

/** 相对亮度（sRGB → 线性），失败时返回 null。 */
function relativeLuminance(color) {
  const rgb = parseCssColorToRgb(color);
  if (!rgb) return null;
  const toLinear = (c) => {
    const n = c / 255;
    return n <= 0.03928 ? n / 12.92 : ((n + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * toLinear(rgb[0]) + 0.7152 * toLinear(rgb[1]) + 0.0722 * toLinear(rgb[2]);
}

/** 亮色底用深字，暗色底用浅字（环内百分比对比度）。 */
function contrastingForegroundOnColor(color) {
  const luminance = relativeLuminance(color);
  if (luminance == null) return "#f8fafc";
  return luminance > 0.55 ? "#0f172a" : "#f8fafc";
}

/**
 * ECharts canvas 文字描边：浅字配深描边、深字配浅描边。
 * width=2 + 高不透明度，确保环内百分比在深/浅扇区上都可见。
 */
function echartsTextStrokeStyle(foreground) {
  const luminance = relativeLuminance(foreground);
  const isLightFg =
    luminance != null
      ? luminance > 0.55
      : (() => {
          const fg = String(foreground || "")
            .trim()
            .toLowerCase();
          return (
            fg === "#f8fafc" ||
            fg === "#fff" ||
            fg === "#ffffff" ||
            fg === "#e2e8f0" ||
            fg.includes("255")
          );
        })();
  return {
    textBorderColor: isLightFg ? "rgba(8, 28, 52, 0.88)" : "rgba(255, 255, 255, 0.88)",
    textBorderWidth: 2,
  };
}

/** 扇区填充色上的环内标签：自适应字色 + 反色描边。 */
function echartsLabelOnFill(fillColor) {
  const color = contrastingForegroundOnColor(fillColor);
  return {
    color,
    fontWeight: 600,
    ...echartsTextStrokeStyle(color),
  };
}

function parseCssColorToRgb(color) {
  const text = String(color || "").trim();
  if (!text) return null;
  const hex = text.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i);
  if (hex) {
    let h = hex[1];
    if (h.length === 3) h = h.split("").map((ch) => ch + ch).join("");
    return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
  }
  const rgb = text.match(/^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)/i);
  if (rgb) return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])];
  return null;
}

function pieDonutTooltipFormatter(params) {
  const item = Array.isArray(params) ? params[0] : params;
  const name = String(item?.name ?? "").trim() || "-";
  const valueRaw = item?.value;
  const value = Number(valueRaw);
  const valueText = Number.isFinite(value) ? String(value) : String(valueRaw ?? "-");
  const percent = Number(item?.percent);
  const percentText = Number.isFinite(percent)
    ? `${percent.toFixed(2).replace(/\.?0+$/, "")}%`
    : "-";
  return `${name}<br/>数值：${valueText}<br/>占比：${percentText}`;
}

function usesWarningLevelPalette(props, mapping = {}) {
  const mode = String(props?.palette_mode ?? props?.paletteMode ?? "")
    .trim()
    .toLowerCase()
    .replace(/-/g, "_");
  if (mode === "warning_level") return true;
  // 客户端重聚合后 mapping.x.field 常为 "label"，需同时看 name（如「风险等级」）。
  // 优先检查 x/label 的 name，避免合成 label 通道把 name 冲成 "label"。
  const candidates = [
    mapping?.x?.[0]?.name,
    mapping?.label?.[0]?.name,
    mapping?.x?.[0]?.field,
    mapping?.label?.[0]?.field,
  ];
  return candidates.some((value) => isWarningLevelDimension(value));
}

/** palette_mode=value|mono：按数值映射单色阶梯；其余（默认/category）按类目轮转分类色板。 */
function usesValueRampPiePalette(props) {
  const mode = String(props?.palette_mode ?? props?.paletteMode ?? "")
    .trim()
    .toLowerCase()
    .replace(/-/g, "_");
  if (
    mode === "category" ||
    mode === "categorical" ||
    mode === "categories" ||
    mode === "cat"
  ) {
    return false;
  }
  return mode === "value" || mode === "mono" || mode === "monochrome";
}

function resolveExplicitColorPalette(props) {
  const raw = props?.palette ?? props?.color_palette ?? props?.colors;
  if (Array.isArray(raw)) {
    const fromProps = raw.map((item) => String(item || "").trim()).filter(Boolean);
    if (fromProps.length > 0) return fromProps;
  } else if (typeof raw === "string" && raw.trim()) {
    const fromProps = raw
      .split(",")
      .map((item) => String(item || "").trim())
      .filter(Boolean);
    if (fromProps.length > 0) return fromProps;
  }
  return null;
}

function resolvePieColorPalette(props) {
  const explicit = resolveExplicitColorPalette(props);
  if (explicit) return explicit;
  if (usesValueRampPiePalette(props)) {
    return readThemeChartPalette(props?.__host);
  }
  return readThemeChartCategoricalPalette(props?.__host);
}

function metricSparkBarItemStyle(host) {
  const palette = readThemeChartPalette(host);
  const top = palette[0] || "#d1fae5";
  const bottom = palette[4] || "#10b981";
  return {
    borderRadius: [0, 0, 0, 0],
    color: {
      type: "linear",
      x: 0,
      y: 0,
      x2: 0,
      y2: 1,
      colorStops: [
        { offset: 0.31, color: top },
        { offset: 0.83, color: bottom },
      ],
    },
  };
}

/** 驾驶舱年度对比分组柱：沿用 theme chart 单色阶梯做两组深浅渐变 */
function cockpitYearDuoBarItemStyle(seriesIndex, { emphasis = false, host } = {}) {
  const palette = readThemeChartPalette(host);
  const pale = palette[0] || "#d1fae5";
  const light = palette[1] || "#a7f3d0";
  const mid = palette[3] || "#34d399";
  const deep = palette[4] || "#10b981";
  const darker = palette[5] || "#059669";
  const presets = [
    {
      color: {
        type: "linear",
        x: 0,
        y: 0,
        x2: 0,
        y2: 1,
        colorStops: [
          { offset: 0, color: emphasis ? light : mid },
          { offset: 0.38, color: emphasis ? mid : deep },
          { offset: 1, color: emphasis ? deep : darker },
        ],
      },
      borderRadius: [0, 0, 0, 0],
      shadowBlur: emphasis ? 14 : 9,
      shadowColor: hexToRgba(deep, 0.45),
      shadowOffsetY: 1,
    },
    {
      color: {
        type: "linear",
        x: 0,
        y: 0,
        x2: 0,
        y2: 1,
        colorStops: [
          { offset: 0, color: emphasis ? "#FFFFFF" : pale },
          { offset: 0.35, color: emphasis ? pale : light },
          { offset: 1, color: emphasis ? light : mid },
        ],
      },
      borderRadius: [0, 0, 0, 0],
      shadowBlur: emphasis ? 16 : 11,
      shadowColor: hexToRgba(mid, 0.4),
      shadowOffsetY: 1,
    },
  ];
  return presets[seriesIndex % presets.length];
}

function resolveColorPalette(props) {
  const explicit = resolveExplicitColorPalette(props);
  if (explicit) return explicit;
  // Default: scene theme chart_1..chart_6 (app/workspace configurable monochrome ramp).
  return readThemeChartPalette(props?.__host);
}

/** 驾驶舱深色 tooltip 底 + 高对比文字（避免灰字落在 ECharts 默认浅黄/白底上） */
const ECHARTS_TOOLTIP_CHROME = {
  backgroundColor: "rgba(8, 24, 48, 0.94)",
  borderColor: "rgba(56, 189, 248, 0.45)",
  borderWidth: 1,
  padding: [8, 12],
};

const ECHARTS_TOOLTIP_TEXT_PRIMARY = "#f0f9ff";

/** ECharts/canvas 无法解析 CSS var，须从宿主 computed style 取实色。 */
function canvasThemeColor(host, token) {
  return readThemeColor(host, token);
}

/** 驾驶舱紧凑柱/线图：绘图区透明，背景交给外层 section/panel theme。 */
const COCKPIT_CARTESIAN_GRID_BG = "transparent";
const COCKPIT_CARTESIAN_SPLIT_LINE = {
  show: true,
  lineStyle: {
    color: "rgba(148, 163, 184, 0.24)",
    width: 1,
  },
};

function echartsTooltipTextStyle(typography, role = "label", host) {
  const fontSize =
    role === "value"
      ? typography.value
      : role === "unit"
        ? typography.unit
        : role === "body"
          ? typography.body
          : typography.label;
  const secondaryColor =
    role === "body" || role === "muted"
      ? canvasThemeColor(host, "text_body")
      : canvasThemeColor(host, "text_highlight");
  return {
    fontSize,
    color:
      role === "unit" || role === "muted"
        ? secondaryColor
        : ECHARTS_TOOLTIP_TEXT_PRIMARY,
    lineHeight: Math.round(fontSize * 1.45),
  };
}

/**
 * ECharts 飘窗挂载点。
 * 勿挂到带 CSS scale 的 stage context-plane：ECharts 按屏幕坐标定位，
 * 挂在缩放容器内会错位，并被 preview-stage-shell 的 overflow:hidden 裁切
 *（左栏环图左侧 hover 时只露出半截飘窗）。
 */
function resolveEchartsTooltipAppendTo(_host) {
  if (typeof document === "undefined") {
    return undefined;
  }
  return document.body;
}

function echartsTooltip(typography, trigger, extra = {}, host) {
  const { textRole = "label", ...rest } = extra;
  const tipZ =
    typeof window !== "undefined" &&
    typeof window.__meiLangBoot?.resolveRuntimeOverlayZIndex === "function"
      ? window.__meiLangBoot.resolveRuntimeOverlayZIndex("tooltip", host)
      : 1300;
  return {
    trigger,
    ...ECHARTS_TOOLTIP_CHROME,
    className: "mei-cockpit-echarts-tooltip",
    appendToBody: true,
    appendTo: resolveEchartsTooltipAppendTo(host),
    // 贴边时把飘窗收进图表可视区，避免再跑出左栏
    confine: true,
    extraCssText:
      `box-shadow:0 8px 24px rgba(0,0,0,0.35);border-radius:0;z-index:${tipZ};`,
    textStyle: echartsTooltipTextStyle(typography, textRole, host),
    ...rest,
  };
}

function resolveLegacyBehavior(props) {
  const compact = props.compact === true || props.compact === "true";
  const variant = String(props.variant ?? "").trim().toLowerCase();
  const barGradient = String(props.barGradient ?? props.bar_gradient ?? "")
    .trim()
    .toLowerCase();
  return {
    stack: !!props.stack,
    percent: props.transform?.mode === "percent",
    dataZoom: props.dataZoom !== false,
    metrics: Array.isArray(props.metrics) ? props.metrics : [],
    compact,
    chartHeight: Number(props.chartHeight) > 0 ? Number(props.chartHeight) : 0,
    barGradient,
    palette: resolveColorPalette(props),
    barLine: variant === "bar-line" || props.barLine === true || props.bar_line === true,
    showLegend:
      props.showLegend !== false &&
      props.showLegend !== "false" &&
      props.show_legend !== false &&
      props.show_legend !== "false",
    chartProps: props,
  };
}

function readLegendRight(props, fallback = 0) {
  const raw = props?.legendRight ?? props?.legend_right;
  if (raw === undefined || raw === null || raw === "") {
    return fallback;
  }
  const n = Number(raw);
  return Number.isFinite(n) ? n : fallback;
}

function readLegendLeft(props, fallback = 0) {
  const raw = props?.legendLeft ?? props?.legend_left;
  if (raw === undefined || raw === null || raw === "") {
    return fallback;
  }
  const n = Number(raw);
  return Number.isFinite(n) ? n : fallback;
}

/** 图例水平位置：left | center | right（默认 right，兼容旧 legendRight）。 */
function resolveLegendAlign(props) {
  const align = String(props?.legendAlign ?? props?.legend_align ?? "")
    .trim()
    .toLowerCase();
  if (align === "left" || align === "center" || align === "right") return align;
  const rawLeft = props?.legendLeft ?? props?.legend_left;
  if (rawLeft !== undefined && rawLeft !== null && rawLeft !== "") return "left";
  return "right";
}

function buildLegendPosition(props) {
  const align = resolveLegendAlign(props);
  if (align === "center") {
    return { left: "center", right: "auto" };
  }
  if (align === "left") {
    return { left: readLegendLeft(props, 0), right: "auto" };
  }
  return { right: readLegendRight(props, 0), left: "auto" };
}

/** 分组系列图例仅显示 group 名（如 2024/2025），不拼 y 指标名。 */
function legendGroupOnlyEnabled(props) {
  const raw = props?.legendGroupOnly ?? props?.legend_group_only;
  return raw === true || raw === "true" || raw === 1 || raw === "1";
}

/**
 * 标题画在 ECharts 画布内（仅显式 titleInPlot）。
 * 默认 false：壳头在画布外左上，对齐园区罚金统计。
 */
function titleInPlotEnabled(props) {
  const raw = props?.titleInPlot ?? props?.title_in_plot;
  return raw === true || raw === "true" || raw === 1 || raw === "1";
}

function categoryAxisVisible(props) {
  const raw = props?.showCategoryAxis ?? props?.show_category_axis;
  if (raw === false || raw === "false" || raw === 0 || raw === "0") return false;
  if (raw === true || raw === "true" || raw === 1 || raw === "1") return true;
  return true;
}

/** tooltip confine：默认 true（收进图表）；false 时允许飘出图表但仍由 ECharts/视口约束。 */
function tooltipConfineEnabled(props) {
  const raw = props?.tooltipConfine ?? props?.tooltip_confine;
  if (raw === false || raw === "false" || raw === 0 || raw === "0") return false;
  if (raw === true || raw === "true" || raw === 1 || raw === "1") return true;
  return true;
}

function readGridInset(props, side, fallback) {
  const key = side.toLowerCase();
  const camel = `grid${key.charAt(0).toUpperCase()}${key.slice(1)}`;
  const snake = `grid_${key}`;
  const raw = props?.[camel] ?? props?.[snake];
  if (raw === undefined || raw === null || raw === "") {
    return fallback;
  }
  const n = Number(raw);
  return Number.isFinite(n) ? n : raw;
}

function gridContainLabelEnabled(props, fallback = true) {
  if (props?.gridContainLabel === false || props?.grid_contain_label === false) {
    return false;
  }
  if (props?.gridContainLabel === true || props?.grid_contain_label === true) {
    return true;
  }
  return fallback;
}

function compactAxisValueLabel(value) {
  const n = Number(value);
  if (!Number.isFinite(n)) {
    return String(value ?? "");
  }
  const abs = Math.abs(n);
  if (abs >= 100000000) {
    return `${(n / 100000000).toFixed(abs >= 1000000000 ? 0 : 1)}亿`;
  }
  if (abs >= 10000) {
    return `${(n / 10000).toFixed(abs % 10000 === 0 ? 0 : 1)}万`;
  }
  if (abs >= 1000) {
    return `${(n / 1000).toFixed(0)}k`;
  }
  return String(n);
}

function valueAxisIntegerEnabled(props) {
  const raw = props?.y_axis_integer ?? props?.yAxisInteger ?? props?.minInterval;
  return (
    raw === true ||
    raw === "true" ||
    raw === 1 ||
    raw === "1" ||
    Number(raw) === 1
  );
}

function compactIntegerAxisValueLabel(value) {
  const n = Number(value);
  if (!Number.isFinite(n)) {
    return String(value ?? "");
  }
  const abs = Math.abs(n);
  if (abs >= 100000000) {
    return `${(n / 100000000).toFixed(abs >= 1000000000 ? 0 : 1)}亿`;
  }
  if (abs >= 10000) {
    return `${(n / 10000).toFixed(abs % 10000 === 0 ? 0 : 1)}万`;
  }
  if (abs >= 1000) {
    return `${(n / 1000).toFixed(0)}k`;
  }
  return String(Math.round(n));
}

function resolveCompactCartesianGrid(props, legacy, categoryAxisRotate = 0) {
  const containLabel = gridContainLabelEnabled(props, true);
  const showLegend = legacy.showLegend === true;
  const chartHeight = Number(legacy.chartHeight) > 0 ? Number(legacy.chartHeight) : 0;
  // Tight cockpit slots (~70–90px): shrink legend/axis insets so x-axis stays visible.
  const tight = chartHeight > 0 && chartHeight <= 96;
  const hideCategory = !categoryAxisVisible(props);
  const bottomDefault = hideCategory
    ? tight
      ? 2
      : 4
    : Math.abs(categoryAxisRotate) >= 30
      ? tight
        ? 28
        : 40
      : tight
        ? 14
        : 22;
  const topDefault = showLegend || titleInPlotEnabled(props) ? (tight ? 14 : 18) : tight ? 2 : 4;
  return {
    left: readGridInset(props, "left", containLabel ? 2 : 24),
    right: readGridInset(props, "right", showLegend || titleInPlotEnabled(props) ? 2 : 6),
    top: readGridInset(props, "top", topDefault),
    bottom: readGridInset(props, "bottom", bottomDefault),
    containLabel: hideCategory ? false : containLabel,
    backgroundColor: COCKPIT_CARTESIAN_GRID_BG,
    borderWidth: 0,
  };
}

function resolveCategoryAxisLabelFormatter(props, extras = {}) {
  let maxChars = Number(props?.label_max_chars);
  // yyyy-mm 时间轴：至少保留 7 字符，避免被分析看板默认 label_max_chars=6 截成「2025-0...」
  if (extras.temporalYearMonth) {
    const floor = 7;
    maxChars = Number.isFinite(maxChars) && maxChars > 0 ? Math.max(maxChars, floor) : floor;
  }
  if (!Number.isFinite(maxChars) || maxChars <= 0) {
    return undefined;
  }
  return (value) => {
    const text = String(value ?? "").trim();
    if (text.length <= maxChars) {
      return text;
    }
    return `${text.slice(0, maxChars)}...`;
  };
}

function resolveCategoryAxisLabelRotate(props) {
  const raw =
    props?.axisLabelRotate ??
    props?.axis_label_rotate ??
    props?.categoryLabelRotate ??
    props?.category_label_rotate;
  const n = Number(raw);
  return Number.isFinite(n) ? n : 0;
}

function buildCategoryAxisLabel(chartProps, typography, extras = {}) {
  const formatter = resolveCategoryAxisLabelFormatter(chartProps, extras);
  const rotate = resolveCategoryAxisLabelRotate(chartProps);
  const host = chartProps?.__host;
  const color = canvasThemeColor(host, "text_muted");
  const label = {
    fontSize: typography.unit,
    color,
    interval: 0,
    ...echartsTextStrokeStyle(color),
  };
  if (formatter) {
    label.formatter = formatter;
  }
  if (rotate) {
    label.rotate = rotate;
  }
  return label;
}

/** YYYY-MM（bucket_date / 年月标签） */
function isYearMonthCategoryLabel(value) {
  return /^\d{4}-\d{2}$/.test(String(value ?? "").trim());
}

function categoriesLookLikeYearMonth(categories) {
  const labels = (Array.isArray(categories) ? categories : [])
    .map((item) => String(item ?? "").trim())
    .filter(Boolean);
  if (labels.length === 0) return false;
  const hits = labels.filter(isYearMonthCategoryLabel).length;
  return hits >= Math.max(1, Math.ceil(labels.length * 0.8));
}

function xFieldLooksTemporal(field) {
  const name = String(field || "").trim().toLowerCase();
  return (
    name === "年月" ||
    name === "月份" ||
    name === "month" ||
    name === "year_month" ||
    name === "year-month" ||
    name.endsWith("年月")
  );
}

/** MM / M（01–12）：年度对比柱的共享横轴。 */
function isMonthNumCategoryLabel(value) {
  return /^(0?[1-9]|1[0-2])$/.test(String(value ?? "").trim());
}

function categoriesLookLikeMonthNum(categories) {
  const labels = (Array.isArray(categories) ? categories : [])
    .map((item) => String(item ?? "").trim())
    .filter(Boolean);
  if (labels.length === 0) return false;
  const hits = labels.filter(isMonthNumCategoryLabel).length;
  return hits >= Math.max(1, Math.ceil(labels.length * 0.8));
}

/** 保留数据首次出现顺序（滚动窗口时间序），仅去重。 */
function uniquePreserveOrder(values) {
  const seen = new Set();
  const out = [];
  for (const raw of values) {
    const key = String(raw ?? "").trim();
    if (!key || seen.has(key)) continue;
    seen.add(key);
    out.push(key);
  }
  return out;
}

function sortCategoriesAsTime(categories) {
  return [...categories].sort((left, right) =>
    String(left ?? "").trim().localeCompare(String(right ?? "").trim(), "en"),
  );
}

function monthNumValue(label) {
  const n = Number.parseInt(String(label ?? "").trim(), 10);
  return Number.isFinite(n) && n >= 1 && n <= 12 ? n : NaN;
}

function sortCategoriesAsMonthNum(categories) {
  return [...categories].sort((left, right) => monthNumValue(left) - monthNumValue(right));
}

/** Wrap-aware chronological month sequence (e.g. 09,10,11,12,01,02). */
function isChronologicalMonthSequence(categories) {
  const nums = (Array.isArray(categories) ? categories : [])
    .map(monthNumValue)
    .filter((n) => Number.isFinite(n));
  if (nums.length < 2) return true;
  for (let i = 1; i < nums.length; i += 1) {
    const prev = nums[i - 1];
    const next = nums[i];
    const expected = prev === 12 ? 1 : prev + 1;
    if (next !== expected) return false;
  }
  return true;
}

function groupsLookLikeYears(groups) {
  const labels = (Array.isArray(groups) ? groups : [])
    .map((item) => String(item ?? "").trim())
    .filter(Boolean);
  if (labels.length === 0) return false;
  const hits = labels.filter((label) => /^\d{4}$/.test(label)).length;
  return hits >= Math.max(1, Math.ceil(labels.length * 0.8));
}

function sortGroupsAsYears(groups) {
  return [...groups].sort(
    (left, right) =>
      Number.parseInt(String(left ?? "").trim(), 10) -
      Number.parseInt(String(right ?? "").trim(), 10),
  );
}

function resolveChannelDisplayName(channels, field, fallback = "") {
  const channel = Array.isArray(channels)
    ? channels.find((item) => item?.field === field)
    : null;
  const name = String(channel?.name || "").trim();
  return name || fallback || String(field || "");
}

function channelList(channel, legacyField, fallbackField) {
  if (Array.isArray(channel) && channel.length > 0) {
    return channel
      .map((item) => {
        if (!item || typeof item !== "object") return null;
        const field = item.field || item.value || "";
        if (!field) return null;
        return { field, name: item.name || field };
      })
      .filter(Boolean);
  }
  const field = legacyField || fallbackField;
  if (!field) return [];
  return [{ field, name: field }];
}

function buildOption(kind, rows, mapping, legacy, diagnostics) {
  const chartKind = normalizeKind(kind);
  if (chartKind === "ranking") {
    return buildRankingOption(rows, mapping, {}, diagnostics).option;
  }
  if (chartKind === "radar") {
    return buildRadarOption(rows, mapping, legacy, diagnostics);
  }
  if (chartKind === "boxplot") {
    return buildBoxplotOption(rows, mapping, legacy, diagnostics);
  }
  if (PIE_KINDS.has(chartKind)) {
    return buildPieOption(chartKind, rows, mapping, diagnostics, legacy);
  }
  if (chartKind === "scatter") {
    return buildScatterOption(rows, mapping, diagnostics, legacy);
  }
  return buildCartesianOption(chartKind, rows, mapping, legacy, diagnostics);
}

function buildCartesianOption(kind, rows, mapping, legacy, diagnostics) {
  const host = legacy.__host;
  const xField = mapping.x[0]?.field;
  if (!xField) {
    diagnostics.push("缺少 mapping.x");
  }
  const yFields = mapping.y.length > 0 ? mapping.y.map((item) => item.field) : inferYFields(rows, xField);
  if (yFields.length === 0) {
    diagnostics.push("缺少 mapping.y");
  }
  if (kind === "trend" && yFields.length !== 1) {
    diagnostics.push("chart.trend 需要且仅支持一个 y 通道");
  }
  let categories = uniquePreserveOrder(rows.map((row) => String(row?.[xField] ?? ""))).filter(
    Boolean,
  );
  const fixedOrder = Array.isArray(legacy.fixedCategoryOrder)
    ? legacy.fixedCategoryOrder.map((item) => String(item || "").trim()).filter(Boolean)
    : resolveFixedCategoryOrder(legacy.chartProps, xField);
  let temporalYearMonth = false;
  if (fixedOrder && fixedOrder.length > 0) {
    // 固定顺序：即使当前筛选只剩一类，也保留连续状态轴（缺省为 0）
    categories = fixedOrder;
  } else if (legacy.sortCategoriesByYTotal) {
    const ranked = orderCartesianCategories(rows, mapping, yFields);
    if (ranked.length > 0) {
      categories = ranked.filter((label) => categories.includes(label));
    }
  } else if (
    categoriesLookLikeYearMonth(categories) ||
    (xFieldLooksTemporal(xField) && categories.some(isYearMonthCategoryLabel))
  ) {
    // 年月按时间升序，勿当离散类目保留 SQL/首次出现顺序。
    categories = sortCategoriesAsTime(categories);
    temporalYearMonth = categoriesLookLikeYearMonth(categories);
  } else if (xFieldLooksTemporal(xField) && categoriesLookLikeMonthNum(categories)) {
    // 年度对比共享月轴（01–12）：calendar 应按月号升序；
    // rolling 跨年窗口已是时间升序时保留，乱序则回退按月号排序（避免乱序轴）。
    if (categories.length >= 12 || !isChronologicalMonthSequence(categories)) {
      categories = sortCategoriesAsMonthNum(categories);
    }
    temporalYearMonth = false;
  }
  if (!temporalYearMonth && categoriesLookLikeYearMonth(categories)) {
    temporalYearMonth = true;
  }
  const grouped = mapping.group[0]?.field;
  let groups = grouped ? unique(rows.map((row) => String(row?.[grouped] ?? ""))).filter(Boolean) : [];
  if (groupsLookLikeYears(groups)) {
    groups = sortGroupsAsYears(groups);
  }
  const series = [];
  const isBar = kind === "column" || kind === "bar";
  const seriesType = isBar ? "bar" : "line";
  const compact = legacy.compact === true || legacy.compact === "true";
  const metricSpark = legacy.barGradient === "metric-spark";
  // 多 y 指标或 mapping.group 多系列（如年度对比线）必须用分类色板；
  // 单色阶梯（chart_1..6）数值着色会让各系列挤在相近绿色里。
  const multiMeasure = yFields.length > 1;
  const multiGroup = groups.length > 1;
  const explicitPalette = resolveExplicitColorPalette(legacy.chartProps || {});
  let palette = Array.isArray(legacy.palette) ? legacy.palette.slice() : [];
  if (!explicitPalette && (multiMeasure || multiGroup)) {
    palette = readThemeChartCategoricalPalette(host);
  }
  for (const yField of yFields) {
    const yDisplayName = resolveChannelDisplayName(mapping.y, yField);
    if (groups.length === 0) {
      const data = categories.map((category) => aggregateValue(rows, xField, category, yField));
      if (legacy.barLine && !isBar) {
        series.push({
          name: `${yDisplayName} · 柱`,
          type: "bar",
          barWidth: compact ? 10 : 14,
          itemStyle: {
            color: {
              type: "linear",
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: "rgba(98, 190, 235, 0.95)" },
                { offset: 1, color: "rgba(98, 190, 235, 0.12)" },
              ],
            },
          },
          data,
          z: 1,
        });
        series.push({
          name: yDisplayName,
          type: "line",
          smooth: true,
          symbol: "circle",
          symbolSize: compact ? 4 : 6,
          itemStyle: {
            color: canvasThemeColor(host, "text_inverse"),
            borderColor: canvasThemeColor(host, "text_unit"),
            borderWidth: 1,
          },
          lineStyle: { color: canvasThemeColor(host, "text_unit"), width: 1.5 },
          data,
          z: 3,
        });
      } else {
        const sparkBar =
          metricSpark && seriesType === "bar" && (kind === "column" || kind === "bar");
        // 单系列柱：按数值映射单色阶梯；多系列保留 option.color 系列色，勿逐柱重着色。
        const coloredData =
          !sparkBar && isBar && !multiMeasure && palette.length > 1
            ? colorizeBarDataByValue(data, palette)
            : data;
        series.push({
          name: yDisplayName,
          type: seriesType,
          smooth: kind === "trend",
          areaStyle: kind === "area" ? {} : undefined,
          stack: legacy.stack ? "total" : undefined,
          barWidth: sparkBar && compact ? 8 : undefined,
          itemStyle: sparkBar
            ? metricSparkBarItemStyle(host)
            : isBar
              ? { borderRadius: [0, 0, 0, 0] }
              : undefined,
          data: coloredData,
        });
      }
    } else {
      const yearDuoGradient = isBar && legacy.barGradient === "cockpit-year-duo";
      const groupOnlyLegend = legendGroupOnlyEnabled(legacy.chartProps || {});
      let groupSeriesIndex = 0;
      for (const groupName of groups) {
        const seriesItem = {
          name: groupOnlyLegend ? groupName : `${groupName} · ${yDisplayName}`,
          type: seriesType,
          smooth: kind === "trend",
          areaStyle: kind === "area" ? {} : undefined,
          stack: legacy.stack ? "total" : undefined,
          data: categories.map((category) =>
            aggregateValue(rows, xField, category, yField, grouped, groupName),
          ),
        };
        if (yearDuoGradient) {
          seriesItem.barWidth = compact ? "34%" : "38%";
          seriesItem.barGap = "24%";
          seriesItem.itemStyle = cockpitYearDuoBarItemStyle(groupSeriesIndex, { host });
          seriesItem.emphasis = {
            focus: "series",
            itemStyle: cockpitYearDuoBarItemStyle(groupSeriesIndex, { emphasis: true, host }),
          };
        } else if (isBar) {
          seriesItem.itemStyle = { borderRadius: [0, 0, 0, 0] };
        }
        series.push(seriesItem);
        groupSeriesIndex += 1;
      }
    }
  }
  if (legacy.percent) {
    applyPercentTransform(series);
  }
  const chartProps = legacy.chartProps || {};
  const themeTypography = readThemeTypography(host);
  const mutedColor = canvasThemeColor(host, "text_muted");
  const categoryAxisRotate = resolveCategoryAxisLabelRotate(chartProps);
  const compactGrid = metricSpark
    ? { left: 2, right: 2, top: 4, bottom: 4, containLabel: false }
    : resolveCompactCartesianGrid(chartProps, legacy, categoryAxisRotate);
  const showCategoryLabels = categoryAxisVisible(chartProps);
  const categoryAxisLabel = showCategoryLabels
    ? buildCategoryAxisLabel(chartProps, themeTypography, {
        temporalYearMonth,
      })
    : { show: false };
  const plotTitleText =
    titleInPlotEnabled(chartProps) ? String(chartProps.title ?? "").trim() : "";
  const option = {
    backgroundColor: "transparent",
    tooltip: echartsTooltip(
      themeTypography,
      "axis",
      { confine: tooltipConfineEnabled(chartProps) },
      host,
    ),
    legend: legacy.showLegend
      ? {
          show: true,
          top: 0,
          ...buildLegendPosition(chartProps),
          orient: "horizontal",
          itemWidth: 10,
          itemHeight: 8,
          itemGap: 6,
          textStyle: {
            fontSize: themeTypography.unit,
            color: mutedColor,
            ...echartsTextStrokeStyle(mutedColor),
          },
        }
      : { show: false },
    title: plotTitleText
      ? {
          text: plotTitleText,
          right: 2,
          top: 0,
          padding: [1, 2, 0, 0],
          textStyle: {
            fontSize: themeTypography.chartTitle || themeTypography.unit,
            fontWeight: 600,
            color: mutedColor,
            ...echartsTextStrokeStyle(mutedColor),
          },
        }
      : undefined,
    toolbox: legacy.compact ? undefined : { feature: { saveAsImage: {} } },
    grid: legacy.compact
      ? compactGrid
      : { left: 44, right: 22, top: legacy.showLegend ? 38 : 28, bottom: 34 },
    xAxis:
      kind === "bar"
        ? { type: "value" }
        : {
            type: "category",
            data: categories,
            // line/area/trend：收紧类目轴两端空隙，避免首末刻度与容器内边距脱节。
            ...(kind === "line" || kind === "area" || kind === "trend"
              ? { boundaryGap: false }
              : {}),
          },
    yAxis: kind === "bar" ? { type: "category", data: categories } : { type: "value" },
    series,
  };
  if (palette.length > 0) {
    option.color = palette;
  }
  if (legacy.compact && !metricSpark) {
    const integerAxis = valueAxisIntegerEnabled(chartProps);
    const valueFormatter = integerAxis ? compactIntegerAxisValueLabel : compactAxisValueLabel;
    if (kind === "bar") {
      option.xAxis = {
        ...option.xAxis,
        axisLabel: {
          fontSize: themeTypography.unit,
          color: mutedColor,
          formatter: valueFormatter,
        },
        splitLine: COCKPIT_CARTESIAN_SPLIT_LINE,
        ...(integerAxis ? { minInterval: 1 } : {}),
      };
      option.yAxis = {
        ...option.yAxis,
        axisLabel: categoryAxisLabel,
        ...(showCategoryLabels ? {} : { axisTick: { show: false } }),
      };
    } else {
      option.xAxis = {
        ...option.xAxis,
        axisLabel: categoryAxisLabel,
        ...(showCategoryLabels ? {} : { axisTick: { show: false } }),
      };
      option.yAxis = {
        ...option.yAxis,
        axisLabel: {
          fontSize: themeTypography.unit,
          color: mutedColor,
          formatter: valueFormatter,
        },
        splitLine: COCKPIT_CARTESIAN_SPLIT_LINE,
        splitNumber: 4,
        ...(integerAxis ? { minInterval: 1 } : {}),
      };
    }
  }
  if (metricSpark) {
    const hideAxis = {
      show: false,
      axisLabel: { show: false },
      axisTick: { show: false },
      axisLine: { show: false },
      splitLine: { show: false },
    };
    option.xAxis = { ...option.xAxis, ...hideAxis };
    option.yAxis = { ...option.yAxis, ...hideAxis };
    option.tooltip = { show: false };
  }
  if (kind === "bar") {
    option.series = option.series.map((item) => ({
      ...item,
      data: categories.map((category) => {
        const value = item.data[categories.indexOf(category)] || 0;
        return value;
      }),
    }));
  }
  if (legacy.dataZoom && categories.length > 16) {
    option.dataZoom = [{ type: "inside" }, { type: "slider" }];
  }
  return option;
}

function buildPieOption(kind, rows, mapping, diagnostics, legacy = {}) {
  const labelField = mapping.label[0]?.field || mapping.x[0]?.field;
  const valueField = mapping.y[0]?.field;
  if (!labelField || !valueField) {
    diagnostics.push("pie/donut/rose 需要 mapping.label(x) 与 mapping.y");
  }
  const host = legacy.__host;
  const chartProps = legacy.chartProps || {};
  const warningLevelPalette = usesWarningLevelPalette(chartProps, mapping);
  const valueRampPalette = usesValueRampPiePalette(chartProps);
  const piePalette = resolvePieColorPalette({ ...chartProps, __host: host });
  const baseData = rows
    .map((row) => ({
      name: String(row?.[labelField] ?? ""),
      value: toNumber(row?.[valueField]),
    }))
    .filter((item) => item.name && Number.isFinite(item.value));
  const data = warningLevelPalette
    ? colorizePieDataByWarningLevel(baseData, host)
    : valueRampPalette
      ? colorizePieDataByValue(baseData, piePalette)
      : colorizePieDataByCategory(baseData, piePalette);
  if (data.length === 0) {
    diagnostics.push(`pie/donut/rose 无有效数据点 (label=${labelField || "-"}, y=${valueField || "-"})`);
  }
  const compact = legacy.compact === true || legacy.compact === "true";
  const chartHeight = Number(legacy.chartHeight) > 0 ? Number(legacy.chartHeight) : 0;
  const themeTypography = readThemeTypography(host);
  const tight = compact && chartHeight > 0 && chartHeight <= 56;
  // compact 默认仍隐藏图例；环内占比见下方 showLabel（donut 非 tight 默认开）。
  const explicitLegendOn =
    chartProps.showLegend === true ||
    chartProps.showLegend === "true" ||
    chartProps.show_legend === true ||
    chartProps.show_legend === "true";
  const explicitLegendOff =
    chartProps.showLegend === false ||
    chartProps.showLegend === "false" ||
    chartProps.show_legend === false ||
    chartProps.show_legend === "false";
  const showLegend = compact ? explicitLegendOn : !explicitLegendOff;
  const explicitLabelOn =
    chartProps.showLabel === true ||
    chartProps.showLabel === "true" ||
    chartProps.show_label === true ||
    chartProps.show_label === "true";
  const explicitLabelOff =
    chartProps.showLabel === false ||
    chartProps.showLabel === "false" ||
    chartProps.show_label === false ||
    chartProps.show_label === "false";
  // donut 强调占比：默认在环内显示 `{d}%`。tight 迷你环（驾驶舱火花图）仍默认隐藏，避免字叠扇区。
  const showLabel = explicitLabelOff
    ? false
    : explicitLabelOn || (kind === "donut" && !tight);
  const compactWithLegend = compact && showLegend;
  const donutRadius = tight
    ? ["58%", "82%"]
    : compactWithLegend
      ? ["38%", "60%"]
      : compact
        ? ["52%", "78%"]
        : ["45%", "72%"];
  const option = {
    tooltip: echartsTooltip(
      themeTypography,
      "item",
      { formatter: pieDonutTooltipFormatter },
      host,
    ),
    legend: showLegend
      ? {
          show: true,
          ...(compact
            ? { bottom: 2, left: "center", orient: "horizontal", itemWidth: 10, itemHeight: 10, itemGap: 14 }
            : { top: 4, left: "center", orient: "horizontal" }),
          textStyle: {
            fontSize: themeTypography.label,
            color: canvasThemeColor(host, "text_body"),
            fontFamily: readThemeUiFontFamily(host),
            ...echartsTextStrokeStyle(canvasThemeColor(host, "text_body")),
          },
        }
      : { show: false },
    toolbox: compact ? undefined : { feature: { saveAsImage: {} } },
    series: [
      {
        type: "pie",
        radius: kind === "donut" ? donutRadius : tight ? "62%" : compact ? "68%" : "70%",
        // Legend and pie share one canvas; reserve vertical space so slices are not clipped to zero height.
        center: compactWithLegend ? ["50%", "44%"] : compact ? ["50%", "50%"] : ["50%", "58%"],
        top: compact ? 0 : 36,
        height: compact ? undefined : "72%",
        label: showLabel
          ? (() => {
              const labelColor = canvasThemeColor(host, "text_primary");
              // 字号跟主题 chart_label / 字阶下限；compact 可略收但不低于 min
              const fontSize = clampThemeFontPx(
                host,
                Math.round(themeTypography.label * (compact ? 0.9 : 1)),
              );
              return {
                show: true,
                position: compact ? "inside" : "outside",
                formatter: compact ? "{d}%" : "{b}\n{d}%",
                fontSize,
                fontFamily: readThemeUiFontFamily(host),
                // 默认主题字色+描边；扇区上色会在 data[].label 上按底色覆盖
                color: labelColor,
                fontWeight: 600,
                ...echartsTextStrokeStyle(labelColor),
              };
            })()
          : { show: false },
        labelLine: { show: showLabel && !compact },
        ...(kind === "rose" ? { roseType: "radius" } : {}),
        data,
      },
    ],
  };
  if (warningLevelPalette) {
    const levelColors = readWarningLevelColors(host);
    option.color = [levelColors.红, levelColors.黄, levelColors.蓝, levelColors.灰];
  } else if (Array.isArray(piePalette) && piePalette.length > 0) {
    option.color = piePalette;
  }
  return option;
}

function buildScatterOption(rows, mapping, diagnostics, legacy = {}) {
  const host = legacy.__host;
  const themeTypography = readThemeTypography(host);
  const xField = mapping.x[0]?.field;
  const yField = mapping.y[0]?.field;
  const sizeField = mapping.size[0]?.field;
  const colorField = mapping.color[0]?.field;
  if (!xField || !yField) {
    diagnostics.push("scatter 需要 mapping.x 与 mapping.y");
  }
  const groups = colorField
    ? unique(rows.map((row) => String(row?.[colorField] ?? "")).filter(Boolean))
    : [""];
  const series = groups.map((groupName) => {
    const points = rows
      .filter((row) => !colorField || String(row?.[colorField] ?? "") === groupName)
      .map((row) => {
        const point = [toNumber(row?.[xField]), toNumber(row?.[yField])];
        if (sizeField) point.push(toNumber(row?.[sizeField]));
        return point;
      })
      .filter((item) => Number.isFinite(item[0]) && Number.isFinite(item[1]));
    return {
      name: groupName || (mapping.y[0]?.name || yField),
      type: "scatter",
      symbolSize: (value) => {
        if (!sizeField) return 12;
        const size = Number(value?.[2]);
        if (!Number.isFinite(size)) return 10;
        return Math.max(6, Math.min(24, size / 8));
      },
      data: points,
    };
  });
  return {
    tooltip: echartsTooltip(themeTypography, "item", {}, host),
    legend: { top: 0, show: !!colorField },
    toolbox: { feature: { saveAsImage: {} } },
    xAxis: { type: "value" },
    yAxis: { type: "value" },
    series,
  };
}

function buildRadarOption(rows, mapping, legacy, diagnostics) {
  const host = legacy.__host;
  const themeTypography = readThemeTypography(host);
  const dimensions = mapping.radarDimensions.length > 0
    ? mapping.radarDimensions
    : mapping.y;
  if (dimensions.length === 0) {
    diagnostics.push("radar 需要 dimensions 或 mapping.y");
  }
  const indicators = dimensions.map((item) => ({
    name: item.name || item.field,
    max: maxByField(rows, item.field) || 100,
  }));
  const labelField = mapping.label[0]?.field || mapping.x[0]?.field;
  const data = rows.slice(0, 12).map((row, index) => ({
    name: labelField ? String(row?.[labelField] ?? `item-${index + 1}`) : `item-${index + 1}`,
    value: dimensions.map((dim) => toNumber(row?.[dim.field])),
  }));
  if (legacy.percent) {
    normalizeRadarData(data);
  }
  return {
    tooltip: echartsTooltip(themeTypography, "item", {}, host),
    legend: { top: 0 },
    toolbox: { feature: { saveAsImage: {} } },
    radar: { indicator: indicators },
    series: [{ type: "radar", data }],
  };
}

function buildBoxplotOption(rows, mapping, legacy, diagnostics) {
  const host = legacy.__host;
  const themeTypography = readThemeTypography(host);
  const labelField = mapping.x[0]?.field || "label";
  const def = mapping.boxplot;
  if (def.length >= 5) {
    const [minField, q1Field, medianField, q3Field, maxField] = def.map((item) => item.field);
    const labels = rows.map((row) => String(row?.[labelField] ?? ""));
    const data = rows.map((row) => [
      toNumber(row?.[minField]),
      toNumber(row?.[q1Field]),
      toNumber(row?.[medianField]),
      toNumber(row?.[q3Field]),
      toNumber(row?.[maxField]),
    ]);
    return {
      tooltip: echartsTooltip(themeTypography, "item", {}, host),
      xAxis: { type: "category", data: labels },
      yAxis: { type: "value" },
      series: [{ type: "boxplot", data }],
    };
  }
  const valueField = mapping.y[0]?.field;
  if (!valueField) {
    diagnostics.push("boxplot 需要 mapping.boxplot 或 mapping.y");
  }
  const grouped = groupNumbersBy(rows, labelField, valueField);
  const labels = Object.keys(grouped);
  const data = labels.map((label) => toBoxStats(grouped[label]));
  return {
    tooltip: echartsTooltip(themeTypography, "item", {}, host),
    xAxis: { type: "category", data: labels },
    yAxis: { type: "value" },
    series: [{ type: "boxplot", data }],
  };
}

function aggregateValue(rows, xField, category, yField, groupField, groupName) {
  const values = rows
    .filter((row) => {
      if (String(row?.[xField] ?? "") !== category) return false;
      if (!groupField) return true;
      return String(row?.[groupField] ?? "") === groupName;
    })
    .map((row) => toNumber(row?.[yField]))
    .filter((value) => Number.isFinite(value));
  if (values.length === 0) return 0;
  return values.reduce((sum, value) => sum + value, 0);
}

function applyPercentTransform(series) {
  if (!Array.isArray(series) || series.length === 0) return;
  const points = Math.max(...series.map((item) => item.data.length));
  for (let idx = 0; idx < points; idx += 1) {
    const total = series.reduce((sum, item) => sum + toNumber(item.data[idx]), 0);
    if (total <= 0) continue;
    for (const item of series) {
      item.data[idx] = Number(((toNumber(item.data[idx]) / total) * 100).toFixed(2));
    }
  }
}

function inferYFields(rows, xField) {
  if (!Array.isArray(rows) || rows.length === 0) return [];
  const first = rows.find((row) => row && typeof row === "object");
  if (!first) return [];
  return Object.keys(first).filter((key) => key !== xField && Number.isFinite(toNumber(first[key])));
}

function resolveRankingLayout(props) {
  const raw = String(
    props?.rankingLayout || props?.labelLayout || props?.layout || "side",
  )
    .trim()
    .toLowerCase();
  if (["above", "label_above", "label-above", "stacked", "top"].includes(raw)) {
    return "above";
  }
  if (
    ["bar", "fill", "inline", "inline-bar", "inline_bar", "ranking-bar", "ranking_bar"].includes(
      raw,
    )
  ) {
    return "bar";
  }
  return "side";
}

function buildRankingItems(rows, mapping, diagnostics) {
  const labelField =
    mapping.x[0]?.field || mapping.label[0]?.field || inferLabelField(rows);
  const valueField = mapping.y[0]?.field || "value";
  const valueName = mapping.y[0]?.name || valueField;
  if (!labelField) {
    diagnostics.push("ranking 需要 mapping.x（排名标签字段）");
  }
  const items = (Array.isArray(rows) ? rows : [])
    .map((row) => ({
      label: String(row?.[labelField] ?? "").trim(),
      value: toNumber(row?.[valueField]),
    }))
    .filter((item) => item.label && Number.isFinite(item.value))
    .sort((left, right) => right.value - left.value);
  return { items, valueName };
}

function buildRankingOption(rows, mapping, props, diagnostics) {
  return buildRankingSideOption(rows, mapping, props, diagnostics);
}

function rankingFillHeightEnabled(props) {
  return (
    props?.fillHeight === true ||
    props?.fillHeight === "true" ||
    props?.fill_height === true ||
    props?.fill_height === "true" ||
    ((props?.compact === true || props?.compact === "true") && !(Number(props?.chartHeight) > 0))
  );
}

function resolveRankingAboveHeight(chartEl, props) {
  if (Number(props.chartHeight) > 0) {
    return Number(props.chartHeight);
  }
  if (rankingFillHeightEnabled(props)) {
    const host = chartEl?.getRootNode?.()?.host;
    const fromHost = Number(host?.clientHeight) || 0;
    const fromParent = Number(chartEl?.parentElement?.clientHeight) || 0;
    const fromSelf = Number(chartEl?.clientHeight) || 0;
    const resolved = Math.max(fromHost, fromParent, fromSelf);
    if (resolved > 0) return resolved;
  }
  return 152;
}

function renderRankingAboveDom(chartEl, model, props, handlers = {}) {
  const onLabelClick =
    typeof handlers === "function" ? handlers : handlers?.onLabelClick;
  const bindTitle = typeof handlers === "function" ? null : handlers?.bindTitle;
  const theme = model.theme || resolveRankingTheme(props);
  const fillHeight = rankingFillHeightEnabled(props);
  const chartHeight = resolveRankingAboveHeight(chartEl, props);
  const items = Array.isArray(model.items) ? model.items : [];
  const maxChars = Number(model.maxChars) > 0 ? Number(model.maxChars) : 28;
  const maxValue = rankingValueAxisMax(items.map((item) => item.value));
  const title = String(props.title || "").trim();
  const showTitle = (props.compact === true || props.compact === "true") && title.length > 0;
  const pullUp = Math.max(0, Number(props.rankingPullUp ?? props.ranking_pull_up ?? 0));
  const padLeft = Math.max(0, Number(props.contentPadLeft ?? props.content_pad_left ?? 0));
  const titleFontPx = readThemeTypography(props.__host).chartTitle;
  const titleH = showTitle ? Math.max(14, Math.ceil(titleFontPx * 1.15)) : 0;
  const listHeight = Math.max(48, chartHeight - titleH + pullUp);
  const slotPx = items.length > 0 ? Math.floor(listHeight / items.length) : 0;
  if (fillHeight) {
    chartEl.style.height = "100%";
    chartEl.style.minHeight = "0";
    chartEl.style.maxHeight = "none";
    chartEl.style.flex = "1 1 auto";
  } else {
    chartEl.style.height = `${chartHeight}px`;
    chartEl.style.minHeight = `${chartHeight}px`;
    chartEl.style.maxHeight = `${chartHeight}px`;
  }
  chartEl.style.overflow = pullUp > 0 ? "visible" : "hidden";
  if (chartEl.parentElement) {
    chartEl.parentElement.style.overflow = pullUp > 0 ? "visible" : "hidden";
  }
  const barColor = theme.barColor || canvasThemeColor(props.__host, "chart_2");
  const trackBg = theme.barBackground || "transparent";
  const trackBorder = theme.barBackgroundBorder || "rgba(100, 116, 139, 0.35)";
  const valueUnit = String(model.valueName || "").trim();
  const showValueUnit = valueUnit.length > 0 && valueUnit.length <= 4;
  const rowsHtml = items
    .map((item, index) => {
      const ratio = maxValue > 0 ? item.value / maxValue : 0;
      const pct = Math.max(8, Math.min(100, Math.round(ratio * 100)));
      const label = formatRankingNameLabel(item.label, maxChars);
      const valueText = formatRankingAboveValue(item.value, showValueUnit ? valueUnit : "");
      return `<div class="mei-rank-above-row" data-idx="${index}" title="${escapeHtml(item.label)}" style="max-height:${Math.max(22, slotPx)}px">
        <div class="mei-rank-above-head">
          <span class="mei-rank-above-label">${escapeHtml(label.display)}</span>
          <span class="mei-rank-above-value">${escapeHtml(valueText)}</span>
        </div>
        <div class="mei-rank-above-track" style="background:${trackBg};border-color:${trackBorder}">
          <div class="mei-rank-above-fill" style="width:${pct}%;background-color:${barColor}"></div>
        </div>
      </div>`;
    })
    .join("");
  const heightCss = fillHeight ? "height:100%;min-height:0;" : `height:${chartHeight}px;`;
  chartEl.innerHTML = `<div class="mei-rank-above" style="${heightCss}padding-left:${padLeft}px;margin-top:${-pullUp}px;box-sizing:border-box">
    ${showTitle ? `<div class="mei-rank-above-title">${escapeHtml(title)}</div>` : ""}
    <div class="mei-rank-above-list">${rowsHtml}</div>
  </div>`;
  chartEl.querySelectorAll(".mei-rank-above-row").forEach((row) => {
    row.addEventListener("click", (event) => {
      const index = Number(row.getAttribute("data-idx"));
      const full = model.fullLabels?.[index];
      if (full && typeof onLabelClick === "function") {
        onLabelClick(full, event);
      }
    });
  });
  if (showTitle && typeof bindTitle === "function") {
    const titleEl = chartEl.querySelector(".mei-rank-above-title");
    bindTitle(titleEl, title);
  }
}

/**
 * 水平排名横条图：标题行（标题+单位）+ 行内文字叠在按比例实心横条底图上。
 * 单位只出现在标题行；数值统一位数、tabular 右对齐；长标签 CSS 截断 + title/点击飘窗。
 */
function renderRankingBarDom(chartEl, model, props, handlers = {}) {
  const onLabelClick =
    typeof handlers === "function" ? handlers : handlers?.onLabelClick;
  const bindTitle = typeof handlers === "function" ? null : handlers?.bindTitle;
  const theme = model.theme || resolveRankingTheme(props);
  const fillHeight = rankingFillHeightEnabled(props);
  const chartHeight = resolveRankingAboveHeight(chartEl, props);
  const items = Array.isArray(model.items) ? model.items : [];
  const maxChars = Number(model.maxChars) > 0 ? Number(model.maxChars) : 16;
  const maxValue = rankingValueAxisMax(items.map((item) => item.value));
  const title = String(props.title || "").trim();
  const valueUnit = String(model.valueName || "").trim();
  const showTitle = title.length > 0;
  const showUnit = valueUnit.length > 0 && valueUnit.length <= 6;
  const pullUp = Math.max(0, Number(props.rankingPullUp ?? props.ranking_pull_up ?? 0));
  const padLeft = Math.max(0, Number(props.contentPadLeft ?? props.content_pad_left ?? 0));
  const titleFontPx = readThemeTypography(props.__host).chartTitle;
  const titleH = showTitle ? Math.max(14, Math.ceil(titleFontPx * 1.2)) : 0;
  const listHeight = Math.max(48, chartHeight - titleH + pullUp);
  const slotPx = items.length > 0 ? Math.floor(listHeight / items.length) : 0;
  if (fillHeight) {
    chartEl.style.height = "100%";
    chartEl.style.minHeight = "0";
    chartEl.style.maxHeight = "none";
    chartEl.style.flex = "1 1 auto";
  } else {
    chartEl.style.height = `${chartHeight}px`;
    chartEl.style.minHeight = `${chartHeight}px`;
    chartEl.style.maxHeight = `${chartHeight}px`;
  }
  chartEl.style.overflow = pullUp > 0 ? "visible" : "hidden";
  if (chartEl.parentElement) {
    chartEl.parentElement.style.overflow = pullUp > 0 ? "visible" : "hidden";
  }
  // 默认实心单色：theme chart_5（zhifa「图表主色」），不是阶梯最浅的 chart_1。
  const barColor =
    String(props.barColor || props.bar_color || "").trim() ||
    canvasThemeColor(props.__host, "chart_5") ||
    theme.barColor ||
    canvasThemeColor(props.__host, "chart_4") ||
    "#10b981";
  const useFixedOneDecimal = items.some((item) => {
    const n = Number(item.value);
    if (!Number.isFinite(n)) return false;
    return Math.abs(n - Math.round(n)) > 1e-9;
  });
  const valueTexts = items.map((item) =>
    formatRankingBarValue(item.value, useFixedOneDecimal),
  );
  const maxValueChars = valueTexts.reduce(
    (peak, text) => Math.max(peak, [...String(text)].length),
    3,
  );
  const valueMinCh = Math.max(3, maxValueChars);
  const rowsHtml = items
    .map((item, index) => {
      const ratio = maxValue > 0 ? item.value / maxValue : 0;
      const pct = Math.max(4, Math.min(100, Math.round(ratio * 1000) / 10));
      const label = formatRankingNameLabel(item.label, maxChars);
      const tip = escapeHtml(item.label);
      return `<div class="mei-rank-bar-row" data-idx="${index}" title="${tip}" style="max-height:${Math.max(22, slotPx)}px">
        <div class="mei-rank-bar-fill" style="width:${pct}%;background-color:${barColor}"></div>
        <div class="mei-rank-bar-content">
          <span class="mei-rank-bar-label">${escapeHtml(label.display)}</span>
          <span class="mei-rank-bar-value" style="min-width:${valueMinCh}ch">${escapeHtml(valueTexts[index])}</span>
        </div>
      </div>`;
    })
    .join("");
  const heightCss = fillHeight ? "height:100%;min-height:0;" : `height:${chartHeight}px;`;
  // 对齐柱图标题写法：标题（单位）
  const titleText =
    showTitle && showUnit ? `${title}（${valueUnit}）` : title;
  chartEl.innerHTML = `<div class="mei-rank-bar" style="${heightCss}padding-left:${padLeft}px;margin-top:${-pullUp}px;box-sizing:border-box">
    ${
      showTitle
        ? `<div class="mei-rank-bar-title"><span class="mei-rank-bar-title-text">${escapeHtml(titleText)}</span></div>`
        : ""
    }
    <div class="mei-rank-bar-list">${rowsHtml}</div>
  </div>`;
  chartEl.querySelectorAll(".mei-rank-bar-row").forEach((row) => {
    row.addEventListener("click", (event) => {
      const index = Number(row.getAttribute("data-idx"));
      const full = model.fullLabels?.[index];
      if (full && typeof onLabelClick === "function") {
        onLabelClick(full, event);
      }
    });
  });
  if (showTitle && typeof bindTitle === "function") {
    const titleEl = chartEl.querySelector(".mei-rank-bar-title-text");
    bindTitle(titleEl, titleText);
  }
}

/** 水平排名数值：同表统一整数或 1 位小数，便于右对齐。 */
function formatRankingBarValue(value, forceOneDecimal) {
  const n = Number(value);
  if (!Number.isFinite(n)) return String(value ?? "").trim();
  if (forceOneDecimal) {
    return (Math.round(n * 10) / 10).toFixed(1);
  }
  return String(Math.trunc(Math.round(n)));
}

function resolveRankingTheme(props) {
  const host = props.__host;
  return {
    barColor:
      String(props.barColor || props.bar_color || "").trim() ||
      canvasThemeColor(host, "chart_5") ||
      canvasThemeColor(host, "chart_4") ||
      "#10b981",
    barBackground:
      String(props.barBackground || props.bar_background || "").trim() ||
      "transparent",
    barBackgroundBorder:
      String(props.barBackgroundBorder || props.bar_background_border || "").trim() ||
      "rgba(100, 116, 139, 0.35)",
  };
}

function resolveRankingTypography(props, slotPx = 0) {
  void slotPx;
  const typography = readThemeTypography(props.__host);
  return {
    labelFontSize: typography.label,
    valueFontSize: typography.value,
    axisLabelFontSize: typography.unit,
    labelLineHeight: Math.round(typography.label * 1.42),
  };
}

function estimateRankingMaxChars(widthPx, fontSize) {
  const unit = Math.max(7, fontSize * 0.58);
  return Math.max(6, Math.floor(widthPx / unit));
}

function formatRankingNameLabel(text, maxChars) {
  const full = String(text ?? "").trim();
  const display = truncateRankingLabel(full, maxChars);
  return { display, full, isTruncated: display !== full };
}

/** 排名数值展示：最多 1 位小数；整数量不带 `.0`（避免浮点长尾） */
function formatRankingValueText(value, unit = "") {
  const n = Number(value);
  let text = String(value ?? "").trim();
  if (Number.isFinite(n)) {
    const rounded = Math.round(n * 10) / 10;
    text = Number.isInteger(rounded)
      ? String(Math.trunc(rounded))
      : rounded.toFixed(1);
  }
  const u = String(unit || "").trim();
  return u ? `${text} ${u}` : text;
}

function formatRankingAboveValue(value, unit) {
  return formatRankingValueText(value, unit);
}

function rankingBarBackgroundStyle(theme, borderRadius = [0, 0, 0, 0]) {
  return {
    color: theme.barBackground,
    borderColor: theme.barBackgroundBorder,
    borderWidth: 1,
    borderRadius,
  };
}

function resolveRankingBarFill(theme, props = null) {
  const gradient = String(props?.barGradient ?? props?.bar_gradient ?? "")
    .trim()
    .toLowerCase();
  const host = props?.__host;
  const palette = readThemeChartPalette(host);
  const solid = theme.barColor || palette[4] || "#10b981";
  if (
    gradient === "ranking-cyan" ||
    gradient === "cyan" ||
    gradient === "true" ||
    gradient === "1" ||
    gradient === "ranking-mono" ||
    gradient === "mono"
  ) {
    const start = palette[5] || "#059669";
    const mid = solid;
    const end = palette[1] || "#a7f3d0";
    return {
      type: "linear",
      x: 0,
      y: 0,
      x2: 1,
      y2: 0,
      colorStops: [
        { offset: 0, color: start },
        { offset: 0.55, color: mid },
        { offset: 1, color: end },
      ],
    };
  }
  return solid;
}

function rankingBarItemStyle(theme, borderRadius = [0, 0, 0, 0], props = null) {
  const fill = resolveRankingBarFill(theme, props);
  const glow = theme.barColor || readThemeChartPalette(props?.__host)[3] || "#34d399";
  return {
    borderRadius,
    color: fill,
    shadowBlur: 6,
    shadowColor: hexToRgba(glow, 0.3),
    shadowOffsetY: 1,
  };
}

function buildRankingBarSeries({
  values,
  valueName,
  barMaxWidth,
  barCategoryGap,
  theme,
  borderRadius = [0, 0, 0, 0],
  valueLabel,
  props = null,
}) {
  const itemStyle = rankingBarItemStyle(theme, borderRadius, props);
  return {
    name: valueName,
    type: "bar",
    data: values,
    barMaxWidth,
    barCategoryGap,
    z: 2,
    showBackground: true,
    backgroundStyle: rankingBarBackgroundStyle(theme, borderRadius),
    itemStyle,
    emphasis: {
      focus: "series",
      itemStyle: {
        ...itemStyle,
        shadowBlur: 10,
        shadowColor: hexToRgba(theme.barColor || "#34d399", 0.42),
      },
    },
    label: valueLabel || { show: false },
  };
}

function rankingTooltipFormatter(items, valueName, typography) {
  const labelPx = typography.label;
  const unitPx = typography.unit;
  return (params) => {
    const point = Array.isArray(params) ? params[0] : params;
    const idx = point?.dataIndex ?? 0;
    const item = items[idx];
    if (!item) return "";
    const title = escapeHtml(item.label);
    const valueText = formatRankingValueText(item.value);
    return `<div style="max-width:min(92vw,420px);line-height:1.45;word-break:break-all;font-size:${labelPx}px;color:${ECHARTS_TOOLTIP_TEXT.primary};">${title}<br/><span style="color:${ECHARTS_TOOLTIP_TEXT.secondary};font-size:${unitPx}px">${escapeHtml(valueName)}: ${escapeHtml(valueText)}</span></div>`;
  };
}

function rankingValueAxisMax(values) {
  const maxValue = values.reduce((peak, value) => Math.max(peak, value), 0);
  if (!Number.isFinite(maxValue) || maxValue <= 0) return 1;
  return Math.ceil(maxValue * 1.06);
}

function buildRankingSideOption(rows, mapping, props, diagnostics) {
  const { items, valueName } = buildRankingItems(rows, mapping, diagnostics);
  const theme = resolveRankingTheme(props);
  const configuredMaxChars = resolveRankingLabelMaxChars(props, "side");
  const categories = items.map((item) => item.label);
  const values = items.map((item) => item.value);
  const compact = props.compact === true || props.compact === "true";
  const chartHeight = Number(props.chartHeight) > 0 ? Number(props.chartHeight) : 0;
  let gridTop = compact ? 4 : 28;
  let gridBottom = compact ? 4 : 24;
  let barMaxWidth = 22;
  let slotPx = 0;
  if (compact && chartHeight > 0 && items.length > 0) {
    gridTop = 4;
    gridBottom = 4;
    const plotH = Math.max(20, chartHeight - gridTop - gridBottom);
    slotPx = plotH / items.length;
    barMaxWidth = Math.min(22, Math.max(8, Math.floor(slotPx * 0.58)));
  }
  const typography = resolveRankingTypography(props, slotPx);
  const themeTypography = readThemeTypography(props.__host);
  // 统一走 theme font-1（chart_label / metric_unit），禁止压成自定义小字号
  const labelFontPx = typography.labelFontSize;
  const valueFontPx = typography.labelFontSize;
  const uiFontFamily = readThemeUiFontFamily(props.__host);
  const charWidth = Math.max(8, Math.round(labelFontPx * 0.58));
  const gridLeftMin = compact ? 76 : 100;
  const gridLeftMax = compact ? 220 : 380;
  const gridLeft = Math.min(
    gridLeftMax,
    Math.max(gridLeftMin, configuredMaxChars * charWidth + (compact ? 18 : 28)),
  );
  // 短标签（2～3 字）：右对齐落在柱左侧槽内（左对齐会锚在绘图区并向右盖住柱形）
  const shortLabel = configuredMaxChars > 0 && configuredMaxChars <= 4;
  const cjkCharPx = Math.max(labelFontPx, Math.round(labelFontPx * 1.05));
  const labelWidthPx = shortLabel
    ? Math.ceil((configuredMaxChars + 1) * cjkCharPx)
    : Math.max(24, gridLeft - (compact ? 4 : 8));
  // 左槽 = 标签宽 + 与柱的小间距；不再额外放大造成外侧空档
  const gridLeftResolved = shortLabel
    ? labelWidthPx + (compact ? 4 : 6)
    : gridLeft;
  const resolvedMaxChars = shortLabel
    ? configuredMaxChars
    : Math.min(
      configuredMaxChars,
      estimateRankingMaxChars(labelWidthPx, labelFontPx),
    );
  const unitText = String(valueName || "").trim();
  const showUnit = unitText.length > 0 && unitText.length <= 4;
  const valueTexts = values.map((value) =>
    formatRankingValueText(value, showUnit ? unitText : ""),
  );
  // 数字偏窄、中文单位偏宽：按混合宽度估算右槽
  const maxValueChars = valueTexts.reduce(
    (peak, text) => Math.max(peak, [...String(text)].length),
    showUnit ? unitText.length + 2 : 2,
  );
  const valueSlotPx = Math.ceil(
    maxValueChars * Math.max(7, valueFontPx * 0.62) + (compact ? 4 : 8),
  );
  const gridRight = Math.min(
    compact ? 76 : 92,
    Math.max(compact ? 48 : 60, valueSlotPx),
  );
  const borderRadius = [0, 4, 4, 0];
  const host = props.__host;
  const option = {
    textStyle: {
      fontFamily: uiFontFamily,
      fontSize: labelFontPx,
    },
    tooltip: echartsTooltip(themeTypography, "axis", {
      axisPointer: { type: "shadow" },
      formatter: rankingTooltipFormatter(items, valueName, themeTypography),
    }, host),
    grid: {
      left: gridLeftResolved,
      right: gridRight,
      top: gridTop,
      bottom: gridBottom,
      containLabel: false,
    },
    xAxis: {
      type: "value",
      min: 0,
      max: rankingValueAxisMax(values),
      splitLine: { show: false },
      axisLabel: { show: false },
      axisTick: { show: false },
      axisLine: { show: false },
    },
    yAxis: [
      {
        type: "category",
        data: categories,
        inverse: true,
        triggerEvent: true,
        axisLabel: {
          show: true,
          interval: 0,
          align: "right",
          color: canvasThemeColor(host, "text_body"),
          fontFamily: uiFontFamily,
          fontSize: labelFontPx,
          fontWeight: 500,
          width: labelWidthPx,
          margin: 4,
          overflow: "truncate",
          ellipsis: "…",
          formatter: (value) =>
            formatRankingNameLabel(value, resolvedMaxChars).display,
          ...echartsTextStrokeStyle(canvasThemeColor(host, "text_body")),
        },
        axisTick: { show: false },
        axisLine: { show: false },
      },
      {
        type: "category",
        data: categories,
        inverse: true,
        position: "right",
        triggerEvent: true,
        axisLabel: {
          show: true,
          interval: 0,
          color: canvasThemeColor(host, "text_value"),
          fontFamily: uiFontFamily,
          fontSize: valueFontPx,
          fontWeight: 600,
          margin: 2,
          formatter: (_value, index) => valueTexts[index] || "",
          ...echartsTextStrokeStyle(canvasThemeColor(host, "text_value")),
        },
        axisTick: { show: false },
        axisLine: { show: false },
      },
    ],
    series: [
      buildRankingBarSeries({
        values,
        valueName,
        barMaxWidth,
        barCategoryGap: compact ? "18%" : "22%",
        theme,
        borderRadius,
        props,
        valueLabel: { show: false },
      }),
    ],
  };
  return {
    option,
    fullLabels: items.map((item) => item.label),
    rowCount: items.length,
    meta: `排名 ${items.length} 项 · 左侧标签最多约 ${resolvedMaxChars} 字（点击查看全文）`,
  };
}

function buildRankingLabelAboveOption(rows, mapping, props, diagnostics) {
  const { items, valueName } = buildRankingItems(rows, mapping, diagnostics);
  const theme = resolveRankingTheme(props);
  const configuredMaxChars = resolveRankingLabelMaxChars(props, "above");
  const values = items.map((item) => item.value);
  const compact = props.compact === true || props.compact === "true";
  const chartHeight = Number(props.chartHeight) > 0 ? Number(props.chartHeight) : 0;
  const gridLeft = compact ? 4 : 10;
  const gridRight = compact ? 36 : 44;
  let gridTop = compact ? 4 : 16;
  let gridBottom = compact ? 4 : 12;
  let barMaxWidth = 10;
  let labelOffsetPx = 20;
  let labelWidth = 300;
  let slotPx = 0;
  if (compact && chartHeight > 0 && items.length > 0) {
    const plotH = Math.max(28, chartHeight - gridTop - gridBottom);
    slotPx = plotH / items.length;
    labelOffsetPx = Math.max(18, Math.floor(slotPx * 0.5));
    barMaxWidth = Math.min(12, Math.max(6, Math.floor(slotPx * 0.22)));
    labelWidth = Math.max(160, Math.floor(chartHeight * 2.1));
  }
  const typography = resolveRankingTypography(props, slotPx);
  const themeTypography = readThemeTypography(props.__host);
  const maxChars = Math.min(
    configuredMaxChars,
    estimateRankingMaxChars(labelWidth, typography.labelFontSize),
  );
  const borderRadius = [0, 0, 0, 0];
  const host = props.__host;
  const option = {
    tooltip: echartsTooltip(themeTypography, "axis", {
      axisPointer: { type: "shadow" },
      formatter: rankingTooltipFormatter(items, valueName, themeTypography),
    }, host),
    grid: {
      left: gridLeft,
      right: gridRight,
      top: gridTop,
      bottom: gridBottom,
      containLabel: false,
    },
    xAxis: {
      type: "value",
      min: 0,
      max: rankingValueAxisMax(values),
      splitLine: { show: false },
      axisLabel: { show: false },
      axisTick: { show: false },
      axisLine: { show: false },
    },
    yAxis: {
      type: "category",
      data: items.map((_, index) => String(index + 1)),
      inverse: true,
      axisLabel: { show: false },
      axisTick: { show: false },
      axisLine: { show: false },
    },
    series: [
      buildRankingBarSeries({
        values,
        valueName,
        barMaxWidth,
        barCategoryGap: compact && chartHeight > 0 ? "28%" : "32%",
        theme,
        borderRadius,
        valueLabel: {
          show: true,
          position: [4, -labelOffsetPx],
          align: "left",
          verticalAlign: "bottom",
          fontSize: typography.labelFontSize,
          lineHeight: typography.labelLineHeight,
          width: labelWidth,
          overflow: "truncate",
          ellipsis: "…",
          color: canvasThemeColor(host, "text_body"),
          formatter: (params) => {
            const item = items[params.dataIndex];
            if (!item) return "";
            const name = formatRankingNameLabel(item.label, maxChars);
            return `{name|${name.display}}\n{val|${item.value}}`;
          },
          rich: {
            name: {
              fontSize: typography.labelFontSize,
              fontWeight: 500,
              color: canvasThemeColor(host, "text_body"),
              align: "left",
              lineHeight: typography.labelLineHeight,
              width: labelWidth,
              overflow: "truncate",
            },
            val: {
              fontSize: typography.valueFontSize,
              fontWeight: 600,
              color: canvasThemeColor(host, "text_unit"),
              align: "right",
              lineHeight: Math.round(typography.valueFontSize * 1.25),
            },
          },
        },
      }),
    ],
  };
  return {
    option,
    fullLabels: items.map((item) => item.label),
    rowCount: items.length,
    meta: `排名 ${items.length} 项 · 标签置顶（约 ${maxChars} 字截断，点击查看全文）`,
  };
}

function isRankingCompact(props) {
  return props?.compact === true || props?.compact === "true";
}

function resolveRankingShellHeight(props, rowCount) {
  const compact = isRankingCompact(props);
  const chartHeight = Number(props?.chartHeight) > 0 ? Number(props.chartHeight) : 0;
  const layout = resolveRankingLayout(props);
  if (compact && chartHeight > 0) {
    return chartHeight;
  }
  const count = Number(rowCount) > 0 ? Number(rowCount) : 0;
  if (count > 0) {
    const perRow = layout === "above" || layout === "bar" ? 50 : 42;
    return Math.max(200, count * perRow);
  }
  return compact ? 64 : 260;
}

function resolveRankingLabelMaxChars(props, layout) {
  const direct = Number(props?.label_max_chars);
  if (Number.isFinite(direct) && direct > 0) return Math.floor(direct);
  const fromTheme = Number(props?._mei?.components?.chart_ranking?.label_max_chars);
  if (Number.isFinite(fromTheme) && fromTheme > 0) return Math.floor(fromTheme);
  return layout === "above" ? 20 : 20;
}

function truncateRankingLabel(text, maxChars) {
  const chars = [...String(text ?? "")];
  if (chars.length <= maxChars) return String(text ?? "");
  return `${chars.slice(0, maxChars).join("")}…`;
}

function inferLabelField(rows) {
  if (!Array.isArray(rows) || rows.length === 0) return "";
  const first = rows.find((row) => row && typeof row === "object");
  if (!first) return "";
  const keys = Object.keys(first);
  return keys.find((key) => key !== "value" && typeof first[key] === "string") || keys[0] || "";
}

function normalizeKind(kind) {
  if (kind === "chart.trend") return "trend";
  if (kind === "chart.ranking") return "ranking";
  if (kind === "chart.ranking-bar" || kind === "ranking_bar") return "ranking-bar";
  return String(kind || "").replace("chart.", "");
}

function groupNumbersBy(rows, keyField, valueField) {
  const out = {};
  for (const row of rows) {
    const key = String(row?.[keyField] ?? "");
    const value = toNumber(row?.[valueField]);
    if (!key || !Number.isFinite(value)) continue;
    if (!out[key]) out[key] = [];
    out[key].push(value);
  }
  return out;
}

function toBoxStats(values) {
  const sorted = [...values].sort((a, b) => a - b);
  if (sorted.length === 0) return [0, 0, 0, 0, 0];
  return [
    sorted[0],
    quantile(sorted, 0.25),
    quantile(sorted, 0.5),
    quantile(sorted, 0.75),
    sorted[sorted.length - 1],
  ];
}

function quantile(sorted, q) {
  const pos = (sorted.length - 1) * q;
  const base = Math.floor(pos);
  const rest = pos - base;
  if (sorted[base + 1] !== undefined) {
    return sorted[base] + rest * (sorted[base + 1] - sorted[base]);
  }
  return sorted[base];
}

function normalizeRadarData(data) {
  if (!Array.isArray(data) || data.length === 0) return;
  const dims = data[0].value.length;
  for (let idx = 0; idx < dims; idx += 1) {
    const max = Math.max(...data.map((item) => toNumber(item.value[idx])), 1);
    for (const item of data) {
      item.value[idx] = Number(((toNumber(item.value[idx]) / max) * 100).toFixed(2));
    }
  }
}

function maxByField(rows, field) {
  const values = rows.map((row) => toNumber(row?.[field])).filter((value) => Number.isFinite(value));
  if (values.length === 0) return 0;
  return Math.max(...values);
}

function resolveDatasetSource(props) {
  const direct = props?.data || props?.value || null;
  if (direct && typeof direct === "object" && Array.isArray(direct.rows)) {
    return direct;
  }
  return props?.dataset?.dataset || props?.dataset || {};
}

function toNumber(value) {
  const n = Number(value);
  return Number.isFinite(n) ? n : NaN;
}

function unique(items) {
  return [...new Set(items)];
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}
