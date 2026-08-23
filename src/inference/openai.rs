//! OpenAI-compatible API backend
//!
//! Works with any server that implements the OpenAI API spec:
//! - vLLM, llama.cpp server, LocalAI, LM Studio, etc.
//! - Also works with actual OpenAI API (but defeats the "local" purpose)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::*;
use super::config::InferenceConfig;

pub struct OpenAICompatibleBackend {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
    embed_model: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    model: String,
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl OpenAICompatibleBackend {
    pub fn new(config: &InferenceConfig) -> Result<Self, InferenceError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| InferenceError::Http(e.to_string()))?;

        Ok(Self {
            client,
            api_url: config.openai.api_url.trim_end_matches('/').to_string(),
            api_key: config.openai.api_key.clone(),
            model: config.openai.model.clone(),
            embed_model: config.openai.embed_model.clone(),
        })
    }
}

#[async_trait]
impl InferenceBackend for OpenAICompatibleBackend {
    async fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        let mut messages = Vec::new();

        if let Some(system) = &request.system {
            messages.push(ChatMessage {
                role: "system".into(),
                content: system.clone(),
            });
        }

        messages.push(ChatMessage {
            role: "user".into(),
            content: request.prompt.clone(),
        });

        let body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop: request.stop.clone(),
            stream: false,
        };

        let mut req = self
            .client
            .post(format!("{}/chat/completions", self.api_url))
            .json(&body);

        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| InferenceError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(InferenceError::GenerationFailed(format!(
                "API returned {}: {}",
                status, text
            )));
        }

        let api_resp: ChatCompletionResponse = resp
            .json()
            .await
            .map_err(|e| InferenceError::Http(format!("JSON decode error: {}", e)))?;

        let choice = api_resp
            .choices
            .first()
            .ok_or_else(|| InferenceError::GenerationFailed("No choices returned".into()))?;

        Ok(GenerateResponse {
            text: choice.message.content.clone(),
            tokens_used: api_resp.usage.map(|u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            model: api_resp.model,
            finish_reason: choice.finish_reason.clone(),
        })
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        let body = EmbeddingRequest {
            model: self.embed_model.clone(),
            input: texts.to_vec(),
        };

        let mut req = self
            .client
            .post(format!("{}/embeddings", self.api_url))
            .json(&body);

        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| InferenceError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(InferenceError::EmbeddingFailed(text));
        }

        let embed_resp: EmbeddingResponse = resp
            .json()
            .await
            .map_err(|e| InferenceError::Http(format!("JSON decode error: {}", e)))?;

        Ok(embed_resp
            .data
            .into_iter()
            .map(|d| d.embedding)
            .collect())
    }

    async fn describe_image(
        &self,
        _image_bytes: &[u8],
        _prompt: &str,
    ) -> Result<String, InferenceError> {
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
        "openai-compatible"
    }

    async fn health_check(&self) -> Result<BackendStatus, InferenceError> {
        let mut req = self.client.get(format!("{}/models", self.api_url));
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| InferenceError::BackendUnavailable(e.to_string()))?;

        Ok(BackendStatus {
            ready: resp.status().is_success(),
            backend_name: "openai-compatible".into(),
            model_loaded: Some(self.model.clone()),
            gpu_available: false, // Can't determine remotely
            vram_used_mb: None,
            vram_total_mb: None,
        })
    }
}
