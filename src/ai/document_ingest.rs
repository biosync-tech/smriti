//! Document Ingestion — text/markdown files → chunked notes in the KG
//!
//! Path A of the local-KG feature: splits plain text and markdown documents
//! into overlapping chunks, creates a parent "document" note and one child
//! note per chunk, and links every chunk back to the parent with a `ChunkOf`
//! edge.  No LLM is required — chunking is purely structural.
//!
//! Supported formats: .txt, .md  (PDF is Path B, pending a Rust PDF dep)
//!
//! Retrieval is handled by `retrieve_context` (see mcp/handlers.rs):
//! the caller embeds the query and passes the vector, Smriti does hybrid
//! FTS5+semantic search, BFS graph expansion, and returns assembled context.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, AppResult};
use crate::models::{CreateNoteRequest, LinkType};
use crate::storage::Database;

// ── Constants ────────────────────────────────────────────────────────────────

/// Default chunk size in characters (~300-400 tokens for most models).
const DEFAULT_CHUNK_CHARS: usize = 1_200;

/// Default overlap between consecutive chunks (helps with context continuity).
const DEFAULT_OVERLAP_CHARS: usize = 200;

/// Maximum content length for the parent document note preview.
const PARENT_PREVIEW_CHARS: usize = 600;

// ── Request / Response ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestDocumentRequest {
    /// Absolute or relative path to the file (.txt or .md)
    pub path: String,
    /// Extra tags to attach to every note created (document + chunks)
    #[serde(default)]
    pub tags: Vec<String>,
    /// Target chunk size in characters (default: 1200)
    pub chunk_size: Option<usize>,
    /// Overlap between chunks in characters (default: 200)
    pub chunk_overlap: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestDocumentResponse {
    /// The parent document note (title = filename, content = first N chars)
    pub document_note_id: String,
    /// All chunk note IDs created (ordered)
    pub chunk_note_ids: Vec<String>,
    /// Inferred title (filename without extension)
    pub title: String,
    /// How many chunks were created
    pub chunk_count: usize,
    /// Total characters ingested
    pub total_chars: usize,
    /// Any tags that were applied
    pub tags: Vec<String>,
}

// ── Chunker ──────────────────────────────────────────────────────────────────

/// Split `text` into overlapping chunks.
///
/// Strategy (in priority order):
///  1. Split on double-newlines (paragraph boundaries) — keeps semantic units.
///  2. If a paragraph block exceeds `chunk_size`, hard-split at the nearest
///     whitespace boundary before the limit.
///
/// Adjacent chunks share `overlap` characters from the end of the previous chunk.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }

    // Split into paragraph blocks (double newline is the boundary)
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for para in &paragraphs {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }

        // If adding this paragraph would exceed chunk_size, flush and start fresh
        if !current.is_empty() && current.len() + para.len() + 2 > chunk_size {
            chunks.push(current.trim().to_string());
            // Carry overlap from previous chunk
            let overlap_start =
                floor_char_boundary(&current, current.len().saturating_sub(overlap));
            current = current[overlap_start..].to_string();
            current.push('\n');
        }

        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(para);

        // If a single paragraph alone exceeds chunk_size, hard-split it
        while current.len() > chunk_size {
            let limit = floor_char_boundary(&current, chunk_size);
            let split_at = current[..limit]
                .rfind(|c: char| c.is_whitespace())
                .unwrap_or(limit);

            chunks.push(current[..split_at].trim().to_string());
            let overlap_start = floor_char_boundary(&current, split_at.saturating_sub(overlap));
            current = current[overlap_start..].to_string();
        }
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    // Remove any empty chunks that slipped through
    chunks.retain(|c| !c.is_empty());
    chunks
}

// ── DocumentIngestor ─────────────────────────────────────────────────────────

/// Stateless ingestion helper — all methods take `&Database` directly so they
/// can be called from both MCP handlers (`&Database`) and REST handlers
/// (`Arc<Database>` which auto-derefs).
pub struct DocumentIngestor;

impl DocumentIngestor {
    /// Ingest a text/markdown file into the knowledge graph.
    ///
    /// Creates:
    ///  - One parent "document" note
    ///  - N chunk notes (one per chunk)
    ///  - N `ChunkOf` links (chunk → document)
    pub fn ingest(db: &Database, req: &IngestDocumentRequest) -> AppResult<IngestDocumentResponse> {
        let path = Path::new(&req.path);

        // ── Validate ──────────────────────────────────────────────────────
        if !path.exists() {
            return Err(AppError::BadRequest(format!(
                "File not found: {}",
                req.path
            )));
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !matches!(ext.as_str(), "txt" | "md" | "") {
            return Err(AppError::BadRequest(format!(
                "Unsupported file type '.{}'. Path A supports .txt and .md (PDF is Path B).",
                ext
            )));
        }

        // ── Read ──────────────────────────────────────────────────────────
        let raw = std::fs::read_to_string(path)
            .map_err(|e| AppError::BadRequest(format!("Cannot read file: {}", e)))?;

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled Document")
            .to_string();

        let total_chars = raw.len();

        // ── Tags ──────────────────────────────────────────────────────────
        // Base tags: user-supplied + a "source" tag encoding the filename
        let source_tag = format!(
            "source:{}",
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
        );
        let mut base_tags = req.tags.clone();
        if !base_tags.contains(&source_tag) {
            base_tags.push(source_tag.clone());
        }
        if !base_tags.contains(&"document".to_string()) {
            base_tags.push("document".to_string());
        }

        // ── Parent note ───────────────────────────────────────────────────
        let preview = safe_truncate(&raw, PARENT_PREVIEW_CHARS);
        let parent_content = format!(
            "**Source:** `{}`\n\n**Size:** {} characters\n\n---\n\n{}",
            req.path, total_chars, preview
        );

        let chunk_size = req.chunk_size.unwrap_or(DEFAULT_CHUNK_CHARS);
        let chunk_overlap = req.chunk_overlap.unwrap_or(DEFAULT_OVERLAP_CHARS);
        let chunks = chunk_text(&raw, chunk_size, chunk_overlap);
        let total_chunks = chunks.len();

        // One SQLite transaction: parent + chunks + ChunkOf links, or nothing.
        let (parent_id, chunk_note_ids) = db.execute(|conn| {
            let tx = conn.unchecked_transaction().map_err(AppError::Database)?;

            let parent_note = crate::storage::operations::insert_note_with_tags(
                &tx,
                CreateNoteRequest {
                    title: title.clone(),
                    content: parent_content,
                    tags: base_tags.clone(),
                },
            )?;

            let mut chunk_note_ids: Vec<String> = Vec::with_capacity(total_chunks);
            for (i, chunk_content) in chunks.iter().enumerate() {
                let chunk_num = i + 1;
                let chunk_title = format!("{} — Chunk {}/{}", title, chunk_num, total_chunks);
                let mut chunk_tags = base_tags.clone();
                chunk_tags.retain(|t| t != "document");
                chunk_tags.push("chunk".to_string());
                let content_with_meta = format!(
                    "> Source: `{}` | Chunk {}/{}\n\n{}",
                    req.path, chunk_num, total_chunks, chunk_content
                );

                let chunk_note = crate::storage::operations::insert_note_with_tags(
                    &tx,
                    CreateNoteRequest {
                        title: chunk_title,
                        content: content_with_meta,
                        tags: chunk_tags,
                    },
                )?;
                crate::storage::operations::insert_link_on_conn(
                    &tx,
                    &chunk_note.id,
                    &parent_note.id,
                    LinkType::ChunkOf,
                )
                .map_err(|e| {
                    AppError::BadRequest(format!("Failed to create ChunkOf link: {}", e))
                })?;
                chunk_note_ids.push(chunk_note.id);
            }

            tx.commit().map_err(AppError::Database)?;
            Ok((parent_note.id, chunk_note_ids))
        })?;

        Ok(IngestDocumentResponse {
            document_note_id: parent_id,
            chunk_note_ids,
            title,
            chunk_count: total_chunks,
            total_chars,
            tags: base_tags,
        })
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Truncate `s` to at most `max_chars`, breaking on a whitespace boundary.
fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn safe_truncate(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    let limit = floor_char_boundary(s, max_chars);
    match s[..limit].rfind(|c: char| c.is_whitespace()) {
        Some(pos) => &s[..pos],
        None => &s[..limit],
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_text_empty() {
        assert!(chunk_text("", 1200, 200).is_empty());
    }

    #[test]
    fn chunk_text_small_fits_in_one() {
        let text = "Hello world.\n\nThis is a short document.";
        let chunks = chunk_text(text, 1200, 200);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].contains("Hello world"));
    }

    #[test]
    fn chunk_text_respects_size() {
        // Generate text that definitely exceeds one chunk
        let text = (0..100)
            .map(|i| {
                format!(
                    "Paragraph {}. Some content about aging and senescence here.",
                    i
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let chunks = chunk_text(&text, 500, 50);
        assert!(chunks.len() > 1, "Should produce multiple chunks");
        for chunk in &chunks {
            // Each chunk should be <= chunk_size + a small tolerance for the overlap prefix
            assert!(chunk.len() <= 600, "Chunk too large: {}", chunk.len());
        }
    }

    #[test]
    fn chunk_text_overlap_present() {
        let text = (0..50)
            .map(|i| format!("Sentence {} with some unique marker_{} words.", i, i))
            .collect::<Vec<_>>()
            .join("\n\n");
        let chunks = chunk_text(&text, 300, 80);
        if chunks.len() >= 2 {
            // The end of chunk N should appear somewhere in chunk N+1
            let tail_of_first = &chunks[0][chunks[0].len().saturating_sub(50)..];
            assert!(
                chunks[1].contains(tail_of_first.trim()),
                "Overlap missing between chunk 0 and chunk 1"
            );
        }
    }

    #[test]
    fn chunk_text_does_not_panic_on_multibyte_boundary() {
        // 3-byte chars; a byte limit that lands mid-character must not panic.
        let text = "中".repeat(80);
        let chunks = chunk_text(&text, 50, 10);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.is_char_boundary(chunk.len()));
            let _ = chunk.chars().count();
        }
    }
}
