use std::net::IpAddr;

/// HTTP 明文登录仅允许受信主机（浏览器在 HTTP 非 localhost 下无 `crypto.subtle`）。
const HTTP_PLAINTEXT_LOGIN_HOSTS: &[&str] = &["localhost", "127.0.0.1", "[::1]"];

fn host_only_from_header(host_header: &str) -> &str {
    let trimmed = host_header.trim();
    if let Some(inner) = trimmed.strip_prefix('[') {
        if let Some((host, _)) = inner.split_once(']') {
            return host;
        }
    }
    trimmed.split(':').next().unwrap_or(trimmed)
}

fn host_matches_static_allowlist(host_only: &str) -> bool {
    HTTP_PLAINTEXT_LOGIN_HOSTS.iter().any(|allowed| {
        let allowed_host = allowed.trim_matches(|c| c == '[' || c == ']');
        host_only.eq_ignore_ascii_case(allowed_host)
    })
}

fn ip_allows_http_plaintext_login(host_only: &str) -> bool {
    let Ok(ip) = host_only.parse::<IpAddr>() else {
        return false;
    };
    match ip {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
    }
}

fn host_matches_env_allowlist(host_only: &str) -> bool {
    let Some(raw) = std::env::var("MEI_HTTP_PLAINTEXT_LOGIN_HOSTS").ok() else {
        return false;
    };
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .any(|allowed| host_only.eq_ignore_ascii_case(allowed.trim_matches(|c| c == '[' || c == ']')))
}

pub(super) fn host_allows_http_plaintext_login(host_header: &str) -> bool {
    let host_only = host_only_from_header(host_header);
    host_matches_static_allowlist(host_only)
        || ip_allows_http_plaintext_login(host_only)
        || host_matches_env_allowlist(host_only)
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
        assert!(!host_allows_http_plaintext_login("zw-spbjw:3002"));
        assert!(!host_allows_http_plaintext_login("example.com"));
    }

    #[test]
    fn plaintext_login_allows_private_and_link_local_ips() {
        assert!(host_allows_http_plaintext_login("10.0.1.193:9527"));
        assert!(host_allows_http_plaintext_login("10.8.0.2:9527"));
        assert!(host_allows_http_plaintext_login("192.168.64.1:3002"));
        assert!(host_allows_http_plaintext_login("172.20.10.3"));
        assert!(host_allows_http_plaintext_login("169.254.10.20"));
        assert!(host_allows_http_plaintext_login("[fe80::1]:3002"));
        assert!(host_allows_http_plaintext_login("[fd00::1234]:3002"));
    }
}
