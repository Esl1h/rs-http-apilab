// lab: integration testing utilities.
//
// These endpoints exist to make proxy behavior observable and provable:
// /lab/calls is the evidence buffer that shows which requests actually
// reached the backend.
use std::convert::Infallible;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::{header, Method};
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::auth;
use crate::router::Ctx;
use crate::seed;
use crate::state::CallRecord;

// echo_allowed_headers is the allowlist reflected by /lab/echo, in
// reflection order. Authorization and Cookie are deliberately absent.
pub const ECHO_HEADER_NAMES: [&str; 11] = [
    "Host",
    "User-Agent",
    "Accept",
    "Accept-Encoding",
    "Accept-Language",
    "Content-Type",
    "Content-Length",
    "X-Request-ID",
    "X-Test-Case",
    "X-Forwarded-For",
    "X-Forwarded-Proto",
];

pub const METHOD_ALLOWED: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];

// echo reflects the method, path, raw query, allowed headers and a body
// hash. It never reflects Authorization, Cookie or the body itself.
pub async fn echo(ctx: Ctx) -> Response {
    let mut reflected = serde_json::Map::new();
    for canonical in ECHO_HEADER_NAMES {
        reflected.insert(
            canonical.to_string(),
            json!(ctx.header(&canonical.to_lowercase())),
        );
    }
    let mut hasher = Sha256::new();
    hasher.update(&ctx.body);
    let body = json!({
        "method": ctx.method.as_str(),
        "path": ctx.uri.path(),
        "query": ctx.uri.query().unwrap_or(""),
        "headers": serde_json::Value::Object(reflected),
        "body_bytes": ctx.body.len(),
        "body_sha256": format!("{:x}", hasher.finalize()),
        "request_id": ctx.request_id,
    });
    ctx.json(200, &body)
}

#[derive(Serialize)]
struct MethodBody {
    method: String,
    received: bool,
    allowed: Vec<&'static str>,
    request_id: String,
}

// method reports the HTTP method used to reach it. It is registered for
// GET, HEAD, POST, PUT, PATCH, DELETE and OPTIONS.
pub async fn method(ctx: Ctx) -> Response {
    let allow_all = "GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS";
    if ctx.method == Method::HEAD {
        // HEAD answers with headers only: set Content-Length to the size
        // the GET body would have and send no body.
        let body = serde_json::to_string(&MethodBody {
            method: "HEAD".to_string(),
            received: true,
            allowed: METHOD_ALLOWED.to_vec(),
            request_id: ctx.request_id.clone(),
        })
        .expect("serializable struct");
        let mut resp = crate::router::text_response(200, "text/plain", "");
        resp.headers_mut().insert(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_str(&body.len().to_string()).expect("ascii digits"),
        );
        resp.headers_mut().insert(
            header::ALLOW,
            header::HeaderValue::from_static("GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS"),
        );
        return resp;
    }
    let mut resp = ctx.json(
        200,
        &MethodBody {
            method: ctx.method.as_str().to_string(),
            received: true,
            allowed: METHOD_ALLOWED.to_vec(),
            request_id: ctx.request_id.clone(),
        },
    );
    resp.headers_mut()
        .insert(header::ALLOW, header::HeaderValue::from_static(allow_all));
    resp
}

#[derive(Serialize)]
struct SlowBody {
    slept_ms: i64,
    max_ms: i64,
    request_id: String,
}

// slow sleeps the requested number of milliseconds (bounded by
// MAX_SLOW_MS) before answering.
pub async fn slow(ctx: Ctx) -> Response {
    let conf = ctx.conf();
    let raw = ctx.query_param("ms").unwrap_or("0").to_string();
    let ms = match raw.parse::<i64>() {
        Ok(v) => v,
        Err(_) => {
            return ctx.error(
                400,
                "invalid_query_value",
                &format!("\"ms\" must be an integer, got \"{raw}\""),
            )
        }
    };
    if !(0..=conf.max_slow_ms).contains(&ms) {
        return ctx.error(
            400,
            "invalid_query_value",
            &format!(
                "\"ms\" must be between 0 and MAX_SLOW_MS={}, got {ms}",
                conf.max_slow_ms
            ),
        );
    }
    tokio::time::sleep(Duration::from_millis(ms as u64)).await;
    ctx.json(
        200,
        &SlowBody {
            slept_ms: ms,
            max_ms: conf.max_slow_ms,
            request_id: ctx.request_id.clone(),
        },
    )
}

#[derive(Serialize)]
struct StatusBody {
    requested_status: u16,
    request_id: String,
}

// status_code answers with the requested status code (200..599).
// 204, 205 and 304 are answered without a body.
pub async fn status_code(ctx: Ctx) -> Response {
    let raw = &ctx.params[0];
    let code = match raw.parse::<u16>() {
        Ok(v) => v,
        Err(_) => {
            return ctx.error(
                400,
                "invalid_status_code",
                &format!("status code must be an integer, got \"{raw}\""),
            )
        }
    };
    if !(200..=599).contains(&code) {
        return ctx.error(
            400,
            "invalid_status_code",
            &format!("status code must be between 200 and 599, got {code}"),
        );
    }
    if code == 204 || code == 205 || code == 304 {
        return axum::http::Response::builder()
            .status(code)
            .body(Body::empty())
            .expect("static response parts are valid");
    }
    let mut resp = ctx.json(
        code,
        &StatusBody {
            requested_status: code,
            request_id: ctx.request_id.clone(),
        },
    );
    // hyper writes "<none>" as the reason phrase of codes it does not
    // name (299, 499, 520...); send a plain word instead.
    if resp.status().canonical_reason().is_none() {
        resp.extensions_mut()
            .insert(hyper::ext::ReasonPhrase::from_static(b"Unknown"));
    }
    resp
}

// response_marker serves the fixed marker body for response inspection
// tests.
pub async fn response_marker(ctx: Ctx) -> Response {
    let mut resp = ctx.text(200, format!("{}\n", seed::RESPONSE_MARKER));
    resp.headers_mut().insert(
        header::HeaderName::from_static("x-lab-marker"),
        header::HeaderValue::from_static(seed::RESPONSE_MARKER),
    );
    resp
}

// large answers a text/plain body of exactly `bytes` bytes (bounded by
// MAX_LARGE_BYTES).
pub async fn large(ctx: Ctx) -> Response {
    let conf = ctx.conf();
    let raw = ctx.query_param("bytes").unwrap_or("0").to_string();
    let n = match raw.parse::<i64>() {
        Ok(v) => v,
        Err(_) => {
            return ctx.error(
                400,
                "invalid_query_value",
                &format!("\"bytes\" must be an integer, got \"{raw}\""),
            )
        }
    };
    if n < 0 || n > conf.max_large_bytes {
        return ctx.error(
            400,
            "invalid_query_value",
            &format!(
                "\"bytes\" must be between 0 and MAX_LARGE_BYTES={}, got {n}",
                conf.max_large_bytes
            ),
        );
    }
    ctx.text(200, "A".repeat(n as usize))
}

#[derive(Serialize)]
struct HeadersBody {
    headers_set: [&'static str; 7],
    request_id: String,
}

// headers answers with a set of custom and security headers, to test
// proxy response inspection and header rewriting.
pub async fn headers(ctx: Ctx) -> Response {
    let mut resp = ctx.json(
        200,
        &HeadersBody {
            headers_set: [
                "X-Frame-Options",
                "X-Content-Type-Options",
                "Referrer-Policy",
                "Content-Security-Policy",
                "Strict-Transport-Security",
                "X-Permitted-Cross-Domain-Policies",
                "X-Lab-Static-Header",
            ],
            request_id: ctx.request_id.clone(),
        },
    );
    let h = resp.headers_mut();
    h.insert(
        header::X_FRAME_OPTIONS,
        header::HeaderValue::from_static("DENY"),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        header::HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        header::HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    h.insert(
        header::STRICT_TRANSPORT_SECURITY,
        header::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    h.insert(
        header::HeaderName::from_static("x-permitted-cross-domain-policies"),
        header::HeaderValue::from_static("none"),
    );
    h.insert(
        header::HeaderName::from_static("x-lab-static-header"),
        header::HeaderValue::from_static("set-by-lab"),
    );
    resp
}

// stream answers with a chunked response produced natively by the body
// stream: five parts `lab-stream-chunk-N-of-5` sent every 100 ms.
// hyper does the chunked framing; no connection takeover is needed
// (unlike the original V implementation).
pub async fn stream(_ctx: Ctx) -> Response {
    let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(1);
    tokio::spawn(async move {
        for i in 1..=5u32 {
            let chunk = format!("lab-stream-chunk-{i}-of-5\n");
            if tx.send(Ok(Bytes::from(chunk))).await.is_err() {
                return;
            }
            if i < 5 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    });
    axum::http::Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .expect("static response parts are valid")
}

#[derive(Serialize)]
struct CallsBody {
    count: usize,
    capacity: usize,
    calls: Vec<CallRecord>,
}

// lab_authorized answers 404 (endpoints disabled), 401 (missing token)
// or 403 (wrong token) as an Err response when the request must stop.
fn lab_authorized(ctx: &Ctx) -> Result<(), Response> {
    let conf = ctx.conf();
    if conf.lab_admin_token.is_empty() {
        return Err(ctx.error(
            404,
            "lab_calls_disabled",
            "/lab/calls endpoints are disabled: LAB_ADMIN_TOKEN is not set",
        ));
    }
    let custom = ctx.header("x-lab-token");
    let token = if custom.is_empty() {
        auth::bearer_token(ctx)
    } else {
        custom
    };
    if token.is_empty() {
        return Err(ctx.error(
            401,
            "unauthorized",
            "missing lab token: send header X-Lab-Token or Authorization: Bearer",
        ));
    }
    if !auth::constant_time_eq(token.as_bytes(), conf.lab_admin_token.as_bytes()) {
        return Err(ctx.error(403, "invalid_lab_token", "invalid lab token"));
    }
    Ok(())
}

// calls lists the requests that reached the backend (chronological,
// oldest first), proving which requests passed the proxy.
pub async fn calls(ctx: Ctx) -> Response {
    if let Err(resp) = lab_authorized(&ctx) {
        return resp;
    }
    let calls = ctx.shared.st.calls_all();
    ctx.json(
        200,
        &CallsBody {
            count: calls.len(),
            capacity: crate::state::CALLS_CAPACITY,
            calls,
        },
    )
}

#[derive(Serialize)]
struct CallsResetBody {
    reset: bool,
    dropped: usize,
    request_id: String,
}

// calls_reset clears the /lab/calls ring buffer.
pub async fn calls_reset(ctx: Ctx) -> Response {
    if let Err(resp) = lab_authorized(&ctx) {
        return resp;
    }
    let dropped = ctx.shared.st.calls_reset();
    ctx.json(
        200,
        &CallsResetBody {
            reset: true,
            dropped,
            request_id: ctx.request_id.clone(),
        },
    )
}
