// webhooks: receives arbitrary webhook payloads and answers a summary,
// plus the documented GraphQL stub (no query is ever executed).
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::router::Ctx;
use axum::response::Response;

#[derive(Serialize)]
struct WebhookSummary {
    received_bytes: usize,
    content_type: String,
    json_valid: bool,
    top_level_keys: Vec<String>,
    body_sha256: String,
    stored: bool,
    request_id: String,
}

// receive accepts any content type up to MAX_BODY_BYTES and answers a
// summary with a proof of receipt (hash) but no body content.
pub async fn receive(ctx: Ctx) -> Response {
    let mut json_valid = false;
    let mut keys: Vec<String> = Vec::new();
    if !ctx.body.is_empty() {
        if let Ok(parsed) = serde_json::from_slice::<Value>(&ctx.body) {
            json_valid = true;
            if let Value::Object(map) = parsed {
                keys = map.keys().cloned().collect();
                keys.sort();
            }
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(&ctx.body);
    ctx.json(
        200,
        &WebhookSummary {
            received_bytes: ctx.body.len(),
            content_type: ctx.content_type(),
            json_valid,
            top_level_keys: keys,
            body_sha256: format!("{:x}", hasher.finalize()),
            stored: false,
            request_id: ctx.request_id.clone(),
        },
    )
}

const MAX_QUERY_LEN: usize = 2048;
const MAX_VARIABLES: usize = 64;

#[derive(Serialize)]
struct GraphqlBody {
    data: std::collections::BTreeMap<&'static str, &'static str>,
    errors: Vec<String>,
    extensions: GraphqlExtensions,
}

#[derive(Serialize)]
struct GraphqlExtensions {
    query_length: usize,
    variables_count: usize,
    variables_keys: Vec<String>,
    request_id: String,
}

// graphql validates {"query": "...", "variables": {...}} and answers the
// fixed documented response; nothing is executed.
pub async fn graphql(ctx: Ctx) -> Response {
    if let Err(resp) = ctx.require_json() {
        return resp;
    }
    let parsed: Value = match ctx.parse_json() {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let obj = match parsed {
        Value::Object(map) => map,
        _ => {
            return ctx.error(
                422,
                "validation_error",
                "request body must be a JSON object with \"query\" and optional \"variables\"",
            )
        }
    };
    let query = match obj.get("query") {
        None => return ctx.error(422, "validation_error", "missing required field \"query\""),
        Some(Value::String(s)) => s.clone(),
        Some(_) => return ctx.error(422, "validation_error", "field \"query\" must be a string"),
    };
    if query.is_empty() {
        return ctx.error(422, "validation_error", "field \"query\" must not be empty");
    }
    if query.len() > MAX_QUERY_LEN {
        return ctx.error(
            422,
            "validation_error",
            &format!(
                "field \"query\" must be at most {MAX_QUERY_LEN} characters, got {}",
                query.len()
            ),
        );
    }
    let mut variables_count = 0;
    let mut variables_keys: Vec<String> = Vec::new();
    if let Some(variables_value) = obj.get("variables") {
        match variables_value {
            Value::Object(variables) => {
                variables_keys = variables.keys().cloned().collect();
                variables_keys.sort();
                variables_count = variables_keys.len();
                if variables_count > MAX_VARIABLES {
                    return ctx.error(
                        422,
                        "validation_error",
                        &format!(
                            "\"variables\" must have at most {MAX_VARIABLES} keys, got {variables_count}"
                        ),
                    );
                }
            }
            _ => {
                return ctx.error(
                    422,
                    "validation_error",
                    "field \"variables\" must be a JSON object",
                )
            }
        }
    }
    let mut data = std::collections::BTreeMap::new();
    data.insert("lab", "ok");
    ctx.json(
        200,
        &GraphqlBody {
            data,
            errors: Vec::new(),
            extensions: GraphqlExtensions {
                query_length: query.len(),
                variables_count,
                variables_keys,
                request_id: ctx.request_id.clone(),
            },
        },
    )
}
