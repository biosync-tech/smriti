//! AI Inference Layer — pluggable backends for LLM inference
//!
//! Feature code talks only to [`InferenceBackend`]. Swap the transport and
//! model names in config / `SMRITI_*` env — never in Rust.
//!
//! Transports ([`BackendKind`]):
//! - [`ollama::OllamaBackend`] — any Ollama tag
//! - [`openai::OpenAICompatibleBackend`] — OpenAI, LM Studio, vLLM, Groq, …
//! - [`local::LocalGgufBackend`] — optional in-process GGUF (`gguf` feature)
//!
//! Research ref: Graph-Based Memory Survey arXiv:2602.05665 — hybrid beats pure vector

pub mod config;
pub mod local;
pub mod manager;
pub mod ollama;
pub mod openai;
pub mod queue;

#[cfg(test)]
pub mod mock;

pub mod audited;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Re-exports
pub use audited::{AuditedInference, CallContext};
pub use config::{BackendKind, InferenceConfig};
pub use local::LocalGgufBackend;
pub use manager::ModelManager;

/// Core inference backend trait — all providers implement this
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Generate text from a prompt (completion/chat)
    async fn generate(&self, request: &GenerateRequest)
        -> Result<GenerateResponse, InferenceError>;

    /// Generate embeddings for one or more texts
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError>;

    /// Describe an image (multimodal — only backends that implement vision)
    async fn describe_image(
        &self,
        image_bytes: &[u8],
        prompt: &str,
    ) -> Result<String, InferenceError>;

    /// Check what this backend can do
    fn capabilities(&self) -> BackendCapabilities;

    /// Human-readable backend name
    fn name(&self) -> &str;

    /// Check if the backend is ready (model loaded, server reachable, etc.)
    async fn health_check(&self) -> Result<BackendStatus, InferenceError>;
}

/// What the user wants the model to do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// The prompt or messages
    pub prompt: String,
    /// System instruction
    pub system: Option<String>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 = deterministic, 1.0 = creative)
    pub temperature: Option<f32>,
    /// Top-p nucleus sampling
    pub top_p: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// Whether to enable thinking/reasoning mode
    pub thinking: Option<bool>,
}

impl Default for GenerateRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            system: None,
            max_tokens: Some(2048),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop: None,
            thinking: None,
        }
    }
}

/// Model's response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub text: String,
    pub tokens_used: Option<TokenUsage>,
    pub model: String,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// What a backend can and cannot do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCapabilities {
    pub text_generation: bool,
    pub embeddings: bool,
    pub vision: bool,
    pub audio: bool,
    pub function_calling: bool,
    pub max_context_length: usize,
    pub model_name: String,
}

/// Backend health/status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendStatus {
    pub ready: bool,
    pub backend_name: String,
    pub model_loaded: Option<String>,
    pub gpu_available: bool,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
}

/// Errors from inference operations
#[derive(thiserror::Error, Debug)]
pub enum InferenceError {
    #[error("Model not loaded: {0}")]
    ModelNotLoaded(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    #[error("Embedding failed: {0}")]
    EmbeddingFailed(String),

    #[error("Vision not supported by this backend")]
    VisionNotSupported,

    #[error("Context length exceeded: {used} > {max}")]
    ContextLengthExceeded { used: usize, max: usize },

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Feature not licensed: {0}")]
    NotLicensed(String),
}

/// Type alias for a shared inference backend
pub type SharedBackend = Arc<dyn InferenceBackend>;

/// Create the appropriate backend based on configuration.
/// Model names stay on the config object; this only picks a transport.
pub async fn create_backend(config: &InferenceConfig) -> Result<SharedBackend, InferenceError> {
    match config.kind().map_err(InferenceError::ConfigError)? {
        BackendKind::Local => {
            tracing::info!(
                "Initializing optional local GGUF backend (model={})",
                config.chat_model()
            );
            let backend = local::LocalGgufBackend::new(config).await?;
            Ok(Arc::new(backend))
        }
        BackendKind::Ollama => {
            tracing::info!(
                "Connecting to Ollama at {} (chat={}, embed={})",
                config.ollama.host,
                config.chat_model(),
                config.embed_model()
            );
            let backend = ollama::OllamaBackend::new(config)?;
            Ok(Arc::new(backend))
        }
        BackendKind::OpenAiCompatible => {
            tracing::info!(
                "Connecting to OpenAI-compatible API at {} (chat={}, embed={})",
                config.openai.api_url,
                config.chat_model(),
                config.embed_model()
            );
            let backend = openai::OpenAICompatibleBackend::new(config)?;
            Ok(Arc::new(backend))
        }
    }
}
