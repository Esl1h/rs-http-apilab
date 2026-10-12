// admin: simulated administration endpoints.
//
// Nothing here changes real state: /admin/config only validates and
// echoes the payload back (JSON-escaped), and /admin/status plus
// /internal/debug expose only non-sensitive operational counters.
use serde::Serialize;
use serde_json::Value;

use crate::router::Ctx;
use crate::seed;
use axum::response::Response;

#[derive(Serialize)]
struct AdminStatus {
    mode: &'static str,
    uptime_seconds: i64,
    requests_total: u64,
    users: usize,
    tickets: usize,
    sessions: usize,
    export_jobs: usize,
}

// status reports non-sensitive operational counters.
pub async fn status(ctx: Ctx) -> Response {
    let st = &ctx.shared.st;
    ctx.json(
        200,
        &AdminStatus {
            mode: "lab",
            uptime_seconds: st.uptime_seconds(),
            requests_total: st.requests_total(),
            users: st.users_count(),
            tickets: st.tickets_all().len(),
            sessions: st.sessions_count(),
            export_jobs: st.jobs_count(),
        },
    )
}

#[derive(Serialize)]
struct AdminConfigResponse {
    valid: bool,
    applied: bool,
    received: Value,
    note: &'static str,
    request_id: String,
}

// config validates a JSON object and echoes it back; lab mode never
// applies configuration.
pub async fn config(ctx: Ctx) -> Response {
    if let Err(resp) = ctx.require_json() {
        return resp;
    }
    let parsed: Value = match ctx.parse_json() {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    match parsed {
        Value::Object(received) => ctx.json(
            200,
            &AdminConfigResponse {
                valid: true,
                applied: false,
                received: Value::Object(received),
                note: "lab mode: configuration is validated and echoed, never applied",
                request_id: ctx.request_id.clone(),
            },
        ),
        _ => ctx.error(
            422,
            "validation_error",
            "request body must be a JSON object of configuration keys",
        ),
    }
}

#[derive(Serialize)]
struct Counters {
    requests_total: u64,
    users: usize,
    tickets: usize,
    sessions: usize,
    export_jobs: usize,
    calls_buffered: usize,
}

#[derive(Serialize)]
struct DebugBody {
    app: &'static str,
    version: &'static str,
    rust_version: &'static str,
    build_date: String,
    started_at: String,
    uptime_seconds: i64,
    counters: Counters,
}

// debug exposes only non-sensitive runtime information.
pub async fn debug(ctx: Ctx) -> Response {
    let st = &ctx.shared.st;
    ctx.json(
        200,
        &DebugBody {
            app: seed::APP_NAME,
            version: seed::APP_VERSION,
            rust_version: seed::RUSTC_VERSION,
            build_date: seed::build_date(),
            started_at: st.started_at(),
            uptime_seconds: st.uptime_seconds(),
            counters: Counters {
                requests_total: st.requests_total(),
                users: st.users_count(),
                tickets: st.tickets_all().len(),
                sessions: st.sessions_count(),
                export_jobs: st.jobs_count(),
                calls_buffered: st.calls_all().len(),
            },
        },
    )
}
