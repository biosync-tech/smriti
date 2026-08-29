use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::api::server::AppState;
use crate::errors::AppError;
use crate::features::consolidation::{
    log_access, run_consolidation_pass, AccessKind, ConsolidationPolicy, ScoreWeights, Thresholds,
};
use crate::features::schema_formation::{
    commit_proposal, explain_schema, list_pending_proposals, reject_proposal,
    resolve_pending_proposal, GatingSignal,
};

#[derive(Debug, Deserialize)]
pub struct RunQuery {
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub policy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectBody {
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AccessBody {
    #[serde(default)]
    pub query_context: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

/// POST /api/v1/consolidation/run
///
/// Accepts `?policy=&dry_run=` and/or a JSON body with the same fields.
pub async fn run(
    State(state): State<AppState>,
    Query(query): Query<RunQuery>,
    body: Option<Json<RunQuery>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let q = body.map(|Json(b)| b).unwrap_or(query);
    let dry_run = q.dry_run.unwrap_or(true);
    let policy = match q.policy.as_deref() {
        Some("standard") => ConsolidationPolicy::Standard,
        Some("aggressive") => ConsolidationPolicy::Aggressive,
        _ => ConsolidationPolicy::Conservative,
    };
    let report = state.db.execute(|conn| {
        run_consolidation_pass(
            conn,
            policy,
            dry_run,
            ScoreWeights::default(),
            Thresholds::default(),
            None,
        )
    })?;
    Ok(Json(serde_json::to_value(report)?))
}

/// GET /api/v1/consolidation/events
pub async fn events(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let rows = state.db.execute(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, note_id, event_type, score_before, score_after, reason, created_at
             FROM consolidation_events
             ORDER BY created_at DESC
             LIMIT 200",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, String>(0)?,
                "note_id": r.get::<_, String>(1)?,
                "event_type": r.get::<_, String>(2)?,
                "score_before": r.get::<_, Option<f64>>(3)?,
                "score_after": r.get::<_, Option<f64>>(4)?,
                "reason": r.get::<_, String>(5)?,
                "created_at": r.get::<_, String>(6)?,
            }))
        })?;
        let mut out = Vec::new();
        for row in mapped {
            out.push(row?);
        }
        Ok(out)
    })?;
    Ok(Json(serde_json::json!({ "events": rows })))
}

/// GET /api/v1/consolidation/proposals
pub async fn proposals(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let pending = state.db.execute(list_pending_proposals)?;
    Ok(Json(serde_json::to_value(pending)?))
}

/// POST /api/v1/consolidation/proposals/:id/accept
pub async fn accept_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let formed = state.db.execute(|conn| {
        let proposal = resolve_pending_proposal(conn, &id)?;
        commit_proposal(
            conn,
            &proposal,
            &GatingSignal::HumanApproved { by: "api".into() },
        )
    })?;
    state.graph_cache.write().await.invalidate();
    Ok(Json(serde_json::to_value(formed)?))
}

/// POST /api/v1/consolidation/proposals/:id/reject
pub async fn reject_proposal_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<RejectBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let by = body.by.unwrap_or_else(|| "api".into());
    let reason = body.reason.unwrap_or_else(|| "rejected via API".into());
    state.db.execute(|conn| {
        let proposal = resolve_pending_proposal(conn, &id)?;
        reject_proposal(conn, &proposal, &by, &reason)
    })?;
    Ok(Json(serde_json::json!({
        "rejected": id,
        "by": by,
        "reason": reason,
    })))
}

/// GET /api/v1/notes/:id/lineage
pub async fn lineage(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let explain = state.db.execute(|conn| explain_schema(conn, &id))?;
    Ok(Json(serde_json::to_value(explain)?))
}

/// POST /api/v1/notes/:id/access
pub async fn record_access(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AccessBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let kind = match body.kind.as_deref() {
        Some("search_hit") => AccessKind::SearchHit,
        Some("graph_traverse") => AccessKind::GraphTraverse,
        Some("mcp_retrieve") => AccessKind::McpRetrieve,
        _ => AccessKind::Read,
    };
    state.db.execute(|conn| {
        log_access(
            conn,
            &id,
            kind,
            body.query_context.as_deref(),
            body.agent_id.as_deref(),
        )
    })?;
    Ok(Json(serde_json::json!({ "ok": true, "note_id": id })))
}
