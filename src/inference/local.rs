//! Local Gemma 4 backend via llama-gguf
//!
//! This embeds the inference engine directly into the Smriti binary.
//! When the `gemma` feature is not enabled, this module provides a
//! stub that returns InferenceError::ModelNotLoaded.
//!
//! Architecture decision: We use llama-gguf because it's pure Rust,
//! supports GGUF quantized models, and provides GPU acceleration
//! via CUDA, Metal, Vulkan, and DX12 — all without external processes.

use async_trait::async_trait;

use super::*;
use super::config::InferenceConfig;

/// Local inference backend using llama-gguf
///
/// When compiled with `--features gemma`, this loads a GGUF model
/// and runs inference in-process. Without the feature, all methods
/// return appropriate errors directing the user to enable the feature
/// or use an alternative backend.
pub struct LocalGemmaBackend {
    config: InferenceConfig,
    model_path: std::path::PathBuf,
    // When llama-gguf is integrated:
    // engine: Option<llama_gguf::Model>,
    ready: bool,
}

impl LocalGemmaBackend {
    pub async fn new(config: &InferenceConfig) -> Result<Self, InferenceError> {
        let model_path = config.model_path();

        let ready = model_path.exists();

        if !ready {
            tracing::warn!(
                "Model not found at {:?}. Run `smriti ai setup` to download, \
                 or use --backend ollama/openai as fallback.",
                model_path
            );
        }

        Ok(Self {
            config: config.clone(),
            model_path,
            ready,
        })
    }

    /// Load the model into memory (called lazily on first inference)
    async fn ensure_loaded(&self) -> Result<(), InferenceError> {
        if !self.ready {
            return Err(InferenceError::ModelNotLoaded(format!(
                "Gemma 4 model not found at {:?}. \
                 Download with: smriti ai setup --model {} --quantization {}",
                self.model_path, self.config.model, self.config.quantization
            )));
        }

        // TODO: When llama-gguf is integrated, load model here
        // This is where the actual GGUF loading happens:
        //
        // let mut params = llama_gguf::ModelParams::default();
        // params.n_gpu_layers = self.config.gpu_layers;
        // params.n_threads = self.config.threads as u32;
        // let model = llama_gguf::Model::load(&self.model_path, params)?;
        // self.engine = Some(model);

        Ok(())
    }
}

#[async_trait]
impl InferenceBackend for LocalGemmaBackend {
    async fn generate(&self, _request: &GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        self.ensure_loaded().await?;

        // TODO: Replace with actual llama-gguf inference when integrated
        //
        // Pseudocode for the real implementation:
        // let engine = self.engine.as_ref().unwrap();
        // let mut ctx = engine.create_context(self.config.context_length)?;
        //
        // // Tokenize
        // let tokens = engine.tokenize(&request.prompt)?;
        //
        // // Check context length
        // if tokens.len() > self.config.context_length {
        //     return Err(InferenceError::ContextLengthExceeded {
        //         used: tokens.len(),
        //         max: self.config.context_length,
        //     });
        // }
        //
        // // Run inference
        // let params = llama_gguf::SamplingParams {
        //     temperature: request.temperature.unwrap_or(0.7),
        //     top_p: request.top_p.unwrap_or(0.9),
        //     max_tokens: request.max_tokens.unwrap_or(2048),
        //     stop: request.stop.clone().unwrap_or_default(),
        // };
        //
        // let output = ctx.generate(&tokens, &params)?;
        // let text = engine.detokenize(&output)?;

        Err(InferenceError::ModelNotLoaded(
            "Local backend not yet compiled with llama-gguf. \
             Add `llama-gguf = \"0.14\"` to Cargo.toml and enable the `gemma` feature. \
             For now, use --backend ollama or --backend openai."
                .into(),
        ))
    }

    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        self.ensure_loaded().await?;

        // TODO: Replace with actual embedding generation
        //
        // Pseudocode:
        // let engine = self.engine.as_ref().unwrap();
        // let mut embeddings = Vec::with_capacity(texts.len());
        // for text in texts {
        //     let tokens = engine.tokenize(text)?;
        //     let embedding = engine.embed(&tokens)?;
        //     embeddings.push(embedding);
        // }
        // Ok(embeddings)

        Err(InferenceError::ModelNotLoaded(
            "Local embedding not yet available. Use --backend ollama or --backend openai."
                .into(),
        ))
    }

    async fn describe_image(
        &self,
        _image_bytes: &[u8],
        _prompt: &str,
    ) -> Result<String, InferenceError> {
        self.ensure_loaded().await?;

        // Gemma 4 31B supports vision — this will work when llama-gguf is integrated
        // with multimodal support
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
        "local-gemma"
    }

    async fn health_check(&self) -> Result<BackendStatus, InferenceError> {
        Ok(BackendStatus {
            ready: self.ready,
            backend_name: "local-gemma".into(),
            model_loaded: if self.ready {
                Some(self.config.model.clone())
            } else {
                None
            },
            gpu_available: false, // Will be detected by llama-gguf
            vram_used_mb: None,
            vram_total_mb: None,
        })
    }
}
