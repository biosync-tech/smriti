//! Ollama backend — any model Ollama can serve
//!
//! Chat and embed model names come from config (`ollama.model`,
//! `ollama.embed_model`). Smriti does not pin a required tag.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::config::InferenceConfig;
use super::*;

pub struct OllamaBackend {
    client: reqwest::Client,
    host: String,
    model: String,
    embed_model: String,
}

#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    system: Option<String>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    top_p: f32,
    num_predict: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    model: String,
    done: bool,
    #[serde(default)]
    eval_count: u32,
    #[serde(default)]
    prompt_eval_count: u32,
}

#[derive(Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

impl OllamaBackend {
    pub fn new(config: &InferenceConfig) -> Result<Self, InferenceError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| InferenceError::Http(e.to_string()))?;

        Ok(Self {
            client,
            host: config.ollama.host.clone(),
            model: config.ollama.model.clone(),
            embed_model: config.ollama.embed_model.clone(),
        })
    }
}

#[async_trait]
impl InferenceBackend for OllamaBackend {
    async fn generate(
        &self,
        request: &GenerateRequest,
    ) -> Result<GenerateResponse, InferenceError> {
        let body = OllamaGenerateRequest {
            model: self.model.clone(),
            prompt: request.prompt.clone(),
            system: request.system.clone(),
            stream: false,
            options: OllamaOptions {
                temperature: request.temperature.unwrap_or(0.7),
                top_p: request.top_p.unwrap_or(0.9),
                num_predict: request.max_tokens.unwrap_or(2048),
                stop: request.stop.clone(),
            },
        };

        let resp = self
            .client
            .post(format!("{}/api/generate", self.host))
            .json(&body)
            .send()
            .await
            .map_err(|e| InferenceError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(InferenceError::GenerationFailed(format!(
                "Ollama returned {}: {}",
                status, text
            )));
        }

        let ollama_resp: OllamaGenerateResponse = resp
            .json()
            .await
            .map_err(|e| InferenceError::Http(format!("JSON decode error: {}", e)))?;

        Ok(GenerateResponse {
            text: ollama_resp.response,
            tokens_used: Some(TokenUsage {
                prompt_tokens: ollama_resp.prompt_eval_count,
                completion_tokens: ollama_resp.eval_count,
                total_tokens: ollama_resp.prompt_eval_count + ollama_resp.eval_count,
            }),
            model: ollama_resp.model,
            finish_reason: if ollama_resp.done {
                Some("stop".into())
            } else {
                None
            },
        })
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        let body = OllamaEmbedRequest {
            model: self.embed_model.clone(),
            input: texts.to_vec(),
        };

        let resp = self
            .client
            .post(format!("{}/api/embed", self.host))
            .json(&body)
            .send()
            .await
            .map_err(|e| InferenceError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(InferenceError::EmbeddingFailed(text));
        }

        let embed_resp: OllamaEmbedResponse = resp
            .json()
            .await
            .map_err(|e| InferenceError::Http(format!("JSON decode error: {}", e)))?;

        Ok(embed_resp.embeddings)
    }

    async fn describe_image(
        &self,
        _image_bytes: &[u8],
        _prompt: &str,
    ) -> Result<String, InferenceError> {
        // Ollama supports vision via /api/generate with images field
        // TODO: implement when needed
        Err(InferenceError::VisionNotSupported)
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            text_generation: true,
            embeddings: true,
            vision: false,
            audio: false,
            function_calling: false,
            max_context_length: 131072,
            model_name: self.model.clone(),
        }
    }

    fn name(&self) -> &str {
        "ollama"
    }

    async fn health_check(&self) -> Result<BackendStatus, InferenceError> {
        let resp = self
            .client
            .get(format!("{}/api/tags", self.host))
            .send()
            .await
            .map_err(|e| InferenceError::BackendUnavailable(e.to_string()))?;

        Ok(BackendStatus {
            ready: resp.status().is_success(),
            backend_name: "ollama".into(),
            model_loaded: Some(self.model.clone()),
            gpu_available: true, // Ollama handles GPU detection
            vram_used_mb: None,
            vram_total_mb: None,
        })
    }
}
