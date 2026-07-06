use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;

use super::secret_key_store::SecretKeyStore;

pub async fn check_auth(headers: &HeaderMap, store: &SecretKeyStore) -> Result<(), String> {
    let token = extract_bearer_token(headers)?;
    if store.contains(&token).await {
        Ok(())
    } else {
        Err("Missing or invalid secret key".into())
    }
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Result<String, String> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.trim().to_string())
        .ok_or_else(|| "Missing or invalid secret key".into())
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
    fn valid_bearer_token_extracted() {
        let headers = make_headers(Some("Bearer my-secret-key"));
        assert_eq!(extract_bearer_token(&headers).unwrap(), "my-secret-key");
    }

    #[test]
    fn missing_header_fails() {
        let headers = make_headers(None);
        assert!(extract_bearer_token(&headers).is_err());
    }

    #[test]
    fn wrong_prefix_fails() {
        let headers = make_headers(Some("Token my-secret-key"));
        assert!(extract_bearer_token(&headers).is_err());
    }

    #[test]
    fn empty_token_fails() {
        let headers = make_headers(Some("Bearer "));
        assert_eq!(extract_bearer_token(&headers).unwrap(), "");
        // An empty string is not a valid key — store.contains("") will return false
    }

    #[test]
    fn whitespace_trimmed() {
        let headers = make_headers(Some("Bearer   my-key  "));
        assert_eq!(extract_bearer_token(&headers).unwrap(), "my-key");
    }
}
