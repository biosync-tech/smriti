//! Model Manager — optional local GGUF cache
//!
//! Default inference is Ollama. This manager only applies when a user
//! explicitly opts into the `local` backend and supplies a GGUF on disk.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use super::config::InferenceConfig;
use super::InferenceError;

/// Opt-in registry. Empty — GGUF files are user-supplied, not shipped.
const KNOWN_MODELS: &[ModelEntry] = &[];

struct ModelEntry {
    model: &'static str,
    quantization: &'static str,
    repo: &'static str,
    filename: &'static str,
    size_bytes: u64,
    sha256: Option<&'static str>,
}

/// Manages model downloads, verification, and local caching
pub struct ModelManager {
    config: InferenceConfig,
    client: reqwest::Client,
}

/// Status of a model download
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub model: String,
    pub quantization: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub downloaded: bool,
    pub verified: bool,
}

impl ModelManager {
    pub fn new(config: &InferenceConfig) -> Result<Self, InferenceError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600)) // 1hr for large downloads
            .build()
            .map_err(|e| InferenceError::Http(e.to_string()))?;

        Ok(Self {
            config: config.clone(),
            client,
        })
    }

    /// List all available models (downloaded and not)
    pub fn list_models(&self) -> Vec<ModelInfo> {
        KNOWN_MODELS
            .iter()
            .map(|entry| {
                let path = self.config.models_dir.join(entry.filename);
                let downloaded = path.exists();
                ModelInfo {
                    model: entry.model.to_string(),
                    quantization: entry.quantization.to_string(),
                    path,
                    size_bytes: entry.size_bytes,
                    downloaded,
                    verified: downloaded, // Assume verified if exists (TODO: check hash)
                }
            })
            .collect()
    }

    /// Check if the configured model is available locally
    pub fn is_model_available(&self) -> bool {
        self.config.model_path().exists()
    }

    /// Download a model from HuggingFace
    pub async fn download_model(
        &self,
        model: &str,
        quantization: &str,
    ) -> Result<PathBuf, InferenceError> {
        let entry = KNOWN_MODELS
            .iter()
            .find(|e| e.model == model && e.quantization == quantization)
            .ok_or_else(|| {
                InferenceError::ModelNotFound(format!(
                    "Unknown model: {} ({}). Available: {:?}",
                    model,
                    quantization,
                    KNOWN_MODELS
                        .iter()
                        .map(|e| format!("{}:{}", e.model, e.quantization))
                        .collect::<Vec<_>>()
                ))
            })?;

        // Ensure models directory exists
        std::fs::create_dir_all(&self.config.models_dir)?;

        let dest = self.config.models_dir.join(entry.filename);

        if dest.exists() {
            tracing::info!("Model already exists at {:?}", dest);
            return Ok(dest);
        }

        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            entry.repo, entry.filename
        );

        tracing::info!("Downloading {} from {}", entry.filename, url);
        tracing::info!("Expected size: {:.1} GB", entry.size_bytes as f64 / 1e9);

        let mut resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| InferenceError::DownloadFailed(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(InferenceError::DownloadFailed(format!(
                "HTTP {}: {}",
                resp.status(),
                url
            )));
        }

        // Stream to a temp file — never hold the GGUF in RAM.
        let tmp_path = dest.with_extension("gguf.part");
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        let mut downloaded = 0u64;
        use tokio::io::AsyncWriteExt;
        loop {
            let chunk = resp
                .chunk()
                .await
                .map_err(|e| InferenceError::DownloadFailed(e.to_string()))?;
            let Some(chunk) = chunk else {
                break;
            };
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
        }
        file.flush().await?;
        tokio::fs::rename(&tmp_path, &dest).await?;

        tracing::info!(
            "Download complete: {:?} ({:.1} GB)",
            dest,
            downloaded as f64 / 1e9
        );

        Ok(dest)
    }

    /// Verify a downloaded model's SHA256 hash
    pub async fn verify_model(&self, path: &Path) -> Result<bool, InferenceError> {
        if !path.exists() {
            return Err(InferenceError::ModelNotFound(format!(
                "{:?} does not exist",
                path
            )));
        }

        use tokio::io::AsyncReadExt;
        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 1024 * 1024];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let hash = format!("{:x}", hasher.finalize());

        tracing::info!("Model SHA256: {}", hash);

        // If we have a known hash, verify it
        let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");

        if let Some(entry) = KNOWN_MODELS.iter().find(|e| e.filename == filename) {
            if let Some(expected) = entry.sha256 {
                if hash != expected {
                    return Err(InferenceError::DownloadFailed(format!(
                        "SHA256 mismatch: expected {}, got {}",
                        expected, hash
                    )));
                }
            }
        }

        Ok(true)
    }

    /// Delete a downloaded model
    pub async fn delete_model(
        &self,
        model: &str,
        quantization: &str,
    ) -> Result<(), InferenceError> {
        let entry = KNOWN_MODELS
            .iter()
            .find(|e| e.model == model && e.quantization == quantization)
            .ok_or_else(|| {
                InferenceError::ModelNotFound(format!(
                    "Unknown model: {} ({})",
                    model, quantization
                ))
            })?;

        let path = self.config.models_dir.join(entry.filename);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
            tracing::info!("Deleted model: {:?}", path);
        }

        Ok(())
    }
}
