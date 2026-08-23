use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::models::*;
use crate::parser;
use crate::web::response::{ok, WebError, WebResult};
use crate::web::AppState;

/// GET /api/v1/notes
pub async fn list_notes(
    State(state): State<AppState>,
    Query(query): Query<NoteListQuery>,
) -> WebResult<Vec<NoteSummary>> {
    let notes = state.db.list_notes(&query).map_err(WebError::from)?;
    Ok(ok(notes))
}

/// POST /api/v1/notes
pub async fn create_note(
    State(state): State<AppState>,
    Json(mut req): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<crate::web::response::ApiResponse<Note>>), WebError> {
    // Auto-extract inline tags
    for tag in parser::extract_tags(&req.content) {
        if !req.tags.contains(&tag) {
            req.tags.push(tag);
        }
    }
    let note = state.db.create_note(req).map_err(WebError::from)?;

    // Create links for any [[wiki-links]] in the content
    let wikilinks = parser::extract_wikilinks(&note.content);
    for wl in &wikilinks {
        if let Ok(Some(target)) = state.db.get_note_by_title(&wl.target) {
            let _ = state
                .db
                .create_link(&note.id, &target.id, LinkType::WikiLink);
        }
    }
    state.graph_cache.write().await.invalidate();
    Ok((StatusCode::CREATED, ok(note)))
}

/// GET /api/v1/notes/:id
pub async fn get_note(State(state): State<AppState>, Path(id): Path<String>) -> WebResult<Note> {
    let note = state.db.get_note(&id).map_err(WebError::from)?;
    Ok(ok(note))
}

/// PUT /api/v1/notes/:id
pub async fn update_note(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNoteRequest>,
) -> WebResult<Note> {
    let note = state.db.update_note(&id, req).map_err(WebError::from)?;
    state.graph_cache.write().await.invalidate();
    Ok(ok(note))
}

/// GET /api/v1/notes/:id/links
pub async fn get_links(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> WebResult<Vec<NoteSummary>> {
    let links = state.db.get_forward_links(&id).map_err(WebError::from)?;
    Ok(ok(links))
}
