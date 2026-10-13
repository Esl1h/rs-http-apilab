// tickets: synthetic support tickets (create + show).
use serde::Deserialize;

use crate::router::Ctx;
use crate::seed;
use axum::response::Response;

const MAX_TITLE_LEN: usize = 120;
const MAX_DESCRIPTION_LEN: usize = 2000;
const MAX_TAGS: usize = 10;
const MAX_TAG_LEN: usize = 40;

#[derive(Deserialize, Default)]
struct TicketRequest {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    tags: Vec<String>,
}

// create validates and stores a new synthetic ticket.
pub async fn create(ctx: Ctx) -> Response {
    if let Err(resp) = ctx.require_json() {
        return resp;
    }
    let body: TicketRequest = match ctx.parse_object() {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    let mut errs: Vec<String> = Vec::new();
    if body.title.is_empty() || body.title.len() > MAX_TITLE_LEN {
        errs.push(format!(
            "\"title\" must be 1..{MAX_TITLE_LEN} characters, got {}",
            body.title.len()
        ));
    }
    if body.description.len() > MAX_DESCRIPTION_LEN {
        errs.push(format!(
            "\"description\" must be at most {MAX_DESCRIPTION_LEN} characters, got {}",
            body.description.len()
        ));
    }
    if !seed::SEVERITIES.contains(&body.severity.as_str()) {
        errs.push(format!(
            "\"severity\" must be one of {}, got \"{}\"",
            seed::SEVERITIES.join(", "),
            body.severity
        ));
    }
    if body.tags.len() > MAX_TAGS {
        errs.push(format!(
            "\"tags\" must have at most {MAX_TAGS} entries, got {}",
            body.tags.len()
        ));
    }
    for tag in &body.tags {
        if tag.len() > MAX_TAG_LEN {
            errs.push(format!(
                "tag \"{tag}\" must be at most {MAX_TAG_LEN} characters"
            ));
            break;
        }
    }
    if !errs.is_empty() {
        return ctx.error(422, "validation_error", &errs.join("; "));
    }
    let ticket = ctx
        .shared
        .st
        .ticket_add(body.title, body.description, body.severity, body.tags);
    ctx.json(201, &ticket)
}

// show returns one synthetic ticket by numeric id.
pub async fn show(ctx: Ctx) -> Response {
    let raw = &ctx.params[0];
    let id = match raw.parse::<u64>() {
        Ok(id) if id >= 1 => id,
        Ok(id) => {
            return ctx.error(
                400,
                "invalid_id",
                &format!("ticket id must be positive, got {id}"),
            )
        }
        Err(_) => {
            return ctx.error(
                400,
                "invalid_id",
                &format!("ticket id must be numeric, got \"{raw}\""),
            )
        }
    };
    match ctx.shared.st.ticket_by_id(id) {
        Some(ticket) => ctx.json(200, &ticket),
        None => ctx.error(404, "not_found", &format!("ticket {id} not found")),
    }
}
