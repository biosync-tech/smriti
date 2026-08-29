# Implementation Prompt: SKILL.state-Inspired Memory Semantics

**Context:** SKILL.state (arXiv:2501.XXXXX) is a Google Research paper demonstrating 16–75x token reduction at long horizons by replacing append-only conversation history in agent runtimes with explicit, mutable, schema-validated JSON state (Σt). Their error taxonomy shows 68% of small-model failures are "premature state overwrite/deletion" — LLMs omitting existing keys when attempting to update state.

**Smriti's role:** Smriti is NOT a SKILL.state runtime — that's the calling agent framework's job (Cowork, Claude Code, etc.). Smriti is the *storage layer* that agent runtimes write to. However, three concrete gaps exist that SKILL.state's findings make actionable:

1. Smriti's `ConflictPolicy::Merge` exists but lacks deterministic JSON merge-patch semantics.
2. No JSON-schema validation on `agent_memory` writes to catch malformed patches.
3. No documentation showing agents how to *use* `agent_memory` as working state instead of replaying `tool_logs`.

**This implementation prompt addresses items 1 and 2.** Item 3 is a docs/pattern-library task, not code.

---

## Part 1: Deterministic JSON Merge-Patch for `ConflictPolicy::Merge`

### Current State

```21:40:src/models/agent.rs
pub enum ConflictPolicy {
    /// Last write wins — overwrites without history. Default for backward compat.
    #[default]
    Overwrite,
    /// Reject the update if a value already exists for this key.
    Reject,
    /// Archive old value to memory_history, then store new value.
    VersionAndKeep,
    /// Mark old value as superseded (timestamp), then store new value.
    Invalidate,
}
```

The `Merge` variant does not exist. Current `store_memory` implementation (lines 455–551 of `src/storage/operations.rs`) treats all four policies as whole-value overwrites, with the only difference being whether old values are archived first.

### Problem

SKILL.state's own error taxonomy: 68% of Gemma-4-31B failures were "premature state overwrite/deletion" — the model reconstructing state from scratch and omitting existing keys. Smriti cannot prevent the calling LLM from sending a partial value, but it *can* refuse to trust that partial value as the whole truth. Deterministic server-side merge-patch semantics (Σt+1 = Σt ⊕ ΔΣt) close this exact failure class.

### Implementation

1. **Add `ConflictPolicy::Merge` to the enum:**

```rust
/// Field-level JSON merge-patch (RFC 7396 semantics):
/// - Non-null fields in the update are added/replaced in the existing value.
/// - `null` fields in the update delete the corresponding key from the existing value.
/// - Nested objects are recursively merged (not replaced).
/// - Arrays are replaced wholesale (merge-patch does not define array merging).
/// SKILL.state ref: closes "premature state overwrite/deletion" error class.
Merge,
```

2. **Update `ConflictPolicy::parse` in `src/models/agent.rs`:**

```rust
pub fn parse(s: &str) -> Self {
    match s {
        "overwrite" => ConflictPolicy::Overwrite,
        "reject" => ConflictPolicy::Reject,
        "version_and_keep" => ConflictPolicy::VersionAndKeep,
        "invalidate" => ConflictPolicy::Invalidate,
        "merge" => ConflictPolicy::Merge,
        _ => ConflictPolicy::Overwrite,
    }
}
```

3. **Add a `json_merge_patch` helper function in `src/storage/operations.rs`:**

Implement RFC 7396 JSON Merge Patch semantics. Reference implementation logic:

```rust
/// Apply JSON Merge Patch (RFC 7396) semantics.
/// - For each field in `patch`:
///   - If value is `null`, delete the field from `base`.
///   - If value is an object and `base` has an object at that key, recursively merge.
///   - Otherwise, replace the field in `base` with the value from `patch`.
/// - Arrays are replaced wholesale (RFC 7396 does not define array merging).
fn json_merge_patch(base: &serde_json::Value, patch: &serde_json::Value) -> serde_json::Value {
    match (base, patch) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(patch_map)) => {
            let mut result = base_map.clone();
            for (key, patch_value) in patch_map {
                if patch_value.is_null() {
                    result.remove(key);
                } else if let Some(base_value) = base_map.get(key) {
                    result.insert(key.clone(), json_merge_patch(base_value, patch_value));
                } else {
                    result.insert(key.clone(), patch_value.clone());
                }
            }
            serde_json::Value::Object(result)
        }
        _ => patch.clone(),
    }
}
```

4. **Add `ConflictPolicy::Merge` case to `store_memory` in `src/storage/operations.rs`:**

Insert this case between `Invalidate` and the closing brace (after line 547). Logic:

```rust
ConflictPolicy::Merge => {
    // Fetch existing value if present
    let existing_value: Option<serde_json::Value> = conn
        .query_row(
            "SELECT value FROM agent_memory
             WHERE agent_id = ?1 AND namespace = ?2 AND key = ?3",
            params![memory.agent_id, memory.namespace, memory.key],
            |row| {
                let json_str: String = row.get(0)?;
                Ok(serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null))
            },
        )
        .optional()?;

    let merged_value = if let Some(base) = existing_value {
        json_merge_patch(&base, &memory.value)
    } else {
        // No existing value — patch becomes the initial value
        memory.value.clone()
    };

    let merged_json = serde_json::to_string(&merged_value)?;

    conn.execute(
        "INSERT INTO agent_memory (id, agent_id, namespace, key, value, created_at, updated_at, ttl_seconds)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(agent_id, namespace, key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at,
            ttl_seconds = excluded.ttl_seconds",
        params![
            memory.id, memory.agent_id, memory.namespace, memory.key,
            merged_json, memory.created_at.to_rfc3339(),
            memory.updated_at.to_rfc3339(), memory.ttl_seconds,
        ],
    )?;
}
```

5. **Write integration tests in `src/storage/operations.rs` (bottom of file, in the `#[cfg(test)]` module):**

```rust
#[test]
fn test_conflict_policy_merge() {
    let db = Database::new(":memory:").unwrap();

    // Initial write
    let req = CreateMemoryRequest {
        namespace: Some("test".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"a": 1, "b": {"c": 2}}),
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Merge,
    };
    db.store_memory("agent1", req).unwrap();

    // Merge patch: update "a", add "d", delete "b.c" via null
    let patch = CreateMemoryRequest {
        namespace: Some("test".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"a": 10, "b": {"c": null}, "d": 3}),
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Merge,
    };
    db.store_memory("agent1", patch).unwrap();

    let result = db.get_memory("agent1", "test", "state").unwrap();
    let expected = serde_json::json!({"a": 10, "b": {}, "d": 3});
    assert_eq!(result.value, expected);
}

#[test]
fn test_conflict_policy_merge_array_replace() {
    let db = Database::new(":memory:").unwrap();

    let req = CreateMemoryRequest {
        namespace: Some("test".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"items": [1, 2, 3]}),
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Merge,
    };
    db.store_memory("agent1", req).unwrap();

    let patch = CreateMemoryRequest {
        namespace: Some("test".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"items": [4, 5]}),
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Merge,
    };
    db.store_memory("agent1", patch).unwrap();

    let result = db.get_memory("agent1", "test", "state").unwrap();
    let expected = serde_json::json!({"items": [4, 5]});
    assert_eq!(result.value, expected);
}

#[test]
fn test_conflict_policy_merge_no_existing_value() {
    let db = Database::new(":memory:").unwrap();

    let req = CreateMemoryRequest {
        namespace: Some("test".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"x": 1}),
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Merge,
    };
    db.store_memory("agent1", req).unwrap();

    let result = db.get_memory("agent1", "test", "state").unwrap();
    let expected = serde_json::json!({"x": 1});
    assert_eq!(result.value, expected);
}
```

6. **Update MCP `memory_store` tool in `src/mcp/handlers.rs`:**

The `conflict_policy` field already exists on `CreateMemoryRequest` and is already parsed from MCP JSON input (line 262: `.map(ConflictPolicy::parse)`). No changes needed — the new `Merge` variant will be automatically available once `ConflictPolicy::parse` recognizes `"merge"`.

7. **Update REST API in `src/web/handlers/kv.rs`:**

The `conflict_policy` field already exists on `CreateMemoryRequest` (line 61 shows it defaulting to `Overwrite`). No changes needed — the new `Merge` variant will be automatically available via the existing Serde deserialization.

---

## Part 2: Optional JSON-Schema Validation on `agent_memory` Writes

### Current State

No validation on `value` field beyond `serde_json::Value` type-checking. Malformed patches (violating expected structure) succeed and corrupt agent state.

### Problem

SKILL.state Limitations (§7): "small model error class" (schema violations) requires "schema ownership and validation reside in the deterministic runtime, not the model." Without validation, a small LLM can send `{"count": "five"}` when the schema requires an integer, and Smriti will store it, causing downstream failures.

### Implementation

1. **Add optional JSON-schema support via `jsonschema` crate:**

Add to `Cargo.toml`:

```toml
jsonschema = "0.20"
```

2. **Extend the `agent_memory` table schema in a new migration:**

Add a `schema_json` column to store an optional JSON Schema (draft-07) for each namespace:

```sql
-- Migration XXX: Add JSON-schema validation support for agent_memory
CREATE TABLE IF NOT EXISTS agent_memory_schemas (
    agent_id TEXT NOT NULL,
    namespace TEXT NOT NULL,
    schema_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (agent_id, namespace)
);
```

3. **Add a `register_memory_schema` function in `src/storage/operations.rs`:**

```rust
pub fn register_memory_schema(
    &self,
    agent_id: &str,
    namespace: &str,
    schema: serde_json::Value,
) -> AppResult<()> {
    self.execute(|conn| {
        let now = Utc::now().to_rfc3339();
        let schema_json = serde_json::to_string(&schema)?;

        // Validate that the provided schema itself is valid JSON Schema
        let _compiled = jsonschema::JSONSchema::compile(&schema)
            .map_err(|e| AppError::Validation(format!("Invalid JSON Schema: {}", e)))?;

        conn.execute(
            "INSERT INTO agent_memory_schemas (agent_id, namespace, schema_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(agent_id, namespace) DO UPDATE SET
                schema_json = excluded.schema_json,
                updated_at = excluded.updated_at",
            params![agent_id, namespace, schema_json, now, now],
        )?;

        Ok(())
    })
}
```

4. **Modify `store_memory` to validate against schema if present:**

At the top of `store_memory` (before the `match policy` block, around line 466), insert:

```rust
// Optional: validate against registered schema for this namespace
let schema_opt: Option<serde_json::Value> = conn
    .query_row(
        "SELECT schema_json FROM agent_memory_schemas
         WHERE agent_id = ?1 AND namespace = ?2",
        params![agent_id, &ns],
        |row| {
            let json_str: String = row.get(0)?;
            Ok(serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null))
        },
    )
    .optional()?;

if let Some(schema) = schema_opt {
    let compiled_schema = jsonschema::JSONSchema::compile(&schema)
        .map_err(|e| AppError::Validation(format!("Schema compilation failed: {}", e)))?;

    if let Err(errors) = compiled_schema.validate(&memory.value) {
        let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
        return Err(AppError::Validation(format!(
            "Memory value violates schema for {}/{}: {}",
            agent_id, ns, error_messages.join(", ")
        )));
    }
}
```

5. **Add a new MCP tool `memory_register_schema` in `src/mcp/handlers.rs`:**

Insert alongside the other `memory_*` handlers:

```rust
"memory_register_schema" => {
    let agent_id = args
        .get("agent_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing agent_id"))?;
    let namespace = args
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let schema = args
        .get("schema")
        .ok_or_else(|| anyhow!("Missing schema"))?
        .clone();

    state.db.register_memory_schema(agent_id, namespace, schema)?;

    json!({
        "success": true,
        "agent_id": agent_id,
        "namespace": namespace,
    })
}
```

6. **Register the new tool in `src/mcp/server.rs`:**

Add to the tool list returned by the `list_tools` handler:

```rust
{
    "name": "memory_register_schema",
    "description": "Register a JSON Schema (draft-07) for a namespace. All future memory_store writes to this namespace will be validated against this schema. Validation errors trigger ConflictPolicy::Reject behavior. SKILL.state ref: prevents small-model schema violations.",
    "inputSchema": {
        "type": "object",
        "properties": {
            "agent_id": {
                "type": "string",
                "description": "Agent identifier"
            },
            "namespace": {
                "type": "string",
                "description": "Namespace to enforce schema on. Default: 'default'."
            },
            "schema": {
                "type": "object",
                "description": "JSON Schema (draft-07) defining the structure of `value` for this namespace."
            }
        },
        "required": ["agent_id", "schema"]
    }
}
```

7. **Write integration tests in `src/storage/operations.rs`:**

```rust
#[test]
fn test_schema_validation_success() {
    let db = Database::new(":memory:").unwrap();

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "count": {"type": "integer"},
            "name": {"type": "string"}
        },
        "required": ["count"]
    });

    db.register_memory_schema("agent1", "typed", schema).unwrap();

    let req = CreateMemoryRequest {
        namespace: Some("typed".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"count": 5, "name": "test"}),
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Overwrite,
    };

    let result = db.store_memory("agent1", req);
    assert!(result.is_ok());
}

#[test]
fn test_schema_validation_failure() {
    let db = Database::new(":memory:").unwrap();

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "count": {"type": "integer"}
        },
        "required": ["count"]
    });

    db.register_memory_schema("agent1", "typed", schema).unwrap();

    let req = CreateMemoryRequest {
        namespace: Some("typed".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"count": "five"}), // Wrong type
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Overwrite,
    };

    let result = db.store_memory("agent1", req);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("violates schema"));
}

#[test]
fn test_schema_validation_missing_required_field() {
    let db = Database::new(":memory:").unwrap();

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "count": {"type": "integer"}
        },
        "required": ["count"]
    });

    db.register_memory_schema("agent1", "typed", schema).unwrap();

    let req = CreateMemoryRequest {
        namespace: Some("typed".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"name": "test"}), // Missing "count"
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Overwrite,
    };

    let result = db.store_memory("agent1", req);
    assert!(result.is_err());
}

#[test]
fn test_schema_validation_no_schema_registered() {
    let db = Database::new(":memory:").unwrap();

    let req = CreateMemoryRequest {
        namespace: Some("untyped".to_string()),
        key: "state".to_string(),
        value: serde_json::json!({"anything": "goes"}),
        ttl_seconds: None,
        conflict_policy: ConflictPolicy::Overwrite,
    };

    let result = db.store_memory("agent1", req);
    assert!(result.is_ok()); // No schema = no validation
}
```

8. **Add REST endpoint `POST /api/v1/agent/:id/memory/schema/:namespace` in `src/web/handlers/kv.rs`:**

```rust
pub async fn register_schema(
    State(app_state): State<AppState>,
    Path((agent_id, namespace)): Path<(String, String)>,
    Json(schema): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    app_state.db.register_memory_schema(&agent_id, &namespace, schema)?;
    Ok(Json(json!({
        "success": true,
        "agent_id": agent_id,
        "namespace": namespace,
    })))
}
```

And register it in `src/web/mod.rs`:

```rust
.route(
    "/api/v1/agent/:id/memory/schema/:namespace",
    post(handlers::kv::register_schema),
)
```

---

## Error Handling

Both features use existing `AppError` variants:

- `AppError::Validation(String)` — for schema validation failures and invalid JSON Schema itself.
- `AppError::Conflict(String)` — already used by `ConflictPolicy::Reject`.

No new error variants needed.

---

## Testing Checklist

- [ ] `cargo test storage::operations::test_conflict_policy_merge` (3 tests)
- [ ] `cargo test storage::operations::test_schema_validation_success`
- [ ] `cargo test storage::operations::test_schema_validation_failure`
- [ ] `cargo test storage::operations::test_schema_validation_missing_required_field`
- [ ] `cargo test storage::operations::test_schema_validation_no_schema_registered`
- [ ] `cargo test --all` (full suite)
- [ ] Manual MCP test:
  ```json
  {
    "method": "tools/call",
    "params": {
      "name": "memory_register_schema",
      "arguments": {
        "agent_id": "test-agent",
        "namespace": "typed",
        "schema": {
          "type": "object",
          "properties": {
            "count": {"type": "integer"}
          },
          "required": ["count"]
        }
      }
    }
  }
  ```
- [ ] Manual MCP test: `memory_store` with `conflict_policy: "merge"` on existing key
- [ ] Manual MCP test: `memory_store` with invalid schema → expect rejection

---

## Documentation Updates (Part 3 — separate from this prompt)

After implementation is complete, the following docs should be updated:

1. **Add to `docs/memory-patterns.md` (new file):**

   Section: "Using `agent_memory` as Execution State (SKILL.state Pattern)"

   - Show the pattern: `memory_store` with `ConflictPolicy::Merge` instead of replaying `tool_logs`.
   - Reference the SKILL.state paper's 16–75x token reduction findings.
   - Explain when to use `Merge` (incremental state updates) vs. `Overwrite` (full state snapshots).
   - Show the schema validation pattern for catching malformed patches.

2. **Update `CLAUDE.md` Known Gaps:**

   Remove:
   - ~~"TASK 7 — Conflict Resolution / Belief Revision on memory_store"~~ (mark as SHIPPED)

3. **Update `README.md` Memory Features:**

   Add bullet:
   - "JSON merge-patch semantics (`ConflictPolicy::Merge`) for safe incremental state updates, preventing premature overwrites by small LLMs (SKILL.state arXiv:2501.XXXXX)"

---

## Research Attribution

- **Paper:** SKILL.state (arXiv:2501.XXXXX) — Google Research, 2025.
- **Key findings:**
  - 68% of small-model (Gemma-4-31B) failures are "premature state overwrite/deletion."
  - 16–75x token reduction at long horizons vs. history-based approaches.
  - Schema validation in deterministic runtime (not model) prevents malformed updates.
- **Smriti's complementary role (§7 Limitations):**
  - Archival of discarded reasoning traces (out of scope for SKILL.state).
  - Multi-agent shared state conflict resolution (explicitly unaddressed by paper).

---

## Definition of Done

- [ ] `ConflictPolicy::Merge` implemented with RFC 7396 semantics
- [ ] `json_merge_patch` helper with 3 passing tests (nested merge, array replace, no existing value)
- [ ] `register_memory_schema` implemented
- [ ] `memory_register_schema` MCP tool added and registered
- [ ] Schema validation integrated into `store_memory` (before policy match)
- [ ] 5 passing tests for schema validation (success, type mismatch, missing field, no schema, array types)
- [ ] REST endpoint `POST /api/v1/agent/:id/memory/schema/:namespace` added
- [ ] `cargo test --all` passes
- [ ] Manual MCP tests pass (see Testing Checklist)
- [ ] No `unwrap()` added in any library code (use `?` and `thiserror`)
- [ ] Ready for PR with description linking to this implementation prompt

---

## Non-Goals (Explicitly Out of Scope)

- **Do NOT implement a SKILL.state runtime** (prompt construction, `Σt` merge-at-inference-time). That's the calling agent framework's job, not a storage layer's job.
- **Do NOT implement array-level merge semantics** beyond RFC 7396 (which replaces arrays wholesale). Array merging is not defined by the standard and introduces ambiguity.
- **Do NOT make schema validation mandatory globally.** It's opt-in per namespace to avoid breaking existing untyped workflows.
- **Do NOT add SKILL.state-specific primitives** beyond what's already a natural fit for Smriti's AGM conflict-resolution layer (Task 7).

---

## Open Questions for Implementer

None — this is a fully scoped prompt. If you encounter ambiguity during implementation, resolve it by:

1. Defaulting to the simplest correct behavior (favor explicitness over magic).
2. Matching the existing Smriti error-handling patterns (use `?` and `AppError` variants).
3. Writing a test case that documents the intended behavior.

---

**Ship criteria:** PR with passing tests, MCP tool works, REST endpoint works, no breaking changes to existing MCP contracts.
