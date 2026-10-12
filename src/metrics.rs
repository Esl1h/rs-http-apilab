// metrics: simple, honest Prometheus text exposition.
//
// Counters are in-memory request counts by route pattern and status
// code, gathered from the state module. No histogram, no process
// exporter: only what this app actually measures.
use crate::router::Ctx;
use crate::seed;
use axum::response::Response;

// prometheus renders the /metrics body in the Prometheus text format
// (version 0.0.4).
pub async fn prometheus(ctx: Ctx) -> Response {
    let st = &ctx.shared.st;
    let mut body = String::new();
    body.push_str("# HELP rs_http_apilab_up Service liveness (1 = up).\n");
    body.push_str("# TYPE rs_http_apilab_up gauge\n");
    body.push_str("rs_http_apilab_up 1\n");
    body.push_str("# HELP rs_http_apilab_uptime_seconds Seconds since process start.\n");
    body.push_str("# TYPE rs_http_apilab_uptime_seconds gauge\n");
    body.push_str(&format!(
        "rs_http_apilab_uptime_seconds {}\n",
        st.uptime_seconds()
    ));
    body.push_str("# HELP rs_http_apilab_build_info Build information (constant 1).\n");
    body.push_str("# TYPE rs_http_apilab_build_info gauge\n");
    body.push_str(&format!(
        "rs_http_apilab_build_info{{app=\"{}\",version=\"{}\"}} 1\n",
        seed::APP_NAME,
        seed::APP_VERSION
    ));
    body.push_str("# HELP rs_http_apilab_requests_total Requests handled by route and status.\n");
    body.push_str("# TYPE rs_http_apilab_requests_total counter\n");
    body.push_str(&st.metrics_counters_text());
    crate::router::text_response(200, "text/plain; version=0.0.4; charset=utf-8", body)
}
