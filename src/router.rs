// router owns the HTTP surface of the app.
//
// axum routes every request through a single catch-all handler into our
// own deterministic route table (the same design as v-http-apilab: one
// dispatch table so every request that reaches the process gets a
// correct 405+Allow / 404 JSON answer, a log line and a /lab/calls
// record, including exotic methods such as TRACE or PROPFIND).
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use percent_encoding::percent_decode_str;
use serde::Serialize;
use serde_json::json;

use crate::config::Config;
use crate::logging::{self, LogEntry};
use crate::requestid;
use crate::state::AppState;

pub type HandlerFuture = Pin<Box<dyn Future<Output = Response> + Send>>;
pub type Handler = Arc<dyn Fn(Ctx) -> HandlerFuture + Send + Sync>;

pub struct Route {
    pub method: Method,
    pub pattern: &'static str,
    pub words: Vec<&'static str>,
    pub handler: Handler,
}

pub struct Shared {
    pub conf: Arc<Config>,
    pub st: Arc<AppState>,
    pub routes: Vec<Route>,
}

// handler adapts an async fn(Ctx) -> Response into the boxed route type.
pub fn handler<F, Fut>(f: F) -> Handler
where
    F: Fn(Ctx) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response> + Send + 'static,
{
    Arc::new(move |ctx| Box::pin(f(ctx)))
}

// Ctx carries everything one handler needs about a request. The body is
// already fully read (bounded by MAX_BODY_BYTES) and never interpreted.
pub struct Ctx {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub query: HashMap<String, String>,
    pub body: Bytes,
    pub params: Vec<String>,
    pub request_id: String,
    pub shared: Arc<Shared>,
}

impl Ctx {
    pub fn conf(&self) -> &Config {
        &self.shared.conf
    }

    pub fn header(&self, name: &str) -> String {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    }

    pub fn content_type(&self) -> String {
        let raw = self.header("content-type");
        raw.split(';').next().unwrap_or("").trim().to_lowercase()
    }

    pub fn content_type_is_json(&self) -> bool {
        let ct = self.content_type();
        ct == "application/json" || ct.ends_with("+json")
    }

    // require_json answers the 415 error response when the request body
    // is not declared as JSON.
    pub fn require_json(&self) -> Result<(), Response> {
        if self.content_type_is_json() {
            return Ok(());
        }
        let provided = self.content_type();
        let provided = if provided.is_empty() {
            "none"
        } else {
            &provided
        };
        Err(self.error(
            415,
            "unsupported_media_type",
            &format!("expected Content-Type application/json, got \"{provided}\""),
        ))
    }

    // parse_json decodes the request body as a serde_json value of type
    // T; Err carries the 400 invalid_json response.
    pub fn parse_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, Response> {
        serde_json::from_slice(&self.body).map_err(|err| {
            self.error(
                400,
                "invalid_json",
                &format!("request body is not valid JSON: {err}"),
            )
        })
    }

    // parse_object is parse_json for struct payloads: serde also builds
    // a struct from a JSON array (fields by position), which would turn
    // `[]` into an all-defaults body instead of a 400.
    pub fn parse_object<T: serde::de::DeserializeOwned>(&self) -> Result<T, Response> {
        if self.body.trim_ascii_start().first() == Some(&b'[') {
            return Err(self.error(400, "invalid_json", "request body must be a JSON object"));
        }
        self.parse_json()
    }

    pub fn query_param(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(|v| v.as_str())
    }

    pub fn error(&self, status: u16, code: &str, message: &str) -> Response {
        error_response(status, code, message, &self.request_id)
    }

    pub fn json<T: Serialize>(&self, status: u16, body: &T) -> Response {
        json_response(status, body)
    }

    pub fn text(&self, status: u16, body: impl Into<Bytes>) -> Response {
        text_response(status, "text/plain", body)
    }
}

// parse_query decodes a raw query string with urlencoded semantics
// (last value wins for repeated keys, matching v-http-apilab's ctx.query).
pub fn parse_query(raw: &str) -> HashMap<String, String> {
    form_urlencoded::parse(raw.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: &'a str,
    request_id: &'a str,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorDetail<'a>,
}

// error_response builds the fixed error envelope:
// {"error":{"code":"...","message":"...","request_id":"..."}}
pub fn error_response(status: u16, code: &str, message: &str, request_id: &str) -> Response {
    let body = json!(ErrorEnvelope {
        error: ErrorDetail {
            code,
            message,
            request_id,
        }
    });
    json_response(status, &body)
}

pub fn json_response<T: Serialize>(status: u16, body: &T) -> Response {
    let payload = serde_json::to_string(body).unwrap_or_else(|_| "{}".to_string());
    Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload))
        .expect("static response parts are valid")
}

pub fn text_response(status: u16, content_type: &str, body: impl Into<Bytes>) -> Response {
    Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body.into()))
        .expect("static response parts are valid")
}

// normalize_words percent-decodes the path and resolves dot-segments
// (RFC 3986), the same normalization v-http-apilab applied before routing:
// "/static/..%2f../x" never reaches a handler, it normalizes first.
pub fn normalize_words(raw_path: &str) -> Vec<String> {
    let decoded = percent_decode_str(raw_path).decode_utf8_lossy().to_string();
    let mut words: Vec<String> = Vec::new();
    for segment in decoded.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                words.pop();
            }
            other => words.push(other.to_string()),
        }
    }
    words
}

// match_words matches url path words against a route pattern, capturing
// ":param" segments in order.
fn match_words(url_words: &[String], route_words: &[&str]) -> Option<Vec<String>> {
    if url_words.len() != route_words.len() {
        return None;
    }
    let mut params = Vec::with_capacity(route_words.len());
    for (i, rw) in route_words.iter().enumerate() {
        if rw.len() > 1 && rw.starts_with(':') {
            params.push(url_words[i].clone());
        } else if rw != &url_words[i] {
            return None;
        }
    }
    Some(params)
}

// allowed_methods_for returns the methods any registered route accepts
// for the given path words, used for the 405 Allow header.
fn allowed_methods_for(routes: &[Route], url_words: &[String]) -> Vec<Method> {
    let mut methods = Vec::new();
    for r in routes {
        if match_words(url_words, &r.words).is_some() && !methods.contains(&r.method) {
            methods.push(r.method.clone());
        }
    }
    methods
}

struct ReqInfo {
    request_id: String,
    method: String,
    raw_path: String,
    raw_query: String,
    remote_addr: String,
    user_agent: String,
    content_length: i64,
    content_type: String,
    x_forwarded_for: String,
    x_test_case: String,
}

// finalize records metrics, the /lab/calls ring entry and the single
// structured log line for the request.
fn finalize(shared: &Shared, req: &ReqInfo, route: &str, status: u16, duration_ms: i64) {
    shared.st.record_metric(route, status);
    shared.st.record_call(
        &req.request_id,
        &req.x_test_case,
        &req.method,
        &req.raw_path,
        route,
        status,
    );
    logging::write_request_line(&LogEntry {
        timestamp: crate::timefmt::now_rfc3339(),
        level: "info",
        msg: "request",
        request_id: req.request_id.clone(),
        method: req.method.clone(),
        path: req.raw_path.clone(),
        query: logging::mask_query(&req.raw_query),
        status,
        duration_ms,
        route: route.to_string(),
        remote_addr: req.remote_addr.clone(),
        user_agent: req.user_agent.clone(),
        content_length: req.content_length,
        content_type: req.content_type.clone(),
        x_forwarded_for: req.x_forwarded_for.clone(),
        x_test_case: req.x_test_case.clone(),
    });
}

// entry is the single axum catch-all: it resolves the request id, reads
// the body with the global limit, dispatches through the route table
// and finalizes (metrics + calls + log + X-Request-ID header).
pub async fn entry(
    State(shared): State<Arc<Shared>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    req: Request,
) -> Response {
    let started = Instant::now();
    let (parts, body) = req.into_parts();
    let request_id = requestid::resolve(&parts.headers);
    let method = parts.method.clone();
    let raw_path = parts.uri.path().to_string();
    let raw_query = parts.uri.query().unwrap_or("").to_string();
    let req_info = || ReqInfo {
        request_id: request_id.clone(),
        method: method.as_str().to_string(),
        raw_path: raw_path.clone(),
        raw_query: raw_query.clone(),
        remote_addr: addr.to_string(),
        user_agent: parts
            .headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string(),
        content_length: parts
            .headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0),
        content_type: parts
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string(),
        x_forwarded_for: parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string(),
        x_test_case: parts
            .headers
            .get("x-test-case")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string(),
    };

    // Global body limit: read at most MAX_BODY_BYTES + 1 so overflow is
    // detectable, then answer 413 with the JSON envelope (logged and
    // recorded in /lab/calls, unlike a backend-level rejection).
    let limit = shared.conf.max_body_bytes.unsigned_abs() as usize + 1;
    let body_bytes = match axum::body::to_bytes(body, limit).await {
        Ok(b) => b,
        Err(_) => {
            let info = req_info();
            let mut resp = error_response(
                413,
                "payload_too_large",
                &format!(
                    "request body exceeds MAX_BODY_BYTES={}",
                    shared.conf.max_body_bytes
                ),
                &request_id,
            );
            finalize(&shared, &info, "body_too_large", 413, elapsed_ms(started));
            resp.headers_mut().insert(
                header::HeaderName::from_static("x-request-id"),
                header::HeaderValue::from_str(&request_id).expect("ascii uuid"),
            );
            return resp;
        }
    };
    if body_bytes.len() as i64 > shared.conf.max_body_bytes {
        let info = req_info();
        let mut resp = error_response(
            413,
            "payload_too_large",
            &format!(
                "request body of {} bytes exceeds MAX_BODY_BYTES={}",
                body_bytes.len(),
                shared.conf.max_body_bytes
            ),
            &request_id,
        );
        finalize(&shared, &info, "body_too_large", 413, elapsed_ms(started));
        resp.headers_mut().insert(
            header::HeaderName::from_static("x-request-id"),
            header::HeaderValue::from_str(&request_id).expect("ascii uuid"),
        );
        return resp;
    }

    let words = normalize_words(&raw_path);
    let ctx = Ctx {
        method: method.clone(),
        uri: parts.uri.clone(),
        headers: parts.headers.clone(),
        query: parse_query(&raw_query),
        body: body_bytes,
        params: Vec::new(),
        request_id: request_id.clone(),
        shared: shared.clone(),
    };

    let mut matched: Option<(&Route, Vec<String>)> = None;
    for route in &shared.routes {
        if route.method == method {
            if let Some(params) = match_words(&words, &route.words) {
                matched = Some((route, params));
                break;
            }
        }
    }

    let (mut response, route_label) = match matched {
        Some((route, params)) => {
            let mut ctx = ctx;
            ctx.params = params;
            let resp = (route.handler)(ctx).await;
            (resp, route.pattern)
        }
        None => handle_unmatched(&shared, &words, &method, &request_id),
    };

    let status = response.status().as_u16();
    let info = req_info();
    finalize(&shared, &info, route_label, status, elapsed_ms(started));
    response.headers_mut().insert(
        header::HeaderName::from_static("x-request-id"),
        header::HeaderValue::from_str(&request_id).expect("ascii uuid"),
    );
    response
}

fn elapsed_ms(started: Instant) -> i64 {
    started.elapsed().as_millis() as i64
}

// handle_unmatched answers 405 with an Allow header when the path exists
// under another method, and a JSON 404 otherwise.
fn handle_unmatched(
    shared: &Shared,
    words: &[String],
    method: &Method,
    request_id: &str,
) -> (Response, &'static str) {
    let path = format!("/{}", words.join("/"));
    let methods = allowed_methods_for(&shared.routes, words);
    if !methods.is_empty() {
        let allow = methods
            .iter()
            .map(|m| m.as_str().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mut resp = error_response(
            405,
            "method_not_allowed",
            &format!(
                "method {} is not allowed for {path}; allow: {allow}",
                method.as_str()
            ),
            request_id,
        );
        resp.headers_mut().insert(
            header::ALLOW,
            header::HeaderValue::from_str(&allow).expect("method tokens are ascii"),
        );
        return (resp, "unmatched");
    }
    let resp = error_response(
        404,
        "not_found",
        &format!("no route for {path}"),
        request_id,
    );
    (resp, "unmatched")
}

// register_routes builds the deterministic route table (keep in sync
// with src/openapi/openapi.json).
pub fn register_routes() -> Vec<Route> {
    let mut routes: Vec<Route> = Vec::new();
    let mut reg = |method: Method, pattern: &'static str, h: Handler| {
        let words: Vec<&'static str> = pattern.split('/').filter(|w| !w.is_empty()).collect();
        routes.push(Route {
            method,
            pattern,
            words,
            handler: h,
        });
    };

    use crate::{
        admin, auth, files, forms, infra, lab, metrics, openapi, reports, search, tickets, users,
        webhooks,
    };

    reg(Method::GET, "/", handler(infra::index));
    reg(Method::GET, "/health", handler(infra::health));
    reg(Method::GET, "/ready", handler(infra::ready));
    reg(Method::GET, "/version", handler(infra::version));
    reg(Method::GET, "/metrics", handler(metrics::prometheus));
    reg(Method::GET, "/openapi.json", handler(openapi::spec_json));
    reg(Method::GET, "/docs", handler(openapi::swagger_ui));
    reg(Method::POST, "/api/v1/auth/login", handler(auth::login));
    reg(Method::POST, "/api/v1/auth/logout", handler(auth::logout));
    reg(Method::GET, "/api/v1/auth/me", handler(auth::me));
    reg(Method::GET, "/api/v1/users", handler(users::index));
    reg(Method::GET, "/api/v1/users/:id", handler(users::show));
    reg(Method::POST, "/api/v1/users", handler(users::create));
    reg(Method::PUT, "/api/v1/users/:id", handler(users::replace));
    reg(Method::PATCH, "/api/v1/users/:id", handler(users::update));
    reg(Method::DELETE, "/api/v1/users/:id", handler(users::destroy));
    reg(Method::GET, "/api/v1/search", handler(search::search));
    reg(Method::GET, "/api/v1/reports", handler(reports::index));
    reg(
        Method::POST,
        "/api/v1/reports/export",
        handler(reports::export),
    );
    reg(
        Method::GET,
        "/api/v1/reports/:id/download",
        handler(reports::download),
    );
    reg(Method::POST, "/api/v1/tickets", handler(tickets::create));
    reg(Method::GET, "/api/v1/tickets/:id", handler(tickets::show));
    reg(
        Method::POST,
        "/api/v1/forms/feedback",
        handler(forms::feedback),
    );
    reg(
        Method::POST,
        "/api/v1/webhooks/receive",
        handler(webhooks::receive),
    );
    reg(Method::POST, "/api/v1/graphql", handler(webhooks::graphql));
    reg(Method::POST, "/api/v1/files/upload", handler(files::upload));
    reg(Method::GET, "/static/:name", handler(files::static_file));
    reg(Method::GET, "/admin/status", handler(admin::status));
    reg(Method::POST, "/admin/config", handler(admin::config));
    reg(Method::GET, "/internal/debug", handler(admin::debug));
    reg(Method::GET, "/lab/echo", handler(lab::echo));
    reg(Method::GET, "/lab/method", handler(lab::method));
    reg(Method::HEAD, "/lab/method", handler(lab::method));
    reg(Method::POST, "/lab/method", handler(lab::method));
    reg(Method::PUT, "/lab/method", handler(lab::method));
    reg(Method::PATCH, "/lab/method", handler(lab::method));
    reg(Method::DELETE, "/lab/method", handler(lab::method));
    reg(Method::OPTIONS, "/lab/method", handler(lab::method));
    reg(Method::GET, "/lab/slow", handler(lab::slow));
    reg(Method::GET, "/lab/status/:code", handler(lab::status_code));
    reg(
        Method::GET,
        "/lab/response-marker",
        handler(lab::response_marker),
    );
    reg(Method::GET, "/lab/large", handler(lab::large));
    reg(Method::GET, "/lab/headers", handler(lab::headers));
    reg(Method::GET, "/lab/stream", handler(lab::stream));
    reg(Method::GET, "/lab/calls", handler(lab::calls));
    reg(Method::POST, "/lab/calls/reset", handler(lab::calls_reset));

    routes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_dot_segments() {
        assert_eq!(normalize_words("/static/../x"), vec!["x"]);
        assert_eq!(normalize_words("/static/.."), Vec::<String>::new());
        assert_eq!(normalize_words("/a/./b"), vec!["a", "b"]);
        assert_eq!(normalize_words("/health/"), vec!["health"]);
        // %2f decodes to a separator before routing (same as v-http-apilab).
        assert_eq!(normalize_words("/static/..%2f.."), Vec::<String>::new());
        assert_eq!(normalize_words("/lab/echo"), vec!["lab", "echo"]);
    }

    #[test]
    fn matches_params() {
        let words = vec!["api".into(), "v1".into(), "users".into(), "42".into()];
        let pattern = ["api", "v1", "users", ":id"];
        assert_eq!(match_words(&words, &pattern), Some(vec!["42".to_string()]));
        assert_eq!(match_words(&words, &["api", "v1", "users"]), None);
    }
}
