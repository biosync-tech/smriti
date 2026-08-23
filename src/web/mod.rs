use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;

use crate::graph::{GraphCache, SharedGraphCache};
use crate::storage::Database;

pub mod handlers;
pub mod response;
pub mod static_files;

/// Shared state for all web UI handlers.
///
/// Carries the database handle, graph cache, and the DB file path
/// (used by `web_stats` to measure disk usage).
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub graph_cache: SharedGraphCache,
    pub db_path: String,
}

/// Build the web UI sub-router.
///
/// This is mounted by `src/api/server.rs` when the `webui` feature is enabled.
/// It covers:
///  - POST /mcp                  — MCP JSON-RPC 2.0 over HTTP
///  - GET/POST  /api/v1/notes    — note CRUD
///  - GET       /api/v1/search   — unified FTS/semantic/hybrid search
///  - GET/PUT/DELETE /api/v1/kv  — web KV store
///  - GET       /api/v1/graph/*  — graph traversal / path
///  - GET       /api/v1/stats    — enhanced stats
///  - GET       /*               — embedded React SPA
pub fn create_web_router(db: Arc<Database>, db_path: String) -> Router {
    let graph_cache: SharedGraphCache = Arc::new(RwLock::new(GraphCache::new()));
    let state = AppState {
        db,
        graph_cache,
        db_path,
    };

    Router::new()
        // MCP JSON-RPC over HTTP (stdio transport is unchanged)
        .route("/mcp", post(handlers::mcp::handle_mcp))
        // Notes
        .route(
            "/api/v1/notes",
            get(handlers::notes::list_notes).post(handlers::notes::create_note),
        )
        .route(
            "/api/v1/notes/{id}",
            get(handlers::notes::get_note).put(handlers::notes::update_note),
        )
        .route("/api/v1/notes/{id}/links", get(handlers::notes::get_links))
        // Unified search
        .route("/api/v1/search", get(handlers::search::search))
        // KV store
        .route("/api/v1/kv", get(handlers::kv::list_kv))
        .route(
            "/api/v1/kv/{key}",
            get(handlers::kv::get_kv)
                .put(handlers::kv::set_kv)
                .delete(handlers::kv::delete_kv),
        )
        // Graph
        .route("/api/v1/graph/full", get(handlers::graph::get_full_graph))
        .route("/api/v1/graph/{id}", get(handlers::graph::get_subgraph))
        .route("/api/v1/graph/path", get(handlers::graph::get_path))
        // Stats
        .route("/api/v1/stats", get(handlers::stats::get_stats))
        // Embedded React SPA (catch-all — must be last)
        .fallback(static_files::serve_static)
        // Localhost-only CORS + tracing
        .layer(middleware::from_fn(localhost_only))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Middleware: reject requests from non-localhost origins with 403.
///
/// Requests without an `Origin` header (e.g. direct curl calls) are allowed
/// through — the privacy boundary is the OS network stack, not auth tokens.
///
/// Allowed origins: `http://localhost:*`, `http://127.0.0.1:*`,
///                  `http://[::1]:*`.
async fn localhost_only(req: Request, next: Next) -> Response {
    if let Some(origin) = req.headers().get(axum::http::header::ORIGIN) {
        if !is_localhost_origin(origin) {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(axum::body::Body::from(
                    r#"{"data":null,"error":{"code":"FORBIDDEN","message":"Non-localhost origins are not allowed"}}"#,
                ))
                .expect("valid response");
        }
    }

    // Preflight: respond to OPTIONS without going to handlers
    if req.method() == Method::OPTIONS {
        return cors_preflight_response(req.headers().get(axum::http::header::ORIGIN));
    }

    let mut response = next.run(req).await;
    add_cors_headers(&mut response);
    response
}

fn is_localhost_origin(origin: &HeaderValue) -> bool {
    let s = match origin.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    // Strip scheme, extract host
    let host = s
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let host = host.split('/').next().unwrap_or("");
    // host may include port — strip it
    let bare = if host.starts_with('[') {
        // IPv6 literal [::1]:port
        host.split(']').next().unwrap_or("").trim_start_matches('[')
    } else {
        host.split(':').next().unwrap_or("")
    };
    matches!(bare, "localhost" | "127.0.0.1" | "::1")
}

fn cors_preflight_response(origin: Option<&HeaderValue>) -> Response {
    let origin_val = origin
        .and_then(|v| v.to_str().ok())
        .unwrap_or("*")
        .to_string();
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("Access-Control-Allow-Origin", origin_val)
        .header(
            "Access-Control-Allow-Methods",
            "GET, POST, PUT, DELETE, OPTIONS",
        )
        .header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization",
        )
        .header("Access-Control-Max-Age", "86400")
        .body(axum::body::Body::empty())
        .expect("valid response")
}

fn add_cors_headers(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(
        "Access-Control-Allow-Origin",
        HeaderValue::from_static("http://localhost:5173"),
    );
    headers.insert(
        "Access-Control-Allow-Methods",
        HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
    );
    headers.insert(
        "Access-Control-Allow-Headers",
        HeaderValue::from_static("Content-Type, Authorization"),
    );
}
