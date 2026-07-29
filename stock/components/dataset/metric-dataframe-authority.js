/**
 * Dataframe authority for Pack-First bootstrap + chart metric short-circuit.
 *
 * Empty `rows: []` must not be treated as a successful authoritative page:
 * chart used to short-circuit-commit empty arrays and paint permanent blank
 * ranking/list widgets (zhifa penalties_top_matter_year_ranking regression).
 *
 * Metrics fallback must also refuse empty `value: []` — otherwise a later
 * refresh overwrites a good bootstrap paint (flash-then-blank).
 *
 * Overlapping refreshRuntimeData: a slower gen with good rows must still upgrade
 * an empty paint even if a newer gen already started (stale-success discard race).
 */

export function hasAuthoritativeDataframeRows(rows) {
  return Array.isArray(rows) && rows.length > 0;
}

/** Chart refreshRuntimeData: only commit dataset-metric short-circuit when rows are non-empty. */
export function shouldCommitDatasetMetricRows(rowsResult) {
  return hasAuthoritativeDataframeRows(rowsResult?.rows);
}

/** Bootstrap/Pack-First: empty dataframe pages are not cache hits — fall through to live query. */
export function isAuthoritativeBootstrapDatasetPage(data) {
  if (!data || typeof data !== "object") return false;
  return hasAuthoritativeDataframeRows(data.rows);
}

/**
 * Metric contract / runtime props payload has rows chart can paint
 * (mirrors resolveRows candidates: .rows / .value[] / dataframe.value.rows).
 */
export function metricContractHasRenderableRows(metric) {
  if (!metric || typeof metric !== "object") return false;
  if (hasAuthoritativeDataframeRows(metric.rows)) return true;
  if (hasAuthoritativeDataframeRows(metric.value)) return true;
  if (
    metric.shape === "dataframe" &&
    metric.value &&
    typeof metric.value === "object" &&
    hasAuthoritativeDataframeRows(metric.value.rows)
  ) {
    return true;
  }
  return false;
}

/** True when chart already holds a paint-worthy runtime payload. */
export function runtimePropsHaveRenderableRows(runtimeProps) {
  if (!runtimeProps || typeof runtimeProps !== "object") return false;
  return (
    metricContractHasRenderableRows(runtimeProps.data) ||
    metricContractHasRenderableRows(runtimeProps.value)
  );
}

/**
 * Whether a dataset-metric rowsResult should be committed onto the chart.
 *
 * - Current gen + non-empty rows → commit
 * - Stale gen + non-empty rows + current paint empty → still commit (fix overlapping refresh race)
 * - Empty rows → never commit (fall through / keep existing)
 */
export function shouldApplyDatasetMetricRowsResult({
  refreshGen,
  currentGen,
  rowsResult,
  runtimeProps,
} = {}) {
  if (!shouldCommitDatasetMetricRows(rowsResult)) {
    return false;
  }
  if (refreshGen === currentGen) {
    return true;
  }
  // Stale success: do not discard good rows while the live paint is still empty.
  return !runtimePropsHaveRenderableRows(runtimeProps);
}

/**
 * Whether metrics-fallback metric should replace runtime props.
 * Never let empty metric wipe a good paint; never apply empty metric at all.
 */
export function shouldApplyMetricFallbackResult({
  refreshGen,
  currentGen,
  metric,
  runtimeProps,
} = {}) {
  if (!metricContractHasRenderableRows(metric)) {
    return false;
  }
  if (refreshGen === currentGen) {
    return true;
  }
  return !runtimePropsHaveRenderableRows(runtimeProps);
}
