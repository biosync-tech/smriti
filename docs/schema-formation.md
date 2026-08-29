# Schema formation (WikiSkill-mapped)

Reference: WikiSkill, arXiv:2608.27454 (Aug 2026). This is a paper, not a library. Smriti implements the architecture, not Trace2Skill / EvoSkill / SkillOpt.

## Mapping

| WikiSkill | Smriti |
|-----------|--------|
| Raw Layer | `Note` episode + `note_access_log` |
| Wiki pattern pages | `Note` schema — created only after a gate |
| Wiki Maintainer | `src/features/schema_formation.rs` |
| Catalog | Wiki-links from a schema to its source episodes |
| Evolution log | `consolidation_events` |
| PURPOSE.md | `schema_sources` |

WikiSkill's ablation found that giving the *inference* consumer the wiki during the consolidation pass *hurt* quality (63.7% → 60.9%). Translation: `retrieve_context` and `notes_search_semantic` never see a proposal. Proposals are events, not notes, until accept.

## Policies

- **Conservative** (default, healthcare/compliance): every cluster is `flagged_for_review`. A human runs `smriti consolidate --accept <id>` or the REST/MCP equivalent.
- **Standard / Aggressive**: optional retrieve-context **proxy**. Sample recent `query_context` values from the source episodes. Score assembled context with vs without the candidate abstract (token groundedness + cluster coverage). Commit only if mean Δ > 0.05. If there is no held-out query text, do not invent a score — leave the proposal flagged.

This proxy is **not** WikiSkill's `R(Tval,k) > Rbest`. Smriti has no labelled validation set. The audit `reason` always says which signal was used (`gating=human_approved` vs `gating=proxy_retrieve_accepted`) and that the proxy is not task-accuracy gating.

## Isolation and rollback

- One schema (create or patch) per proposal.
- Rejecting a proposal writes `schema_proposal_rejected` and does not mutate committed schema notes. Skills/proposals roll back; the wiki that already landed does not.
- Episodes stay queryable. `parent_schema_id` is set on accept. Nothing is deleted.

## CLI

```bash
smriti consolidate                  # dry-run, conservative
smriti consolidate --apply
smriti consolidate --explain <id>   # score + lineage
smriti consolidate --accept <proposal_id>
smriti consolidate --reject <proposal_id> -r "too broad"
smriti consolidate --llm --apply    # Ollama if configured; else flag only
```
