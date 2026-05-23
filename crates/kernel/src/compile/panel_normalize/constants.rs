pub(crate) const SLOT_HEAD: &str = "head";
pub(crate) const SLOT_BODY: &str = "body";
pub(crate) const PROP_HAS_HEAD: &str = "__mei_has_head";
pub(crate) const PROP_METRIC_CARD: &str = "__mei_metric_card";
pub(crate) const PROP_LAYOUT_POLICY: &str = "__mei_layout_policy";
pub(crate) const PROP_LAYOUT_GAP: &str = "__mei_layout_gap";
pub(crate) const PROP_LAYOUT_PADDING: &str = "__mei_layout_padding";
pub(crate) const PROP_LAYOUT_COLUMNS: &str = "__mei_layout_columns";
pub(crate) const LAYOUT_POLICY_METRICS_STRIP: &str = "metrics_strip";
pub(crate) const LAYOUT_POLICY_METRICS_2_1: &str = "metrics_2_1";
pub(crate) const LAYOUT_POLICY_METRIC_COMPOUND_2_1: &str = "metric_compound_2_1";
pub(crate) const DEFAULT_METRICS_STRIP_GAP: &str = "8px";
pub(crate) const DEFAULT_METRICS_STRIP_PADDING: &str = "12px";
pub(crate) const DEFAULT_METRICS_2_1_GAP: &str = "8px";
pub(crate) const DEFAULT_METRICS_2_1_PADDING: &str = "12px 14px";
pub(crate) const DEFAULT_METRICS_2_1_COLUMNS: [&str; 3] = ["114px", "114px", "234px"];
pub(crate) const DEFAULT_METRIC_COMPOUND_2_1_GAP: &str = "2px";
pub(crate) const COCKPIT_PANEL_PADDING_MIN: f64 = 12.0;
pub(crate) const COCKPIT_PANEL_PADDING_MAX: f64 = 24.0;
pub(crate) const COCKPIT_CARD_GAP_TARGET: f64 = 8.0;
pub(crate) const COCKPIT_CARD_GAP_MIN: f64 = 4.0;
pub(crate) const COCKPIT_CARD_GAP_MAX: f64 = 12.0;

#[derive(Debug, Clone)]
pub(crate) struct PolicySpacing {
    pub(crate) gap: String,
    pub(crate) padding: String,
}
