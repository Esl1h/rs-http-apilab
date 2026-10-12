// requestid resolves the per-request correlation id.
//
// The client may pass X-Request-ID; when present and well-formed it is
// echoed back, otherwise a fresh UUID v4 is generated. The id is never
// interpreted: it is only validated, stored and JSON-escaped.
use axum::http::HeaderMap;

pub const HEADER_NAME: &str = "x-request-id";
pub const MAX_LEN: usize = 128;

pub fn resolve(headers: &HeaderMap) -> String {
    if let Some(value) = headers.get(HEADER_NAME) {
        if let Ok(text) = value.to_str() {
            if is_valid(text) {
                return text.to_string();
            }
        }
    }
    uuid::Uuid::new_v4().to_string()
}

// is_valid accepts 1..MAX_LEN printable ASCII characters without spaces
// or control bytes. Longer or malformed ids are replaced, not echoed.
pub fn is_valid(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_LEN {
        return false;
    }
    bytes.iter().all(|b| (0x21..=0x7e).contains(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_printable_ascii() {
        assert!(is_valid("abc-123"));
        assert!(is_valid(&"x".repeat(128)));
    }

    #[test]
    fn rejects_bad_ids() {
        assert!(!is_valid(""));
        assert!(!is_valid(&"x".repeat(129)));
        assert!(!is_valid("with space"));
        assert!(!is_valid("tab\tchar"));
        assert!(!is_valid("uniçodé"));
    }
}
