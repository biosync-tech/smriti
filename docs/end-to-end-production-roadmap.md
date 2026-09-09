# End-to-End Production Roadmap
## Making Smriti Production-Ready with Google PG-Level Features

**Status:** Phase 1.1 Complete (Generic Edge Attributes)  
**Created:** 2026-09-09  
**Goal:** Complete implementation of competitive strategy against Google's Procedural Graphs

---

## ✅ Phase 1.1: Generic Edge Attributes (COMPLETED)

**Research Ref:** Google Procedural Graphs arXiv:2609.09153 §3.1  
**Implementation:** Smriti's typed edges now support Google PG's Φ (phi) — arbitrary JSON attributes on edges.

### Changes Shipped:
1. **`src/models/link.rs`**: Added `attributes: Option<serde_json::Value>` field to `Link` struct
   - Stores `{condition, guidance, pitfalls, ...}` as JSON
   - Added `Link::with_attributes()` constructor
   - Example: `{"condition": "runway < 6mo", "guidance": "submit early IND", "pitfalls": "no rolling submission"}`

2. **`src/storage/db.rs`**: Migration 011 — `ALTER TABLE links ADD COLUMN attributes TEXT`

3. **`src/storage/operations.rs`**: 
   - Added `insert_link_with_attributes_on_conn()` to support attribute persistence
   - Maintains backward compat via existing `insert_link_on_conn()` wrapper

### Next: Expose via MCP
MCP tool `notes_link` needs optional `attributes` parameter. See Phase 1.2 below.

---

## 🚧 Phase 1.2: Graph Guidance MCP Tool (IN PROGRESS)

**Research Ref:** Google PG's Ψ (psi) — generative guidance from local graph neighborhoods  
**Goal:** Ship `notes_graph_guidance` MCP tool

### Spec:
```json
{
  "name": "notes_graph_guidance",
  "parameters": {
    "note_id": "abc123",
    "hop_count": 2
  },
  "result": {
    "local_context": [
      {"note_id": "...", "title": "...", "link_type": "semantic", "attributes": {"guidance": "..."}},
      ...
    ],
    "suggested_next_actions": [
      "Create link to X",
      "Update attribute Y"
    ]
  }
}
```

### Implementation Plan:
1. **File:** `src/mcp/handlers.rs`  
   - Add `handle_notes_graph_guidance(params)` handler
   - BFS from `note_id` for `hop_count` hops
   - Collect {note, link_type, attributes} for neighborhood
   - Return structured guidance

2. **File:** `src/mcp/server.rs`  
   - Register `notes_graph_guidance` tool in tool catalog
   - Wire handler to dispatcher

3. **Test:**
   ```bash
   # Create note A with links to B, C
   # B→C link has attributes: {"condition": "precedes", "guidance": "check B before C"}
   # Query: notes_graph_guidance(A, hop_count=2)
   # Expected: return B, C, edge attributes
   ```

---

## 🚧 Phase 1.3: Rejection Memory Loop

**Research Ref:** Google PG §4.3 — offline refinement with rejection memory  
**Goal:** Consolidation rejections feed back into scorer to prevent re-flagging

### Spec:
When a consolidation proposal is **rejected**, record:
- Note ID
- Consolidation score at rejection time
- Rejection reason (human-provided)
- Timestamp

Scorer **reads** rejection memory:
- If note was rejected in last 90 days → suppress from new proposals
- After 90 days → re-evaluate (score may have changed)

### Implementation Plan:
1. **File:** `src/storage/db.rs` — Migration 013  
   ```sql
   CREATE TABLE IF NOT EXISTS consolidation_rejections (
       id TEXT PRIMARY KEY,
       note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
       score_at_rejection REAL NOT NULL,
       reason TEXT NOT NULL,
       rejected_at TEXT NOT NULL,
       suppress_until TEXT NOT NULL  -- rejected_at + 90 days
   );
   CREATE INDEX idx_rejections_note ON consolidation_rejections(note_id, suppress_until DESC);
   ```

2. **File:** `src/features/consolidation.rs`  
   - Update `score_notes_for_consolidation()` to filter out notes in rejection memory
   - Add `record_rejection(note_id, reason)` function

3. **File:** `src/cli/commands.rs` (already has `smriti reject <proposal_id>`)  
   - Update `handle_reject_proposal()` to call `record_rejection()`

4. **File:** `src/api/routes/consolidation.rs`  
   - Update `POST /api/v1/consolidation/proposals/:id/reject` to persist rejection

---

## 🚧 Phase 2.1: ConsolidationPolicy Extensions

**Current:** `Conservative` only (flag, never auto-promote)  
**Goal:** Add `Standard` and `Aggressive` modes per competitive strategy doc

### Enum Extension (`src/features/consolidation.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsolidationPolicy {
    /// Never auto-promote. Only flag for human review. (Default, compliance-safe)
    Conservative,
    /// Auto-promote on threshold. Flag low-scorers. Require human approval for archival.
    Standard,
    /// Auto-promote + auto-archive flagged items after grace period (e.g. 30 days).
    Aggressive,
}
```

### Implementation:
1. **Update `run_consolidation(policy, agent_id)`** to branch on policy:
   - `Conservative`: existing behavior (flag only)
   - `Standard`: if `consolidation_score > 0.75` AND ≥3 episodes cluster → auto-create schema
   - `Aggressive`: Standard + auto-archive notes with `score < 0.2` and `last_accessed_at > 30 days ago`

2. **MCP tool:** `notes_consolidate` already accepts `policy` param — just wire new variants

3. **Config:** `config.toml` defaults to `Conservative`

---

## 🚧 Phase 2.2: Validation Split & Gating

**Research Ref:** Google PG §5 — validation split prevents overfitting  
**Goal:** Hold out 20% of notes; consolidation proposals validated against hold-out

### Spec:
1. On first consolidation run, randomly sample 20% of notes → mark as `validation_set = true`
2. Consolidation scorer **trains** on remaining 80%
3. Before **accepting** a schema promotion:
   - Run retrieval task on validation set
   - Measure: precision@5, graph coherence
   - If validation metrics drop → reject promotion

### Implementation Plan:
1. **Migration 014:** `ALTER TABLE notes ADD COLUMN validation_set INTEGER DEFAULT 0`

2. **File:** `src/features/consolidation.rs`  
   - Add `assign_validation_split(db, split_ratio=0.2)`
   - Update `score_notes_for_consolidation()` to filter `validation_set = 0`

3. **File:** `src/features/validation_metrics.rs` (NEW)  
   ```rust
   pub struct ValidationMetrics {
       pub retrieval_precision_at_5: f32,
       pub graph_coherence: f32,
       pub access_reduction: f32,
   }
   pub fn compute_validation_metrics(db, proposal_id) -> ValidationMetrics { ... }
   ```

4. **File:** `src/cli/commands.rs`  
   - Update `smriti approve <proposal_id>` to run validation before committing

---

## 🚧 Phase 2.3: Validation Metrics

Three metrics from Google PG paper (Table 3):

### 1. RetrievalPrecision@5
- Query: 20 held-out notes with known ground-truth links
- Measure: how many of top-5 search results match ground truth
- Threshold: > 0.80 to accept promotion

### 2. GraphCoherence
- Measure: average clustering coefficient after schema merge
- Higher = more tightly connected subgraphs
- Threshold: coherence must not drop > 5% post-promotion

### 3. AccessReduction
- Measure: % reduction in `note_access_log` writes after consolidation
- Target: 30–60% reduction (episodes accessed via schema, not individually)

**Implementation:** All three computed in `validation_metrics.rs`, returned as struct

---

## 🚧 Phase 3: Documentation & Marketing

### README Update
**File:** `/workspace/README.md`

Add after existing research citations:

```markdown
## Production Validation

> "Procedural graphs outperform memory baselines across 7 benchmarks and 4 LLMs,  
> with a 23% improvement in task success rate and 41% reduction in redundant actions."  
> — Google Research, *Procedural Graphs*, arXiv:2609.09153, 2026

Smriti ships the **production-ready, compliance-first** version of Google's research prototype:
- ✅ Generic edge attributes (Google PG's Φ)
- ✅ Graph guidance from local neighborhoods (Google PG's Ψ)
- ✅ Rejection memory loop prevents re-flagging
- ✅ Validation gating before schema promotion
- ✅ Append-only event log + hash chain (Google PG has neither)
- ✅ Bi-temporal edges for audit trails (clinical trials requirement)
- ✅ Self-hosted — no cloud API dependency

See [`docs/competitive-strategy-google-pg.md`](docs/competitive-strategy-google-pg.md) for details.
```

### Landing Page Update
**File:** `/workspace/smriti-landing/index.html`

Add new section after "How It Works":

```html
<section id="research-foundation">
  <h2>Research Foundation</h2>
  <p>
    Smriti's architecture is validated by Google Research's 
    <a href="https://arxiv.org/abs/2609.09153">Procedural Graphs paper</a> (2026),
    which demonstrated 23% improvement in task success rate across 7 benchmarks.
  </p>
  <table class="comparison-table">
    <thead>
      <tr>
        <th>Feature</th>
        <th>Google PG<br/>(Research)</th>
        <th>Smriti<br/>(Production)</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td>Generic edge attributes</td>
        <td>✅ Φ (phi)</td>
        <td>✅ JSON attributes</td>
      </tr>
      <tr>
        <td>Graph guidance</td>
        <td>✅ Ψ (psi)</td>
        <td>✅ notes_graph_guidance</td>
      </tr>
      <tr>
        <td>Self-evolution</td>
        <td>✅ Offline loop</td>
        <td>✅ Consolidation + rejection memory</td>
      </tr>
      <tr>
        <td>Validation gating</td>
        <td>✅ Hold-out set</td>
        <td>✅ 20% validation split</td>
      </tr>
      <tr>
        <td>Immutable audit log</td>
        <td>❌ No</td>
        <td>✅ Hash-chained events</td>
      </tr>
      <tr>
        <td>Bi-temporal edges</td>
        <td>❌ No</td>
        <td>✅ valid_from / valid_until</td>
      </tr>
      <tr>
        <td>Compliance-ready</td>
        <td>❌ Research only</td>
        <td>✅ ICH E6(R3) aligned</td>
      </tr>
      <tr>
        <td>Self-hosted</td>
        <td>❌ Cloud API</td>
        <td>✅ Single binary, SQLite</td>
      </tr>
    </tbody>
  </table>
</section>
```

---

## 🧪 Phase 4: Integration Tests

**File:** `/workspace/tests/integration_google_pg_features.rs` (NEW)

```rust
#[tokio::test]
async fn test_edge_attributes_roundtrip() {
    // Create link with attributes
    let attrs = json!({"condition": "runway < 6mo", "guidance": "submit early"});
    let link = db.create_link_with_attributes("A", "B", LinkType::Semantic, Some(attrs)).unwrap();
    
    // Read back
    let retrieved = db.get_link(&link.id).unwrap();
    assert_eq!(retrieved.attributes.unwrap()["guidance"], "submit early");
}

#[tokio::test]
async fn test_graph_guidance_tool() {
    // Create A→B→C chain with attributes
    // Query notes_graph_guidance(A, hop_count=2)
    // Assert: returns B, C, attributes
}

#[tokio::test]
async fn test_rejection_memory_suppression() {
    // Score note → generates proposal
    // Reject proposal with reason
    // Re-run consolidation
    // Assert: note not in new proposals (suppressed for 90 days)
}

#[tokio::test]
async fn test_validation_gating() {
    // Assign validation split
    // Create schema promotion proposal
    // Approve with validation
    // Assert: metrics computed, promotion only if metrics > threshold
}
```

---

## 📝 Phase 5: CLAUDE.md Update

**File:** `/workspace/CLAUDE.md`

### Add to "Shipped" table:
| # | Item | Location |
|---|------|----------|
| 22| Generic edge attributes (Google PG Φ) | src/models/link.rs + Migration 011 |
| 23| Graph guidance MCP tool | src/mcp/handlers.rs + notes_graph_guidance |
| 24| Rejection memory loop | consolidation_rejections table + src/features/consolidation.rs |
| 25| ConsolidationPolicy Standard/Aggressive | src/features/consolidation.rs enum |
| 26| Validation split & gating | Migration 014 + src/features/validation_metrics.rs |

### Update "Research Anchors" table:
| Paper | arXiv ID | Key Finding for Smriti |
|-------|----------|------------------------|
| Google Procedural Graphs | 2609.09153 | Generic edge attributes (Φ) + graph guidance (Ψ) + validation gating improve task success 23% over memory baselines |

---

## 🎯 Success Criteria (Definition of Done)

- [x] Generic edge attributes shipped (Link.attributes)
- [ ] `notes_graph_guidance` MCP tool live
- [ ] Rejection memory prevents re-flagging
- [ ] Standard/Aggressive consolidation policies work
- [ ] Validation split assigned, metrics computed
- [ ] All integration tests pass (`cargo test --all`)
- [ ] README updated with Google PG validation quote
- [ ] Landing page comparison table live
- [ ] CLAUDE.md reflects shipped features
- [ ] CI green on main

---

## 🚀 Deployment Checklist

1. **Merge to main:** Once all features + tests pass
2. **Tag release:** `v0.2.0-google-pg-alignment`
3. **Publish crate:** `cargo publish` (crates.io)
4. **Update landing page:** Netlify auto-deploy on push
5. **Announce:**
   - HN: "Show HN: Smriti — Production-ready agent memory (validates Google's Procedural Graphs)"
   - Reddit /r/MachineLearning
   - X thread from @Biosync_ai

---

## Timeline (Production-Ready Sprint)

- **Week 1:** Phase 1.2–1.3 (graph guidance + rejection memory)
- **Week 2:** Phase 2.1–2.3 (policies + validation)
- **Week 3:** Phase 3 (docs + marketing)
- **Week 4:** Phase 4–5 (tests + polish)
- **Week 5:** Release + announce

**Target Ship Date:** 2026-10-14 (5 weeks)

---

## Notes

- **Transient build issue:** `clap_complete-4.6.0` requires `edition2024` feature. Not blocking (CI passes on main). Will resolve when Rust 1.84+ lands.
- **Phase 1.1 complete** means first competitive gap vs Google PG is closed. Remaining phases are incremental.
- **Conservative policy default** maintains compliance-first positioning for healthcare customers.
