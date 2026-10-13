// users: CRUD over the synthetic user table.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::router::Ctx;
use crate::seed;
use axum::response::Response;

const MAX_PER_PAGE: i64 = 100;

#[derive(Deserialize, Default)]
struct UserBody {
    #[serde(default)]
    name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    department: String,
    #[serde(default)]
    role: String,
}

#[derive(Serialize)]
struct UsersPage {
    page: i64,
    per_page: i64,
    total: usize,
    total_pages: i64,
    items: Vec<seed::User>,
}

// parse_id validates a numeric path id and answers 400 when malformed.
fn parse_id(ctx: &Ctx, raw: &str) -> Result<u64, Option<Response>> {
    match raw.parse::<i64>() {
        Ok(id) if id >= 1 => Ok(id as u64),
        Ok(id) => Err(Some(ctx.error(
            400,
            "invalid_id",
            &format!("user id must be positive, got {id}"),
        ))),
        Err(_) => Err(Some(ctx.error(
            400,
            "invalid_id",
            &format!("user id must be numeric, got \"{raw}\""),
        ))),
    }
}

// validate_field checks one field of a user payload and returns its
// error message, or None when the field is valid.
fn validate_field(field: &str, value: &str) -> Option<String> {
    match field {
        "name" if value.is_empty() || value.len() > 100 => Some(format!(
            "\"name\" must be 1..100 characters, got {}",
            value.len()
        )),
        "email"
            if value.len() < 3
                || value.len() > 254
                || !value.contains('@')
                || !value.contains('.')
                || value.contains(' ') =>
        {
            Some(format!(
                "\"email\" must be a syntactic email (user@domain.tld), got \"{value}\""
            ))
        }
        "department" if value.is_empty() || value.len() > 50 => Some(format!(
            "\"department\" must be 1..50 characters, got {}",
            value.len()
        )),
        "role" if !seed::ROLES.contains(&value) => Some(format!(
            "\"role\" must be one of {}, got \"{value}\"",
            seed::ROLES.join(", ")
        )),
        _ => None,
    }
}

fn validate_body(body: &UserBody) -> Vec<String> {
    [
        ("name", &body.name),
        ("email", &body.email),
        ("department", &body.department),
        ("role", &body.role),
    ]
    .iter()
    .filter_map(|(field, value)| validate_field(field, value))
    .collect()
}

fn respond_unprocessable(ctx: &Ctx, errs: Vec<String>) -> Response {
    ctx.error(422, "validation_error", &errs.join("; "))
}

// parse_positive_int validates an optional numeric query parameter.
// Empty/absent yields the provided default; malformed answers 400.
fn parse_positive_int(
    ctx: &Ctx,
    raw: Option<&str>,
    field: &str,
    default: i64,
) -> Result<i64, Option<Response>> {
    let raw = match raw {
        None | Some("") => return Ok(default),
        Some(v) => v,
    };
    match raw.parse::<i64>() {
        Ok(v) if v >= 1 => Ok(v),
        Ok(v) => Err(Some(ctx.error(
            422,
            "validation_error",
            &format!("\"{field}\" must be at least 1, got {v}"),
        ))),
        Err(_) => Err(Some(ctx.error(
            400,
            "invalid_query_value",
            &format!("\"{field}\" must be an integer, got \"{raw}\""),
        ))),
    }
}

// index lists users with pagination and an optional department filter.
pub async fn index(ctx: Ctx) -> Response {
    let page = match parse_positive_int(&ctx, ctx.query_param("page"), "page", 1) {
        Ok(v) => v,
        Err(resp) => return resp.unwrap(),
    };
    let per_page = match parse_positive_int(&ctx, ctx.query_param("per_page"), "per_page", 20) {
        Ok(v) => v,
        Err(resp) => return resp.unwrap(),
    };
    if per_page > MAX_PER_PAGE {
        return ctx.error(
            422,
            "validation_error",
            &format!("\"per_page\" must be at most {MAX_PER_PAGE}, got {per_page}"),
        );
    }
    let department = ctx.query_param("department").unwrap_or("").to_string();
    let filtered: Vec<_> = ctx
        .shared
        .st
        .users_all()
        .into_iter()
        .filter(|u| department.is_empty() || u.department == department)
        .collect();
    let total = filtered.len();
    let total_pages = if total == 0 {
        0
    } else {
        ((total as i64) + per_page - 1) / per_page
    };
    if page > total_pages && total > 0 {
        return ctx.error(
            422,
            "validation_error",
            &format!("\"page\" {page} is out of range (total_pages={total_pages})"),
        );
    }
    let start = ((page - 1) * per_page) as usize;
    let end = (start + per_page as usize).min(total);
    let items = if start >= total {
        Vec::new()
    } else {
        filtered[start..end].to_vec()
    };
    ctx.json(
        200,
        &UsersPage {
            page,
            per_page,
            total,
            total_pages,
            items,
        },
    )
}

// show returns one user by numeric id.
pub async fn show(ctx: Ctx) -> Response {
    let id = match parse_id(&ctx, &ctx.params[0]) {
        Ok(id) => id,
        Err(resp) => return resp.unwrap(),
    };
    match ctx.shared.st.user_by_id(id) {
        Some(user) => ctx.json(200, &user),
        None => ctx.error(404, "not_found", &format!("user {id} not found")),
    }
}

// create validates and appends a new user.
pub async fn create(ctx: Ctx) -> Response {
    if let Err(resp) = ctx.require_json() {
        return resp;
    }
    let body: UserBody = match ctx.parse_object() {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let errs = validate_body(&body);
    if !errs.is_empty() {
        return respond_unprocessable(&ctx, errs);
    }
    let user = ctx
        .shared
        .st
        .user_add(body.name, body.email, body.department, body.role);
    ctx.json(201, &user)
}

// replace fully updates an existing user (PUT).
pub async fn replace(ctx: Ctx) -> Response {
    let id = match parse_id(&ctx, &ctx.params[0]) {
        Ok(id) => id,
        Err(resp) => return resp.unwrap(),
    };
    if let Err(resp) = ctx.require_json() {
        return resp;
    }
    let body: UserBody = match ctx.parse_object() {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let errs = validate_body(&body);
    if !errs.is_empty() {
        return respond_unprocessable(&ctx, errs);
    }
    match ctx
        .shared
        .st
        .user_replace(id, body.name, body.email, body.department, body.role)
    {
        Some(user) => ctx.json(200, &user),
        None => ctx.error(404, "not_found", &format!("user {id} not found")),
    }
}

// update partially updates an existing user (PATCH). The body is any
// subset of the writable fields; unknown fields are rejected.
pub async fn update(ctx: Ctx) -> Response {
    let id = match parse_id(&ctx, &ctx.params[0]) {
        Ok(id) => id,
        Err(resp) => return resp.unwrap(),
    };
    if let Err(resp) = ctx.require_json() {
        return resp;
    }
    let raw: serde_json::Map<String, serde_json::Value> = match ctx.parse_json() {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let mut patch: HashMap<String, String> = HashMap::new();
    let mut errs: Vec<String> = Vec::new();
    for (key, value) in &raw {
        if !["name", "email", "department", "role"].contains(&key.as_str()) {
            errs.push(format!(
                "unknown field \"{key}\" (allowed: name, email, department, role)"
            ));
            continue;
        }
        match value.as_str() {
            Some(str_value) => match validate_field(key, str_value) {
                Some(msg) => errs.push(msg),
                None => {
                    patch.insert(key.clone(), str_value.to_string());
                }
            },
            None => errs.push(format!("field \"{key}\" must be a JSON string")),
        }
    }
    if !errs.is_empty() {
        return respond_unprocessable(&ctx, errs);
    }
    if patch.is_empty() {
        return ctx.error(
            422,
            "validation_error",
            "empty patch: provide at least one of name, email, department, role",
        );
    }
    match ctx.shared.st.user_patch(id, &patch) {
        Some(user) => ctx.json(200, &user),
        None => ctx.error(404, "not_found", &format!("user {id} not found")),
    }
}

// destroy removes a user (204 on success).
pub async fn destroy(ctx: Ctx) -> Response {
    let id = match parse_id(&ctx, &ctx.params[0]) {
        Ok(id) => id,
        Err(resp) => return resp.unwrap(),
    };
    if ctx.shared.st.user_delete(id) {
        axum::http::Response::builder()
            .status(204)
            .body(axum::body::Body::empty())
            .expect("static response parts are valid")
    } else {
        ctx.error(404, "not_found", &format!("user {id} not found"))
    }
}
