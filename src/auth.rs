// auth: synthetic login/logout/me flow.
//
// This is NOT real authentication: credentials are fixed lab values and
// the token is an in-memory opaque string. The flow exists so lab tests
// can exercise Authorization headers, login bodies and 401 paths. The
// password is never logged or echoed.
use serde::{Deserialize, Serialize};

use crate::router::Ctx;
use axum::response::Response;

#[derive(Deserialize, Default)]
struct LoginRequest {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    token_type: &'static str,
    expires_in: i64,
    username: String,
    request_id: String,
}

// constant_time_eq compares two byte strings in constant time (for
// synthetic credentials; replaces V's crypto.subtle compare).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// bearer_token extracts the token from "Authorization: Bearer <token>".
pub fn bearer_token(ctx: &Ctx) -> String {
    let header = ctx.header("authorization");
    let (scheme, rest) = header.split_at(header.len().min(7));
    if scheme.eq_ignore_ascii_case("bearer ") {
        rest.to_string()
    } else {
        String::new()
    }
}

// login validates the fixed synthetic credentials and issues a token.
pub async fn login(ctx: Ctx) -> Response {
    if let Err(resp) = ctx.require_json() {
        return resp;
    }
    let body: LoginRequest = match ctx.parse_object() {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    if body.username.is_empty() || body.password.is_empty() {
        return ctx.error(
            422,
            "validation_error",
            "both \"username\" and \"password\" are required",
        );
    }
    let conf = ctx.conf();
    let user_ok = constant_time_eq(body.username.as_bytes(), conf.auth_username.as_bytes());
    let pass_ok = constant_time_eq(body.password.as_bytes(), conf.auth_password.as_bytes());
    if !user_ok || !pass_ok {
        return ctx.error(401, "invalid_credentials", "invalid username or password");
    }
    let token = ctx
        .shared
        .st
        .session_create(&body.username, conf.token_ttl_seconds);
    ctx.json(
        200,
        &LoginResponse {
            token,
            token_type: "Bearer",
            expires_in: conf.token_ttl_seconds,
            username: body.username,
            request_id: ctx.request_id.clone(),
        },
    )
}

#[derive(Serialize)]
struct LogoutResponse {
    logged_out: bool,
    request_id: String,
}

// logout revokes the presented Bearer token.
pub async fn logout(ctx: Ctx) -> Response {
    let token = bearer_token(&ctx);
    if token.is_empty() {
        return ctx.error(401, "unauthorized", "missing Bearer token");
    }
    if ctx.shared.st.session_delete(&token) {
        ctx.json(
            200,
            &LogoutResponse {
                logged_out: true,
                request_id: ctx.request_id.clone(),
            },
        )
    } else {
        ctx.error(401, "unauthorized", "invalid or expired Bearer token")
    }
}

#[derive(Serialize)]
struct MeBody {
    username: String,
    display_name: &'static str,
    role: &'static str,
    email: &'static str,
    request_id: String,
}

// me echoes the synthetic profile bound to the Bearer token.
pub async fn me(ctx: Ctx) -> Response {
    let token = bearer_token(&ctx);
    if token.is_empty() {
        return ctx.error(401, "unauthorized", "missing Bearer token");
    }
    match ctx.shared.st.session_lookup(&token) {
        Some(username) => ctx.json(
            200,
            &MeBody {
                username,
                display_name: "Lab Administrator (synthetic)",
                role: "admin",
                email: "admin@lab.example",
                request_id: ctx.request_id.clone(),
            },
        ),
        None => ctx.error(401, "unauthorized", "invalid or expired Bearer token"),
    }
}
