# Consolidation Proxy Gating

**Status: implemented** (retrieve-context proxy). Conservative policy remains the healthcare default.

This is a **retrieval QA proxy**, not WikiSkill task-accuracy gating. Audit rows always say which signal was used.

## What it does

When Standard or Aggressive would auto-commit a schema proposal:

1. **Held-out queries** — sample distinct `note_access_log.query_context` values that previously hit one or more source episodes.
2. **Without vs with** — run a small FTS retrieve for each query, then score groundedness/coverage against the cluster. The candidate schema text is scored as an extra document; it is **not** inserted into `notes` for the test.
3. **Accept or flag** — accept only if mean groundedness lift ≥ `proxy.min_delta` (default 0.05). Otherwise persist the proposal as `flagged_for_review` (events only — live retrieval cannot see it).
4. **Unavailable** — no `query_context` samples ⇒ flag for human review. Never invent a pass.

Conservative policy **never** calls the proxy.

```bash
smriti consolidate --policy conservative --apply
smriti proposals
smriti approve <proposal_id|episode_id>
smriti reject <proposal_id|episode_id> -r "Not a meaningful pattern"
```

## Isolation

Pending proposals live in `consolidation_events.reason` as `SCHEMA_PROPOSAL ` + JSON. They are not `notes` rows. `retrieve_context` and `notes_search_semantic` cannot leak a half-formed wiki (WikiSkill ablation: 63.7% → 60.9% when inference sees the wiki during training).

## Honest disclaimer

**This is not WikiSkill `R(Tval,k) > Rbest`.**

WikiSkill (arXiv:2608.27454) gates abstractions on held-out **task trajectories**. Smriti has no labelled `y_i` set. The proxy tests whether a candidate abstract would improve **local retrieval coverage** for logged queries.

| WikiSkill | Smriti proxy |
|-----------|--------------|
| Task-level accuracy (e.g. Minecraft build success) | Retrieval groundedness / coverage |
| Multi-turn agent traces | Single-query context assembly |
| `gating=task_accuracy` | `gating=human_approved` or `gating=proxy_retrieve_accepted` |

`consolidation_events.reason` always includes the gating tag and, for proxy accepts, `not WikiSkill task-accuracy`.

## Event logging

```sql
SELECT note_id, event_type, reason, created_at
FROM consolidation_events
WHERE event_type IN ('flagged_for_review', 'promoted_to_schema', 'schema_proposal_rejected')
ORDER BY created_at DESC;
```

Typical reasons:

- `gating=human_approved by=…`
- `gating=proxy_retrieve_accepted n_queries=… mean_delta=… (retrieve-context proxy, not WikiSkill task-accuracy)`
- `gating=proxy_retrieve_rejected …`
- `llm_unavailable` / `llm_failed` — cluster flagged; extractive text is never labeled as LLM output

## Healthcare / compliance

Use Conservative (default). Zero auto-promote. Reject and accept are append-only; episodes are never deleted.

## Research

- WikiSkill, arXiv:2608.27454 — architecture reference (not a library)
- Graph-Based Memory Survey, arXiv:2602.05665 — hybrid retrieval
- CLS, McClelland 1995 — episodes → schemas via replay
