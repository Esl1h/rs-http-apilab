// search: substring search over the synthetic in-memory corpus.
use serde::Serialize;

use crate::router::Ctx;
use crate::seed;
use axum::response::Response;

const MAX_QUERY_LEN: usize = 256;
const MAX_LIMIT: i64 = 50;

#[derive(Serialize)]
struct SearchHit<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    id: String,
    title: std::borrow::Cow<'a, str>,
}

#[derive(Serialize)]
struct SearchBody<'a> {
    query: &'a str,
    limit: i64,
    results: Vec<SearchHit<'a>>,
}

// search matches users, reports and synthetic documents by substring
// (case-insensitive) and returns up to `limit` hits.
pub async fn search(ctx: Ctx) -> Response {
    let query = match ctx.query_param("q") {
        None | Some("") => {
            return ctx.error(
                422,
                "validation_error",
                "missing required query parameter \"q\"",
            )
        }
        Some(q) => q.to_string(),
    };
    if query.len() > MAX_QUERY_LEN {
        return ctx.error(
            400,
            "invalid_query_value",
            &format!(
                "\"q\" must be at most {MAX_QUERY_LEN} characters, got {}",
                query.len()
            ),
        );
    }
    let limit_raw = ctx.query_param("limit").unwrap_or("10");
    let limit = match limit_raw.parse::<i64>() {
        Ok(v) => v,
        Err(_) => {
            return ctx.error(
                400,
                "invalid_query_value",
                &format!("\"limit\" must be an integer, got \"{limit_raw}\""),
            )
        }
    };
    if !(1..=MAX_LIMIT).contains(&limit) {
        return ctx.error(
            422,
            "validation_error",
            &format!("\"limit\" must be between 1 and {MAX_LIMIT}, got {limit}"),
        );
    }

    let needle = query.to_lowercase();
    let mut results: Vec<SearchHit> = Vec::new();
    if !needle.is_empty() {
        for u in ctx.shared.st.users_all() {
            if results.len() as i64 >= limit {
                break;
            }
            if u.name.to_lowercase().contains(&needle)
                || u.email.to_lowercase().contains(&needle)
                || u.department.to_lowercase().contains(&needle)
            {
                results.push(SearchHit {
                    kind: "user",
                    id: u.id.to_string(),
                    title: u.name.into(),
                });
            }
        }
        for r in seed::reports() {
            if results.len() as i64 >= limit {
                break;
            }
            if r.title.to_lowercase().contains(&needle)
                || r.category.to_lowercase().contains(&needle)
            {
                results.push(SearchHit {
                    kind: "report",
                    id: r.report_id.clone(),
                    title: r.title.into(),
                });
            }
        }
        for d in &seed::SEARCH_DOCS {
            if results.len() as i64 >= limit {
                break;
            }
            if d.title.to_lowercase().contains(&needle) {
                results.push(SearchHit {
                    kind: "document",
                    id: d.id.to_string(),
                    title: d.title.into(),
                });
            }
        }
    }
    ctx.json(
        200,
        &SearchBody {
            query: &query,
            limit,
            results,
        },
    )
}
