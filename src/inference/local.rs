//! Optional in-process GGUF backend (any llama.cpp-compatible model).
//!
//! When the `gguf` feature (legacy alias: `gemma`) is not enabled, this
//! module is a stub that returns `InferenceError::ModelNotLoaded`.
//!
//! llama-gguf is pure Rust and can use CUDA, Metal, Vulkan, or DX12
//! without a second process. The model file is whatever GGUF you point
//! `inference.model` + `quantization` at.

use async_trait::async_trait;

use super::*;
use super::config::InferenceConfig;

/// In-process GGUF backend. Model identity comes from config, not this type.
pub struct LocalGgufBackend {
    config: InferenceConfig,
    model_path: std::path::PathBuf,
    ready: bool,
}

/// Deprecated name kept so older call sites still compile.
pub type LocalGemmaBackend = LocalGgufBackend;

impl LocalGgufBackend {
    pub async fn new(config: &InferenceConfig) -> Result<Self, InferenceError> {
        let model_path = config.model_path();

        let ready = model_path.exists();

        if !ready {
            tracing::warn!(
                "GGUF not found at {:?}. Place a model file there, or use \
                 backend = \"ollama\" / \"openai\" with any model those serve.",
                model_path
            );
        }

        Ok(Self {
            config: config.clone(),
            model_path,
            ready,
        })
    }

    async fn ensure_loaded(&self) -> Result<(), InferenceError> {
        if !self.ready {
            return Err(InferenceError::ModelNotLoaded(format!(
                "GGUF not found at {:?}. Set inference.model / quantization, \
                 or switch backend to ollama or openai.",
                self.model_path
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl InferenceBackend for LocalGgufBackend {
    async fn generate(&self, _request: &GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        self.ensure_loaded().await?;

        Err(InferenceError::ModelNotLoaded(
            "Local GGUF backend is not compiled in. Enable the `gguf` feature \
             (or legacy `gemma`) and add llama-gguf, or use backend = \"ollama\" \
             / \"openai\" with whatever model you already run."
                .into(),
        ))
    }

    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        self.ensure_loaded().await?;

        Err(InferenceError::ModelNotLoaded(
            "Local GGUF embeddings are not compiled in. Use an Ollama or \
             OpenAI-compatible embed model instead."
                .into(),
        ))
    }

    async fn describe_image(
        &self,
        _image_bytes: &[u8],
        _prompt: &str,
    ) -> Result<String, InferenceError> {
        self.ensure_loaded().await?;
        Err(InferenceError::VisionNotSupported)
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            text_generation: true,
            embeddings: true,
            vision: false,
            audio: false,
            function_calling: false,
            max_context_length: self.config.context_length,
            model_name: self.config.model.clone(),
        }
    }

    fn name(&self) -> &str {
        "local-gguf"
    }

    async fn health_check(&self) -> Result<BackendStatus, InferenceError> {
        Ok(BackendStatus {
            ready: self.ready,
            backend_name: "local-gguf".into(),
            model_loaded: if self.ready {
                Some(self.config.model.clone())
            } else {
                None
            },
            gpu_available: false,
            vram_used_mb: None,
            vram_total_mb: None,
        })
    }
}
