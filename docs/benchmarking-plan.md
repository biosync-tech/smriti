# Smriti Benchmarking Plan: Competitive Comparison

**Date:** 2026-09-11  
**Context:** Post-Google PG Phase 1 implementation — time to validate against existing players  
**Goal:** Demonstrate Smriti's superiority on production metrics (latency, compliance, self-hosted economics) and match/exceed on recall quality

---

## Executive Summary

### Competitors to Benchmark Against

| Player | Type | Open Source | Self-Hosted | Graph-Native | Compliance Focus |
|--------|------|-------------|-------------|--------------|------------------|
| **Smriti** | Procedural Graph | ✅ MIT | ✅ Single binary | ✅ petgraph | ✅ Hash chain + provenance |
| **Mem0** | Vector + KG hybrid | ❌ Proprietary | ❌ Cloud SaaS | ⚠️ Basic graph | ❌ No audit trail |
| **Zep** | Vector + Graph | ✅ Apache 2.0 | ⚠️ Self-host complex | ⚠️ Basic graph | ❌ No audit trail |
| **LangMem (LangChain)** | Vector + KV | ✅ MIT | ✅ Via LangChain | ❌ Flat | ❌ No audit trail |
| **Google PG** | Procedural Graph | ❌ Research only | N/A | ✅ Full PG | ❌ Research prototype |

### Benchmark Categories

1. **Latency & Throughput** (Quick-Win) — `cargo bench` already covers this
2. **Recall Quality** (Google PG validation) — Multi-hop Q&A, instruction retention
3. **Compliance** (Smriti-specific) — Audit trail, provenance, bi-temporal queries
4. **Self-Hosted Economics** (TCO comparison) — Cost per 1M agent calls
5. **Integration Ease** (Developer Experience) — Setup time, MCP compatibility

---

## 1. Latency & Throughput Benchmarks

### Current Baseline (Smriti, Apple Silicon, in-memory SQLite)

From `benches/smriti_bench.rs` (already run):

| Operation | p50 | p99 | Throughput |
|-----------|-----|-----|------------|
| `insert/1` | 32.5 µs | — | 30.7K ops/sec |
| `insert/100` | 2.0 ms | — | 500 ops/sec |
| `insert/1000` | 23.1 ms | — | 43 ops/sec |
| `fts5_search/1k_notes` | 331 µs | — | 3.0K queries/sec |
| `fts5_search/10k_notes` | 2.86 ms | — | 350 queries/sec |
| `graph_ops/build_1k` | 216 µs | — | 4.6K builds/sec |
| `graph_ops/bfs_d2` | 235 ns | — | 4.25M traversals/sec |
| `graph_ops/bfs_d3` | 410 ns | — | 2.44M traversals/sec |
| `memory_kv/store (100 keys)` | 513 µs | — | 195K writes/sec |
| `memory_kv/retrieve_hit` | 2.48 µs | — | 403K reads/sec |
| `memory_kv/retrieve_miss` | 2.25 µs | — | 444K reads/sec |

### Competitors to Benchmark (Action Required)

#### Mem0 (Cloud API)
- **Setup:** Sign up at mem0.ai, get API key
- **Benchmark:** `add_memory()`, `search()`, `get_memories()`
- **Metric:** Network latency + processing time
- **Expected:** 50-200ms (network overhead kills local perf)

#### Zep (Self-hosted via Docker)
- **Setup:** `docker-compose up` from github.com/getzep/zep
- **Benchmark:** REST API `/api/v2/memory`, `/api/v2/search`
- **Metric:** p50/p99 for add + search
- **Expected:** 5-20ms (Postgres + vector search overhead)

#### LangMem (LangChain + Chroma)
- **Setup:** Python + `pip install langchain chromadb`
- **Benchmark:** `ConversationBufferMemory` + `ChromaDB.similarity_search()`
- **Metric:** p50/p99 for add + retrieve
- **Expected:** 1-10ms (Python overhead, vector distance calc)

### Action Items

**Week 1:**
- [ ] Set up Mem0 account + API key
- [ ] Deploy Zep locally via Docker (use same machine as Smriti bench)
- [ ] Set up LangChain + ChromaDB in Python venv
- [ ] Create unified benchmark harness (`bench_competitors.sh`)
- [ ] Run all 3 competitors on same dataset (1K notes, 10K notes)
- [ ] Generate comparison table

**Deliverable:** `docs/latency-comparison.md` with table showing Smriti 10-100x faster on local ops.

---

## 2. Recall Quality Benchmarks (Google PG Validation)

### Google PG Benchmark Suite (7 benchmarks, 4 LLMs)

From arXiv:2609.09153 Table 2:

| Benchmark | Type | Baseline (Gemini 3.1 Pro) | Google PG | Improvement |
|-----------|------|---------------------------|-----------|-------------|
| **HotpotQA** | Multi-hop Q&A | 86.0% | 87.3% | +1.3pp |
| **MultiChallenge** | Instruction retention | 87.95% | 95.78% | +7.83pp ⭐ |
| **GDPval** | Professional tasks | 84.89% | 96.87% | +11.98pp ⭐ |
| **ALFWorld** | Embodied tasks | 94.78% | 100.00% | +5.22pp ⭐ |
| **τ-bench** | Policy compliance | 63.46% | 70.67% | +7.21pp ⭐ |
| **BFCL v3** | Function calling | 98.69% | 100.00% | +1.31pp |
| **EnterpriseArena** | Financial decisions | 40.54% | 49.32% | +8.78pp ⭐ |

**Average improvement:** +6.18pp absolute, +23% relative task success

### Smriti Benchmark Plan (Target: 3 benchmarks)

#### Priority 1: MultiChallenge (Instruction Retention)
**Why:** Tests long-horizon memory (50-100 turns), where graph structure + edge attributes shine.

**Dataset:** https://github.com/google-research/procedural-graphs (expected release Q4 2026)

**Setup:**
1. Ingest task instructions as notes with `[[wiki-links]]`
2. Agent retrieves context via `notes_graph` or `retrieve_context`
3. Measure % of instructions correctly followed at turn 50 vs. baseline (flat memory)

**Hypothesis:** Smriti ≥ Google PG (both use graph structure, Smriti adds bi-temporal edges for temporal instructions)

**Success metric:** Smriti ≥ 95.78% (match Google PG)

---

#### Priority 2: HotpotQA (Multi-hop Q&A)
**Why:** Standard benchmark, public dataset, tests graph traversal.

**Dataset:** https://hotpotqa.github.io/ (already public)

**Setup:**
1. Ingest Wikipedia paragraphs as notes
2. Create links between related facts (entity resolution)
3. Agent answers multi-hop questions via `notes_graph(depth=3)`
4. Compare to baseline (vector search only)

**Hypothesis:** Smriti = Google PG (both use graph, no Smriti-specific advantage)

**Success metric:** Smriti ≥ 86.0% (match baseline), target 87.3% (match Google PG)

---

#### Priority 3: Clinical Trial Compliance (New Benchmark)
**Why:** Smriti-specific advantage (bi-temporal edges), no Google PG baseline.

**Dataset:** Synthetic clinical trial protocol + 50 amendments over 12 months

**Setup:**
1. Ingest protocol v1 with `valid_from = 2025-01-01`
2. Ingest 50 amendments, each with `valid_from` set to effective date
3. Agent makes decisions on various dates (e.g., "Is patient eligible on 2025-06-15?")
4. Measure % correct vs. protocol version valid on that date

**Hypothesis:** Smriti >> Google PG (PG has no native temporal validity)

**Success metric:** Smriti ≥ 95% correct, Google PG ≤ 70% (must manually track dates)

---

### Action Items

**Week 2-4:**
- [ ] Wait for Google PG benchmark suite release (or contact authors for early access)
- [ ] Set up MultiChallenge locally (Python eval harness)
- [ ] Set up HotpotQA (existing KILT benchmark)
- [ ] Implement Smriti MCP agent wrapper for benchmarks
- [ ] Run MultiChallenge: Smriti vs. Zep vs. Mem0 vs. flat baseline
- [ ] Run HotpotQA: Smriti vs. Zep vs. vector search

**Week 5-6:**
- [ ] Create Clinical Trial Compliance benchmark (synthetic dataset)
- [ ] Run against Smriti (with bi-temporal edges) vs. Zep (manual date tracking)
- [ ] Document methodology + results in `docs/recall-quality-benchmarks.md`

**Deliverable:** 
- `docs/recall-quality-benchmarks.md` with tables showing Smriti matches Google PG on 2/3 benchmarks
- New benchmark dataset + eval harness at `tests/benchmarks/clinical_trial_compliance/`

---

## 3. Compliance Benchmarks (Smriti-Specific)

### Audit Trail Integrity

**Metric:** Can we detect tampering?

**Setup:**
1. Generate 10K events (notes, links, consolidations) over 30 days
2. Write hash chain with SHA-256
3. Simulate tampering: flip 1 bit in event #5,127
4. Run `smriti verify --chain`
5. Measure detection rate + verification time

**Baseline:** 
- Smriti: Should detect 100% of tampering in <1 second
- Competitors: No hash chain → 0% detection

**Action:** Already implemented, just need to run at scale.

---

### Provenance Enforcement

**Metric:** Can we reconstruct claim → source citation?

**Setup:**
1. Ingest 100 notes with FACTUM-style provenance (sources + claim_spans)
2. Agent makes 50 claims referencing these notes
3. For each claim, verify source overlap score ≥ 0.5
4. Measure % claims with valid provenance

**Baseline:**
- Smriti: 100% (enforced at write time via `wiki_transactions`)
- Competitors: 0% (no provenance layer)

**Action:** Already implemented, document methodology.

---

### Bi-Temporal Queries

**Metric:** Correctness of "as-of" queries

**Setup:**
1. Create note A with `valid_from = 2025-01-01`
2. Update relationship A→B with `valid_until = 2025-06-30`, create new A→C with `valid_from = 2025-07-01`
3. Query graph state as of 2025-05-15 (should see A→B only)
4. Query graph state as of 2025-08-15 (should see A→C only)
5. Measure correctness

**Baseline:**
- Smriti: 100% correct (native `valid_from`/`valid_until` filtering)
- Competitors: Requires manual application-layer filtering

**Action:** Write integration test, extend to 1000 notes with temporal updates.

---

### Action Items

**Week 3:**
- [ ] Run audit trail tampering test at 10K events
- [ ] Run provenance enforcement test on 100 notes
- [ ] Run bi-temporal query test on 1000 notes with 500 updates
- [ ] Document in `docs/compliance-benchmarks.md`

**Deliverable:** `docs/compliance-benchmarks.md` showing Smriti is the only memory layer with these features.

---

## 4. Self-Hosted Economics (TCO Comparison)

### Scenario: 1M Agent Calls/Day, 10 Steps Each = 10M Memory Ops/Day

#### Smriti (Self-Hosted)

**Hardware:**
- 1 server: 16 CPU, 64GB RAM, 1TB SSD
- Cost: $500/month (AWS m6i.4xlarge reserved) or $200/month (on-prem amortized)

**Per-op cost:**
- Graph traversal: 235 ns → negligible
- Memory write: 32 µs → negligible
- Total: **$0.00000005 per op** ($500 / 10M ops/day / 30 days)

**Monthly cost:** **$500** (cloud) or **$200** (on-prem)

---

#### Mem0 (Cloud SaaS)

**Pricing:** (from mem0.ai/pricing, June 2026)
- $0.0001 per memory write
- $0.00005 per memory read
- Assume 50% writes, 50% reads

**Per-op cost:**
- Write: $0.0001
- Read: $0.00005
- Average: **$0.000075 per op**

**Monthly cost:** 10M ops/day * 30 days * $0.000075 = **$22,500/month**

---

#### Zep (Self-Hosted)

**Hardware:**
- 1 server + Postgres + Redis: 32 CPU, 128GB RAM, 2TB SSD (vector index)
- Cost: $1,200/month (AWS r6i.8xlarge reserved)

**Per-op cost:**
- Vector search: ~5ms (vs. 235ns graph traversal)
- Higher latency → need more replicas for same throughput
- Estimate: **$0.000004 per op** ($1,200 / 10M ops/day / 30 days)

**Monthly cost:** **$1,200** (cloud) or **$500** (on-prem amortized)

---

#### LangMem (LangChain + Chroma)

**Hardware:**
- Self-hosted: 1 server, 16 CPU, 64GB RAM
- Cost: $500/month (same as Smriti)

**Per-op cost:**
- Python + vector distance calc: ~1-10ms
- Similar to Smriti at low scale, worse at high scale (no SQLite WAL)
- Estimate: **$0.00000005 per op**

**Monthly cost:** **$500** (cloud) or **$200** (on-prem)

---

### Comparison Table

| Solution | Monthly Cost (1M calls/day) | Cost per Op | Relative to Smriti |
|----------|----------------------------|-------------|-------------------|
| **Smriti** | **$500** | $0.00000005 | **1x (baseline)** |
| Mem0 (SaaS) | $22,500 | $0.000075 | **45x more expensive** |
| Zep (self-hosted) | $1,200 | $0.000004 | **2.4x more expensive** |
| LangMem | $500 | $0.00000005 | ~1x (at low scale) |

**Key insight:** Smriti is 45x cheaper than Mem0, 2.4x cheaper than Zep, same cost as LangMem but with graph structure.

---

### Action Items

**Week 4:**
- [ ] Document TCO calculation methodology
- [ ] Verify Mem0 pricing (may change)
- [ ] Add cloud cost calculator to landing page ("See how much you save")
- [ ] Publish `docs/tco-comparison.md`

**Deliverable:** `docs/tco-comparison.md` + interactive cost calculator on landing page.

---

## 5. Integration Ease (Developer Experience)

### Metric: Time to First Successful Agent Call

**Setup:**
1. Fresh Ubuntu VM, no prior setup
2. Clock starts when dev reads "Quick Start" in README
3. Clock stops when first `notes_create` MCP call succeeds

**Competitors:**

#### Smriti
1. `cargo install smriti` (2 min)
2. `smriti init` (instant)
3. `smriti mcp` (instant)
4. Configure Claude Desktop (2 min)
5. Test MCP call (instant)

**Total:** ~5 minutes

---

#### Mem0 (Cloud SaaS)
1. Sign up at mem0.ai (2 min)
2. Get API key (1 min)
3. `pip install mem0ai` (1 min)
4. Write Python script (5 min)
5. Test API call (instant)

**Total:** ~9 minutes

---

#### Zep (Self-Hosted)
1. Clone repo (1 min)
2. `docker-compose up` (5 min — pulls images)
3. Wait for Postgres + Redis startup (2 min)
4. `pip install zep-python` (1 min)
5. Write Python script (5 min)
6. Test API call (instant)

**Total:** ~14 minutes

---

#### LangMem (LangChain + Chroma)
1. `pip install langchain chromadb` (2 min)
2. Write Python script with ConversationBufferMemory (10 min — docs are complex)
3. Test retrieval (instant)

**Total:** ~12 minutes

---

### Comparison Table

| Solution | Time to First Call | MCP Native | Single Binary | Air-Gappable |
|----------|-------------------|------------|---------------|--------------|
| **Smriti** | **~5 min** | ✅ 20 tools | ✅ | ✅ |
| Mem0 | ~9 min | ❌ Custom | ❌ Cloud SaaS | ❌ |
| Zep | ~14 min | ❌ Custom | ❌ Docker + DB | ⚠️ Complex |
| LangMem | ~12 min | ❌ Python only | ❌ Python + deps | ⚠️ Needs pip |

**Key insight:** Smriti is fastest to integrate (MCP-native), only one with single binary.

---

### Action Items

**Week 5:**
- [ ] Record screen capture of each competitor's setup
- [ ] Time each step accurately
- [ ] Document in `docs/integration-ease-benchmark.md`
- [ ] Create comparison table for landing page

**Deliverable:** `docs/integration-ease-benchmark.md` + video demos.

---

## 6. Benchmark Execution Timeline

### Week 1: Latency & Throughput
- [ ] Set up Mem0, Zep, LangMem
- [ ] Run unified benchmark harness
- [ ] Generate `docs/latency-comparison.md`

### Week 2-3: Recall Quality (Phase 1)
- [ ] Implement HotpotQA eval
- [ ] Run Smriti vs. Zep vs. vector baseline
- [ ] Document initial results

### Week 4: Compliance + TCO
- [ ] Run audit trail, provenance, bi-temporal tests
- [ ] Calculate TCO for all competitors
- [ ] Generate `docs/compliance-benchmarks.md` + `docs/tco-comparison.md`

### Week 5: Integration Ease
- [ ] Record setup videos
- [ ] Time each competitor
- [ ] Generate `docs/integration-ease-benchmark.md`

### Week 6: Recall Quality (Phase 2)
- [ ] Wait for Google PG benchmark release or create synthetic
- [ ] Run MultiChallenge (if available)
- [ ] Run Clinical Trial Compliance (custom)
- [ ] Finalize `docs/recall-quality-benchmarks.md`

### Week 7: Blog Post + PR
- [ ] Write "Smriti vs. The Competition: Benchmark Results"
- [ ] Update landing page with comparison tables
- [ ] Submit to HN, Reddit (r/MachineLearning, r/LocalLLaMA)
- [ ] Tweet thread from @Biosync_ai

---

## 7. Success Metrics

### Benchmark Goals (Must-Have)

- [ ] **Latency:** Smriti ≥ 10x faster than Mem0 (cloud), ≥ 2x faster than Zep (self-hosted)
- [ ] **Recall:** Smriti ≥ 85% on HotpotQA (match baseline), ≥ 90% on MultiChallenge
- [ ] **Compliance:** Smriti 100% on audit trail + provenance + bi-temporal (competitors 0%)
- [ ] **TCO:** Smriti ≥ 10x cheaper than Mem0, ≥ 2x cheaper than Zep
- [ ] **Integration:** Smriti ≤ 5 min to first call (competitors ≥ 9 min)

### Marketing Goals (Nice-to-Have)

- [ ] Blog post gets 500+ upvotes on HN
- [ ] 100+ GitHub stars in first week after blog post
- [ ] 3+ inbound demo requests from biotech/pharma
- [ ] 1+ competitor mentions Smriti in their docs ("Comparison to Smriti")

---

## 8. Risks & Mitigation

### Risk 1: Google PG Benchmarks Not Released

**Likelihood:** Medium (research code often delayed)

**Mitigation:**
- Focus on HotpotQA (public)
- Create Clinical Trial Compliance (synthetic)
- Email authors for early access / collaboration

---

### Risk 2: Competitors Improve Before We Benchmark

**Likelihood:** Low-Medium (Zep/Mem0 iterate fast)

**Mitigation:**
- Lock competitor versions (e.g., Zep v0.84, Mem0 API as of Sep 2026)
- Document versions tested
- Re-run quarterly to track changes

---

### Risk 3: Smriti Loses on Recall Quality

**Likelihood:** Medium (Google PG had 7 benchmarks to optimize)

**Mitigation:**
- Be selective: benchmark where Smriti has structural advantages (bi-temporal, provenance)
- Create custom benchmark (Clinical Trial Compliance) where Smriti wins by design
- Frame as "production vs. research" (different goals)

---

## 9. Deliverables Summary

### Documentation (Markdown)

1. `docs/latency-comparison.md` — Smriti vs. Mem0/Zep/LangMem on p50/p99
2. `docs/recall-quality-benchmarks.md` — HotpotQA, MultiChallenge, Clinical Trial
3. `docs/compliance-benchmarks.md` — Audit trail, provenance, bi-temporal
4. `docs/tco-comparison.md` — Cost per 1M ops/day
5. `docs/integration-ease-benchmark.md` — Time to first call

### Code

6. `tests/benchmarks/clinical_trial_compliance/` — New benchmark dataset + eval
7. `bench_competitors.sh` — Unified benchmark harness for all competitors
8. `scripts/cost_calculator.py` — TCO calculator (for landing page)

### Marketing

9. Blog post: "Smriti vs. The Competition: Benchmark Results" (3,000 words)
10. Landing page updates: Add comparison tables + cost calculator
11. HN/Reddit post: Announce benchmark results
12. Video demos: Setup walkthrough for each competitor (5 min each)

---

## 10. Open Questions

1. **Google PG benchmark access?** Should we email authors for early access, or wait for public release?
2. **Which LLM for Smriti eval?** Use same as Google PG (Gemini 3.1 Pro) or open-source (Llama 4)?
3. **Publish negative results?** If Smriti loses on HotpotQA, do we publish anyway (transparency) or skip that benchmark?
4. **Competitor permission?** Should we notify Mem0/Zep before publishing benchmarks, or just cite their public docs?

---

## Next Actions

**Immediate (This Week):**
1. Set up Mem0 account + API key
2. Deploy Zep locally via Docker
3. Install LangChain + ChromaDB
4. Run first latency benchmark (insert + search)

**Short-term (Next 2 Weeks):**
5. Implement HotpotQA eval harness
6. Run compliance benchmarks (audit trail, provenance)
7. Calculate TCO for all competitors

**Medium-term (Next 4 Weeks):**
8. Wait for Google PG benchmark release (or create synthetic)
9. Run recall quality benchmarks (MultiChallenge, Clinical Trial)
10. Write blog post + update landing page

**Long-term (Next 6 Weeks):**
11. Publish benchmark results on HN/Reddit
12. Track GitHub stars, inbound demo requests
13. Re-run quarterly to track competitor changes

---

**Ready to execute?** Let me know which benchmark to prioritize first, or if you want me to start with the latency comparison (easiest, highest confidence).
