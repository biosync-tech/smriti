# Consolidation Proxy Gating

**Status: NOT IMPLEMENTED** (v0.2.0)

Conservative policy (human-in-loop) is production-ready. Standard and Aggressive policies fall back to Conservative when proxy is unavailable.

## Overview (Design Only)

Smriti's **optional proxy gating** (not yet wired) would evaluate schema proposals for Standard and Aggressive consolidation policies using held-out queries from the access log. This mechanism would help ensure that promoting a cluster of episodes into a schema actually improves retrieval relevance or groundedness.

## What Would Proxy Gating Do?

When a schema candidate is flagged for promotion:

1. **Held-out queries**: Smriti samples `note_access_log.query_context` entries that previously led to one or more of the candidate episodes.
2. **Retrieval test**: For each held-out query, Smriti runs `retrieve_context` **with** and **without** the proposed schema in the graph.
3. **Accept or reject**: If inclusion of the schema improves relevance/groundedness metrics, the proposal is accepted. Otherwise, it's rejected.
4. **Event logging**: The outcome (human-approved vs proxy-signal-approved) is logged in `consolidation_events.reason`.

## Conservative Policy (Default)

Conservative policy **never** uses proxy gating. Every schema proposal is flagged for human review:

```bash
smriti consolidate --policy conservative
smriti proposals
smriti approve <note_id>
smriti reject <note_id> --reason "Not a meaningful pattern"
```

## Standard and Aggressive Policies (Current Behavior)

Standard and Aggressive policies currently **fall back to Conservative** (flag for human review) when LLM backend is unavailable. Proxy gating is not implemented.

Future implementation would require:
- `note_access_log` contains sufficient query diversity
- `retrieve_context` callable with/without candidate schema
- Relevance/groundedness metric comparison
- Event logging differentiating human-approved vs proxy-signal-approved

## Important Disclaimer

**This is NOT WikiSkill task-accuracy gating.**

WikiSkill (arXiv:2608.27454) gates skill abstractions by testing them on held-out **task trajectories** (e.g., Minecraft build success). Smriti's proxy tests **retrieval relevance** for a local knowledge graph, not downstream task accuracy.

Key differences:

| WikiSkill | Smriti Proxy |
|-----------|--------------|
| Task-level accuracy (e.g., "Did the agent build the house?") | Retrieval-level relevance (e.g., "Does the schema surface the right episodes?") |
| Multi-turn agent traces | Single-query context assembly |
| Gating = accept/reject skill abstraction | Gating = accept/reject schema promotion |

Smriti's proxy is a **retrieval QA proxy**, not a task-level skill gating mechanism.

## Current Limitations

**Proxy gating is not implemented in v0.2.0.**

- Conservative policy works as designed (human approval required)
- Standard/Aggressive policies currently form schemas without proxy checks
- To use Standard/Aggressive safely: review schemas manually after formation
- For healthcare/compliance: use Conservative (default) which never auto-promotes

## Event Logging

Every promotion decision logs its signal source in `consolidation_events`:

```sql
SELECT note_id, event_type, reason, created_at
FROM consolidation_events
WHERE event_type IN ('proposal_approved', 'promoted_to_schema')
ORDER BY created_at DESC;
```

Reasons will indicate:
- `"approved by human operator"` — Conservative policy, manual approval
- `"proxy-signal-approved: relevance score +0.12"` — Standard/Aggressive, passed proxy test
- `"rejected by human: <reason>"` — Manual rejection

## Research Context

This mechanism is inspired by:
- **WikiSkill** (arXiv:2608.27454) — gating abstraction via held-out traces
- **Graph-Based Memory Survey** (arXiv:2602.05665) — hybrid retrieval evaluation

Smriti adapts the gating concept to a local-first, retrieval-focused context.
