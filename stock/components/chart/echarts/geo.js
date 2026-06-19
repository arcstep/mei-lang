import {
import { color } from "../mei/theme-style.js";
  deferUntilDisplayed,
  shouldReactToPreviewUpdated,
  escapeHtml,
  parseProps,
  setQueryStateFilter,
  subscribeQueryState,
  queryStateIdOf,
} from "../../dataset/runtime-query.js";
import {
  buildValueByCode,
  choroplethRange,
  dispatchMapSelection,
  fetchGeoJson,
  resolveFeatureJoinKey,
} from "../../gis/layer-spec.js";
import { createComponentTracer } from "../../perf/render-trace.js";
import { ensureEChartsGlobal } from "../../vendor/runtime-libs.js";

const TAG = "mei-chart-geo";

if (!customElements.get(TAG)) {
  class MeiChartGeoElement extends HTMLElement {
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
      if (typeof this._onPreviewUpdated === "function") {
        window.removeEventListener("meilang:preview-updated", this._onPreviewUpdated);
      } else {
        window.removeEventListener("meilang:preview-updated", this.refresh);
      }
      if (this._unsubscribeQueryState) {
        this._unsubscribeQueryState();
      }
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
      }
      if (this.chart) {
        this.chart.dispose();
        this.chart = null;
      }
    }

    bootstrap() {
      if (!this.shadowRoot) {
        this.attachShadow({ mode: "open" });
      }
      const props = parseProps(this);
      this.shadowRoot.innerHTML = shellHtml(props);
      this.chartEl = this.shadowRoot.querySelector(".chart");
      this.metaEl = this.shadowRoot.querySelector(".meta");
      this.errorEl = this.shadowRoot.querySelector(".error");
      this._renderTrace = createComponentTracer(this, TAG, {});
      this.refresh = () => this.renderGeo();
      this._onPreviewUpdated = (event) => {
        if (!shouldReactToPreviewUpdated(event, this)) {
          return;
        }
        this.refresh();
      };
      window.addEventListener("meilang:preview-updated", this._onPreviewUpdated);
      this.resizeObserver = new ResizeObserver(() => {
        if (this.chart) this.chart.resize();
      });
      this.resizeObserver.observe(this);
      this._queryStateId = queryStateIdOf(props);
      this._sharedFilters = {};
      this._renderTrace.mark("bootstrap", {
        query_state_id: this._queryStateId || "",
      });
      this._unsubscribeQueryState = subscribeQueryState(this._queryStateId, (state) => {
        this._sharedFilters = state?.filters || {};
        this.renderGeo();
      });
      this.renderGeo();
    }

    async renderGeo() {
      const props = parseProps(this);
      const joinKey = String(props.joinKey || "code").trim() || "code";
      const mapName = String(props.mapName || props.mapId || "mei-geo").trim();
      const geojsonUrl = String(
        props.geojsonUrl || props.geoJsonUrl || props.layer?.url || "",
      ).trim();
      this._renderTrace?.mark("render_start", {
        join_key: joinKey,
        geojson_url: geojsonUrl,
      });
      this.metaEl.textContent = geojsonUrl ? `join: ${joinKey}` : "未配置 geojsonUrl";
      try {
        this._renderTrace?.mark("echarts_load_start");
        const echarts = await ensureECharts();
        this._renderTrace?.mark("echarts_load_done");
        const geojson = props.featureCollection
          ? props.featureCollection
          : await fetchGeoJson(geojsonUrl);
        this._renderTrace?.mark("geojson_ready", {
          feature_count: Array.isArray(geojson?.features) ? geojson.features.length : 0,
        });
        const valueMap = buildValueByCode(props, joinKey);
        const palette = props.palette || props.choropleth?.palette;
        const { min, max, colors } = choroplethRange(valueMap, palette);
        echarts.registerMap(mapName, geojson);
        const data = (geojson.features || []).map((feature) => {
          const { code, joinKey: usedKey } = resolveFeatureJoinKey(
            feature.properties,
            joinKey,
          );
          const value = Number(valueMap[code]);
          return {
            name: String(feature.properties?.name || code || ""),
            value: Number.isNaN(value) ? null : value,
            code,
            joinKey: usedKey,
          };
        });
        const layoutSize = String(props.layoutSize || props.layout_size || "92%").trim() || "92%";
        const layoutCenter = props.layoutCenter || props.layout_center || ["50%", "52%"];
        const option = {
          backgroundColor: "transparent",
          tooltip: {
            trigger: "item",
            formatter: (params) => {
              const v = params.value;
              const val = v == null || Number.isNaN(Number(v)) ? "—" : v;
              return `${params.name}<br/>${val}`;
            },
          },
          visualMap: {
            show: Object.keys(valueMap).length > 0,
            min,
            max: max === min ? min + 1 : max,
            inRange: { color: colors },
            textStyle: { color: color("text_muted") },
            left: 8,
            bottom: 8,
          },
          series: [
            {
              type: "map",
              map: mapName,
              layoutCenter,
              layoutSize,
              roam: props.roam !== false,
              emphasis: {
                label: { show: true, color: color("text_inverse") },
                itemStyle: { areaColor: props.highlightColor || color("text_accent") },
              },
              itemStyle: {
                areaColor: colors[0],
                borderColor: props.borderColor || color("chart_2"),
                borderWidth: props.borderWidth ?? 0.8,
              },
              data,
            },
          ],
        };
        if (!this.chart) {
          this.chart = echarts.init(this.chartEl);
          this.chart.on("click", (params) => {
            const row = params?.data || {};
            const selectionDimension = String(
              props.selection_dimension || props.selectionDimension || row.joinKey || joinKey || ""
            ).trim();
            dispatchMapSelection({
              source: "chart.geo",
              layerId: mapName,
              joinKey: row.joinKey || joinKey,
              code: row.code || "",
              name: row.name || "",
              value: row.value,
            });
            if (this._queryStateId && selectionDimension && row.code) {
              setQueryStateFilter(this._queryStateId, selectionDimension, row.code, {
                filterIntentSource: "chart_selection",
                transitionSource: "chart_selection",
              });
            }
          });
        }
        this.chart.setOption(option, true);
        this.errorEl.textContent = "";
        this._renderTrace?.mark("render_done", {
          data_count: data.length,
        });
      } catch (error) {
        this.errorEl.textContent = String(error?.message || error);
        this._renderTrace?.mark("render_error", {
          message: String(error?.message || error),
        });
      }
    }
  }
  customElements.define(TAG, MeiChartGeoElement);
}

function shellHtml(props) {
  const title = String(props.title ?? "区划图");
  const height = Number(props.chartHeight) > 0 ? Number(props.chartHeight) : 280;
  return `
    <style>
      :host { display: block; width: 100%; min-width: 0; }
      .wrap {
        display: grid; gap: 8px; padding: 14px;
        border-radius: 14px;
        border: 1px solid rgba(148,163,184,.2);
        background: rgba(15,23,42,.64);
      }
      .head { display: flex; justify-content: space-between; color: #e2e8f0; }
      .title { margin: 0; font-size: 14px; }
      .meta { font-size: 12px; color: #94a3b8; }
      .chart { width: 100%; min-height: ${height}px; height: ${height}px; }
      .error { font-size: 12px; color: #fca5a5; min-height: 18px; }
    </style>
    <section class="wrap">
      <div class="head">
        <h4 class="title">${escapeHtml(title)}</h4>
        <span class="meta"></span>
      </div>
      <div class="chart"></div>
      <div class="error"></div>
    </section>
  `;
}

async function ensureECharts() {
  return ensureEChartsGlobal();
}
