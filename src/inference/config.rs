//! Configuration for the inference layer

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Which backend to use: "local", "ollama", or "openai"
    #[serde(default = "default_backend")]
    pub backend: String,

    /// Model identifier
    #[serde(default = "default_model")]
    pub model: String,

    /// Quantization level for local backend
    #[serde(default = "default_quantization")]
    pub quantization: String,

    /// Number of GPU layers (-1 = auto, 0 = CPU only)
    #[serde(default = "default_gpu_layers")]
    pub gpu_layers: i32,

    /// Context length in tokens
    #[serde(default = "default_context_length")]
    pub context_length: usize,

    /// Directory to store downloaded models
    #[serde(default = "default_models_dir")]
    pub models_dir: PathBuf,

    /// Number of threads for CPU inference
    #[serde(default = "default_threads")]
    pub threads: usize,

    /// Ollama-specific configuration
    #[serde(default)]
    pub ollama: OllamaConfig,

    /// OpenAI-compatible API configuration
    #[serde(default)]
    pub openai: OpenAIConfig,

    /// Embedding-specific settings
    #[serde(default)]
    pub embedding: EmbeddingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_host")]
    pub host: String,
    #[serde(default = "default_ollama_model")]
    pub model: String,
    #[serde(default = "default_ollama_embed_model")]
    pub embed_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    #[serde(default = "default_openai_url")]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_openai_embed_model")]
    pub embed_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Auto-embed notes on create/update
    #[serde(default = "default_true")]
    pub auto_embed: bool,
    /// Batch size for embedding queue
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Embedding dimensions (must match sqlite-vec table)
    #[serde(default = "default_dimensions")]
    pub dimensions: usize,
}

// --- Defaults ---

fn default_backend() -> String {
    "ollama".into()
}

fn default_model() -> String {
    "llama3.2".into()
}

fn default_quantization() -> String {
    "Q4_K_M".into()
}

fn default_gpu_layers() -> i32 {
    -1
}

fn default_context_length() -> usize {
    8192
}

fn default_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn default_models_dir() -> PathBuf {
    dirs_next_or_fallback()
}

fn default_ollama_host() -> String {
    "http://localhost:11434".into()
}

fn default_ollama_model() -> String {
    "llama3.2".into()
}

fn default_ollama_embed_model() -> String {
    "all-minilm".into()
}

fn default_openai_url() -> String {
    "http://localhost:8080/v1".into()
}

fn default_openai_embed_model() -> String {
    "all-minilm".into()
}

fn default_true() -> bool {
    true
}

fn default_batch_size() -> usize {
    32
}

fn default_dimensions() -> usize {
    384
}

fn dirs_next_or_fallback() -> PathBuf {
    // Try XDG data dir, fallback to ~/.local/share/smriti/models
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/share/smriti/models")
    } else {
        PathBuf::from("./models")
    }
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            model: default_model(),
            quantization: default_quantization(),
            gpu_layers: default_gpu_layers(),
            context_length: default_context_length(),
            models_dir: default_models_dir(),
            threads: default_threads(),
            ollama: OllamaConfig::default(),
            openai: OpenAIConfig::default(),
            embedding: EmbeddingConfig::default(),
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: default_ollama_host(),
            model: default_ollama_model(),
            embed_model: default_ollama_embed_model(),
        }
    }
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_url: default_openai_url(),
            api_key: String::new(),
            model: default_model(),
            embed_model: default_openai_embed_model(),
        }
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            auto_embed: default_true(),
            batch_size: default_batch_size(),
            dimensions: default_dimensions(),
        }
    }
}

impl InferenceConfig {
    /// Load config from a TOML file, falling back to defaults
    pub fn load(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                // Try to parse the [inference] section
                if let Ok(val) = toml::from_str::<toml::Value>(&content) {
                    if let Some(inference) = val.get("inference") {
                        if let Ok(cfg) = inference.clone().try_into() {
                            return cfg;
                        }
                    }
                }
                tracing::warn!(
                    "Could not parse inference config from {:?}, using defaults",
                    path
                );
                Self::default()
            }
            Err(_) => Self::default(),
        }
    }

    /// Get the expected model file path
    pub fn model_path(&self) -> PathBuf {
        let filename = format!("{}-{}.gguf", self.model, self.quantization);
        self.models_dir.join(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_backend_is_ollama_not_gemma() {
        let cfg = InferenceConfig::default();
        assert_eq!(cfg.backend, "ollama");
        assert!(
            !cfg.model.to_lowercase().contains("gemma"),
            "chat model should not default to Gemma, got {}",
            cfg.model
        );
        assert!(
            !cfg.ollama.model.to_lowercase().contains("gemma"),
            "Ollama chat model should not default to Gemma, got {}",
            cfg.ollama.model
        );
        assert!(
            !cfg.ollama.embed_model.to_lowercase().contains("gemma"),
            "Ollama embed model should not default to Gemma, got {}",
            cfg.ollama.embed_model
        );
        assert!(
            !cfg.openai.embed_model.to_lowercase().contains("gemma"),
            "OpenAI embed model should not default to Gemma, got {}",
            cfg.openai.embed_model
        );
        assert_eq!(cfg.ollama.embed_model, "all-minilm");
        assert_eq!(cfg.embedding.dimensions, 384);
    }
}
