// logging emits one structured JSON log line per request to stdout.
//
// Never logged: Authorization, Cookie, Set-Cookie, passwords or tokens.
// Query-string values for sensitive keys are masked as "[REDACTED]".
use serde::Serialize;

// Sensitive query keys whose values are always masked in logs.
pub const SENSITIVE_KEYS: [&str; 12] = [
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "access_token",
    "authorization",
    "auth",
    "key",
    "cookie",
    "session",
];

#[derive(Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: &'static str,
    pub msg: &'static str,
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub query: String,
    pub status: u16,
    pub duration_ms: i64,
    pub route: String,
    pub remote_addr: String,
    pub user_agent: String,
    pub content_length: i64,
    pub content_type: String,
    pub x_forwarded_for: String,
    pub x_test_case: String,
}

#[derive(Serialize)]
struct EventLine<'a> {
    timestamp: String,
    level: &'a str,
    msg: &'a str,
}

// write_request_line emits the single per-request JSON line to stdout.
// std's stdout lock makes each writeln atomic enough for line readers;
// like v-http-apilab, writes are serialized behind a single println! call.
pub fn write_request_line(entry: &LogEntry) {
    if let Ok(line) = serde_json::to_string(entry) {
        println!("{line}");
    }
}

// write_event emits a non-request operational line (startup).
pub fn write_event(level: &str, msg: &str) {
    let line = EventLine {
        timestamp: crate::timefmt::now_rfc3339(),
        level,
        msg,
    };
    if let Ok(line) = serde_json::to_string(&line) {
        println!("{line}");
    }
}

// mask_query rewrites sensitive key values of a raw query string to
// "[REDACTED]". Non-sensitive pairs pass through untouched.
pub fn mask_query(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    raw.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, _)) if SENSITIVE_KEYS.contains(&key.to_lowercase().as_str()) => {
                format!("{key}=[REDACTED]")
            }
            _ => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_sensitive_keys_case_insensitive() {
        assert_eq!(
            mask_query("password=abc&normal=1&Token=x"),
            "password=[REDACTED]&normal=1&Token=[REDACTED]"
        );
    }

    #[test]
    fn keeps_flag_style_pairs() {
        assert_eq!(mask_query("a=1&b"), "a=1&b");
        assert_eq!(mask_query(""), "");
    }
}
