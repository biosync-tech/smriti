//! Audit-overhead benchmark — paper §6.1.
//!
//! Measures the per-call overhead of the AuditedInference wrapper vs a
//! direct InferenceBackend call. Both routes use a deterministic in-process
//! mock backend so the only thing being timed is the audit layer itself
//! (event hash + INSERT INTO events + INSERT INTO llm_audit).
//!
//! Run:  cargo bench --bench audit_overhead
//! Reports: target/criterion/audit_overhead/<group>/report/index.html

use std::sync::Arc;

use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

use smriti::inference::{
    AuditedInference, BackendCapabilities, BackendStatus, CallContext, GenerateRequest,
    GenerateResponse, InferenceBackend, InferenceError, TokenUsage,
};
use smriti::storage::Database;

// ─── Inline mock backend ────────────────────────────────────────
//
// The crate-internal MockBackend is `#[cfg(test)]`-gated, so we duplicate
// the minimum surface area here. Returns a constant 64-byte response.

struct BenchMock;

#[async_trait]
impl InferenceBackend for BenchMock {
    async fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, InferenceError> {
        Ok(GenerateResponse {
            text: "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt".into(),
            model: "bench-mock:v1".into(),
            tokens_used: Some(TokenUsage {
                prompt_tokens: 64,
                completion_tokens: 16,
                total_tokens: 80,
            }),
            finish_reason: Some("stop".into()),
        })
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, InferenceError> {
        Ok(texts.iter().map(|_| vec![0.1f32; 384]).collect())
    }

    async fn describe_image(&self, _b: &[u8], _p: &str) -> Result<String, InferenceError> {
        Ok("mock".into())
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            text_generation: true,
            embeddings: true,
            vision: false,
            audio: false,
            function_calling: false,
            max_context_length: 4096,
            model_name: "bench-mock:v1".into(),
        }
    }

    fn name(&self) -> &str {
        "bench-mock"
    }

    async fn health_check(&self) -> Result<BackendStatus, InferenceError> {
        Ok(BackendStatus {
            ready: true,
            backend_name: "bench-mock".into(),
            model_loaded: Some("bench-mock:v1".into()),
            gpu_available: false,
            vram_used_mb: None,
            vram_total_mb: None,
        })
    }
}

// ─── Benchmark setup ────────────────────────────────────────────

fn make_request() -> GenerateRequest {
    GenerateRequest {
        prompt: "Summarize the patient's progress notes from the last visit.".into(),
        system: Some("You are a clinical research assistant.".into()),
        max_tokens: Some(512),
        temperature: Some(0.0),
        top_p: None,
        stop: None,
        thinking: None,
    }
}

fn audit_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Baseline: bare InferenceBackend, no audit layer
    c.bench_function("baseline_generate", |b| {
        let backend: Arc<dyn InferenceBackend> = Arc::new(BenchMock);
        let req = make_request();
        b.iter(|| {
            rt.block_on(async {
                let res = backend.generate(&req).await.unwrap();
                criterion::black_box(res)
            })
        });
    });

    // Audited: AuditedInference around the same mock + a fresh in-memory DB.
    // Each iteration writes one events row + one llm_audit row.
    c.bench_function("audited_generate", |b| {
        let inner: Arc<dyn InferenceBackend> = Arc::new(BenchMock);
        let db = Arc::new(Database::new(":memory:").expect("in-memory db"));
        let audited = AuditedInference::new(inner, db.clone());
        let ctx = CallContext::new("notes_summarize")
            .with_agent("bench_agent")
            .with_notes(vec!["n1".into(), "n2".into()])
            .with_template("summarize@v1");
        let req = make_request();
        b.iter(|| {
            rt.block_on(async {
                let res = audited.generate_audited(&req, &ctx).await.unwrap();
                criterion::black_box(res)
            })
        });
    });

    // Audited but every iteration starts with a fresh DB. Isolates the
    // append_event prev_hash lookup cost (linear vs constant in chain length
    // is a real concern; this verifies it's constant).
    c.bench_function("audited_generate_growing_chain_1k", |b| {
        let inner: Arc<dyn InferenceBackend> = Arc::new(BenchMock);
        let db = Arc::new(Database::new(":memory:").expect("in-memory db"));
        let audited = AuditedInference::new(inner, db.clone());
        let ctx = CallContext::new("notes_summarize")
            .with_agent("bench_agent")
            .with_template("summarize@v1");
        let req = make_request();

        // Pre-seed the chain with 1000 events to test scaling
        rt.block_on(async {
            for _ in 0..1000 {
                let _ = audited.generate_audited(&req, &ctx).await;
            }
        });

        b.iter(|| {
            rt.block_on(async {
                let res = audited.generate_audited(&req, &ctx).await.unwrap();
                criterion::black_box(res)
            })
        });
    });
}

criterion_group!(benches, audit_overhead);
criterion_main!(benches);
