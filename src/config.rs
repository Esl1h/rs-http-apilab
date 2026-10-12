// config loads runtime settings from environment variables.
//
// Every knob of the lab is an env var so the same binary can be used to
// exercise different scenarios without rebuilding. A misconfigured lab
// fails loudly instead of silently falling back to defaults.
use std::env;

pub const VALID_LOG_LEVELS: [&str; 4] = ["debug", "info", "warn", "error"];

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub max_body_bytes: i64,
    pub read_timeout_ms: i64,
    pub lab_admin_token: String,
    pub ready_force_fail: bool,
    pub max_slow_ms: i64,
    pub max_large_bytes: i64,
    pub auth_username: String,
    pub auth_password: String,
    pub token_ttl_seconds: i64,
}

fn env_str(key: &str, default: &str) -> String {
    match env::var(key) {
        Ok(v) if !v.is_empty() => v,
        _ => default.to_string(),
    }
}

fn env_bool(key: &str) -> bool {
    match env::var(key) {
        Ok(v) => !v.is_empty(),
        Err(_) => false,
    }
}

fn env_i64(key: &str, default: i64) -> Result<i64, String> {
    let raw = match env::var(key) {
        Ok(v) if !v.is_empty() => v,
        _ => return Ok(default),
    };
    raw.parse::<i64>()
        .map_err(|_| format!("must be an integer, got \"{raw}\""))
}

pub fn load() -> Result<Config, String> {
    let port = env_i64("PORT", 8080).map_err(|e| format!("PORT: {e}"))?;
    if !(1..=65535).contains(&port) {
        return Err(format!("PORT: must be between 1 and 65535, got {port}"));
    }
    let max_body_bytes =
        env_i64("MAX_BODY_BYTES", 1_048_576).map_err(|e| format!("MAX_BODY_BYTES: {e}"))?;
    if max_body_bytes < 1 {
        return Err("MAX_BODY_BYTES: must be at least 1".to_string());
    }
    let read_timeout_ms =
        env_i64("READ_TIMEOUT_MS", 30_000).map_err(|e| format!("READ_TIMEOUT_MS: {e}"))?;
    if read_timeout_ms < 1000 {
        return Err("READ_TIMEOUT_MS: must be at least 1000".to_string());
    }
    let max_slow_ms = env_i64("MAX_SLOW_MS", 10_000).map_err(|e| format!("MAX_SLOW_MS: {e}"))?;
    if max_slow_ms < 1 {
        return Err("MAX_SLOW_MS: must be at least 1".to_string());
    }
    let max_large_bytes =
        env_i64("MAX_LARGE_BYTES", 1_048_576).map_err(|e| format!("MAX_LARGE_BYTES: {e}"))?;
    if max_large_bytes < 1 {
        return Err("MAX_LARGE_BYTES: must be at least 1".to_string());
    }
    let log_level = env_str("LOG_LEVEL", "info").to_lowercase();
    if !VALID_LOG_LEVELS.contains(&log_level.as_str()) {
        return Err(format!(
            "LOG_LEVEL: must be one of {}, got \"{log_level}\"",
            VALID_LOG_LEVELS.join(", ")
        ));
    }
    // LOG_LEVEL is validated above but not read at runtime: request log
    // lines are always emitted at "info", exactly like v-http-lab.
    Ok(Config {
        port: port as u16,
        max_body_bytes,
        read_timeout_ms,
        lab_admin_token: env_str("LAB_ADMIN_TOKEN", ""),
        ready_force_fail: env_bool("READY_FORCE_FAIL"),
        max_slow_ms,
        max_large_bytes,
        auth_username: env_str("LAB_USERNAME", "admin"),
        auth_password: env_str("LAB_PASSWORD", "lab-password-123456"),
        token_ttl_seconds: 3600,
    })
}
