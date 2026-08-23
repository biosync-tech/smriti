//! AI-Powered Features
//!
//! High-level AI features over the pluggable InferenceBackend trait
//! (Ollama by default, OpenAI-compatible, or optional local GGUF).
//!
//! Research references:
//! - Graph-Based Memory Survey arXiv:2602.05665 — hybrid search beats pure vector
//! - Graph-Native Belief Revision arXiv:2603.17244 — AGM conflict resolution
//! - MAGMA arXiv:2601.03236 — typed graph layers reduce token usage 95%
//! - Zep/Graphiti arXiv:2501.13956 — bi-temporal edges improve recall 18.5%

pub mod document_ingest;
pub mod embedder;
pub mod ingest;
pub mod linker;
pub mod rag;
pub mod summarizer;
pub mod tagger;

pub use document_ingest::{DocumentIngestor, IngestDocumentRequest, IngestDocumentResponse};
pub use embedder::AutoEmbedder;
pub use ingest::MultimodalIngestor;
pub use linker::AiLinker;
pub use rag::{RagEngine, RagQuery, RagResponse};
pub use summarizer::Summarizer;
pub use tagger::AutoTagger;
