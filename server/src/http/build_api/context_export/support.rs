use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

pub(super) fn markdown_error(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
        .body(format!("## Mei Build Context Error\n\n{message}\n"))
        .unwrap()
        .into_response()
}

pub(super) fn percent_encode_component(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*b))
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}
