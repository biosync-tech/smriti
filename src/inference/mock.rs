//! Mock inference backend for tests — returns canned responses.
#![cfg(test)]

use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use super::{
    BackendCapabilities, BackendStatus, GenerateRequest, GenerateResponse, InferenceBackend,
    InferenceError, TokenUsage,
};

#[derive(Clone)]
pub struct MockBackend {
    pub canned_text: Arc<Mutex<String>>,
    pub call_count: Arc<Mutex<u32>>,
    pub canned_embedding: Vec<f32>,
}

impl MockBackend {
    pub fn new(canned_text: &str) -> Self {
        Self {
            canned_text: Arc::new(Mutex::new(canned_text.to_string())),
            call_count: Arc::new(Mutex::new(0)),
            canned_embedding: vec![0.1; 384],
        }
    }

    pub fn calls(&self) -> u32 {
        *self.call_count.lock().unwrap()
    }
}

#[async_trait]
impl InferenceBackend for MockBackend {
    async fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        *self.call_count.lock().unwrap() += 1;
        Ok(GenerateResponse {
            text: self.canned_text.lock().unwrap().clone(),
            model: "mock:v1".into(),
            tokens_used: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
            finish_reason: Some("stop".into()),
        })
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        Ok(texts
            .iter()
            .map(|_| self.canned_embedding.clone())
            .collect())
    }

    async fn describe_image(&self, _bytes: &[u8], _prompt: &str) -> Result<String, InferenceError> {
        Ok("mock image description".into())
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            text_generation: true,
            embeddings: true,
            vision: false,
            audio: false,
            function_calling: false,
            max_context_length: 4096,
            model_name: "mock".into(),
        }
    }

    fn name(&self) -> &str {
        "mock"
    }

    async fn health_check(&self) -> Result<BackendStatus, InferenceError> {
        Ok(BackendStatus {
            ready: true,
            backend_name: "mock".into(),
            model_loaded: Some("mock:v1".into()),
            gpu_available: false,
            vram_used_mb: None,
            vram_total_mb: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_backend_returns_canned_text() {
        let m = MockBackend::new("hello");
        let req = GenerateRequest::default();
        let res = m.generate(&req).await.unwrap();
        assert_eq!(res.text, "hello");
        assert_eq!(m.calls(), 1);
    }
}
