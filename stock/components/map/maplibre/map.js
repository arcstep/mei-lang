import {
  deferUntilDisplayed,
  fetchRuntimeMetrics,
  findRuntimeMetricInResults,
  parseProps,
  queryStateIdOf,
  recordRuntimeDatasetQueryError,
  runtimeCallerMeta,
  setQueryStateFilter,
  shouldReactToPreviewUpdated,
  subscribeQueryState,
  escapeHtml,
} from "../../dataset/runtime-query.js";
import {
  MAP_DATA_LABEL_FONT,
  basemapLabelLayers,
  buildBasemapStyle,
  buildMapLayerMetricPropsPatch,
  buildValueByCode,
  choroplethRange,
  collectMapLayerMetricRefs,
  dispatchMapSelection,
  enrichFeatureWithLayerMetrics,
  enrichGeoJsonWithLayerMetrics,
  inferLayerMetricLabel,
  mapLayersNeedRuntimeMetrics,
  normalizeJoinCode,
  normalizeMapSpec,
  resolveLayerDataLabels,
  resolveLayerDataPayload,
  resolveFeatureJoinKey,
  resolveLayerJoinKey,
  resolveLayerSource,
  valueToColor,
} from "../../gis/layer-spec.js";
import { createComponentTracer } from "../../perf/render-trace.js";
import { ensureMapLibreGlobal } from "../../vendor/runtime-libs.js";

const TAG = "mei-map-maplibre";
const MAPLIBRE_LOCAL_CSS = "/workspace-components/vendor/maplibre/maplibre-gl.css";

const LAYER_TOGGLE_ICON_HTML = `<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
  <path d="M12 4 4 8l8 4 8-4-8-4Z"></path>
  <path d="m4 12 8 4 8-4"></path>
  <path d="m4 16 8 4 8-4"></path>
</svg>`;

function basemapTileJsonUrl(basemap) {
  const base = String(basemap?.tilesUrl || "").trim().replace(/\/+$/, "");
  const path = String(basemap?.tilesJsonPath || "").trim();
  if (!base) return path || "(unknown TileJSON URL)";
  if (!path) return base;
  return path.startsWith("/") ? `${base}${path}` : `${base}/${path}`;
}

function basemapUnavailableMessage(basemap, error) {
  const tilejsonUrl = basemapTileJsonUrl(basemap);
  const detail = String(error?.message || error || "").trim();
  return [
    `地图底图服务不可访问：${tilejsonUrl}`,
    "当前场景使用了地图底图，请确认对应 TileJSON / 瓦片服务已启动并可访问，或在 mapSpec.basemap 中改为可用地址。",
    detail ? `详情：${detail}` : "",
  ]
    .filter(Boolean)
    .join(" ");
}

if (!customElements.get(TAG)) {
  class MeiMapMaplibreElement extends HTMLElement {
    connectedCallback() {
      this._cleanupDefer = deferUntilDisplayed(this, () => {
        this._cleanupDefer = null;
        this.bootstrap();
      });
    }

    disconnectedCallback() {
      if (typeof this._cleanupDefer === "function") {
        this._cleanupDefer();
      }
      if (this._refreshFrame) {
        cancelAnimationFrame(this._refreshFrame);
        this._refreshFrame = null;
      }
      if (this._layerControlLayoutFrame) {
        cancelAnimationFrame(this._layerControlLayoutFrame);
        this._layerControlLayoutFrame = null;
      }
      this._resizeObserver?.disconnect();
      this._resizeObserver = null;
      if (typeof this._onPreviewUpdated === "function") {
        window.removeEventListener("meilang:preview-updated", this._onPreviewUpdated);
      } else {
        window.removeEventListener("meilang:preview-updated", this.refresh);
      }
      if (typeof this._unsubscribeQueryState === "function") {
        this._unsubscribeQueryState();
        this._unsubscribeQueryState = null;
      }
      document.removeEventListener("mei:manage-tab-change", this._onManageTabChange);
      window.removeEventListener("pageshow", this._onPageShow);
      if (typeof this._onDocumentPointerDown === "function") {
        document.removeEventListener("pointerdown", this._onDocumentPointerDown, true);
        this._onDocumentPointerDown = null;
      }
      if (typeof this._onDocumentKeyDown === "function") {
        document.removeEventListener("keydown", this._onDocumentKeyDown, true);
        this._onDocumentKeyDown = null;
      }
      if (this.map) {
        this.map.remove();
        this.map = null;
      }
      this.clearPopup();
    }

    bootstrap() {
      const props = parseProps(this);
      this._queryStateId = queryStateIdOf(props);
      this._renderTrace = createComponentTracer(this, TAG, {});
      if (!this.shadowRoot) {
        this.attachShadow({ mode: "open" });
      }
      if (!this.shadowRoot.querySelector(".map")) {
        this.shadowRoot.innerHTML = shellHtml(props);
      }
      this.mapContainer = this.shadowRoot.querySelector(".map");
      this.layerControlEl = this.shadowRoot.querySelector(".layer-control");
      this.layerToggleEl = this.shadowRoot.querySelector(".layer-toggle");
      this.statusEl =
        this.shadowRoot.querySelector(".status-focal") ||
        this.shadowRoot.querySelector(".status");
      this.errorEl = this.shadowRoot.querySelector(".error");
      this._layerRegistry = this._layerRegistry || {};
      this._layerVisibility = this._layerVisibility || {};
      this._runtimeLayerProps = this._runtimeLayerProps || null;
      this._sharedFilters = this._sharedFilters || {};
      this._boundLayerEvents = this._boundLayerEvents || new Set();
      this._mapStyleReady = false;
      this._syncLayersTask = null;
      this._layerMetricsTask = null;
      this._layerControlOpen = this._layerControlOpen === true;
      this.refresh = () => this.scheduleRefresh();
      this._onManageTabChange = () => this.scheduleRefresh();
      this._onPageShow = () => this.scheduleRefresh();
      this._onPreviewUpdated = (event) => {
        if (!shouldReactToPreviewUpdated(event, this)) {
          return;
        }
        this.refresh();
      };
      window.addEventListener("meilang:preview-updated", this._onPreviewUpdated);
      document.addEventListener("mei:manage-tab-change", this._onManageTabChange);
      window.addEventListener("pageshow", this._onPageShow);
      this.bindLayerToggleEvents();
      if (!this._onDocumentPointerDown) {
        this._onDocumentPointerDown = (event) => {
          const path = typeof event.composedPath === "function" ? event.composedPath() : [];
          if (path.includes(this)) return;
          this.setLayerControlOpen(false);
        };
        document.addEventListener("pointerdown", this._onDocumentPointerDown, true);
      }
      if (!this._onDocumentKeyDown) {
        this._onDocumentKeyDown = (event) => {
          if (event.key === "Escape") {
            this.setLayerControlOpen(false);
          }
        };
        document.addEventListener("keydown", this._onDocumentKeyDown, true);
      }
      this._renderTrace.mark("bootstrap", {
        query_state_id: this._queryStateId || "",
      });
      this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
        this._sharedFilters = state?.filters || {};
        void this.refreshLayerMetrics();
      });
      this.scheduleRefresh({ forceRender: !this.map });
    }

    effectiveProps() {
      const base = parseProps(this);
      if (!this._runtimeLayerProps) return base;
      return { ...base, ...this._runtimeLayerProps };
    }

    async refreshLayerMetrics(options = {}) {
      const sync = options.sync !== false;
      const task = async () => {
        if (!this.isConnected || !this.map || !this._mapStyleReady) {
          return false;
        }
        const props = parseProps(this);
        const { layers } = normalizeMapSpec(props);
        if (!mapLayersNeedRuntimeMetrics(layers, props)) {
          this._runtimeLayerProps = null;
          return false;
        }
        const refs = collectMapLayerMetricRefs(layers, props);
        if (!refs.length) return false;
        const anchorProps = {
          ...props,
          value: { __mei_runtime_ref: refs[0].ref },
        };
        const metricIds = refs.map((item) => item.metricId);
        this._renderTrace?.mark("runtime_query_start", {
          metric_count: metricIds.length,
        });
        try {
          const result = await fetchRuntimeMetrics(anchorProps, {
            metricIds,
            queryStateId: this._queryStateId,
            filters: this._sharedFilters,
            meta: runtimeCallerMeta(this, TAG),
          });
          if (!this.isConnected || !this.map) {
            return false;
          }
          if (!result) {
            this._renderTrace?.mark("runtime_query_skip", { reason: "capability_or_ref" });
            return false;
          }
          this._runtimeLayerProps = buildMapLayerMetricPropsPatch(
            props,
            layers,
            result,
            findRuntimeMetricInResults,
          );
          this._renderTrace?.mark("runtime_query_done", {
            metric_count: Array.isArray(result?.metrics) ? result.metrics.length : 0,
            client_total_ms: result?.perf?.client_total_ms ?? "",
            server_total_ms:
              result?.perf?.server_handler_total_ms ?? result?.perf?.total_ms ?? "",
          });
          if (sync) {
            const effectiveProps = this.effectiveProps();
            const { layers: normalizedLayers } = normalizeMapSpec(effectiveProps);
            await this.syncLayers(normalizedLayers, effectiveProps);
          }
          return true;
        } catch (error) {
          const message = String(error?.message || error);
          this._renderTrace?.mark("runtime_query_error", { message });
          const meta = runtimeCallerMeta(this, TAG);
          recordRuntimeDatasetQueryError({
            kind: "metric_query",
            datasetId: refs[0]?.ref?.dataset_id || "",
            message,
            sceneId: meta.scene_id,
            target: meta.target,
            component: meta.component || TAG,
            panelId: meta.panel_id,
            metricId: metricIds.join(","),
            phase: "map_layer_metrics",
          });
          return false;
        }
      };
      this._layerMetricsTask = (this._layerMetricsTask || Promise.resolve()).then(task);
      return this._layerMetricsTask;
    }

    scheduleRefresh(options = {}) {
      if (options.forceRender) {
        this._forceRenderPending = true;
      }
      if (this._refreshFrame) {
        cancelAnimationFrame(this._refreshFrame);
      }
      this._refreshFrame = requestAnimationFrame(() => {
        this._refreshFrame = null;
        void this.flushRefresh();
      });
    }

    async flushRefresh() {
      if (!this.isConnected) return;
      if (this._refreshInFlight) {
        this._refreshQueued = true;
        return;
      }
      this._refreshInFlight = true;
      try {
        const domProps = parseProps(this);
        const signature = stablePropsSignature(domProps);
        const needsFullRender =
          this._forceRenderPending ||
          !this.map ||
          this._propsSignature !== signature;
        this._forceRenderPending = false;
        if (needsFullRender) {
          this._propsSignature = signature;
          this._runtimeLayerProps = null;
          this._syncLayersTask = null;
          this._layerMetricsTask = null;
          const { basemap, layers } = normalizeMapSpec(domProps);
          const layout = resolveMapLayout(domProps, basemap);
          this._layout = layout;
          this.applyViewportChrome(layout);
          this.renderLayerControl(layers, domProps);
          await this.renderMap(domProps, basemap, layers, layout);
        } else {
          const props = this.effectiveProps();
          const { basemap, layers } = normalizeMapSpec(props);
          const layout = resolveMapLayout(props, basemap);
          this._layout = layout;
          this.applyViewportChrome(layout);
          this.renderLayerControl(layers, props);
          this.applyMapViewportPadding(layout);
          this.map?.resize();
          if (mapLayersNeedRuntimeMetrics(layers, domProps)) {
            void this.refreshLayerMetrics();
          }
        }
        this.scheduleLayerControlLayout();
      } finally {
        this._refreshInFlight = false;
        if (this._refreshQueued) {
          this._refreshQueued = false;
          this.scheduleRefresh();
        }
      }
    }

    reportRuntimeError(kind, basemap, error, extra = {}) {
      const detail = String(error?.message || error || kind || "runtime error").trim();
      if (!detail) return;
      const meta = runtimeCallerMeta(this, TAG);
      const api = basemapTileJsonUrl(basemap);
      const phase = String(extra.phase || "").trim();
      const dedupeKey =
        kind === "map_runtime_error"
          ? [kind, phase, api, meta.panel_id || ""].join("|")
          : [kind, phase, api, detail, meta.panel_id || ""].join("|");
      const now = Date.now();
      if (this._lastRuntimeErrorKey === dedupeKey && now - (this._lastRuntimeErrorAt || 0) < 3000) {
        return;
      }
      this._lastRuntimeErrorKey = dedupeKey;
      this._lastRuntimeErrorAt = now;
      recordRuntimeDatasetQueryError({
        kind: String(kind || "map_runtime_error"),
        datasetId: "__maplibre__",
        api,
        message: detail,
        sceneId: meta.scene_id,
        target: meta.target,
        component: meta.component || TAG,
        panelId: meta.panel_id,
        phase,
      });
      if (typeof console !== "undefined" && typeof console.error === "function") {
        console.error(`[${TAG}] ${kind}`, {
          api,
          phase,
          message: detail,
          panelId: meta.panel_id || "",
          sceneId: meta.scene_id || "",
          target: meta.target || "",
        });
      }
    }

    async renderMap(props, basemap, layers, layout) {
      const renderToken = (this._renderToken || 0) + 1;
      this._renderToken = renderToken;
      try {
        this._renderTrace?.mark("maplibre_load_start");
        await ensureMapLibre();
        this._renderTrace?.mark("maplibre_load_done");
        if (!this.isConnected || renderToken !== this._renderToken) {
          return;
        }
        const maplibregl = window.maplibregl;
        if (this.map) {
          this.detachLayerToggleFromMap();
          this.map.remove();
          this.map = null;
        }
        this._boundLayerEvents = new Set();
        this._mapStyleReady = false;
        this.map = new maplibregl.Map({
          container: this.mapContainer,
          center: basemap.center,
          zoom: basemap.defaultZoom ?? basemap.zoom ?? 11,
          minZoom: basemap.minZoom ?? 10,
          maxZoom: basemap.maxZoom ?? 18,
          style: buildBasemapStyle(basemap),
          attributionControl: false,
        });
        const map = this.map;
        map.addControl(new maplibregl.NavigationControl(), "top-right");
        this.mountLayerToggleInNav();
        map.on("load", async () => {
          if (!this.isConnected || renderToken !== this._renderToken || this.map !== map) {
            return;
          }
          try {
            this._mapStyleReady = true;
            this._renderTrace?.mark("style_load", {
              layer_count: layers.length,
            });
            this.applyMapViewportPadding(layout);
            this._renderTrace?.mark("sync_layers_start", {
              layer_count: layers.length,
            });
            const domProps = parseProps(this);
            let syncProps = domProps;
            const metricLayers = normalizeMapSpec(domProps).layers;
            try {
              if (mapLayersNeedRuntimeMetrics(metricLayers, domProps)) {
                await this.refreshLayerMetrics({ sync: false });
                syncProps = this.effectiveProps();
              }
            } catch (_) {
              syncProps = domProps;
            }
            const { layers: syncLayerList } = normalizeMapSpec(syncProps);
            await this.syncLayers(syncLayerList, syncProps);
            this._renderTrace?.mark("sync_layers_done", {
              layer_count: syncLayerList.length,
            });
            this.ensureBasemapLabels(basemap);
            const labelsOn = basemap.showLabels !== false && basemap.show_labels !== false;
            if (this.statusEl) {
              this.statusEl.textContent = `底图 ${basemap.tilesUrl} · 业务层 ${layers.length}${labelsOn ? " · 标注开" : ""}`;
            }
            this.errorEl.textContent = "";
            this.map?.resize();
            this.mountLayerToggleInNav();
            this.scheduleLayerControlLayout();
            this._renderTrace?.mark("render_done", {
              layer_count: syncLayerList.length,
            });
          } catch (err) {
            const message = String(err?.message || err);
            this.errorEl.textContent = message;
            this._renderTrace?.mark("render_error", {
              message,
              tilejson_url: basemapTileJsonUrl(basemap),
            });
            this.reportRuntimeError("map_render_error", basemap, message, {
              phase: "sync_layers",
            });
            this.renderLayerControl(layers, props);
            this.mountLayerToggleInNav();
            this.scheduleLayerControlLayout();
          }
        });
        this.bindMapResize(this._layout?.fill ?? layout.fill);
        map.on("error", (event) => {
          if (!this.isConnected || renderToken !== this._renderToken || this.map !== map) {
            return;
          }
          const message = basemapUnavailableMessage(basemap, event?.error);
          this.errorEl.textContent = message;
          this._renderTrace?.mark("map_error", {
            message: String(event?.error?.message || event?.error || "map error"),
            tilejson_url: basemapTileJsonUrl(basemap),
          });
          this.reportRuntimeError("map_runtime_error", basemap, message, {
            phase: "style_or_tile_load",
          });
        });
      } catch (error) {
        const message = String(error?.message || error);
        this.errorEl.textContent = message;
        this._renderTrace?.mark("render_error", {
          message,
          tilejson_url: basemapTileJsonUrl(basemap),
        });
        this.reportRuntimeError("map_render_error", basemap, message, {
          phase: "map_init",
        });
      }
    }

    async syncLayers(layers, props) {
      const task = async () => {
        if (!this.map) return;
        const nextRegistry = {};
        for (const layer of layers) {
          if (!this.map) return;
          const id = String(layer?.id || "").trim();
          if (!id) continue;
          if (this._layerVisibility[id] === undefined) {
            this._layerVisibility[id] = layer.visible !== false;
          }
          await this.addLayerSpec(layer, props, nextRegistry);
          this.setLayerVisible(id, this._layerVisibility[id] !== false, nextRegistry);
        }
        if (!this.map) return;
        this._layerRegistry = nextRegistry;
        this.renderLayerControl(layers, props);
      };
      this._syncLayersTask = (this._syncLayersTask || Promise.resolve()).then(task);
      return this._syncLayersTask;
    }

    renderLayerControl(layers, props) {
      if (!this.layerControlEl || !this.layerToggleEl) return;
      const show =
        props.layerControl !== false &&
        props.layer_control !== false &&
        layers.some((layer) => String(layer?.label || "").trim());
      if (!show) {
        this.layerControlEl.innerHTML = "";
        if (this.layerToggleEl) {
          this.layerToggleEl.hidden = true;
        }
        this.detachLayerToggleFromMap();
        this.layerControlEl.hidden = true;
        this._layerControlOpen = false;
        return;
      }
      const toggle = this.ensureLayerToggleElement();
      if (!toggle) return;
      toggle.hidden = false;
      const items = layers
        .filter((layer) => String(layer?.label || "").trim())
        .map((layer) => {
          const id = String(layer.id).trim();
          const checked = this._layerVisibility[id] !== false;
          return `<label class="layer-item">
            <input type="checkbox" data-layer-id="${escapeHtml(id)}" ${checked ? "checked" : ""}/>
            <span>${escapeHtml(String(layer.label))}</span>
          </label>`;
        })
        .join("");
      this.layerControlEl.innerHTML = `
        <div class="layer-control-head">
          <div class="layer-control-title">图层</div>
          <button type="button" class="layer-control-close" aria-label="关闭图层面板" title="关闭">×</button>
        </div>
        <div class="layer-control-list">${items}</div>
      `;
      this.bindLayerControlPanelEvents();
      this.layerControlEl.querySelectorAll("input[data-layer-id]").forEach((input) => {
        input.addEventListener("change", () => {
          const layerId = input.getAttribute("data-layer-id");
          this._layerVisibility[layerId] = input.checked;
          this.setLayerVisible(layerId, input.checked);
        });
      });
      this.mountLayerToggleInNav();
    }

    bindLayerControlPanelEvents() {
      if (!this.layerControlEl || this._layerControlPanelBound) return;
      this.layerControlEl.addEventListener("click", (event) => {
        const closeBtn = event.target.closest(".layer-control-close");
        if (!closeBtn) return;
        event.preventDefault();
        event.stopPropagation();
        this.setLayerControlOpen(false);
      });
      this._layerControlPanelBound = true;
    }

    setLayerControlOpen(open) {
      this._layerControlOpen = open === true;
      if (this.layerControlEl) {
        this.layerControlEl.hidden = !this._layerControlOpen;
      }
      if (this.layerToggleEl) {
        this.layerToggleEl.setAttribute("aria-pressed", this._layerControlOpen ? "true" : "false");
        this.layerToggleEl.setAttribute(
          "title",
          this._layerControlOpen ? "收起图层面板" : "打开图层面板",
        );
      }
      this.scheduleLayerControlLayout();
    }

    bindLayerToggleEvents() {
      const btn = this.ensureLayerToggleElement();
      if (!btn || this._layerToggleBound) return;
      btn.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        this.setLayerControlOpen(!this._layerControlOpen);
      });
      this._layerToggleBound = true;
    }

    ensureLayerToggleElement() {
      const wrap = this.shadowRoot?.querySelector(".map-wrap");
      if (!wrap) return null;
      if (this.layerToggleEl?.isConnected) {
        return this.layerToggleEl;
      }
      const btn = document.createElement("button");
      btn.className = "layer-toggle";
      btn.type = "button";
      btn.hidden = true;
      btn.setAttribute("aria-pressed", "false");
      btn.title = "打开图层面板";
      btn.innerHTML = LAYER_TOGGLE_ICON_HTML;
      const panel = wrap.querySelector(".layer-control");
      wrap.insertBefore(btn, panel || null);
      this.layerToggleEl = btn;
      this._layerToggleBound = false;
      this.bindLayerToggleEvents();
      return btn;
    }

    detachLayerToggleFromMap() {
      const wrap = this.shadowRoot?.querySelector(".map-wrap");
      const btn = this.layerToggleEl;
      if (!wrap || !btn) return;
      if (btn.parentElement === wrap) return;
      btn.classList.remove("maplibregl-ctrl", "mei-layer-toggle");
      btn.style.width = "";
      btn.style.height = "";
      btn.style.top = "";
      btn.style.right = "";
      btn.style.left = "";
      btn.style.position = "";
      const panel = wrap.querySelector(".layer-control");
      wrap.insertBefore(btn, panel || null);
    }

    /** 将图层按钮挂入 MapLibre NavigationControl 的 ctrl-group（作为第四枚工具钮） */
    mountLayerToggleInNav() {
      const btn = this.layerToggleEl;
      if (!btn || btn.hidden) return;
      const group = this.mapContainer?.querySelector(
        ".maplibregl-ctrl-top-right .maplibregl-ctrl-group",
      );
      if (!group) return;
      btn.classList.add("maplibregl-ctrl", "mei-layer-toggle");
      if (btn.parentElement !== group) {
        group.appendChild(btn);
      }
      btn.style.width = "";
      btn.style.height = "";
      btn.style.top = "";
      btn.style.right = "";
      btn.style.left = "";
      btn.style.position = "";
      this.positionLayerControlPanel();
    }

    scheduleLayerControlLayout() {
      if (this._layerControlLayoutFrame) {
        cancelAnimationFrame(this._layerControlLayoutFrame);
      }
      this._layerControlLayoutFrame = requestAnimationFrame(() => {
        this._layerControlLayoutFrame = null;
        this.mountLayerToggleInNav();
        this.positionLayerControlPanel();
      });
    }

    positionLayerControlPanel() {
      const wrap = this.shadowRoot?.querySelector(".map-wrap");
      if (
        !wrap ||
        !this.layerToggleEl ||
        this.layerToggleEl.hidden ||
        !this._layerControlOpen ||
        !this.layerControlEl ||
        this.layerControlEl.hidden
      ) {
        return;
      }
      const gap = 8;
      const wrapRect = wrap.getBoundingClientRect();
      const navGroup = this.mapContainer?.querySelector(
        ".maplibregl-ctrl-top-right .maplibregl-ctrl-group",
      );
      const navRect = navGroup?.getBoundingClientRect();
      const toggleRect = this.layerToggleEl.getBoundingClientRect();
      const anchorRect = navRect || toggleRect;
      const focus = this._layout?.focusInsetPx;
      const anchorBottomInWrap = Math.round(
        (navRect?.bottom ?? toggleRect.bottom) - wrapRect.top,
      );
      const anchorRightInWrap = Math.round(wrapRect.right - anchorRect.right + gap);

      this.layerControlEl.style.transform = "none";
      this.layerControlEl.style.right = `${anchorRightInWrap}px`;
      this.layerControlEl.style.left = "auto";

      if (focus && navRect) {
        const focusBottom = Number(focus.bottom) || 0;
        const focusTop = Number(focus.top) || 0;
        const focusLeft = Number(focus.left) || 0;
        let panelTop = anchorBottomInWrap + gap;
        let maxHeight = wrapRect.height - panelTop - focusBottom - gap;
        if (maxHeight < 160) {
          panelTop = Math.max(focusTop + gap, anchorBottomInWrap + gap);
          maxHeight = wrapRect.height - panelTop - focusBottom - gap;
        }
        this.layerControlEl.style.top = `${panelTop}px`;
        this.layerControlEl.style.bottom = "auto";
        this.layerControlEl.style.maxHeight = `${Math.max(120, Math.round(maxHeight))}px`;
        const panelWidth = this.layerControlEl.offsetWidth || 260;
        const panelLeftInWrap = wrapRect.width - anchorRightInWrap - panelWidth;
        if (panelLeftInWrap < focusLeft + gap) {
          this.layerControlEl.style.right = "auto";
          this.layerControlEl.style.left = `${Math.round(focusLeft + gap)}px`;
          this.layerControlEl.style.maxWidth = `${Math.max(
            180,
            Math.round(anchorRect.left - wrapRect.left - focusLeft - gap * 2),
          )}px`;
        }
        return;
      }

      const anchorTopInWrap = Math.round(toggleRect.top - wrapRect.top);
      const spaceAbove = Math.max(120, anchorTopInWrap - gap - 12);
      this.layerControlEl.style.top = "auto";
      this.layerControlEl.style.bottom = `${Math.round(wrapRect.height - anchorTopInWrap + gap)}px`;
      this.layerControlEl.style.maxHeight = `${spaceAbove}px`;
    }

    setLayerVisible(layerId, visible, registry = this._layerRegistry) {
      const entry = registry?.[layerId];
      if (!entry || !this.map) return;
      const v = visible ? "visible" : "none";
      for (const mapLayerId of entry.mapLayerIds) {
        if (this.map.getLayer(mapLayerId)) {
          this.map.setLayoutProperty(mapLayerId, "visibility", v);
        }
      }
    }

    async addLayerSpec(layerSpec, props, registry = this._layerRegistry) {
      const layerId = String(layerSpec.id || "layer").trim();
      const joinKey = resolveLayerJoinKey(layerSpec);
      const layerProps = resolveLayerDataPayload(props, layerSpec);
      this._renderTrace?.mark("layer_source_start", {
        layer_id: layerId,
      });
      const geojson = await resolveLayerSource(layerSpec, layerProps);
      this._renderTrace?.mark("layer_source_ready", {
        layer_id: layerId,
        feature_count: Array.isArray(geojson?.features) ? geojson.features.length : 0,
      });
      const sourceId = `src-${layerId}`;
      if (this.map.getSource(sourceId)) {
        this.map.getSource(sourceId).setData(geojson);
      } else {
        this.map.addSource(sourceId, { type: "geojson", data: geojson });
      }

      const type = String(layerSpec.type || "polygon").toLowerCase();
      const style = layerSpec.style || {};
      const choropleth = layerSpec.choropleth || {};
      const choroplethOn = choropleth.enabled !== false;
      const fillOpacityRaw = style.fillOpacity ?? style.fill_opacity;
      const outlineOnly =
        layerSpec.outlineOnly === true ||
        layerSpec.outline_only === true ||
        (!choroplethOn &&
          (fillOpacityRaw === 0 || fillOpacityRaw === "0"));
      const mapLayerIds = [];

      const valueMap = choroplethOn
        ? buildValueByCode(layerProps, joinKey)
        : {};
      const { min, max, colors } = choroplethRange(
        valueMap,
        choropleth.palette || style.palette,
      );
      const dataLabels = resolveLayerDataLabels(layerSpec, { choroplethOn });

      if (type === "point") {
        const fillLayerId = `fill-${layerId}`;
        const circleColor = style.circleColor || style.circle_color || "#f472b6";
        const circleRadius = Number(style.circleRadius ?? style.circle_radius ?? 5);
        const circleStrokeColor = style.circleStrokeColor || style.circle_stroke_color || "#fce7f3";
        const circleStrokeWidth = Number(style.circleStrokeWidth ?? style.circle_stroke_width ?? 1);
        const pointData = enrichGeoJsonWithLayerMetrics(geojson, {
          joinKey,
          valueMap,
          layerSpec,
          dataLabels,
        });
        this.map.getSource(sourceId).setData(pointData);
        if (!this.map.getLayer(fillLayerId)) {
          this.map.addLayer({
            id: fillLayerId,
            type: "circle",
            source: sourceId,
            paint: {
              "circle-color": circleColor,
              "circle-radius": circleRadius,
              "circle-stroke-color": circleStrokeColor,
              "circle-stroke-width": circleStrokeWidth,
            },
          });
        }
        mapLayerIds.push(fillLayerId);
        this.addDataLabelLayer(layerId, sourceId, dataLabels, mapLayerIds, style);
        this.bindLayerEvents(fillLayerId, layerId, joinKey, layerSpec);
        registry[layerId] = { mapLayerIds, sourceId };
        this._renderTrace?.mark("layer_ready", {
          layer_id: layerId,
          map_layer_count: mapLayerIds.length,
        });
        return;
      }

      const fillId = `fill-${layerId}`;
      const lineId = `line-${layerId}`;
      const fillColor = style.fillColor || "#1e3a5f";
      const dataWithColors = {
        type: "FeatureCollection",
        features: (geojson.features || []).map((feature) => {
          const { code } = resolveFeatureJoinKey(feature.properties, joinKey);
          const value = valueMap[normalizeJoinCode(code)];
          const color =
            value != null && choroplethOn
              ? valueToColor(value, min, max, colors)
              : fillColor;
          const enriched = enrichFeatureWithLayerMetrics(feature, {
            joinKey,
            valueMap,
            layerSpec,
            dataLabels,
          });
          return {
            ...enriched,
            properties: {
              ...enriched.properties,
              __fill: color,
            },
          };
        }),
      };
      this.map.getSource(sourceId).setData(dataWithColors);

      if (!outlineOnly && !this.map.getLayer(fillId)) {
        this.map.addLayer({
          id: fillId,
          type: "fill",
          source: sourceId,
          paint: {
            "fill-color": ["coalesce", ["get", "__fill"], fillColor],
            "fill-opacity":
              fillOpacityRaw != null && fillOpacityRaw !== ""
                ? Number(fillOpacityRaw)
                : 0.45,
          },
        });
        mapLayerIds.push(fillId);
      }
      if (!this.map.getLayer(lineId)) {
        this.map.addLayer({
          id: lineId,
          type: "line",
          source: sourceId,
          paint: {
            "line-color": style.lineColor || "#38bdf8",
            "line-width": style.lineWidth ?? 1.2,
            "line-opacity": style.lineOpacity ?? style.line_opacity ?? 1,
          },
        });
        mapLayerIds.push(lineId);
      }
      if (!outlineOnly) {
        this.bindLayerEvents(fillId, layerId, joinKey, layerSpec);
      }
      this.bindLayerEvents(lineId, layerId, joinKey, layerSpec);
      this.addDataLabelLayer(layerId, sourceId, dataLabels, mapLayerIds, style, { outlineOnly });
      registry[layerId] = { mapLayerIds, sourceId };
      this._renderTrace?.mark("layer_ready", {
        layer_id: layerId,
        map_layer_count: mapLayerIds.length,
      });
    }

    addDataLabelLayer(layerId, sourceId, dataLabels, mapLayerIds, style = {}, options = {}) {
      if (!dataLabels?.enabled || !this.map || options.outlineOnly) return;
      const labelLayerId = `label-${layerId}`;
      if (this.map.getLayer(labelLayerId)) {
        mapLayerIds.push(labelLayerId);
        return;
      }
      this.map.addLayer({
        id: labelLayerId,
        type: "symbol",
        source: sourceId,
        minzoom: Number.isFinite(dataLabels.minZoom) ? dataLabels.minZoom : 10,
        maxzoom: Number.isFinite(dataLabels.maxZoom) ? dataLabels.maxZoom : 22,
        layout: {
          "text-field": ["coalesce", ["get", "__mei_label_text"], ["get", "name"]],
          "text-font": MAP_DATA_LABEL_FONT,
          "text-size": [
            "interpolate",
            ["linear"],
            ["zoom"],
            10,
            Math.max(9, dataLabels.textSize - 1),
            14,
            dataLabels.textSize + 1,
          ],
          "text-anchor": "center",
          "text-line-height": 1.15,
          "text-allow-overlap": false,
          "text-ignore-placement": false,
          "symbol-placement": "point",
        },
        paint: {
          "text-color": dataLabels.textColor,
          "text-halo-color": dataLabels.textHaloColor,
          "text-halo-width": dataLabels.textHaloWidth,
        },
        filter: ["has", "__mei_label_text"],
      });
      mapLayerIds.push(labelLayerId);
    }

    bindLayerEvents(mapLayerId, logicalId, joinKey, layerSpec = {}) {
      if (this._boundLayerEvents?.has(mapLayerId)) {
        return;
      }
      this._boundLayerEvents.add(mapLayerId);
      this.map.on("click", mapLayerId, (event) => {
        const feature = event.features?.[0];
        if (!feature) return;
        const resolved = resolveFeatureJoinKey(feature.properties, joinKey);
        const props = parseProps(this);
        const selectionDimension = String(
          props.selection_dimension || props.selectionDimension || resolved.joinKey || joinKey || ""
        ).trim();
        dispatchMapSelection({
          source: "map.maplibre",
          layerId: logicalId,
          joinKey: resolved.joinKey,
          code: resolved.code,
          name: String(feature.properties?.name || ""),
          properties: feature.properties,
        });
        if (this._queryStateId && selectionDimension && resolved.code) {
          setQueryStateFilter(this._queryStateId, selectionDimension, resolved.code, {
            filterIntentSource: "chart_selection",
            transitionSource: "chart_selection",
          });
        }
        this.showFeaturePopup(event, feature, layerSpec);
      });
      this.map.on("mouseenter", mapLayerId, () => {
        this.map.getCanvas().style.cursor = "pointer";
      });
      this.map.on("mouseleave", mapLayerId, () => {
        this.map.getCanvas().style.cursor = "";
      });
    }

    clearPopup() {
      if (this._popup) {
        this._popup.remove();
        this._popup = null;
      }
    }

    showFeaturePopup(event, feature, layerSpec = {}) {
      if (!this.map || !window.maplibregl) return;
      const html = this.buildPopupHtml(feature, layerSpec);
      if (!html) {
        this.clearPopup();
        return;
      }
      const lngLat = event?.lngLat;
      if (!lngLat) return;
      this.clearPopup();
      this._popup = new window.maplibregl.Popup({
        closeButton: true,
        closeOnClick: true,
        maxWidth: "320px",
      })
        .setLngLat(lngLat)
        .setHTML(html)
        .addTo(this.map);
    }

    buildPopupHtml(feature, layerSpec = {}) {
      const popupFields = Array.isArray(layerSpec.popupFields)
        ? layerSpec.popupFields
        : Array.isArray(layerSpec.popup_fields)
          ? layerSpec.popup_fields
          : Array.isArray(layerSpec.tooltipFields)
            ? layerSpec.tooltipFields
            : Array.isArray(layerSpec.tooltip_fields)
              ? layerSpec.tooltip_fields
              : null;
      const type = String(layerSpec.type || "polygon").trim().toLowerCase();
      const metricLabel = inferLayerMetricLabel(layerSpec);
      const defaults =
        popupFields && popupFields.length > 0
          ? popupFields
          : type === "point"
            ? ["企业名称", "检查次数", "处罚次数", "处罚金额合计", "所属园区", "所属街道"]
            : [
                { field: "name", label: "名称" },
                { field: "__mei_value", label: metricLabel },
              ];
      const rows = [];
      for (const fieldDef of defaults) {
        const field = typeof fieldDef === "string" ? fieldDef : String(fieldDef?.field || "").trim();
        if (!field) continue;
        const label =
          typeof fieldDef === "object" && fieldDef?.label
            ? String(fieldDef.label)
            : field;
        let raw = feature?.properties?.[field];
        if ((raw == null || raw === "") && field === "__mei_value") {
          raw = feature?.properties?.value;
        }
        if (raw == null || raw === "") continue;
        rows.push(
          `<div class="popup-row"><span class="popup-label">${escapeHtml(label)}</span><span class="popup-value">${escapeHtml(String(raw))}</span></div>`,
        );
      }
      if (rows.length === 0) {
        return "";
      }
      return `<div class="popup-wrap">${rows.join("")}</div>`;
    }

    ensureBasemapLabels(basemap) {
      const showLabels = basemap.showLabels !== false && basemap.show_labels !== false;
      if (!showLabels || !this.map) return;
      for (const layer of basemapLabelLayers(basemap)) {
        try {
          if (!this.map.getLayer(layer.id)) {
            this.map.addLayer(layer);
          }
        } catch {
          /* 部分 MBTiles 可能缺少对应 source-layer */
        }
      }
    }

    bindMapResize(fill) {
      this._resizeObserver?.disconnect();
      this._resizeObserver = null;
      if (!fill || !this.mapContainer) return;
      this._resizeObserver = new ResizeObserver(() => {
        if (this.map) this.map.resize();
        this.scheduleLayerControlLayout();
      });
      this._resizeObserver.observe(this.mapContainer);
      const wrap = this.shadowRoot?.querySelector(".map-wrap");
      if (wrap) {
        this._resizeObserver.observe(wrap);
      }
    }

    applyViewportChrome(layout) {
      const wrap = this.shadowRoot?.querySelector(".wrap");
      if (wrap) {
        wrap.classList.toggle("wrap-cockpit-bleed", Boolean(layout?.cockpitBleed));
      }
      const guide = this.shadowRoot?.querySelector(".focus-guide");
      if (guide) {
        guide.hidden = !layout?.cockpitBleed || !layout?.showFocusGuide;
      }
    }

    applyMapViewportPadding(layout) {
      if (!this.map || !layout?.cockpitBleed || !layout.focusInset) return;
      const pad = layout.focusInsetPx;
      if (pad) {
        this.map.setPadding(pad);
      }
    }
  }
  customElements.define(TAG, MeiMapMaplibreElement);
}

function stablePropsSignature(props) {
  try {
    return JSON.stringify(props || {});
  } catch (_) {
    return "";
  }
}

/** 驾驶舱全幅底图 + 中间观察区：地图铺满，控件落在 focusInset 内 */
function resolveMapFocusInset(props, basemap = {}) {
  const mapSpec = props.mapSpec || props.map || {};
  const raw =
    props.mapViewport ||
    props.map_viewport ||
    props.mapFocusInset ||
    props.map_focus_inset ||
    mapSpec.mapViewport ||
    mapSpec.map_viewport ||
    mapSpec.focusInset ||
    mapSpec.focus_inset ||
    basemap.mapViewport ||
    basemap.focusInset;
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const inset = raw.focusInset || raw.focus_inset || raw;
  const top = cssLength(inset.top ?? raw.top, "0px");
  const right = cssLength(inset.right ?? raw.right, "0px");
  const bottom = cssLength(inset.bottom ?? raw.bottom, "0px");
  const left = cssLength(inset.left ?? raw.left, "0px");
  if (top === "0px" && right === "0px" && bottom === "0px" && left === "0px") {
    return null;
  }
  const mode = String(raw.mode || raw.layoutMode || raw.layout_mode || "").trim();
  return {
    mode,
    top,
    right,
    bottom,
    left,
    showFocusGuide:
      raw.showFocusGuide === true ||
      raw.show_focus_guide === true ||
      raw.showFocusFrame === true,
    focusInsetPx: {
      top: cssLengthToPx(top),
      right: cssLengthToPx(right),
      bottom: cssLengthToPx(bottom),
      left: cssLengthToPx(left),
    },
  };
}

function cssLength(value, fallback) {
  if (value == null || value === "") {
    return fallback;
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return `${value}px`;
  }
  const text = String(value).trim();
  return text || fallback;
}

function cssLengthToPx(length) {
  const text = String(length || "").trim();
  const match = text.match(/^([\d.]+)px$/);
  if (match) {
    return Number(match[1]);
  }
  return 0;
}

function resolveMapLayout(props, basemap = {}) {
  const focusInset = resolveMapFocusInset(props, basemap);
  const fill =
    props.mapFill === true ||
    props.mapFill === "true" ||
    String(props.mapHeight ?? "").trim() === "100%";
  const mode = String(
    props.mapLayoutMode ||
      props.map_layout_mode ||
      focusInset?.mode ||
      "",
  ).trim();
  const cockpitBleed =
    mode === "cockpitBleed" ||
    mode === "cockpit_bleed" ||
    (focusInset != null && fill);

  if (cockpitBleed && focusInset) {
    const vars = [
      `--map-focus-top:${focusInset.top}`,
      `--map-focus-right:${focusInset.right}`,
      `--map-focus-bottom:${focusInset.bottom}`,
      `--map-focus-left:${focusInset.left}`,
    ].join(";");
    return {
      fill: true,
      cockpitBleed: true,
      focusInset,
      focusInsetPx: focusInset.focusInsetPx,
      showFocusGuide: focusInset.showFocusGuide,
      host: `display:block;width:100%;height:100%;min-height:0;${vars}`,
      wrap: "position:relative;width:100%;height:100%;min-height:0;overflow:hidden;",
      mapWrap: "position:absolute;inset:0;",
      map: `position:absolute;inset:0;width:100%;height:100%;${vars}`,
      statusPos:
        "left:calc(var(--map-focus-left) + 12px);top:calc(var(--map-focus-top) + 8px);right:calc(var(--map-focus-right) + 56px);",
      navCtrlPos:
        "top:calc(var(--map-focus-top) + 10px) !important;right:calc(var(--map-focus-right) + 10px) !important;",
    };
  }

  if (fill) {
    return {
      fill: true,
      host: "display:block;width:100%;height:100%;min-height:0;",
      wrap: "display:flex;flex-direction:column;gap:6px;height:100%;min-height:0;",
      mapWrap: "position:relative;flex:1 1 auto;min-height:0;",
      map: "position:absolute;inset:0;width:100%;height:100%;",
      statusPos: "",
    };
  }
  const height = Number(props.mapHeight) > 0 ? Number(props.mapHeight) : 420;
  return {
    fill: false,
    host: "display:block;width:100%;min-width:0;",
    wrap: "display:grid;gap:6px;",
    mapWrap: `position:relative;height:${height}px;`,
    map: `width:100%;height:${height}px;`,
    statusPos: "",
  };
}

function shellHtml(props) {
  const title = String(props.title ?? "地图");
  const showTitle = props.showTitle !== false && props.show_title !== false;
  const showStatus = props.showStatus !== false && props.show_status !== false;
  const { basemap } = normalizeMapSpec(props);
  const layout = resolveMapLayout(props, basemap);
  const wrapFrame =
    layout.cockpitBleed
      ? ""
      : "border-radius:14px;border:1px solid rgba(148,163,184,.2);background:#0a1628;";
  const statusInHead = !layout.cockpitBleed && showStatus;
  const statusBlock = layout.cockpitBleed
    ? showStatus
      ? `<div class="status status-focal" style="${layout.statusPos}"></div>`
      : ""
    : "";
  return `
    <link rel="stylesheet" href="${MAPLIBRE_LOCAL_CSS}" />
    <style>
      :host { ${layout.host} }
      .wrap {
        ${layout.wrap}
        ${wrapFrame}
      }
      .wrap:not(.wrap-cockpit-bleed) {
        display: flex;
        flex-direction: column;
        gap: 6px;
      }
      .head {
        display: flex; justify-content: space-between; align-items: baseline;
        flex: 0 0 auto;
        padding: 10px 14px 0; color: #e2e8f0;
      }
      .title { margin: 0; font-size: 14px; }
      .status { font-size: 12px; color: #94a3b8; }
      .status-focal {
        position: absolute; z-index: 6; pointer-events: none;
        line-height: 1.35;
      }
      .map-wrap { ${layout.mapWrap} }
      .map { ${layout.map} }
      .map .maplibregl-ctrl-top-right {
        ${layout.cockpitBleed ? layout.navCtrlPos : ""}
      }
      .map .maplibregl-ctrl-top-right .maplibregl-ctrl-group {
        border: 1px solid rgba(84, 160, 255, 0.35);
        border-radius: 10px;
        overflow: hidden;
        background: linear-gradient(180deg, rgba(11, 31, 56, 0.94) 0%, rgba(8, 22, 40, 0.96) 100%);
        box-shadow: 0 10px 26px rgba(0, 8, 20, 0.42);
      }
      .map .maplibregl-ctrl-group button {
        width: 32px;
        height: 32px;
        background: transparent;
        color: #d8e8ff;
        border: 0;
      }
      .map .maplibregl-ctrl-group button + button {
        border-top: 1px solid rgba(84, 160, 255, 0.18);
      }
      .map .maplibregl-ctrl-group button:hover {
        background: rgba(49, 102, 173, 0.32);
      }
      .map .maplibregl-ctrl-group button:focus-visible {
        outline: 1px solid rgba(125, 211, 252, 0.75);
        outline-offset: -1px;
      }
      .map .maplibregl-ctrl button.maplibregl-ctrl-zoom-in .maplibregl-ctrl-icon,
      .map .maplibregl-ctrl button.maplibregl-ctrl-zoom-out .maplibregl-ctrl-icon,
      .map .maplibregl-ctrl button.maplibregl-ctrl-compass .maplibregl-ctrl-icon {
        filter: brightness(1.55) saturate(0.72) hue-rotate(180deg);
      }
      .focus-guide {
        position: absolute;
        top: var(--map-focus-top);
        right: var(--map-focus-right);
        bottom: var(--map-focus-bottom);
        left: var(--map-focus-left);
        border: 1px dashed rgba(56,189,248,.28);
        border-radius: 8px;
        pointer-events: none;
        z-index: 3;
        box-shadow: inset 0 0 0 1px rgba(15,23,42,.15);
      }
      .map .maplibregl-ctrl-group button.mei-layer-toggle {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 32px;
        height: 32px;
        padding: 0;
        color: #d8e8ff;
        cursor: pointer;
      }
      .map .maplibregl-ctrl-group button.mei-layer-toggle:hover {
        background: rgba(49, 102, 173, 0.32);
      }
      .map .maplibregl-ctrl-group button.mei-layer-toggle[aria-pressed="true"] {
        background: rgba(49, 102, 173, 0.42);
        box-shadow: inset 0 0 0 1px rgba(125, 211, 252, 0.35);
      }
      .map .maplibregl-ctrl-group button.mei-layer-toggle svg {
        width: 18px;
        height: 18px;
        pointer-events: none;
        filter: brightness(1.55) saturate(0.72) hue-rotate(180deg);
      }
      .layer-control {
        position: absolute;
        z-index: 50;
        top: 0;
        right: 0;
        min-width: 220px;
        max-width: 280px;
        padding: 10px 12px;
        border-radius: 10px;
        border: 1px solid rgba(56,189,248,.35);
        background: rgba(15,23,42,.92);
        color: #e2e8f0;
        font-size: 12px;
        box-shadow: 0 8px 24px rgba(0,0,0,.35);
        pointer-events: auto;
        max-height: min(360px, calc(100% - 24px));
        overflow: auto;
      }
      .layer-control-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        margin-bottom: 8px;
      }
      .layer-control-title {
        font-weight: 600;
        margin: 0;
        color: #f8fafc;
        font-size: 13px;
      }
      .layer-control-close {
        flex: 0 0 auto;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 24px;
        height: 24px;
        padding: 0;
        border: 1px solid rgba(148, 163, 184, 0.35);
        border-radius: 6px;
        background: rgba(30, 41, 59, 0.85);
        color: #e2e8f0;
        font-size: 16px;
        line-height: 1;
        cursor: pointer;
      }
      .layer-control-close:hover {
        background: rgba(49, 102, 173, 0.45);
        border-color: rgba(125, 211, 252, 0.55);
        color: #f8fafc;
      }
      .layer-control-close:focus-visible {
        outline: 1px solid rgba(125, 211, 252, 0.8);
        outline-offset: 2px;
      }
      .layer-control-list { display: grid; gap: 6px; }
      .layer-item {
        display: flex; align-items: flex-start; gap: 8px; cursor: pointer;
        line-height: 1.35;
      }
      .layer-item input { margin-top: 2px; accent-color: #38bdf8; }
      .map .maplibregl-popup-content {
        background: rgba(9, 20, 35, 0.96);
        color: #dbeafe;
        border: 1px solid rgba(56, 189, 248, 0.28);
        border-radius: 10px;
        box-shadow: 0 14px 28px rgba(0, 0, 0, 0.38);
        padding: 10px 12px;
      }
      .map .maplibregl-popup-tip {
        border-top-color: rgba(9, 20, 35, 0.96);
        border-bottom-color: rgba(9, 20, 35, 0.96);
      }
      .popup-wrap { display: grid; gap: 6px; min-width: 180px; }
      .popup-row {
        display: grid;
        grid-template-columns: 88px 1fr;
        gap: 8px;
        align-items: baseline;
      }
      .popup-label { color: #93c5fd; font-size: 12px; }
      .popup-value {
        color: #f8fafc;
        font-size: 12px;
        text-align: right;
        word-break: break-all;
      }
      .error {
        position: absolute; left: 12px; right: 12px; bottom: 4px; z-index: 6;
        font-size: 12px; color: #fca5a5; min-height: 18px; pointer-events: none;
      }
      .wrap:not(.wrap-cockpit-bleed) .error {
        position: static; padding: 0 14px 8px; flex: 0 0 auto;
      }
    </style>
    <section class="wrap">
      <div class="head" style="${showTitle && statusInHead ? "" : "display:none"}">
        <h4 class="title">${escapeHtml(title)}</h4>
        <span class="status"></span>
      </div>
      <div class="map-wrap">
        <div class="map"></div>
        <div class="focus-guide" hidden></div>
        ${statusBlock}
        <button class="layer-toggle" type="button" hidden aria-pressed="false" title="打开图层面板">${LAYER_TOGGLE_ICON_HTML}</button>
        <div class="layer-control" hidden></div>
      </div>
      <div class="error"></div>
    </section>
  `;
}

function ensureMapLibre() {
  return ensureMapLibreGlobal();
}
