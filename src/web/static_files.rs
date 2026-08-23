use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use include_dir::{include_dir, Dir};
use mime_guess::from_path;

/// The compiled React SPA is embedded at build time from `smriti-web/dist/`.
///
/// The directory is re-embedded on each `cargo build`.  Run
/// `cd smriti-web && npm run build` first to populate it.
///
/// During local development set `SMRITI_DEV_PROXY=1` and start Vite separately
/// on port 5173 — requests will be forwarded to the Vite dev server instead.
static DIST: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/smriti-web/dist");

/// Axum handler: serve embedded static assets.
///
/// - Known asset paths are served with their MIME type.
/// - Any unknown path returns `index.html` (SPA catch-all).
pub async fn serve_static(req: Request) -> Response {
    let path = req.uri().path().trim_start_matches('/');

    // Resolve the file from the embedded directory
    let (body, mime) = if let Some(file) = DIST.get_file(path) {
        let mime = from_path(path).first_or_octet_stream();
        (file.contents().to_vec(), mime.to_string())
    } else {
        // SPA catch-all: serve index.html for unrecognised GET routes
        match DIST.get_file("index.html") {
            Some(f) => (f.contents().to_vec(), "text/html; charset=utf-8".into()),
            None => {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from(
                        "smriti-web/dist not found — run: cd smriti-web && npm run build",
                    ))
                    .expect("valid response");
            }
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&mime)
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
        )
        .body(Body::from(body))
        .expect("valid response")
}
