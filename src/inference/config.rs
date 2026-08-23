//! Configuration for the inference layer.
//!
//! Model names are opaque plugin strings. Smriti never compiles in a
//! required chat or embedding model. Defaults (`llama3.2`, `all-minilm`)
//! are starters you replace via TOML or `SMRITI_*` env vars.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Transport used to reach a model. The model *name* is never part of this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// Local Ollama HTTP API — any tag Ollama can pull.
    Ollama,
    /// Any OpenAI-compatible HTTP API (OpenAI, LM Studio, vLLM, Groq,
    /// Together, llama.cpp server, OpenRouter, …).
    OpenAiCompatible,
    /// Optional in-process GGUF (feature `gguf`; `gemma` is a deprecated alias).
    Local,
}

impl BackendKind {
    /// Parse a user-supplied backend id. Aliases exist so people can plug
    /// in whatever they already run without renaming it to our enum.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ollama" => Ok(Self::Ollama),
            "openai" | "openai-compatible" | "openai_compatible" | "custom" | "lmstudio"
            | "vllm" | "groq" | "together" | "openrouter" | "llamacpp" | "llama.cpp" => {
                Ok(Self::OpenAiCompatible)
            }
            "local" | "gguf" | "local-gguf" => Ok(Self::Local),
            other => Err(format!(
                "Unknown backend '{other}'. Use ollama, openai \
                 (aliases: custom, lmstudio, vllm, groq, together), or local."
            )),
        }
    }
}

/// Top-level inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Transport: `ollama`, `openai` / `custom`, or `local`. Not a model name.
    #[serde(default = "default_backend")]
    pub backend: String,

    /// Chat model id for the `local` backend (GGUF stem). Other backends
    /// read `ollama.model` / `openai.model`. Any string the provider accepts.
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
    /// Any Ollama chat tag (`llama3.2`, `qwen2.5:14b`, `mistral`, …).
    #[serde(default = "default_ollama_model")]
    pub model: String,
    /// Any Ollama embedding tag. Width must match `embedding.dimensions`.
    #[serde(default = "default_ollama_embed_model")]
    pub embed_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// Base URL — OpenAI, LM Studio, vLLM, Groq, llama.cpp, OpenRouter, etc.
    #[serde(default = "default_openai_url")]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    /// Any chat model id the endpoint serves.
    #[serde(default = "default_model")]
    pub model: String,
    /// Any embedding model id the endpoint serves.
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
    /// Vector width. Default 384 matches `all-minilm` and the shipped
    /// `notes_vec` table. Change this when you plug in a different embedder
    /// (e.g. 768 for nomic-embed-text) and recreate `notes_vec`.
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
    /// Load config from a TOML file, then apply `SMRITI_*` env overrides.
    pub fn load(path: &std::path::Path) -> Self {
        let mut cfg = match std::fs::read_to_string(path) {
            Ok(content) => {
                // Try to parse the [inference] section
                if let Ok(val) = toml::from_str::<toml::Value>(&content) {
                    if let Some(inference) = val.get("inference") {
                        if let Ok(cfg) = inference.clone().try_into() {
                            cfg
                        } else {
                            tracing::warn!(
                                "Could not parse inference config from {:?}, using defaults",
                                path
                            );
                            Self::default()
                        }
                    } else {
                        Self::default()
                    }
                } else {
                    tracing::warn!(
                        "Could not parse inference config from {:?}, using defaults",
                        path
                    );
                    Self::default()
                }
            }
            Err(_) => Self::default(),
        };
        cfg.apply_env_overrides();
        cfg
    }

    /// Transport for this config. Model names are resolved separately.
    pub fn kind(&self) -> Result<BackendKind, String> {
        BackendKind::parse(&self.backend)
    }

    /// Chat model string for the active backend. Opaque to Smriti.
    pub fn chat_model(&self) -> &str {
        match self.kind().unwrap_or(BackendKind::Ollama) {
            BackendKind::Ollama => &self.ollama.model,
            BackendKind::OpenAiCompatible => &self.openai.model,
            BackendKind::Local => &self.model,
        }
    }

    /// Embedding model string for the active backend. Opaque to Smriti.
    pub fn embed_model(&self) -> &str {
        match self.kind().unwrap_or(BackendKind::Ollama) {
            BackendKind::Ollama => &self.ollama.embed_model,
            BackendKind::OpenAiCompatible => &self.openai.embed_model,
            BackendKind::Local => &self.model,
        }
    }

    /// Overlay process env. Safe to call more than once.
    pub fn apply_env_overrides(&mut self) {
        self.apply_overrides(|k| std::env::var(k).ok());
    }

    /// Testable override hook. Keys are `SMRITI_*` names.
    pub fn apply_overrides<F>(&mut self, get: F)
    where
        F: Fn(&str) -> Option<String>,
    {
        if let Some(v) = get("SMRITI_INFERENCE_BACKEND") {
            if !v.trim().is_empty() {
                self.backend = v;
            }
        }
        if let Some(v) = get("SMRITI_CHAT_MODEL") {
            if !v.trim().is_empty() {
                self.model = v.clone();
                self.ollama.model = v.clone();
                self.openai.model = v;
            }
        }
        if let Some(v) = get("SMRITI_EMBED_MODEL") {
            if !v.trim().is_empty() {
                self.ollama.embed_model = v.clone();
                self.openai.embed_model = v;
            }
        }
        if let Some(v) = get("SMRITI_OLLAMA_HOST") {
            if !v.trim().is_empty() {
                self.ollama.host = v;
            }
        }
        if let Some(v) = get("SMRITI_OPENAI_API_URL").or_else(|| get("SMRITI_OPENAI_URL")) {
            if !v.trim().is_empty() {
                self.openai.api_url = v;
            }
        }
        if let Some(v) = get("SMRITI_OPENAI_API_KEY") {
            self.openai.api_key = v;
        }
        if let Some(v) = get("SMRITI_EMBED_DIMENSIONS") {
            if let Ok(n) = v.trim().parse::<usize>() {
                if n > 0 {
                    self.embedding.dimensions = n;
                }
            }
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
    }

    #[test]
    fn backend_aliases_are_plugin_points() {
        assert_eq!(BackendKind::parse("ollama").unwrap(), BackendKind::Ollama);
        for alias in [
            "openai",
            "custom",
            "lmstudio",
            "vllm",
            "groq",
            "together",
            "openrouter",
            "llama.cpp",
        ] {
            assert_eq!(
                BackendKind::parse(alias).unwrap(),
                BackendKind::OpenAiCompatible,
                "{alias}"
            );
        }
        assert_eq!(BackendKind::parse("gguf").unwrap(), BackendKind::Local);
        assert!(BackendKind::parse("unknown-vendor").is_err());
    }

    #[test]
    fn overrides_swap_models_without_code_changes() {
        let mut cfg = InferenceConfig::default();
        cfg.apply_overrides(|k| match k {
            "SMRITI_INFERENCE_BACKEND" => Some("custom".into()),
            "SMRITI_CHAT_MODEL" => Some("qwen2.5:14b".into()),
            "SMRITI_EMBED_MODEL" => Some("nomic-embed-text".into()),
            "SMRITI_EMBED_DIMENSIONS" => Some("768".into()),
            "SMRITI_OPENAI_API_URL" => Some("http://127.0.0.1:1234/v1".into()),
            _ => None,
        });
        assert_eq!(cfg.kind().unwrap(), BackendKind::OpenAiCompatible);
        assert_eq!(cfg.chat_model(), "qwen2.5:14b");
        assert_eq!(cfg.embed_model(), "nomic-embed-text");
        assert_eq!(cfg.embedding.dimensions, 768);
        assert_eq!(cfg.openai.api_url, "http://127.0.0.1:1234/v1");
    }
}
