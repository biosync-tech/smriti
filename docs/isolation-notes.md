# Request-Path Isolation Notes

## LLM Call Isolation (v0.2.0)

### Current Implementation

`BackendAbstractor` (in `src/features/schema_formation.rs`) wraps a `SharedBackend` so sync `db.execute` can call an async LLM **without starting a second Tokio runtime**:

```rust
std::thread::spawn(move || handle.block_on(llm_abstract(backend.as_ref(), &input)))
    .join()
```

`block_on` runs on the **worker thread**, not nested on the request runtime. The caller still waits for the join. There is still no second runtime.

### What This Achieves

- LLM call can be interrupted if the Tokio runtime shuts down
- Other Tokio tasks can run concurrently during the LLM call
- Panics in the LLM task are caught and converted to errors

### What This Does NOT Achieve

- The **request path is still blocked** waiting for schema formation to complete
- HTTP API calls to `POST /api/v1/consolidation/run` will timeout if LLM is slow
- CLI `smriti consolidate` will hang if LLM hangs

### True Off-Request-Path Isolation

To achieve true isolation, consolidation would need to:

1. Spawn the entire `run_consolidation_pass` in a background task
2. Return immediately with a job ID
3. Allow querying job status via separate endpoint
4. Commit results asynchronously when complete

This is a significant refactor requiring:
- Job queue or task tracking table
- Async commit mechanism (SQLite connection cannot cross thread boundaries easily)
- Status polling API
- Cancellation mechanism

### Recommendation for v1.0

**Conservative policy (default)** is the safe path:
- Flags proposals for review
- Never auto-promotes
- Human approval via `smriti approve` can run asynchronously (user-initiated)
- No timeout risk (user decides when to approve)

**Standard/Aggressive policies** should be used only:
- In CLI context (user can wait)
- With fast local LLMs (< 5s response time)
- Or in cron jobs where timeout is not a concern

For production web API with Standard/Aggressive, implement the background job queue first.

## References

- WikiSkill paper notes schema formation happens "offline" (not synchronous with agent execution)
- CLS consolidation in neuroscience happens during sleep/downtime, not during task execution
- Smriti's Conservative policy aligns with this: flag during runtime, consolidate offline
