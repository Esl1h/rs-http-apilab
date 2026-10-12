// forms: application/x-www-form-urlencoded form endpoint.
//
// The urlencoded body is parsed with form_urlencoded (the same rules as
// the query string) and only validated here. The submitted values are
// echoed back JSON-escaped, never interpreted.
use serde::Serialize;

use crate::router::Ctx;
use axum::response::Response;

const MAX_NAME_LEN: usize = 100;
const MAX_COMMENT_LEN: usize = 500;
const MAX_EMAIL_LEN: usize = 254;

#[derive(Serialize)]
struct FeedbackResponse {
    received: bool,
    name: String,
    rating: i64,
    comment: String,
    email: String,
    request_id: String,
}

// feedback validates and echoes a urlencoded feedback form.
pub async fn feedback(ctx: Ctx) -> Response {
    let ct = ctx.content_type();
    if ct != "application/x-www-form-urlencoded" {
        let got = if ct.is_empty() { "none" } else { &ct };
        return ctx.error(
            415,
            "unsupported_media_type",
            &format!("expected Content-Type application/x-www-form-urlencoded, got \"{got}\""),
        );
    }
    let form: std::collections::HashMap<String, String> = form_urlencoded::parse(&ctx.body)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let rating_raw = match form.get("rating") {
        Some(v) => v.clone(),
        None => {
            return ctx.error(
                422,
                "validation_error",
                "missing required form field \"rating\"",
            )
        }
    };
    let rating = match rating_raw.parse::<i64>() {
        Ok(v) => v,
        Err(_) => {
            return ctx.error(
                400,
                "invalid_form_value",
                &format!("\"rating\" must be an integer between 1 and 5, got \"{rating_raw}\""),
            )
        }
    };
    if !(1..=5).contains(&rating) {
        return ctx.error(
            422,
            "validation_error",
            &format!("\"rating\" must be between 1 and 5, got {rating}"),
        );
    }
    let name = form.get("name").cloned().unwrap_or_default();
    if name.len() > MAX_NAME_LEN {
        return ctx.error(
            422,
            "validation_error",
            &format!(
                "\"name\" must be at most {MAX_NAME_LEN} characters, got {}",
                name.len()
            ),
        );
    }
    let comment = form.get("comment").cloned().unwrap_or_default();
    if comment.len() > MAX_COMMENT_LEN {
        return ctx.error(
            422,
            "validation_error",
            &format!(
                "\"comment\" must be at most {MAX_COMMENT_LEN} characters, got {}",
                comment.len()
            ),
        );
    }
    let email = form.get("email").cloned().unwrap_or_default();
    if email.len() > MAX_EMAIL_LEN {
        return ctx.error(
            422,
            "validation_error",
            &format!(
                "\"email\" must be at most {MAX_EMAIL_LEN} characters, got {}",
                email.len()
            ),
        );
    }
    ctx.json(
        200,
        &FeedbackResponse {
            received: true,
            name,
            rating,
            comment,
            email,
            request_id: ctx.request_id.clone(),
        },
    )
}
