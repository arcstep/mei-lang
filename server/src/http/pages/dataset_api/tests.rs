use axum::body::Bytes;

use super::support::parse_dataset_query_request;

#[test]
fn dataset_query_request_deserializes_chart_runtime_payload() {
    let body = Bytes::from(
        r#"{"scene_id":"home","target":"scenes/home.mei","dataset_id":"scenes/02-行政检查.mei::administrative_inspection_dashboard_ds","metric_id":"scenes/02-行政检查.mei::inspections_6m_count_trend","page":1,"page_size":20,"filters":{},"query_state":{"filters":{}},"full":false,"summary":false}"#,
    );
    let request = parse_dataset_query_request("zhifa", &body).expect("parse request");
    assert_eq!(request.scene_id.as_deref(), Some("home"));
    assert_eq!(request.page, Some(1));
    assert_eq!(request.page_size, Some(20));
    assert_eq!(
        request.metric_id.as_deref(),
        Some("scenes/02-行政检查.mei::inspections_6m_count_trend")
    );
}
