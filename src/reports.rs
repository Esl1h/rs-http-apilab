// reports: synthetic report listing, export jobs and CSV download.
use serde::{Deserialize, Serialize};

use crate::router::Ctx;
use crate::seed;
use axum::response::Response;

#[derive(Serialize)]
struct ReportsBody<'a> {
    count: usize,
    items: &'a [seed::Report],
}

// index lists the fixed synthetic report catalog.
pub async fn index(ctx: Ctx) -> Response {
    let items = seed::reports();
    ctx.json(
        200,
        &ReportsBody {
            count: items.len(),
            items: &items,
        },
    )
}

#[derive(Deserialize, Default)]
struct ExportRequest {
    #[serde(default)]
    report_id: String,
    #[serde(default)]
    format: String,
}

#[derive(Serialize)]
struct ExportBody {
    job_id: String,
    status: &'static str,
    report_id: String,
    format: String,
    request_id: String,
}

// export validates the request and queues a synthetic job (nothing
// actually runs; the job list is inspectable via /internal/debug).
pub async fn export(ctx: Ctx) -> Response {
    if let Err(resp) = ctx.require_json() {
        return resp;
    }
    let body: ExportRequest = match ctx.parse_object() {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    if body.report_id.is_empty() || body.format.is_empty() {
        return ctx.error(
            422,
            "validation_error",
            "both \"report_id\" and \"format\" are required",
        );
    }
    let def = match seed::report_by_id(&body.report_id) {
        Some(def) => def,
        None => {
            return ctx.error(
                404,
                "not_found",
                &format!("report {} not found", body.report_id),
            )
        }
    };
    if body.format != "csv" && body.format != "json" {
        return ctx.error(
            422,
            "validation_error",
            &format!(
                "\"format\" must be one of csv, json, got \"{}\"",
                body.format
            ),
        );
    }
    let job = ctx.shared.st.job_add(def.id, &body.format);
    ctx.json(
        202,
        &ExportBody {
            job_id: job.job_id,
            status: "queued",
            report_id: job.report_id,
            format: job.format,
            request_id: ctx.request_id.clone(),
        },
    )
}

// csv_quote quotes one CSV field when needed.
fn csv_quote(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

// download serves the synthetic CSV content of a report.
pub async fn download(ctx: Ctx) -> Response {
    let def = match seed::report_by_id(&ctx.params[0]) {
        Some(def) => def,
        None => {
            return ctx.error(
                404,
                "not_found",
                &format!("report {} not found", ctx.params[0]),
            )
        }
    };
    let mut body = String::from("region,label,value\n");
    for row in 1..=def.rows {
        body.push_str(&format!(
            "{},synthetic_{}_{},{}\n",
            csv_quote(&format!("Region {row}")),
            def.id,
            row,
            row * 7
        ));
    }
    axum::http::Response::builder()
        .status(200)
        .header(axum::http::header::CONTENT_TYPE, "text/csv")
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}.csv\"", def.id),
        )
        .body(axum::body::Body::from(body))
        .expect("static response parts are valid")
}
