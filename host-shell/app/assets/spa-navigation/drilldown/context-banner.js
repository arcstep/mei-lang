  function patchDrilldownTableByMetric(tableMetricId) {
    const metricId = String(tableMetricId || "").trim();
    if (!metricId) return true;
    const table = document.querySelector("mei-dataset-table");
    if (!(table instanceof HTMLElement)) return false;
    let props = {};
    try {
      props = JSON.parse(table.dataset.props || "{}");
    } catch (_) {
      props = {};
    }
    if (!props || typeof props !== "object") return true;
    const currentData = props.data && typeof props.data === "object" ? props.data : {};
    const runtimeRef =
      (currentData && currentData.__mei_runtime_ref) ||
      (props.dataset && props.dataset.__mei_runtime_ref) ||
      {};
    const datasetId = String(
      runtimeRef.dataset_id ||
        currentData.from_dataset ||
        currentData.id ||
        props?.dataset?.id ||
        "",
    ).trim();
    if (!datasetId) return true;
    props.data = { __ref: "metric", id: metricId, from_dataset: datasetId };
    table.dataset.props = JSON.stringify(props);
    const remount = document.createElement("mei-dataset-table");
    remount.dataset.props = table.dataset.props;
    table.replaceWith(remount);
    dispatchPreviewUpdated("drilldown");
    return true;
  }

  function renderDrilldownContextBanner(title, note) {
    const header = String(title || "").trim();
    const body = String(note || "").trim();
    if (!header && !body) return;
    let banner = document.getElementById(DRILLDOWN_CONTEXT_BANNER_ID);
    if (!banner) {
      banner = document.createElement("div");
      banner.id = DRILLDOWN_CONTEXT_BANNER_ID;
      banner.className = "access-drilldown-context-banner";
      document.body.appendChild(banner);
    }
    banner.innerHTML =
      `<div class="access-drilldown-context-title">${header || "指标口径"}</div>` +
      (body ? `<div class="access-drilldown-context-note">${body}</div>` : "");
  }

  function clearDrilldownContextBanner() {
    const banner = document.getElementById(DRILLDOWN_CONTEXT_BANNER_ID);
    if (banner) {
      banner.remove();
    }
  }

  function applyDrilldownContextFromQuery() {
    clearTimeout(drilldownContextRetryTimer);
    if (!isAccessRoute()) {
      clearDrilldownContextBanner();
      return;
    }
    let parsed = null;
    try {
      parsed = new URL(window.location.href);
    } catch (_) {
      return;
    }
    const metricId = String(parsed.searchParams.get("drill_metric") || "").trim();
    if (!metricId) {
      clearDrilldownContextBanner();
      return;
    }
    const title = parsed.searchParams.get("drill_title") || "";
    const note = parsed.searchParams.get("drill_note") || "";
    renderDrilldownContextBanner(title, note);
    const tableMetricId = String(parsed.searchParams.get("drill_table_metric") || "").trim();
    if (!tableMetricId) return;
    let attempts = 0;
    const retry = () => {
      attempts += 1;
      if (patchDrilldownTableByMetric(tableMetricId) || attempts >= 24) {
        return;
      }
      drilldownContextRetryTimer = window.setTimeout(retry, 90);
    };
    retry();
  }

