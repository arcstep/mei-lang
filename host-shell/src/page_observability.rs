#[derive(Debug, Clone, Copy, Default)]
pub struct PageHtmlPayloadStats {
    pub html_bytes: usize,
    pub data_props_count: usize,
    pub data_props_bytes: usize,
}

const DATA_PROPS_ATTR: &str = "data-props=\"";
const DATA_PROPS_ATTR_LEGACY: &str = "data-props='";

pub fn measure_page_html_payload(html: &str) -> PageHtmlPayloadStats {
    let html_bytes = html.len();
    let mut data_props_count = 0usize;
    let mut data_props_bytes = 0usize;

    let mut search_from = 0usize;
    while search_from < html.len() {
        let tail = &html[search_from..];
        let (attr, payload_start) = if let Some(rel) = tail.find(DATA_PROPS_ATTR) {
            (DATA_PROPS_ATTR, search_from + rel + DATA_PROPS_ATTR.len())
        } else if let Some(rel) = tail.find(DATA_PROPS_ATTR_LEGACY) {
            (
                DATA_PROPS_ATTR_LEGACY,
                search_from + rel + DATA_PROPS_ATTR_LEGACY.len(),
            )
        } else {
            break;
        };
        let payload = &html[payload_start..];
        let end_rel = if attr == DATA_PROPS_ATTR {
            payload.find('"')
        } else {
            payload.find('\'')
        };
        if let Some(end_rel) = end_rel {
            data_props_count += 1;
            data_props_bytes += end_rel;
            search_from = payload_start + end_rel + 1;
        } else {
            break;
        }
    }

    PageHtmlPayloadStats {
        html_bytes,
        data_props_count,
        data_props_bytes,
    }
}

pub fn fill_manage_wall_clock_placeholders(
    mut html: String,
    ssr_http_response_body_ms: u64,
    handler_html_ready_ms: u64,
) -> String {
    let body = ssr_http_response_body_ms.to_string();
    let ready = handler_html_ready_ms.to_string();
    html = html.replace("__MEI_SSR_HTTP_BODY_MS__", body.as_str());
    html = html.replace("__MEI_HANDLER_HTML_READY_MS__", ready.as_str());
    html
}

pub fn fill_page_load_observability_placeholders(
    mut html: String,
    compile_ms: u64,
    compile_cache_hit: bool,
    html_bytes: usize,
    data_props_bytes: usize,
    data_props_count: usize,
) -> String {
    html = html.replace("__MEI_COMPILE_MS__", &compile_ms.to_string());
    html = html.replace(
        "__MEI_COMPILE_CACHE_HIT__",
        if compile_cache_hit { "1" } else { "0" },
    );
    html = html.replace("__MEI_HTML_BYTES__", &html_bytes.to_string());
    html = html.replace("__MEI_DATA_PROPS_BYTES__", &data_props_bytes.to_string());
    html = html.replace("__MEI_DATA_PROPS_COUNT__", &data_props_count.to_string());
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_page_html_payload_counts_data_props() {
        let html = r#"<div data-props="{\"a\":1}"></div><span data-props='{"b":2}'></span>"#;
        let stats = measure_page_html_payload(html);
        assert_eq!(stats.data_props_count, 2);
        assert!(stats.data_props_bytes > 0);
    }

    #[test]
    fn fill_page_load_observability_replaces_placeholders() {
        let html = fill_page_load_observability_placeholders(
            r#"<body data-mei-handler-html-ready-ms="__MEI_HANDLER_HTML_READY_MS__" data-mei-compile-ms="__MEI_COMPILE_MS__" data-mei-data-props-bytes="__MEI_DATA_PROPS_BYTES__"></body>"#
                .to_string(),
            12,
            false,
            100,
            42,
            3,
        );
        assert!(html.contains("data-mei-compile-ms=\"12\""));
        assert!(html.contains("data-mei-data-props-bytes=\"42\""));
    }
}
