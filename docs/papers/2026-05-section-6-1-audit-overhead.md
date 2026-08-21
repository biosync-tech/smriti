# §6.1 Audit overhead

> **Status:** First-pass draft with measured numbers from `cargo bench --bench audit_overhead` on the authors' development hardware (May 2026, Apple Silicon, in-memory SQLite). Numbers will be re-run on a clean machine before camera-ready submission.

---

We measure the cost of enforcing invariants I1 (hash-chained events) and I3 (audit denormalization) on the LLM-call request path. The hypothesis under test is whether the audit layer adds overhead small enough to be deployable in production: specifically, that audit cost is constant in chain length and small relative to the latency of the underlying LLM call.

## Setup

We compare three configurations against an in-process deterministic mock LLM that returns a fixed 64-token response with no inference latency. Isolating against a synthetic backend is intentional: it produces a *floor* for total request latency and lets us measure audit cost in absolute terms rather than as a fraction of inference time. Real-world audit cost is bounded above by what we report.

- **Baseline.** The mock is invoked directly through the `InferenceBackend` trait; no audit code runs.
- **Audited (fresh chain).** The mock is wrapped in `AuditedInference` against a fresh in-memory database. Each call writes one events row (with the I1 hash chain) and one denormalized `llm_audit` row.
- **Audited (1k-event chain).** Same as the previous configuration, but the database is pre-seeded with 1,000 audit events so that each measured call performs a `prev_hash` lookup against a non-trivial chain. This isolates whether chain-length amortizes into per-call cost.

All three configurations are exercised through the criterion benchmarking harness [CITE: bhargava-2014-criterion] with 100 sampled iterations per configuration after a 3-second warm-up. Hardware is an Apple M-series laptop with 32 GB RAM; SQLite runs in-memory with WAL disabled (the latter is the `:memory:` path's default). Per-call hash computation uses the `sha2` crate.

## Results

| Configuration | Mean per-call latency | 95% confidence interval |
|---|---:|---:|
| Baseline (no audit) | 59.8 ns | [59.1 ns, 60.5 ns] |
| Audited, fresh chain | 19.0 µs | [18.7 µs, 19.5 µs] |
| Audited, 1k-event chain | 19.9 µs | [19.2 µs, 21.2 µs] |

The audit layer adds approximately **19 µs of fixed overhead per call**. The cost is dominated by two `INSERT` statements (one to `events`, one to `llm_audit`) and three SHA-256 computations (request hash, response hash, event hash). Pre-seeding the chain with 1,000 prior events increases per-call cost by 0.9 µs (~5%, within the observed sample variance) — confirming that the `prev_hash` lookup is `O(1)` in chain length, as expected from the indexed `ORDER BY ingestion_time DESC LIMIT 1` query path.

## Interpretation

For a typical LLM call against a local Ollama instance running an 8B-parameter model, end-to-end latency is in the 30–200 ms range depending on prompt length and decoding budget. Audit overhead at 19 µs represents **0.01–0.06%** of total request time — orders of magnitude below the noise floor of any production LLM workload. For cloud-hosted inference, where network round-trip alone is typically 10–50 ms, the relative overhead is even smaller.

The constancy of overhead in chain length is the property that matters for long-running deployments. A naïve implementation could degrade quadratically: O(n) per call to walk the entire chain, summing to O(n²) over the system's lifetime. Smriti's design avoids this by storing only a pointer to the immediate predecessor (`prev_hash`) and indexing on `ingestion_time DESC`, producing `O(log n)` lookup that is dominated by constant-factor SQLite overhead in practice. We confirm empirically that 1,000 prior events do not change per-call cost meaningfully.

The 19 µs figure is, however, an *audit-floor* measurement: the mock backend returns instantly. In a real deployment, both audit and LLM paths execute concurrently in the request handler. Whether the audit work parallelizes well with the inference call (which is dominated by GPU compute or remote network I/O) is a future-work question. The conservative interpretation — that the entire 19 µs is added serially to the LLM call — is the one used here.

## Threats to validity

Three caveats:

1. **In-memory SQLite.** Real deployments use file-backed SQLite with WAL mode. We expect WAL-mode `INSERT` to be 2–5× slower than the in-memory case. Even at 100 µs per call, audit remains ≪1% of LLM latency.
2. **Synthetic mock.** The mock is intentionally deterministic and zero-latency. A real local LLM introduces tens of milliseconds of variance per call that would dominate any audit-overhead signal in absolute timing terms. We argue the absolute floor (~19 µs) is the relevant number, not the audit-as-percentage-of-mock figure.
3. **Single-process workload.** All measurements use a single benchmark thread. Concurrent multi-agent workloads contend on the SQLite write lock and may show different per-call cost. We expect contention to scale linearly with concurrent writers up to the WAL checkpoint frequency; analysis of that regime is left for future work alongside Smriti's planned multi-agent grant model (§5).
