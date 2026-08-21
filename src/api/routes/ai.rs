//! AI-powered API endpoints
//!
//! All AI features are gated by the feature licensing system.
//! Core tier gets: ai_status, ai_embed
//! Pro tier gets: ai_query, ai_summarize, ai_tag, ai_link, ai_ingest

use axum::extract::{Json, Path, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::ai;
use crate::api::server::AiAppState;
use crate::errors::AppError;
use crate::inference::{self, InferenceBackend};
use crate::licensing::SmritiFeature;

/// Status information for the AI system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatus {
    pub status: String,
    pub capabilities: serde_json::Value,
    pub tier: String,
    pub available_features: Vec<String>,
}

/// GET /api/v1/ai/status — Check inference engine status
pub async fn ai_status(
    State(state): State<AiAppState>,
) -> Result<impl IntoResponse, AppError> {
    let status = state
        .backend
        .health_check()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let capabilities = state.backend.capabilities();

    let available_features: Vec<String> = state
        .feature_gate
        .available_features()
        .iter()
        .map(|f| format!("{:?}", f))
        .collect();

    let response = AiStatus {
        status: format!("{:?}", status),
        capabilities: serde_json::to_value(&capabilities)
            .unwrap_or(serde_json::Value::Null),
        tier: format!("{:?}", state.feature_gate.active_tier()),
        available_features,
    };

    Ok(Json(response))
}

/// GET /api/v1/ai/models — List available models
pub async fn list_models(
    State(state): State<AiAppState>,
) -> Result<impl IntoResponse, AppError> {
    let manager = inference::ModelManager::new(&state.inference_config)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let models = manager.list_models();

    Ok(Json(serde_json::json!({
        "models": models,
        "default": state.inference_config.model,
    })))
}

/// RAG query request (wraps the inner RagQuery)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryRequest {
    #[serde(flatten)]
    pub query: ai::rag::RagQuery,
}

/// POST /api/v1/ai/query — RAG query over knowledge graph
pub async fn ai_query(
    State(state): State<AiAppState>,
    Json(req): Json<RagQueryRequest>,
) -> Result<impl IntoResponse, AppError> {
    state
        .feature_gate
        .require(SmritiFeature::RagQuery)
        .map_err(AppError::BadRequest)?;

    let config = req
        .query
        .config
        .clone()
        .unwrap_or_default();
    let engine = ai::rag::RagEngine::new(state.db.clone(), state.backend.clone(), config);

    let response = engine
        .query(&req.query)
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(Json(serde_json::to_value(&response).unwrap_or(serde_json::Value::Null)))
}

/// POST /api/v1/ai/summarize — Summarize notes
pub async fn ai_summarize(
    State(state): State<AiAppState>,
    Json(request): Json<ai::summarizer::SummarizeRequest>,
) -> Result<impl IntoResponse, AppError> {
    state
        .feature_gate
        .require(SmritiFeature::AiSummarization)
        .map_err(AppError::BadRequest)?;

    let summarizer = ai::summarizer::Summarizer::new(state.db.clone(), state.backend.clone());

    let response = summarizer
        .summarize(&request)
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(Json(serde_json::to_value(&response).unwrap_or(serde_json::Value::Null)))
}

/// POST /api/v1/ai/tag/{note_id} — Auto-suggest tags for a note
pub async fn ai_tag(
    State(state): State<AiAppState>,
    Path(note_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state
        .feature_gate
        .require(SmritiFeature::AiAutoTagging)
        .map_err(AppError::BadRequest)?;

    let tagger = ai::tagger::AutoTagger::new(state.db.clone(), state.backend.clone());

    let response = tagger
        .suggest_tags(&note_id, 10)
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(Json(serde_json::to_value(&response).unwrap_or(serde_json::Value::Null)))
}

/// POST /api/v1/ai/link/{note_id} — Find semantic connections
pub async fn ai_link(
    State(state): State<AiAppState>,
    Path(note_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state
        .feature_gate
        .require(SmritiFeature::AiSmartLinking)
        .map_err(AppError::BadRequest)?;

    let linker = ai::linker::AiLinker::new(state.db.clone(), state.backend.clone());

    let suggestions = linker
        .find_semantic_links(&note_id, 10, 0.3)
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(Json(serde_json::to_value(&suggestions).unwrap_or(serde_json::Value::Null)))
}

/// Embedding storage response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingStored {
    pub note_id: String,
    pub dimensions: usize,
    pub model: String,
    pub stored: bool,
}

/// POST /api/v1/ai/embed/{note_id} — Generate embedding for a note
pub async fn ai_embed(
    State(state): State<AiAppState>,
    Path(note_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let note = state.db.get_note(&note_id)?;
    let text = format!("{}\n\n{}", note.title, note.content);

    let embeddings = state
        .backend
        .embed(&[text])
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let embedding = embeddings
        .into_iter()
        .next()
        .ok_or_else(|| AppError::BadRequest("No embedding returned".into()))?;

    let dims = embedding.len();
    let backend_name = state.backend.name().to_string();

    // Store in DB
    state
        .db
        .store_embedding(&note_id, &embedding, Some(&backend_name))?;

    let response = EmbeddingStored {
        note_id,
        dimensions: dims,
        model: backend_name,
        stored: true,
    };

    Ok(Json(response))
}

/// Queue statistics response (unified type for both enabled/disabled states)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatsResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// GET /api/v1/ai/queue/stats — Get embedding queue stats
pub async fn ai_queue_stats(
    State(state): State<AiAppState>,
) -> Result<Json<QueueStatsResponse>, AppError> {
    if let Some(ref embedder) = state.auto_embedder {
        let stats = embedder.stats().await;
        Ok(Json(QueueStatsResponse {
            status: "active".to_string(),
            pending: Some(stats.pending),
            processed: Some(stats.processed),
            failed: Some(stats.failed),
            last_error: stats.last_error,
        }))
    } else {
        Ok(Json(QueueStatsResponse {
            status: "disabled".to_string(),
            pending: None,
            processed: None,
            failed: None,
            last_error: None,
        }))
    }
}
