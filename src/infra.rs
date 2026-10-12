// infra: index, health, readiness and version endpoints.
use serde::Serialize;

use crate::router::Ctx;
use crate::seed;
use axum::response::Response;

#[derive(Serialize)]
struct IndexBody {
    name: &'static str,
    version: &'static str,
    description: &'static str,
    endpoints: [&'static str; 8],
}

pub async fn index(ctx: Ctx) -> Response {
    ctx.json(
        200,
        &IndexBody {
            name: seed::APP_NAME,
            version: seed::APP_VERSION,
            description: "deterministic synthetic HTTP lab API",
            endpoints: [
                "/health",
                "/ready",
                "/version",
                "/metrics",
                "/openapi.json",
                "/docs",
                "/lab/echo",
                "/lab/calls",
            ],
        },
    )
}

#[derive(Serialize)]
struct StatusBody {
    status: &'static str,
}

// health always answers 200 with status ok (liveness).
pub async fn health(ctx: Ctx) -> Response {
    ctx.json(200, &StatusBody { status: "ok" })
}

// ready answers 200 unless READY_FORCE_FAIL is set (readiness drill).
pub async fn ready(ctx: Ctx) -> Response {
    if ctx.conf().ready_force_fail {
        ctx.json(
            503,
            &StatusBody {
                status: "unavailable",
            },
        )
    } else {
        ctx.json(200, &StatusBody { status: "ready" })
    }
}

#[derive(Serialize)]
struct VersionBody {
    app: &'static str,
    version: &'static str,
    rust_version: &'static str,
    build_date: String,
    deterministic_seed: bool,
}

// version reports the app version, the rustc toolchain baked at build
// time and the build date (reproducible via SOURCE_DATE_EPOCH).
pub async fn version(ctx: Ctx) -> Response {
    ctx.json(
        200,
        &VersionBody {
            app: seed::APP_NAME,
            version: seed::APP_VERSION,
            rust_version: seed::RUSTC_VERSION,
            build_date: seed::build_date(),
            deterministic_seed: true,
        },
    )
}

// keep the module referenced even if only re-exports are used later
