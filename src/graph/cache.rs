use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::errors::AppResult;
use crate::models::{NoteListQuery, SortOrder};
use crate::storage::Database;

use super::KnowledgeGraph;

/// Cached in-memory knowledge graph that avoids full rebuilds on every query.
/// Wrap in `Arc<RwLock<GraphCache>>` and store in AppState.
///
/// Design ref: MAGMA arXiv:2601.03236 §3.2 dual-stream pattern.
pub struct GraphCache {
    graph: KnowledgeGraph,
    dirty: bool,
    built_at: Option<DateTime<Utc>>,
}

impl Default for GraphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphCache {
    /// Create a new cache in the dirty state so the first query triggers a build.
    pub fn new() -> Self {
        Self {
            graph: KnowledgeGraph::new(),
            dirty: true,
            built_at: None,
        }
    }

    /// Mark the cache as stale. The next `ensure_fresh` call will rebuild.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Rebuild the graph from the database if the cache is dirty.
    /// Returns a shared reference to the inner `KnowledgeGraph`.
    pub fn ensure_fresh(&mut self, db: &Database) -> AppResult<&KnowledgeGraph> {
        if self.dirty {
            self.rebuild(db)?;
        }
        Ok(&self.graph)
    }

    /// When the graph was last successfully built, or `None` if never built.
    pub fn built_at(&self) -> Option<DateTime<Utc>> {
        self.built_at
    }

    /// Force-rebuild the graph from the links + notes tables.
    fn rebuild(&mut self, db: &Database) -> AppResult<()> {
        let links = db.get_all_links()?;

        // ASSUMES: NoteListQuery, SortOrder are the query types from src/models/note.rs
        let notes = db.list_notes(&NoteListQuery {
            limit: 10000,
            offset: 0,
            sort: SortOrder::UpdatedDesc,
            tag: None,
        })?;

        let mut titles: HashMap<String, String> = HashMap::with_capacity(notes.len());
        let mut tag_counts: HashMap<String, usize> = HashMap::with_capacity(notes.len());
        for note in &notes {
            titles.insert(note.id.clone(), note.title.clone());
            tag_counts.insert(note.id.clone(), note.tag_count);
        }

        self.graph = KnowledgeGraph::from_links(&links, &titles, &tag_counts);
        self.dirty = false;
        self.built_at = Some(Utc::now());

        Ok(())
    }
}

/// Convenience alias used in `AppState`.
pub type SharedGraphCache = Arc<RwLock<GraphCache>>;
