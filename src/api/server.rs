use axum::http::Method;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::ai::AutoEmbedder;
use crate::graph::{GraphCache, SharedGraphCache};
use crate::inference::{AuditedInference, InferenceBackend, InferenceConfig, SharedBackend};
use crate::licensing::FeatureGate;
use crate::storage::Database;

use super::routes::{agent, ai as ai_routes, consolidation, graph, notes, semantic};

/// Shared application state passed to all handlers
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub graph_cache: SharedGraphCache,
}

/// Extended application state for AI endpoints
#[derive(Clone)]
pub struct AiAppState {
    pub db: Arc<Database>,
    pub backend: Arc<AuditedInference>,
    pub feature_gate: FeatureGate,
    pub inference_config: InferenceConfig,
    pub auto_embedder: Option<Arc<AutoEmbedder>>,
}

impl AiAppState {
    /// Return the backend as a type-erased `SharedBackend` for call sites that
    /// still expect `Arc<dyn InferenceBackend>` (e.g. existing AI feature
    /// constructors). The coercion `Arc<AuditedInference> → Arc<dyn InferenceBackend>`
    /// is valid because `AuditedInference: InferenceBackend`.
    pub fn backend_dyn(&self) -> SharedBackend {
        self.backend.clone() as Arc<dyn InferenceBackend>
    }
}

/// Build the complete Axum router with all routes
pub fn create_router(db: Arc<Database>) -> Router {
    let graph_cache = Arc::new(RwLock::new(GraphCache::new()));
    let state = AppState { db, graph_cache };

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(Any);

    Router::new()
        // Health check
        .route("/health", get(health_check))
        // Note routes
        .route(
            "/api/v1/notes",
            get(notes::list_notes).post(notes::create_note),
        )
        .route(
            "/api/v1/notes/{id}",
            get(notes::get_note)
                .put(notes::update_note)
                .delete(notes::delete_note),
        )
        .route("/api/v1/notes/search", get(notes::search_notes))
        .route("/api/v1/notes/{id}/backlinks", get(notes::get_backlinks))
        .route("/api/v1/notes/{id}/links", get(notes::get_forward_links))
        .route("/api/v1/notes/{id}/tags", post(notes::add_tags))
        // Path A: local KG — document ingest + context retrieval
        .route("/api/v1/ingest/document", post(notes::ingest_document))
        .route("/api/v1/retrieve", post(notes::retrieve_context))
        .route("/api/v1/consolidation/run", post(consolidation::run))
        .route("/api/v1/consolidation/events", get(consolidation::events))
        .route(
            "/api/v1/consolidation/proposals",
            get(consolidation::proposals),
        )
        .route(
            "/api/v1/consolidation/proposals/{id}/accept",
            post(consolidation::accept_proposal),
        )
        .route(
            "/api/v1/consolidation/proposals/{id}/reject",
            post(consolidation::reject_proposal_route),
        )
        .route("/api/v1/notes/{id}/lineage", get(consolidation::lineage))
        .route(
            "/api/v1/notes/{id}/access",
            post(consolidation::record_access),
        )
        // Embedding / semantic search routes
        .route("/api/v1/notes/{id}/embed", post(semantic::store_embedding))
        .route(
            "/api/v1/notes/search/semantic",
            post(semantic::semantic_search),
        )
        .route("/api/v1/notes/search/hybrid", post(semantic::hybrid_search))
        // Graph routes
        .route("/api/v1/graph", get(graph::get_full_graph))
        .route("/api/v1/graph/path", get(graph::get_path))
        .route("/api/v1/graph/{id}", get(graph::get_subgraph))
        .route("/api/v1/stats", get(graph::get_stats))
        // Agent routes
        .route(
            "/api/v1/agent/{agent_id}/memory",
            get(agent::list_memory).post(agent::store_memory),
        )
        .route(
            "/api/v1/agent/{agent_id}/memory/{namespace}/{key}",
            get(agent::get_memory),
        )
        .route(
            "/api/v1/agent/{agent_id}/memory/{namespace}/{key}/history",
            get(agent::get_memory_history),
        )
        .route(
            "/api/v1/agent/{agent_id}/tool-logs",
            get(agent::get_tool_logs).post(agent::log_tool_call),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Build the router with AI endpoints (requires inference backend)
pub fn create_ai_router(
    db: Arc<Database>,
    backend: Arc<AuditedInference>,
    inference_config: InferenceConfig,
    feature_gate: FeatureGate,
    auto_embedder: Option<Arc<AutoEmbedder>>,
) -> Router {
    let ai_state = AiAppState {
        db,
        backend,
        feature_gate,
        inference_config,
        auto_embedder,
    };

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(Any);

    Router::new()
        // AI status and models
        .route("/api/v1/ai/status", get(ai_routes::ai_status))
        .route("/api/v1/ai/models", get(ai_routes::list_models))
        // AI features
        .route("/api/v1/ai/query", post(ai_routes::ai_query))
        .route("/api/v1/ai/summarize", post(ai_routes::ai_summarize))
        .route("/api/v1/ai/tag/{note_id}", post(ai_routes::ai_tag))
        .route("/api/v1/ai/link/{note_id}", post(ai_routes::ai_link))
        .route("/api/v1/ai/embed/{note_id}", post(ai_routes::ai_embed))
        .route("/api/v1/ai/queue/stats", get(ai_routes::ai_queue_stats))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(ai_state)
}

async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "name": "smriti",
    }))
}

/// Start the API server on the given address.
///
/// When the `webui` feature is enabled the server mounts the embedded React SPA
/// and uses localhost-only CORS instead of the open `allow_origin(Any)` policy.
pub async fn start_server(
    db: Arc<Database>,
    host: &str,
    port: u16,
    db_path: String,
) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host, port);

    #[cfg(feature = "webui")]
    let app = crate::web::create_web_router(db, db_path);

    #[cfg(not(feature = "webui"))]
    let app = {
        let _ = db_path; // unused when webui is off
        create_router(db)
    };

    tracing::info!("Starting API server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
