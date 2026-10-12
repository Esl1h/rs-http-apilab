// files: multipart upload (analyzed in memory) and the /static allowlist.
//
// Nothing is written to disk: only a sanitized name, the size and the
// SHA-256 are reported. The multipart parser is axum's (multer), fed
// from the already-bounded body bytes.
use axum::body::Body;
use axum::extract::{FromRequest, Multipart};
use axum::http::{header, Request};
use axum::response::Response;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::router::Ctx;
use crate::seed;

const MAX_FILENAME_LEN: usize = 128;

#[derive(Serialize)]
struct UploadSummary {
    filename: String,
    size: usize,
    content_type: String,
    sha256: String,
    stored: bool,
    request_id: String,
}

// sanitize_filename reduces a client filename to a safe token: only the
// base name, restricted to letters, digits, dot, dash and underscore.
// Returns "upload.bin" when nothing safe remains.
pub fn sanitize_filename(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = cleaned.trim_start_matches('.');
    if name.is_empty() || name.len() > MAX_FILENAME_LEN {
        "upload.bin".to_string()
    } else {
        name.to_string()
    }
}

// upload accepts multipart/form-data (field "file") or
// application/octet-stream (name via ?name=) and answers the analysis.
pub async fn upload(ctx: Ctx) -> Response {
    let ct = ctx.content_type();
    if ct == "multipart/form-data" {
        return upload_multipart(ctx).await;
    }
    if ct == "application/octet-stream" {
        let name = match ctx.query_param("name") {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                return ctx.error(
                    422,
                    "validation_error",
                    "octet-stream uploads require a \"?name=\" query parameter",
                )
            }
        };
        if ctx.body.is_empty() {
            return ctx.error(422, "validation_error", "request body must not be empty");
        }
        return summary(&ctx, &name, "application/octet-stream", &ctx.body);
    }
    let got = if ct.is_empty() { "none" } else { &ct };
    ctx.error(
        415,
        "unsupported_media_type",
        &format!("expected multipart/form-data or application/octet-stream, got \"{got}\""),
    )
}

// upload_multipart reconstructs a request around the already-read body
// so axum's Multipart extractor can parse it.
async fn upload_multipart(ctx: Ctx) -> Response {
    let mut builder = Request::builder()
        .method(ctx.method.clone())
        .uri(ctx.uri.clone());
    for (name, value) in &ctx.headers {
        builder = builder.header(name, value);
    }
    let req = builder
        .body(Body::from(ctx.body.clone()))
        .expect("request parts come from a parsed request");
    let mut multipart = match Multipart::from_request(req, &()).await {
        Ok(mp) => mp,
        Err(_) => {
            return ctx.error(
                400,
                "invalid_multipart",
                "multipart body could not be parsed",
            )
        }
    };
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().map(str::to_string);
        let content_type = field.content_type().map(str::to_string);
        let data = match field.bytes().await {
            Ok(data) => data,
            Err(_) => {
                return ctx.error(
                    400,
                    "invalid_multipart",
                    "multipart body could not be parsed",
                )
            }
        };
        if data.is_empty() {
            return ctx.error(
                422,
                "validation_error",
                "multipart part \"file\" must not be empty",
            );
        }
        let name = filename.unwrap_or_else(|| "upload.bin".to_string());
        let ct = content_type.unwrap_or_default();
        let ct = if ct.is_empty() {
            "application/octet-stream"
        } else {
            &ct
        };
        return summary(&ctx, &name, ct, &data);
    }
    ctx.error(422, "validation_error", "missing multipart part \"file\"")
}

fn summary(ctx: &Ctx, raw_name: &str, content_type: &str, data: &[u8]) -> Response {
    let mut hasher = Sha256::new();
    hasher.update(data);
    ctx.json(
        201,
        &UploadSummary {
            filename: sanitize_filename(raw_name),
            size: data.len(),
            content_type: content_type.to_string(),
            sha256: format!("{:x}", hasher.finalize()),
            stored: false,
            request_id: ctx.request_id.clone(),
        },
    )
}

// static_file serves only names present in the embedded allowlist.
// Path traversal is rejected explicitly (400) instead of relying on
// the 404 that an unknown name would produce.
pub async fn static_file(ctx: Ctx) -> Response {
    let name = &ctx.params[0];
    if name.contains('\\') || name.contains('\0') || name.contains("..") {
        return ctx.error(
            400,
            "path_traversal_rejected",
            &format!("invalid static file name \"{name}\""),
        );
    }
    match seed::static_file_by_name(name) {
        Some(file) => {
            let mut resp = crate::router::text_response(200, file.content_type, file.body);
            resp.headers_mut().insert(
                header::X_CONTENT_TYPE_OPTIONS,
                header::HeaderValue::from_static("nosniff"),
            );
            resp
        }
        None => ctx.error(
            404,
            "not_found",
            &format!("static file \"{name}\" is not in the allowlist"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_filenames() {
        assert_eq!(sanitize_filename("../../etc/secret.conf"), "secret.conf");
        assert_eq!(sanitize_filename("a\\b\\c.txt"), "c.txt");
        assert_eq!(sanitize_filename("weird name!.bin"), "weird_name_.bin");
        assert_eq!(sanitize_filename("..."), "upload.bin");
        assert_eq!(sanitize_filename(""), "upload.bin");
    }
}
