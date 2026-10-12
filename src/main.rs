// Returning `Result<_, Response>` is idiomatic for this codebase; axum's
// Response is large but cheap to move, so silence the lint crate-wide.
#![allow(clippy::result_large_err)]

// rs-http-apilab: a deterministic synthetic HTTP API simulator.
//
// Rust port of the v-http-lab concept: every response is derived from
// fixed synthetic in-memory data; running the binary is enough to have
// the API answering on the configured port.
mod admin;
mod auth;
mod config;
mod files;
mod forms;
mod infra;
mod lab;
mod logging;
mod metrics;
mod openapi;
mod reports;
mod requestid;
mod router;
mod search;
mod seed;
mod state;
mod tickets;
mod timefmt;
mod users;
mod webhooks;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::routing::any;
use axum::Router;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use tower::ServiceExt;

// axum::serve does not expose any HTTP-level configuration, so the
// accept loop is ours (the same pattern axum uses internally): it lets
// READ_TIMEOUT_MS map onto hyper's header read timeout with native
// millisecond granularity and keeps a 16 KiB read buffer, mirroring
// v-http-lab's documented header-size limit.
const HTTP1_MAX_BUF_SIZE: usize = 16 * 1024;

#[tokio::main]
async fn main() {
    let conf = match config::load() {
        Ok(conf) => Arc::new(conf),
        Err(err) => {
            eprintln!("rs-http-apilab: invalid configuration: {err}");
            std::process::exit(1);
        }
    };
    let shared = Arc::new(router::Shared {
        st: Arc::new(state::AppState::new()),
        conf: conf.clone(),
        routes: router::register_routes(),
    });

    let app: Router = Router::new()
        .route("/", any(router::entry))
        .route("/{*path}", any(router::entry))
        .with_state(shared);
    let make_service = app.into_make_service_with_connect_info::<SocketAddr>();

    logging::write_event(
        "info",
        &format!(
            "rs-http-apilab {} starting on 0.0.0.0:{}",
            seed::APP_VERSION,
            conf.port
        ),
    );

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", conf.port)).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("rs-http-apilab: server failed to start: {err}");
            std::process::exit(1);
        }
    };
    let read_timeout = Duration::from_millis(conf.read_timeout_ms as u64);

    loop {
        let (stream, _remote_addr) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(err) => {
                eprintln!("rs-http-apilab: accept error: {err}");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let per_conn_service = match make_service.clone().oneshot(_remote_addr).await {
            Ok(service) => service,
            Err(err) => match err {},
        };
        tokio::spawn(async move {
            let mut http = hyper::server::conn::http1::Builder::new();
            http.timer(TokioTimer::default())
                .header_read_timeout(read_timeout)
                .max_buf_size(HTTP1_MAX_BUF_SIZE);
            if let Err(err) = http
                .serve_connection(
                    TokioIo::new(stream),
                    TowerToHyperService::new(per_conn_service),
                )
                .await
            {
                tracing_quiet(err);
            }
        });
    }
}

// Connection-level errors (client resets, oversized heads, malformed
// requests) never reach the application; keep them out of stdout so
// every stdout line stays valid JSON. v-http-lab had the same property.
fn tracing_quiet(_err: hyper::Error) {}
