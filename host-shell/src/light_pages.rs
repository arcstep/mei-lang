use axum::http::{header, HeaderValue};
use axum::response::{Html, IntoResponse, Response};

const LIGHT_PAGE_CACHE_CONTROL: &str = "private, no-cache, no-store, must-revalidate";

pub(crate) fn light_page_response(html: String) -> Response {
    let mut response = Html(html).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(LIGHT_PAGE_CACHE_CONTROL),
    );
    response
}
