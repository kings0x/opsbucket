use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;

pub fn check_auth(headers: &HeaderMap, secret_key: &str) -> Result<(), String> {
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.trim());

    match header {
        Some(key) if key == secret_key => Ok(()),
        _ => Err("Missing or invalid secret key".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::AUTHORIZATION;
    use axum::http::HeaderMap;

    fn make_headers(auth_value: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(val) = auth_value {
            headers.insert(AUTHORIZATION, val.parse().unwrap());
        }
        headers
    }

    #[test]
    fn valid_key_passes() {
        let headers = make_headers(Some("Bearer my-secret-key"));
        assert!(check_auth(&headers, "my-secret-key").is_ok());
    }

    #[test]
    fn invalid_key_fails() {
        let headers = make_headers(Some("Bearer wrong-key"));
        assert!(check_auth(&headers, "my-secret-key").is_err());
    }

    #[test]
    fn missing_header_fails() {
        let headers = make_headers(None);
        assert!(check_auth(&headers, "my-secret-key").is_err());
    }

    #[test]
    fn key_in_query_param_not_accepted() {
        let headers = make_headers(None);
        assert!(check_auth(&headers, "my-secret-key").is_err());
    }
}
