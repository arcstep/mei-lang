/// HTTP 明文登录仅允许受信主机（浏览器在 HTTP 非 localhost 下无 `crypto.subtle`）。
const HTTP_PLAINTEXT_LOGIN_HOSTS: &[&str] =
    &["localhost", "127.0.0.1", "[::1]", "23.211.135.152"];

fn host_only_from_header(host_header: &str) -> &str {
    let trimmed = host_header.trim();
    if let Some(inner) = trimmed.strip_prefix('[') {
        if let Some((host, _)) = inner.split_once(']') {
            return host;
        }
    }
    trimmed.split(':').next().unwrap_or(trimmed)
}

pub(super) fn host_allows_http_plaintext_login(host_header: &str) -> bool {
    let host_only = host_only_from_header(host_header);
    HTTP_PLAINTEXT_LOGIN_HOSTS.iter().any(|allowed| {
        let allowed_host = allowed.trim_matches(|c| c == '[' || c == ']');
        host_only.eq_ignore_ascii_case(allowed_host)
    })
}

#[cfg(test)]
mod tests {
    use super::host_allows_http_plaintext_login;

    #[test]
    fn plaintext_login_host_allowlist() {
        assert!(host_allows_http_plaintext_login("localhost"));
        assert!(host_allows_http_plaintext_login("localhost:3002"));
        assert!(host_allows_http_plaintext_login("127.0.0.1:9527"));
        assert!(host_allows_http_plaintext_login("[::1]:3002"));
        assert!(host_allows_http_plaintext_login("23.211.135.152:3002"));
        assert!(!host_allows_http_plaintext_login("zw-spbjw:3002"));
        assert!(!host_allows_http_plaintext_login("example.com"));
    }
}
