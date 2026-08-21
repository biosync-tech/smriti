# Smriti + Gemma 4 Integration Architecture
**Status: Design Phase (March 2026)**
**Target Release: v0.2.0 (Q2 2026)**
**License: Apache 2.0 (matching Gemma 4 model)**

---

## 1. Executive Summary

Smriti is integrating Google's **Gemma 4 31B Dense** model to become the first fully **local, AI-native knowledge graph engine**—zero API dependencies, ships as a single Rust binary.

### Vision
```
Smriti + Gemma 4 = LLM-first knowledge graph
                   • Semantic search (embeddings)
                   • RAG query engine
                   • Smart linking (similarity)
                   • Auto-tagging
                   • Multimodal ingestion
                   All running locally in one binary.
```

### Key Commitments
- **Single binary**: Gemma 4 inference engine embedded via llama-gguf crate
- **Zero cloud** (unless user opts for external backend)
- **Apache 2.0 licensed**: Full commercial freedom
- **Backward compatible**: All AI features are opt-in
- **Open Core**: Free tier + Pro tier with AI features

---

## 2. Gemma 4 Model Selection

### Model Specification

**Google Gemma 4 31B Dense**
- **Parameters**: 31.3B (all active; no sparse MoE)
- **Context Window**: 256K tokens
- **Architecture**: Transformer-based, trained on 500B+ tokens
- **Outputs**: Text + embeddings (via pooling)
- **License**: Apache 2.0 (commercial use permitted)
- **Model Family**: Foundational model by Google, widely trusted

### Quantization Variants

| Variant  | VRAM Req | File Size | Quality | Target Hardware |
|----------|----------|-----------|---------|-----------------|
| FP32     | 128 GB   | ~125 GB   | Max     | Data centers    |
| FP16     | 64 GB    | ~62 GB    | Near-max| High-end GPU    |
| Q8_0     | 34 GB    | ~35 GB    | Excellent | RTX 6000, A100  |
| Q5_K_M   | 22 GB    | ~22 GB    | Very good | RTX 4090, L40   |
| Q4_K_M   | 18 GB    | ~18 GB    | Good   | **Recommended** |
| Q3_K_M   | 12 GB    | ~12 GB    | Fair   | Consumer GPU    |
| Q2_K     | 8 GB     | ~8 GB     | Low    | Jetson AGX      |

**Recommended default: Q4_K_M** (18 GB VRAM)
- Best quality-to-size trade-off
- Fits RTX 3090 Ti, RTX 4080 Super, M2 Max/Ultra
- <50ms latency per 50-token generation on modern GPU
- CPU fallback (AVX-512): <500ms per 50 tokens

### Model License & Commercial Use

Gemma 4 is released under **Apache 2.0**, which permits:
- ✅ Commercial distribution (selling Smriti Pro)
- ✅ Bundling (shipping quantized model with binary)
- ✅ Modifications (quantization, fine-tuning)
- ✅ Closed-source distribution (Smriti Pro can be proprietary)

**Attribution required**: Include "Powered by Google Gemma 4" in UI.

---

## 3. Integration Architecture

### 3.1 Inference Engine: llama-gguf (Pure Rust)

**Why llama-gguf?**
1. **Pure Rust**: No C++ dependencies, compiles to single binary
2. **GGUF format**: Gemma 4 models ship as GGUF quantized (not safetensors)
3. **Hardware acceleration**: CUDA, Metal, Vulkan, DX12 with GPU fallback
4. **SIMD optimized**: AVX2, AVX-512, NEON for CPU inference
5. **Memory efficient**: Works with 8GB+ RAM (lower quantizations)
6. **Maintained**: Active Rust ecosystem (llama.cpp bindings)

**Cargo.toml addition:**
```toml
[dependencies]
llama-gguf = "0.14"
tokenizers = { version = "0.14", features = ["http"] }

[features]
default = ["mcp", "sync"]
gemma = ["llama-gguf"]    # Optional Gemma 4 integration
```

### 3.2 Pluggable Backend Trait

All inference routed through a single trait, enabling swappable backends:

```rust
// src/inference/mod.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub text: String,
    pub tokens_used: usize,
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    Length,
    StopSequence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub dimension: usize,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCapabilities {
    pub max_context: usize,
    pub embedding_dim: Option<usize>,
    pub supports_embedding: bool,
    pub supports_vision: bool,
    pub quantization: Option<String>,
}

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Inference error: {0}")]
    InferenceError(String),

    #[error("GPU out of memory")]
    OutOfMemory,

    #[error("Request timeout after {0}s")]
    Timeout(u64),

    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Backend not configured: {0}")]
    NotConfigured(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Generate text from a prompt.
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, InferenceError>;

    /// Generate embeddings for a batch of texts.
    async fn embed(&self, texts: &[&str]) -> Result<EmbeddingResponse, InferenceError>;

    /// Return capabilities of this backend.
    fn capabilities(&self) -> BackendCapabilities;

    /// Human-readable name (e.g., "Local Gemma 4", "Ollama").
    fn name(&self) -> &str;

    /// Check if backend is healthy.
    async fn health_check(&self) -> Result<(), InferenceError>;
}

/// Shared inference context across Smriti.
pub struct InferenceContext {
    pub backend: Arc<dyn InferenceBackend>,
    pub config: InferenceConfig,
}
```

### 3.3 Backend Implementations

#### LocalGemmaBackend (llama-gguf)

```rust
// src/inference/local.rs

use llama_gguf::Model;
use tokio::sync::Mutex;

pub struct LocalGemmaBackend {
    model: Arc<Mutex<Model>>,
    config: LocalInferenceConfig,
    tokenizer: Arc<tokenizers::Tokenizer>,
}

pub struct LocalInferenceConfig {
    pub model_path: PathBuf,
    pub quantization: QuantizationType,
    pub gpu_layers: i32,  // -1 = auto, 0 = CPU, N > 0 = N layers on GPU
    pub context_length: usize,
    pub num_threads: Option<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuantizationType {
    FP32,
    FP16,
    Q8_0,
    Q5_K_M,
    Q4_K_M,   // Recommended default
    Q3_K_M,
    Q2_K,
}

impl LocalGemmaBackend {
    pub async fn new(config: LocalInferenceConfig) -> Result<Self, InferenceError> {
        if !config.model_path.exists() {
            return Err(InferenceError::ModelNotFound(
                format!("{:?}", config.model_path)
            ));
        }

        let model = Model::load_file(
            &config.model_path,
            Default::default(),
        )
        .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        let tokenizer = tokenizers::Tokenizer::from_pretrained(
            "google/gemma-4-31b",
            None,
        )
        .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            config,
            tokenizer: Arc::new(tokenizer),
        })
    }

    /// Generate embeddings via mean pooling of token embeddings.
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, InferenceError> {
        let model = self.model.lock().await;
        // Tokenize
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        // Get token embeddings from model (assuming available in llama-gguf)
        // Mean pooling across sequence
        let embedding = model.embed(&encoding.get_ids().to_vec())
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        Ok(embedding)
    }
}

#[async_trait]
impl InferenceBackend for LocalGemmaBackend {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        let model = self.model.lock().await;

        let output = model.generate(
            &request.prompt,
            request.max_tokens.unwrap_or(512),
            request.temperature.unwrap_or(0.7),
            request.top_p.unwrap_or(0.9),
        )
        .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        Ok(GenerateResponse {
            text: output.text,
            tokens_used: output.tokens_used,
            finish_reason: if output.truncated {
                FinishReason::Length
            } else {
                FinishReason::Stop
            },
        })
    }

    async fn embed(&self, texts: &[&str]) -> Result<EmbeddingResponse, InferenceError> {
        let mut embeddings = Vec::new();
        for text in texts {
            let emb = self.embed_text(text).await?;
            embeddings.push(emb);
        }

        let dim = embeddings.first().map(|e| e.len()).unwrap_or(0);

        Ok(EmbeddingResponse {
            embeddings,
            dimension: dim,
            model: "gemma-4-31b-it".to_string(),
        })
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_context: self.config.context_length,
            embedding_dim: Some(3072),  // Gemma 4 embedding dimension
            supports_embedding: true,
            supports_vision: false,  // TODO: Future work
            quantization: Some(format!("{:?}", self.config.quantization)),
        }
    }

    fn name(&self) -> &str {
        "Local Gemma 4"
    }

    async fn health_check(&self) -> Result<(), InferenceError> {
        let model = self.model.lock().await;
        model.health_check()
            .map_err(|e| InferenceError::InferenceError(e.to_string()))
    }
}
```

#### OllamaBackend (For users with local Ollama instance)

```rust
// src/inference/ollama.rs

pub struct OllamaBackend {
    client: reqwest::Client,
    host: String,
    model: String,
}

#[async_trait]
impl InferenceBackend for OllamaBackend {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        let response = self.client
            .post(format!("{}/api/generate", self.host))
            .json(&serde_json::json!({
                "model": self.model,
                "prompt": request.prompt,
                "stream": false,
            }))
            .send()
            .await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        let body: serde_json::Value = response.json().await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        Ok(GenerateResponse {
            text: body["response"].as_str().unwrap_or("").to_string(),
            tokens_used: body["eval_count"].as_u64().unwrap_or(0) as usize,
            finish_reason: FinishReason::Stop,
        })
    }

    async fn embed(&self, texts: &[&str]) -> Result<EmbeddingResponse, InferenceError> {
        let mut embeddings = Vec::new();

        for text in texts {
            let response = self.client
                .post(format!("{}/api/embed", self.host))
                .json(&serde_json::json!({
                    "model": self.model,
                    "input": text,
                }))
                .send()
                .await
                .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

            let body: serde_json::Value = response.json().await
                .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

            let emb: Vec<f32> = serde_json::from_value(body["embedding"].clone())
                .map_err(|e| InferenceError::InferenceError(e.to_string()))?;
            embeddings.push(emb);
        }

        let dim = embeddings.first().map(|e| e.len()).unwrap_or(0);

        Ok(EmbeddingResponse {
            embeddings,
            dimension: dim,
            model: self.model.clone(),
        })
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_context: 128000,
            embedding_dim: Some(384),
            supports_embedding: true,
            supports_vision: false,
            quantization: None,
        }
    }

    fn name(&self) -> &str {
        "Ollama"
    }

    async fn health_check(&self) -> Result<(), InferenceError> {
        self.client
            .get(format!("{}/api/health", self.host))
            .send()
            .await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;
        Ok(())
    }
}
```

#### OpenAICompatibleBackend (LM Studio, vLLM, Text Generation WebUI, etc.)

```rust
// src/inference/openai.rs

pub struct OpenAICompatibleBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

#[async_trait]
impl InferenceBackend for OpenAICompatibleBackend {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        let response = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&serde_json::json!({
                "model": self.model,
                "messages": [
                    { "role": "user", "content": request.prompt }
                ],
                "temperature": request.temperature.unwrap_or(0.7),
                "top_p": request.top_p.unwrap_or(0.9),
                "max_tokens": request.max_tokens.unwrap_or(512),
            }))
            .send()
            .await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        let body: serde_json::Value = response.json().await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        Ok(GenerateResponse {
            text: body["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            tokens_used: body["usage"]["completion_tokens"]
                .as_u64()
                .unwrap_or(0) as usize,
            finish_reason: FinishReason::Stop,
        })
    }

    async fn embed(&self, texts: &[&str]) -> Result<EmbeddingResponse, InferenceError> {
        let response = self.client
            .post(format!("{}/v1/embeddings", self.base_url))
            .json(&serde_json::json!({
                "model": self.model,
                "input": texts,
            }))
            .send()
            .await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        let body: serde_json::Value = response.json().await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        let embeddings: Vec<Vec<f32>> = body["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|item| serde_json::from_value(item["embedding"].clone()).ok())
            .collect();

        let dim = embeddings.first().map(|e| e.len()).unwrap_or(0);

        Ok(EmbeddingResponse {
            embeddings,
            dimension: dim,
            model: self.model.clone(),
        })
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_context: 8192,
            embedding_dim: Some(384),
            supports_embedding: true,
            supports_vision: false,
            quantization: None,
        }
    }

    fn name(&self) -> &str {
        "OpenAI-Compatible"
    }

    async fn health_check(&self) -> Result<(), InferenceError> {
        self.client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;
        Ok(())
    }
}
```

### 3.4 Model Manager

Handles download, verification, caching, and lifecycle of model files.

```rust
// src/inference/manager.rs

use sha2::{Sha256, Digest};
use std::fs;
use std::path::PathBuf;

pub struct ModelManager {
    models_dir: PathBuf,
    cache: Arc<RwLock<HashMap<String, Arc<dyn InferenceBackend>>>>,
}

pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub quantization: QuantizationType,
    pub size_gb: f32,
    pub sha256: String,
    pub download_url: String,
    pub huggingface_repo: String,
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Result<Self, InferenceError> {
        fs::create_dir_all(&models_dir)?;
        Ok(Self {
            models_dir,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// List all available Gemma 4 quantizations.
    pub fn available_models() -> Vec<ModelMetadata> {
        vec![
            ModelMetadata {
                id: "gemma4-31b-q4km".to_string(),
                name: "Gemma 4 31B (Q4_K_M - Recommended)".to_string(),
                quantization: QuantizationType::Q4_K_M,
                size_gb: 18.0,
                sha256: "a1b2c3d4e5f6...".to_string(),
                download_url: "https://huggingface.co/QuantFactory/Gemma-4-31B-Instruct-GGUF/resolve/main/Gemma-4-31B-Instruct-Q4_K_M.gguf".to_string(),
                huggingface_repo: "QuantFactory/Gemma-4-31B-Instruct-GGUF".to_string(),
            },
            ModelMetadata {
                id: "gemma4-31b-q5km".to_string(),
                name: "Gemma 4 31B (Q5_K_M - Higher Quality)".to_string(),
                quantization: QuantizationType::Q5_K_M,
                size_gb: 22.0,
                sha256: "b2c3d4e5f6g7...".to_string(),
                download_url: "https://huggingface.co/QuantFactory/Gemma-4-31B-Instruct-GGUF/resolve/main/Gemma-4-31B-Instruct-Q5_K_M.gguf".to_string(),
                huggingface_repo: "QuantFactory/Gemma-4-31B-Instruct-GGUF".to_string(),
            },
            // ... more quantizations
        ]
    }

    /// Download a model with progress tracking and SHA256 verification.
    pub async fn download_model(
        &self,
        model_id: &str,
        progress_callback: Option<Box<dyn Fn(u64, u64)>>,
    ) -> Result<PathBuf, InferenceError> {
        let metadata = Self::available_models()
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| InferenceError::ModelNotFound(model_id.to_string()))?;

        let model_path = self.models_dir.join(format!("{}.gguf", model_id));

        if model_path.exists() {
            // Verify existing model
            if self.verify_model(&model_path, &metadata.sha256).await? {
                return Ok(model_path);
            }
        }

        // Download
        let client = reqwest::Client::new();
        let response = client
            .get(&metadata.download_url)
            .send()
            .await
            .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

        let total_size = response.content_length().unwrap_or(0);
        let mut hasher = Sha256::new();
        let mut file = fs::File::create(&model_path)?;
        let mut downloaded = 0u64;

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| InferenceError::InferenceError(e.to_string()))?;

            hasher.update(&chunk);
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            if let Some(cb) = &progress_callback {
                cb(downloaded, total_size);
            }
        }

        let computed_hash = format!("{:x}", hasher.finalize());
        if computed_hash != metadata.sha256 {
            fs::remove_file(&model_path)?;
            return Err(InferenceError::InferenceError(
                format!(
                    "SHA256 mismatch: expected {}, got {}",
                    metadata.sha256, computed_hash
                )
            ));
        }

        Ok(model_path)
    }

    /// Verify model file integrity.
    async fn verify_model(&self, path: &PathBuf, expected_sha256: &str) -> Result<bool, InferenceError> {
        let data = fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let computed = format!("{:x}", hasher.finalize());
        Ok(computed == expected_sha256)
    }

    /// Get or create a cached inference backend.
    pub async fn get_backend(
        &self,
        model_id: &str,
        config: InferenceConfig,
    ) -> Result<Arc<dyn InferenceBackend>, InferenceError> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(backend) = cache.get(model_id) {
                return Ok(Arc::clone(backend));
            }
        }

        // Create and cache
        let model_path = self.models_dir.join(format!("{}.gguf", model_id));

        let backend: Arc<dyn InferenceBackend> = match &config {
            InferenceConfig::Local(local_config) => {
                Arc::new(LocalGemmaBackend::new(local_config.clone()).await?)
            }
            InferenceConfig::Ollama(ollama_config) => {
                Arc::new(OllamaBackend::new(ollama_config.clone()))
            }
            InferenceConfig::OpenAICompatible(openai_config) => {
                Arc::new(OpenAICompatibleBackend::new(openai_config.clone()))
            }
        };

        // Store in cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(model_id.to_string(), Arc::clone(&backend));
        }

        Ok(backend)
    }
}
```

---

## 4. Feature Pipeline

### 4.1 Auto-Embedding Pipeline

On note create/update, embeddings are generated asynchronously in background.

```rust
// src/ai/embedder.rs

use tokio::sync::mpsc;

pub struct EmbeddingQueue {
    tx: mpsc::UnboundedSender<EmbeddingTask>,
    config: EmbeddingConfig,
}

pub struct EmbeddingTask {
    pub note_id: String,
    pub content: String,
    pub priority: u8,  // 1 = low, 10 = high
}

impl EmbeddingQueue {
    pub async fn new(
        db: Arc<Database>,
        backend: Arc<dyn InferenceBackend>,
        config: EmbeddingConfig,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Spawn background worker
        tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut priority_heap = std::collections::BinaryHeap::new();

            while let Some(task) = rx.recv().await {
                priority_heap.push(std::cmp::Reverse(task));

                if batch.len() >= config.batch_size {
                    Self::process_batch(&db, &backend, &mut batch).await;
                }
            }

            if !batch.is_empty() {
                Self::process_batch(&db, &backend, &mut batch).await;
            }
        });

        Self { tx, config }
    }

    pub fn queue_embedding(&self, task: EmbeddingTask) -> Result<(), InferenceError> {
        self.tx.send(task)
            .map_err(|e| InferenceError::InferenceError(e.to_string()))
    }

    async fn process_batch(
        db: &Database,
        backend: &dyn InferenceBackend,
        batch: &mut Vec<EmbeddingTask>,
    ) {
        let texts: Vec<&str> = batch.iter().map(|t| t.content.as_str()).collect();

        match backend.embed(&texts).await {
            Ok(response) => {
                for (task, embedding) in batch.iter().zip(response.embeddings.iter()) {
                    if let Err(e) = db.store_embedding(&task.note_id, embedding).await {
                        tracing::error!("Failed to store embedding for {}: {}", task.note_id, e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Embedding batch failed: {}", e);
                // Retry with exponential backoff
            }
        }

        batch.clear();
    }
}

pub struct EmbeddingConfig {
    pub enabled: bool,
    pub batch_size: usize,
    pub max_queue_size: usize,
    pub embedding_dimension: usize,
}
```

### 4.2 RAG Query Engine

Combines semantic search + FTS5 + graph context to answer questions.

```rust
// src/ai/rag.rs

pub struct RAGQueryEngine {
    db: Arc<Database>,
    backend: Arc<dyn InferenceBackend>,
    config: RAGConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGQuery {
    pub question: String,
    pub top_k: Option<usize>,
    pub context_depth: Option<usize>,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGResponse {
    pub answer: String,
    pub sources: Vec<SourceNote>,
    pub confidence: f32,
    pub inference_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceNote {
    pub id: String,
    pub title: String,
    pub excerpt: String,
    pub relevance_score: f32,
}

pub struct RAGConfig {
    pub top_k: usize,
    pub context_depth: usize,
    pub context_window_tokens: usize,
    pub max_source_tokens: usize,
}

impl RAGQueryEngine {
    pub async fn query(&self, rag_query: RAGQuery) -> Result<RAGResponse, InferenceError> {
        let start = std::time::Instant::now();
        let top_k = rag_query.top_k.unwrap_or(self.config.top_k);

        // Step 1: Generate query embedding
        let query_embedding = self.backend
            .embed(&[&rag_query.question])
            .await?
            .embeddings
            .into_iter()
            .next()
            .ok_or(InferenceError::InferenceError("No embedding returned".to_string()))?;

        // Step 2: Semantic search (sqlite-vec)
        let semantic_results = self.db
            .search_notes_by_embedding(&query_embedding, top_k)
            .await?;

        // Step 3: BFS graph expansion (get related notes)
        let mut all_notes = semantic_results.clone();
        for result in semantic_results.iter().take(3) {
            let neighbors = self.db
                .get_neighbors(&result.id, self.config.context_depth)
                .await?;
            all_notes.extend(neighbors);
        }

        // Step 4: Assemble context with token budgeting
        let context = self.assemble_context(all_notes, self.config.max_source_tokens)?;

        // Step 5: Generate answer with Gemma 4
        let system_prompt = rag_query.system_prompt.unwrap_or_else(|| {
            "You are an expert knowledge assistant. Answer the user's question based \
             on the provided context. Cite specific notes when relevant. \
             If the context doesn't contain relevant information, say so clearly."
                .to_string()
        });

        let prompt = format!(
            "{}

CONTEXT:
{}

QUESTION:
{}

ANSWER:",
            system_prompt, context, rag_query.question
        );

        let response = self.backend
            .generate(GenerateRequest {
                prompt,
                max_tokens: Some(512),
                temperature: Some(0.3),  // Lower temp for factuality
                top_p: Some(0.9),
                stop_sequences: None,
            })
            .await?;

        // Step 6: Extract source citations from context
        let sources = self.extract_sources(&context);

        let elapsed = start.elapsed().as_millis() as u64;

        Ok(RAGResponse {
            answer: response.text,
            sources,
            confidence: 0.8,  // TODO: Implement confidence scoring
            inference_time_ms: elapsed,
        })
    }

    fn assemble_context(
        &self,
        notes: Vec<SearchResult>,
        max_tokens: usize,
    ) -> Result<String, InferenceError> {
        let mut context = String::new();
        let mut token_count = 0;

        for note in notes {
            let note_text = format!(
                "[{}]\nTitle: {}\n{}\n\n",
                note.id, note.title, note.content
            );

            let tokens = note_text.split_whitespace().count();
            if token_count + tokens > max_tokens {
                break;
            }

            context.push_str(&note_text);
            token_count += tokens;
        }

        Ok(context)
    }

    fn extract_sources(&self, context: &str) -> Vec<SourceNote> {
        // Parse [note-id] format from context
        let re = regex::Regex::new(r"\[([a-f0-9-]+)\]").unwrap();

        re.captures_iter(context)
            .filter_map(|cap| {
                let id = cap.get(1)?.as_str().to_string();
                Some(SourceNote {
                    id,
                    title: String::new(),
                    excerpt: String::new(),
                    relevance_score: 0.8,
                })
            })
            .collect()
    }
}
```

### 4.3 AI Smart Linking

Suggests semantic connections between notes using embeddings (replaces keyword Jaccard).

```rust
// src/ai/linker.rs

pub struct AILinker {
    db: Arc<Database>,
    backend: Arc<dyn InferenceBackend>,
    similarity_threshold: f32,
}

impl AILinker {
    pub async fn suggest_links(&self, note_id: &str) -> Result<Vec<SuggestedLink>, InferenceError> {
        // Get embedding of current note
        let note = self.db.get_note(note_id).await?;
        let embedding = self.db.get_embedding(note_id).await?;

        if embedding.is_none() {
            return Err(InferenceError::InferenceError("No embedding for note".to_string()));
        }

        let embedding = embedding.unwrap();

        // Search for semantically similar notes
        let similar_notes = self.db
            .search_notes_by_embedding(&embedding, 20)
            .await?;

        let suggestions = similar_notes
            .into_iter()
            .filter(|result| {
                result.id != note_id && result.similarity > self.similarity_threshold as f64
            })
            .map(|result| SuggestedLink {
                target_note_id: result.id,
                target_title: result.title,
                reason: "Semantic similarity".to_string(),
                confidence: result.similarity as f32,
            })
            .collect();

        Ok(suggestions)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedLink {
    pub target_note_id: String,
    pub target_title: String,
    pub reason: String,
    pub confidence: f32,
}
```

### 4.4 Auto-Tagging

Suggests tags for new notes based on semantic analysis.

```rust
// src/ai/tagger.rs

pub struct AutoTagger {
    backend: Arc<dyn InferenceBackend>,
    db: Arc<Database>,
}

impl AutoTagger {
    pub async fn suggest_tags(&self, note_content: &str) -> Result<Vec<SuggestedTag>, InferenceError> {
        let prompt = format!(
            "Analyze this note and suggest 3-5 relevant tags (single words or short phrases, \
             comma-separated). Be specific and avoid generic terms.\n\nContent:\n{}",
            note_content
        );

        let response = self.backend
            .generate(GenerateRequest {
                prompt,
                max_tokens: Some(64),
                temperature: Some(0.5),
                top_p: Some(0.9),
                stop_sequences: Some(vec!["\n".to_string()]),
            })
            .await?;

        let tags: Vec<SuggestedTag> = response
            .text
            .split(',')
            .map(|tag| tag.trim())
            .filter(|tag| !tag.is_empty() && tag.len() < 30)
            .map(|tag| SuggestedTag {
                name: tag.to_string(),
                confidence: 0.7,
            })
            .collect();

        Ok(tags)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedTag {
    pub name: String,
    pub confidence: f32,
}
```

### 4.5 Multimodal Ingestion

Create notes from images (with Gemma 4 vision) or other media.

```rust
// src/ai/ingest.rs

pub struct MultimodalIngester {
    backend: Arc<dyn InferenceBackend>,
    db: Arc<Database>,
}

impl MultimodalIngester {
    pub async fn ingest_image(&self, image_path: &Path) -> Result<IngestedNote, InferenceError> {
        // Read image
        let image_data = std::fs::read(image_path)?;
        let image_base64 = base64::encode(&image_data);

        // Ask Gemma 4 to describe it
        let prompt = "Describe this image in detail, suitable for creating a knowledge note. \
                      Include key concepts, objects, and relationships.".to_string();

        // TODO: Once llama-gguf supports vision, use:
        // let response = self.backend.generate_with_image(prompt, image_base64).await?;

        let response = self.backend
            .generate(GenerateRequest {
                prompt: format!("{}\n\n[Image would be analyzed here]", prompt),
                max_tokens: Some(512),
                temperature: Some(0.6),
                top_p: Some(0.9),
                stop_sequences: None,
            })
            .await?;

        // Extract title from response
        let title = self.extract_title_from_description(&response.text)?;

        // Create note
        let note = self.db
            .create_note(
                &title,
                &response.text,
                Some(vec!["image".to_string()]),
            )
            .await?;

        Ok(IngestedNote {
            note_id: note.id,
            title: note.title,
            content: note.content,
            source_type: "image".to_string(),
        })
    }

    fn extract_title_from_description(&self, description: &str) -> Result<String, InferenceError> {
        // Simple: use first sentence as title
        let title = description
            .split('.')
            .next()
            .unwrap_or("Untitled")
            .trim()
            .to_string();

        Ok(if title.len() > 200 {
            title[..200].to_string()
        } else {
            title
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestedNote {
    pub note_id: String,
    pub title: String,
    pub content: String,
    pub source_type: String,
}
```

---

## 5. New Module Structure

```
src/
├── inference/                  # NEW — Core inference abstraction
│   ├── mod.rs                 # Traits, errors, types
│   ├── config.rs              # InferenceConfig enum
│   ├── local.rs               # LocalGemmaBackend (llama-gguf)
│   ├── ollama.rs              # OllamaBackend
│   ├── openai.rs              # OpenAICompatibleBackend
│   ├── manager.rs             # ModelManager
│   └── queue.rs               # EmbeddingQueue background worker
│
├── ai/                         # NEW — AI-powered features
│   ├── mod.rs
│   ├── rag.rs                 # RAG query engine
│   ├── embedder.rs            # Auto-embedding pipeline
│   ├── summarizer.rs          # Note summarization
│   ├── tagger.rs              # Auto-tagging
│   ├── linker.rs              # AI-powered smart linking
│   └── ingest.rs              # Multimodal ingestion
│
├── licensing/                  # NEW — Feature gating
│   ├── mod.rs
│   ├── features.rs            # Feature flags
│   └── key.rs                 # License key validation
│
├── models/                     # EXISTING
├── storage/                    # EXISTING
│   ├── db.rs                  # Extended with embeddings table
│   └── ...
├── parser/                     # EXISTING
├── graph/                      # EXISTING
├── api/                        # EXISTING (new routes added)
│   ├── routes/
│   │   ├── ai.rs              # NEW — /api/v1/ai/* endpoints
│   │   ├── notes.rs           # Extended with embed endpoint
│   │   └── ...
│   └── ...
├── mcp/                        # EXISTING (new tools added)
└── cli/                        # EXISTING (new commands added)
```

---

## 6. New MCP Tools

All new tools use the `gemma` feature flag. When disabled, tools return "Feature not available" error.

| Tool | Input | Output | Tier | Breaking |
|------|-------|--------|------|----------|
| `ai_query` | `{ question: str, top_k?: int }` | `{ answer: str, sources: SourceNote[] }` | Pro | No |
| `ai_summarize` | `{ note_id: str, max_tokens?: int }` | `{ summary: str, source: str }` | Pro | No |
| `ai_tag` | `{ note_id: str }` | `{ suggestions: SuggestedTag[] }` | Pro | No |
| `ai_link` | `{ note_id: str, threshold?: float }` | `{ suggestions: SuggestedLink[] }` | Pro | No |
| `ai_ingest_image` | `{ image_path: str }` | `{ note_id: str, title: str }` | Pro | No |
| `ai_embed` | `{ note_id: str }` | `{ embedding_dim: int, stored: bool }` | Core | No |
| `ai_status` | `{}` | `{ backend: str, model: str, capabilities: {...} }` | Core | No |

**Example MCP Tool (ai_query):**

```rust
// src/mcp/tools/ai_query.rs

pub async fn ai_query(
    db: Arc<Database>,
    backend: Arc<dyn InferenceBackend>,
    params: serde_json::Value,
) -> Result<serde_json::Value, ToolError> {
    let question = params["question"]
        .as_str()
        .ok_or(ToolError::InvalidParams("question required".to_string()))?;

    let top_k = params["top_k"]
        .as_u64()
        .map(|k| k as usize)
        .unwrap_or(10);

    let engine = RAGQueryEngine::new(db, backend);

    let response = engine
        .query(RAGQuery {
            question: question.to_string(),
            top_k: Some(top_k),
            context_depth: Some(2),
            system_prompt: None,
        })
        .await
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    Ok(serde_json::json!({
        "answer": response.answer,
        "sources": response.sources,
        "inference_time_ms": response.inference_time_ms,
    }))
}
```

---

## 7. New REST API Endpoints

### AI Query (RAG)
```
POST /api/v1/ai/query
Content-Type: application/json

{
  "question": "What are the main principles of effective note-taking?",
  "top_k": 10,
  "context_depth": 2,
  "system_prompt": "(optional custom system prompt)"
}

Response:
{
  "answer": "Based on your knowledge base, effective note-taking...",
  "sources": [
    {
      "id": "note-uuid",
      "title": "Note Title",
      "excerpt": "Relevant passage...",
      "relevance_score": 0.92
    }
  ],
  "confidence": 0.85,
  "inference_time_ms": 1250
}
```

### Auto-Tagging
```
POST /api/v1/ai/tag
Content-Type: application/json

{
  "note_id": "uuid"
}

Response:
{
  "suggestions": [
    { "name": "productivity", "confidence": 0.92 },
    { "name": "learning", "confidence": 0.87 }
  ]
}
```

### Smart Linking
```
POST /api/v1/ai/link
Content-Type: application/json

{
  "note_id": "uuid",
  "threshold": 0.75
}

Response:
{
  "suggestions": [
    {
      "target_note_id": "other-uuid",
      "target_title": "Related Note",
      "reason": "Semantic similarity",
      "confidence": 0.89
    }
  ]
}
```

### Image Ingestion
```
POST /api/v1/ai/ingest/image
Content-Type: multipart/form-data

[Binary image data]

Response:
{
  "note_id": "new-uuid",
  "title": "Extracted from image",
  "content": "Full description generated by Gemma 4...",
  "source_type": "image"
}
```

### Embedding Status
```
GET /api/v1/ai/status

Response:
{
  "backend": "Local Gemma 4",
  "model": "gemma-4-31b-it",
  "quantization": "Q4_K_M",
  "context_window": 8192,
  "capabilities": {
    "supports_embedding": true,
    "supports_vision": false,
    "embedding_dimension": 3072
  },
  "health": "healthy"
}
```

---

## 8. Configuration

### ~/.config/smriti/config.toml

```toml
[inference]
# Backend: "local" (default, Gemma 4), "ollama", "openai-compatible"
backend = "local"

# Model to use (backend-specific)
model = "gemma-4-31b-it"

# Quantization level (local backend only)
# Options: FP32, FP16, Q8_0, Q5_K_M, Q4_K_M (default), Q3_K_M, Q2_K
quantization = "Q4_K_M"

# Number of model layers to offload to GPU
# -1 = auto-detect, 0 = CPU only, N > 0 = N layers on GPU
gpu_layers = -1

# Context length in tokens (max recommended: 8192 for speed)
context_length = 8192

# Directory for downloaded models
models_dir = "~/.local/share/smriti/models"

# Show progress during model inference
show_progress = true

[inference.ollama]
# Only used when backend = "ollama"
host = "http://localhost:11434"
model = "gemma4:31b"
timeout_seconds = 120

[inference.openai]
# Only used when backend = "openai-compatible"
api_url = "http://localhost:8080/v1"
api_key = ""  # Leave empty for local (LM Studio, vLLM, etc)
model = "gemma-4-31b-it"
timeout_seconds = 120

[embedding]
# Automatically generate embeddings on note create/update
auto_embed = true

# Batch size for embedding generation
batch_size = 32

# Embedding vector dimension (must match sqlite-vec schema)
# Gemma 4 = 3072, Most others = 384-768
dimensions = 3072

# Maximum queue size (notes waiting for embedding)
max_queue_size = 10000

[ai]
# Enable RAG (Retrieval-Augmented Generation) queries
rag_enabled = true

# Number of top semantic matches to use as context
rag_top_k = 10

# Depth of graph neighborhood to include
rag_context_depth = 2

# Token budget for context assembly
rag_context_window = 4096

# Enable auto-tagging on note creation
auto_tag = true

# Enable AI smart linking suggestions
auto_link = true

# Enable AI daily digest
daily_digest = true

# Semantic similarity threshold for linking (0.0-1.0)
link_threshold = 0.75

# Max tokens for summarization
summarize_max_tokens = 512

[licensing]
# Feature tier: "free", "pro", "enterprise"
tier = "pro"

# License key (if using Pro/Enterprise)
key = ""

# Auto-check license expiry
check_expiry = true
```

---

## 9. Monetization Strategy Analysis

### Open Core Model (Recommended)

#### Core (Free, MIT License)
- Full knowledge graph (notes, tags, wiki-links)
- Full-text search (FTS5)
- Graph visualization & traversal
- Manual embeddings (bring your own vectors via API)
- CLI + Web UI + REST API + MCP
- Sync engine (WebDAV)
- Community support

**Distribution**: Open-source on GitHub, MIT license

#### Pro ($29/month or $249/year per seat)
- Local Gemma 4 31B inference engine (ships with binary)
- Auto-embedding pipeline (background)
- RAG query engine (ask questions over knowledge)
- AI smart linking (semantic similarity replaces keyword matching)
- Auto-tagging (suggest tags from content)
- AI daily digest (natural language summary of activity)
- Multimodal ingestion (images → notes via vision)
- Priority support & updates
- Commercial license included (Apache 2.0)

**Distribution**: Closed-source binary + proprietary license

#### Enterprise (Custom Pricing)
- Everything in Pro, plus:
- Multi-user with RBAC (role-based access control)
- Audit logging (who accessed what, when)
- Custom model fine-tuning support
- On-premises deployment assistance
- SLA with guaranteed response time
- API uptime guarantee (99.9%)

**Distribution**: Custom contracts, on-prem or managed hosting

### Why This Model Works

1. **Marginal costs near zero** — Gemma 4 runs locally, no API bills
2. **No vendor lock-in for free tier** — MIT-licensed, users can self-host
3. **Lock-in through quality** — AI features become indispensable (like dark mode)
4. **Natural upsell path** — Free → Pro → Enterprise
5. **Defensible moat** — Fine-tuned models and integrations (Task 7, 8)
6. **Hardware partnerships** — Bundle with NVIDIA Jetson, Intel NUC, Raspberry Pi 5
7. **B2B channel** — Sell to knowledge management platforms (Obsidian, Roam Research)

### Revenue Projections (Conservative)

Assuming:
- 50K free users (organic growth, year 1)
- 5% conversion to Pro ($29/month) = 2,500 users = **$870K/year**
- 1% conversion to Enterprise = 500 orgs = **$5M-20M/year** (custom pricing)
- Hosting + support = **$500K-1M/year**

**Total Year 1 Revenue (conservative): $6.4M-21M**

Costs:
- Infrastructure: $100K/year
- Staff (10 engineers): $1.5M/year
- Operations: $200K/year
- **Total: $1.8M/year**

**Gross Margin: 71-91%**

### Licensing Implementation

```rust
// src/licensing/features.rs

pub enum Tier {
    Free,
    Pro,
    Enterprise,
}

pub struct LicenseKey {
    pub tier: Tier,
    pub email: String,
    pub expires_at: DateTime<Utc>,
    pub seats: usize,  // Number of users
    pub signature: String,  // Ed25519 signature
}

impl LicenseKey {
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }

    pub fn has_feature(&self, feature: &str) -> bool {
        match (self.tier, feature) {
            (Tier::Free, "rag") => false,
            (Tier::Free, "auto_embed") => false,
            (Tier::Free, "ai_link") => false,
            (Tier::Pro, _) => true,
            (Tier::Enterprise, _) => true,
            _ => false,
        }
    }
}
```

---

## 10. Implementation Priority

Work on these tasks in order. Do not skip ahead.

### Phase 1: Core Inference (Weeks 1-3)
1. **InferenceBackend trait** + error types (src/inference/mod.rs)
2. **LocalGemmaBackend** integration with llama-gguf (src/inference/local.rs)
3. **ModelManager** — download, SHA256 verification, caching (src/inference/manager.rs)
4. **Tests**: Unit tests for each backend, integration test with real model

### Phase 2: Auto-Embedding (Weeks 4-6)
5. **sqlite-vec table** in schema (src/storage/db.rs)
6. **EmbeddingQueue** background worker (src/inference/queue.rs)
7. **API endpoint**: POST /api/v1/notes/:id/embed
8. **Tests**: Verify embeddings stored correctly, batch processing

### Phase 3: RAG Query Engine (Weeks 7-9)
9. **RAG query engine** (src/ai/rag.rs)
10. **Search by embedding** in database (src/storage/operations.rs)
11. **API endpoint**: POST /api/v1/ai/query
12. **MCP tool**: ai_query
13. **Tests**: End-to-end RAG tests with real queries

### Phase 4: Agentic Features (Weeks 10-12)
14. **AI Smart Linking** (src/ai/linker.rs)
15. **Auto-Tagging** (src/ai/tagger.rs)
16. **New MCP tools**: ai_link, ai_tag
17. **New API endpoints**: /api/v1/ai/link, /api/v1/ai/tag

### Phase 5: Multimodal & Polish (Weeks 13-16)
18. **Multimodal ingestion** (src/ai/ingest.rs)
19. **Summarization** (src/ai/summarizer.rs)
20. **Licensing** (src/licensing/)
21. **Feature gating** (Pro tier locked features)
22. **Documentation** (API docs, config guide, examples)

### Phase 6: Release & Monitoring (Weeks 17-20)
23. **Criterion benchmarks** for new modules
24. **Load testing** (concurrent RAG queries)
25. **Docker build** (bundle Gemma 4 in image)
26. **GitHub release** (v0.2.0, announce Pro tier)

---

## 11. Performance Targets

| Operation | Target | Hardware | Notes |
|-----------|--------|----------|-------|
| Embed (single note) | < 100ms | RTX 4090 + Q4_K_M | Includes tokenization |
| Embed batch (32 notes) | < 500ms | " | Amortized ~15ms/note |
| RAG query (10K notes) | < 2s | " | Search + generation |
| Auto-tag | < 500ms | " | Single forward pass |
| Summarize (note) | < 1s | " | 512 output tokens |
| Model load (cold) | < 5s | " | Cached in VRAM after |
| Model load (cached) | < 100ms | " | Already in memory |

**CPU Fallback (AVX-512):**
- Embed (single) < 500ms
- RAG query (10K) < 15s
- Auto-tag < 2s

---

## 12. Risk Mitigation

### Risk: Model Size (18-34 GB VRAM)

**Mitigation:**
- Default to Q4_K_M (18 GB) for ease of adoption
- Offer Q3_K_M (12 GB) for laptops, Q2_K (8 GB) for Jetson
- CPU fallback with SIMD (slow but works)
- Clear documentation: "Requires 16+ GB RAM (GPU recommended)"

### Risk: Binary Size Growth

**Mitigation:**
- llama-gguf adds ~5 MB to binary
- Model files downloaded separately (not bundled)
- Optional `gemma` feature flag allows compilation without llama-gguf

### Risk: Breaking MCP Changes

**Mitigation:**
- All new MCP tools are **additive** (no existing tools modified)
- Use versioned endpoints if changes needed later

### Risk: Inference Latency

**Mitigation:**
- Embeddings processed async in background queue
- RAG queries cached (same question → instant response)
- Model quantization (Q4_K_M balances speed/quality)

### Risk: Backward Compatibility

**Mitigation:**
- All AI features behind feature flag + license check
- Existing code paths unchanged
- Rollback plan: disable `gemma` feature to restore v0.1.x behavior

---

## 13. Research Anchors

| Paper | arXiv ID | Relevance |
|-------|----------|-----------|
| Gemma 4: Scaling Open Models Responsibly | arXiv:2501.08988 | Model architecture, scaling laws |
| Retrieval-Augmented Generation (RAG) | arXiv:2005.11401 | RAG pipeline design |
| Graph-Based Memory Survey | arXiv:2602.05665 | Hybrid (FTS5 + semantic) beats pure vector |
| MAGMA: Multi-Graph Attention | arXiv:2601.03236 | Typed graph layers reduce token usage |
| Zep / Bi-temporal Graphs | arXiv:2501.13956 | Valid_from/valid_until for evolving knowledge |

---

## 14. Commit Convention (Smriti v0.2.0)

```
feat(inference): add llama-gguf backend for local Gemma 4 inference
feat(ai): implement RAG query engine with hybrid FTS5+semantic search
feat(api): add POST /api/v1/ai/query endpoint
feat(mcp): add ai_query tool for RAG queries over knowledge graph
feat(ai): add auto-embedding pipeline with background queue
feat(inference): add ModelManager for Gemma 4 download and caching
feat(ai): add AI smart linking (semantic similarity via embeddings)
feat(ai): add auto-tagging with Gemma 4
feat(api): add POST /api/v1/ai/tag and POST /api/v1/ai/link endpoints
feat(ai): add multimodal ingestion (images → notes)
feat(licensing): add feature tier gating (Free/Pro/Enterprise)
feat(inference): add Ollama and OpenAI-compatible backends
refactor(inference): extract InferenceBackend trait, support multiple backends
test(ai): add integration tests for RAG, auto-linking, auto-tagging
test(inference): add benchmarks for embedding and inference latency
docs: add GEMMA4_INTEGRATION.md with architecture and monetization
chore: bump llama-gguf to 0.14, tokenizers to 0.14
```

---

## 15. Appendix: Gemma 4 Model Files

### Recommended GGUF Quantizations

| Model ID | File | Size | VRAM | Source |
|----------|------|------|------|--------|
| gemma4-31b-q4km | Gemma-4-31B-Instruct-Q4_K_M.gguf | 18 GB | 18 GB | HuggingFace (QuantFactory) |
| gemma4-31b-q5km | Gemma-4-31B-Instruct-Q5_K_M.gguf | 22 GB | 22 GB | " |
| gemma4-31b-q8 | Gemma-4-31B-Instruct-Q8_0.gguf | 35 GB | 34 GB | " |
| gemma4-7b-q4km | Gemma-4-7B-Instruct-Q4_K_M.gguf | 4.5 GB | 4.5 GB | " |

**Download URL pattern:**
```
https://huggingface.co/QuantFactory/Gemma-4-31B-Instruct-GGUF/resolve/main/{model}.gguf
```

### SHA256 Checksums (Example)

```
a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t0u1v2w3x4y5z6a7b8c9d0e1f  gemma-4-31b-q4km
b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t0u1v2w3x4y5z6a7b8c9d0e1f2g3  gemma-4-31b-q5km
```

(Actual checksums to be populated from official sources)

---

## 16. Key Decisions & Rationale

| Decision | Rationale |
|----------|-----------|
| **Gemma 4 31B Dense** | Best quality, Apache 2.0 license, proven on benchmarks |
| **Q4_K_M quantization** | Sweet spot: 18GB VRAM, <100ms latency, good quality |
| **llama-gguf backend** | Pure Rust, GGUF native, single binary, GPU acceleration |
| **Pluggable backends** | Support Ollama, OpenAI-compatible (LM Studio, vLLM) |
| **sqlite-vec for embeddings** | No new processes, same .db file, WAL mode safe |
| **Async embedding queue** | Non-blocking note creation, background batch processing |
| **Open Core licensing** | Proven SaaS model (Obsidian, GitHub Copilot) |
| **Feature flag gating** | Backward compat, easy A/B testing, CI/CD clean |

---

**Document Version: 1.0 (March 2026)**
**Target Release: Smriti v0.2.0 (Q2 2026)**
**Status: Design Phase — Ready for implementation**

