// openapi: serves the embedded OpenAPI 3 specification and the Swagger
// UI page.
//
// The spec lives in src/openapi/openapi.json and is embedded into the
// binary at compile time, so /openapi.json works with no files on disk.
// Keep it in sync with the routes registered in src/router.rs.
use crate::router::Ctx;
use axum::response::Response;

const SPEC: &str = include_str!("openapi/openapi.json");

// spec_json serves the OpenAPI document.
pub async fn spec_json(_ctx: Ctx) -> Response {
    crate::router::text_response(200, "application/json", SPEC)
}

// swagger_ui serves a minimal Swagger UI page pointing at
// /openapi.json. The UI assets load from a CDN, so rendering this page
// needs internet access in the browser; the API itself is fully
// self-contained.
pub async fn swagger_ui(_ctx: Ctx) -> Response {
    crate::router::text_response(200, "text/html; charset=utf-8", SWAGGER_HTML)
}

const SWAGGER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>rs-http-apilab API</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5.17.14/swagger-ui.css">
</head>
<body>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@5.17.14/swagger-ui-bundle.js" charset="utf-8"></script>
<script>
  window.addEventListener('load', function () {
    window.ui = SwaggerUIBundle({
      url: '/openapi.json',
      dom_id: '#swagger-ui',
      deepLinking: true
    });
  });
</script>
</body>
</html>"#;
