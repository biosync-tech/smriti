use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crate::storage::Database;

use super::handlers;

/// MCP Server implementation using JSON-RPC 2.0 over stdio
///
/// Implements the Model Context Protocol for AI agent integration.
/// Agents can create/read/search notes, manage memory, and traverse the knowledge graph.
pub struct McpServer {
    db: Arc<Database>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl McpServer {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Run the MCP server, reading JSON-RPC messages from stdin and writing responses to stdout
    pub fn run(&self) -> anyhow::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        eprintln!("MCP server started. Listening for JSON-RPC messages on stdin...");

        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
                Ok(req) => self.handle_request(req),
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                },
            };

            let output = serde_json::to_string(&response)?;
            writeln!(stdout, "{}", output)?;
            stdout.flush()?;
        }

        Ok(())
    }

    fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let id = req.id.unwrap_or(Value::Null);

        let result = match req.method.as_str() {
            "initialize" => self.handle_initialize(&req.params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(&req.params),
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(&req.params),
            "notifications/initialized" => {
                // Client notification — no response needed but we return OK
                Ok(json!({}))
            }
            _ => Err((-32601, format!("Method not found: {}", req.method))),
        };

        match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: Some(value),
                error: None,
            },
            Err((code, message)) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code,
                    message,
                    data: None,
                }),
            },
        }
    }

    fn handle_initialize(&self, _params: &Value) -> Result<Value, (i32, String)> {
        Ok(json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {
                "tools": {},
                "resources": { "listChanged": false }
            },
            "serverInfo": {
                "name": "smriti",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    fn handle_tools_list(&self) -> Result<Value, (i32, String)> {
        Ok(json!({
            "tools": [
                {
                    "name": "notes_create",
                    "description": "Create a new note with markdown content. Wiki-links [[like this]] and #tags are auto-detected.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "Note title" },
                            "content": { "type": "string", "description": "Markdown content" },
                            "tags": { "type": "array", "items": { "type": "string" }, "description": "Tags" }
                        },
                        "required": ["title", "content"]
                    }
                },
                {
                    "name": "notes_read",
                    "description": "Read a note by ID or title",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string", "description": "Note ID or title" }
                        },
                        "required": ["id"]
                    }
                },
                {
                    "name": "notes_search",
                    "description": "Full-text search across all notes",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query" },
                            "limit": { "type": "integer", "description": "Max results (default: 10)" }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "notes_list",
                    "description": "List recent notes",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "description": "Max notes (default: 20)" },
                            "tag": { "type": "string", "description": "Filter by tag" }
                        }
                    }
                },
                {
                    "name": "notes_graph",
                    "description": "Get the knowledge graph or a subgraph around a note. Optionally filter by link type (e.g. semantic, causal, temporal, wikilink).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "center_id": { "type": "string", "description": "Center note ID (optional, returns full graph if omitted)" },
                            "depth": { "type": "integer", "description": "Depth for subgraph (default: 2)" },
                            "link_type": { "type": "string", "description": "Comma-separated link type filter (e.g. 'semantic,causal'). If omitted, all types are included." },
                            "path_to": { "type": "string", "description": "If set with center_id, returns shortest path from center_id to path_to (optionally filtered by link_type)" }
                        }
                    }
                },
                {
                    "name": "memory_store",
                    "description": "Store a key-value pair in agent memory with optional conflict resolution policy (arXiv:2603.17244 AGM belief revision).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent identifier" },
                            "key": { "type": "string", "description": "Memory key" },
                            "value": { "description": "Value to store (any JSON)" },
                            "namespace": { "type": "string", "description": "Namespace (default: 'default')" },
                            "ttl_seconds": { "type": "integer", "description": "Time-to-live in seconds" },
                            "conflict_policy": {
                                "type": "string",
                                "enum": ["overwrite", "reject", "version_and_keep", "invalidate"],
                                "description": "Conflict resolution: overwrite (default), reject (fail if exists), version_and_keep (archive old), invalidate (supersede old)"
                            }
                        },
                        "required": ["agent_id", "key", "value"]
                    }
                },
                {
                    "name": "memory_retrieve",
                    "description": "Retrieve a value from agent memory",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent identifier" },
                            "key": { "type": "string", "description": "Memory key" },
                            "namespace": { "type": "string", "description": "Namespace (default: 'default')" }
                        },
                        "required": ["agent_id", "key"]
                    }
                },
                {
                    "name": "memory_list",
                    "description": "List all memory entries for an agent",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent identifier" },
                            "namespace": { "type": "string", "description": "Filter by namespace" }
                        },
                        "required": ["agent_id"]
                    }
                },
                {
                    "name": "memory_history",
                    "description": "List past (superseded) values for a memory key. Only populated when memory_store is called with conflict_policy 'version_and_keep' or 'invalidate'.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent identifier" },
                            "key": { "type": "string", "description": "Memory key" },
                            "namespace": { "type": "string", "description": "Namespace (default: 'default')" }
                        },
                        "required": ["agent_id", "key"]
                    }
                },
                {
                    "name": "notes_search_semantic",
                    "description": "Semantic search using a pre-computed embedding vector. Optionally provide a text query for hybrid FTS5+semantic search with reciprocal rank fusion.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "embedding": {
                                "type": "array",
                                "items": { "type": "number" },
                                "description": "Query embedding vector (e.g. 384-dim float array)"
                            },
                            "query": {
                                "type": "string",
                                "description": "Optional text query for hybrid FTS5+semantic search"
                            },
                            "limit": { "type": "integer", "description": "Max results (default: 10)" },
                            "fts_weight": {
                                "type": "number",
                                "description": "Weight for FTS5 in hybrid mode, 0.0-1.0 (default: 0.5). Semantic weight = 1 - fts_weight."
                            }
                        },
                        "required": ["embedding"]
                    }
                },
                {
                    "name": "wiki_transaction_submit",
                    "description": "Atomically apply a batch of wiki operations (create/update notes, create links, upsert sources) inside a single SAVEPOINT. Every content write must carry claim spans — provenance is enforced by default (FACTUM arXiv:2601.05866, Citation-Grounded arXiv:2512.12117). Set pending=true to queue for human review instead of committing.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string" },
                            "operations": { "type": "array", "description": "Array of WikiOp objects tagged by 'op' field: create_note, update_note, create_link, upsert_source" },
                            "rationale": { "type": "string" },
                            "pending": { "type": "boolean", "description": "If true, queue for review; default false" },
                            "require_provenance": { "type": "boolean", "description": "If true (default), reject writes without claims" }
                        },
                        "required": ["agent_id", "operations"]
                    }
                },
                {
                    "name": "wiki_transaction_commit",
                    "description": "Commit a previously queued pending transaction by id.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "transaction_id": { "type": "string" }
                        },
                        "required": ["transaction_id"]
                    }
                },
                {
                    "name": "wiki_transaction_reject",
                    "description": "Reject a pending transaction with a reason (review inbox pattern).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "transaction_id": { "type": "string" },
                            "rejected_by": { "type": "string" },
                            "reason": { "type": "string" }
                        },
                        "required": ["transaction_id"]
                    }
                },
                {
                    "name": "wiki_transaction_list_pending",
                    "description": "List pending transactions awaiting review (oldest first).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer" }
                        }
                    }
                },
                {
                    "name": "wiki_verify",
                    "description": "Run the full integrity sweep: referential integrity, provenance overlap re-check, event-log hash chain, orphan detection. Never mutates data.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "contradictions_detect",
                    "description": "Scan recent notes for candidate contradictions using semantic + recency + authority weighted scoring (MemoTime arXiv:2510.13614). Never auto-resolves — populates the review inbox.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "scan_limit": { "type": "integer", "description": "Max notes to scan pairwise (default 50)" }
                        }
                    }
                },
                {
                    "name": "contradictions_list",
                    "description": "List open contradiction candidates, highest confidence first.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer" }
                        }
                    }
                },
                {
                    "name": "notes_consolidate",
                    "description": "Run a CLS-inspired consolidation pass (McClelland 1995). Scores every episode note on replay frequency, structural centrality, context diversity, and recency. Flags low-score notes for review, surfaces promotion-eligible clusters. Use dry_run=true (default) to preview without persisting changes. Never deletes — archives via memory_history for ICH E6(R3) compliance.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "dry_run": { "type": "boolean", "description": "If true (default), compute scores but do not persist changes." },
                            "policy": {
                                "type": "string",
                                "enum": ["conservative", "standard", "aggressive"],
                                "description": "Consolidation policy. Conservative (default): flag only. Standard: promote on threshold. Aggressive: auto-archive past grace period."
                            },
                            "agent_id": { "type": "string", "description": "Agent identifier for audit trail (optional)." }
                        }
                    }
                },
                {
                    "name": "ingest_document",
                    "description": "Ingest a local text or markdown file into the knowledge graph. Creates a parent document note and one chunk note per section, linked with ChunkOf edges. No LLM required — chunking is structural. Supports .txt and .md files.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Absolute or relative path to the file (.txt or .md)" },
                            "tags": { "type": "array", "items": { "type": "string" }, "description": "Extra tags to apply to document and chunk notes" },
                            "chunk_size": { "type": "integer", "description": "Target chunk size in characters (default: 1200, ≈300-400 tokens)" },
                            "chunk_overlap": { "type": "integer", "description": "Overlap between consecutive chunks in characters (default: 200)" }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "retrieve_context",
                    "description": "Retrieve assembled context from the knowledge graph for a given query. Combines FTS5 keyword search (always) + optional semantic search (if embedding provided) + BFS graph expansion + schema-first ranking. Returns a ready-to-use context string that local LLMs can use directly for answer generation.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "The user's question or topic" },
                            "embedding": {
                                "type": "array",
                                "items": { "type": "number" },
                                "description": "Optional query embedding vector. If provided, enables hybrid semantic+FTS retrieval (recommended). If omitted, FTS-only retrieval is used."
                            },
                            "top_k": { "type": "integer", "description": "Max seed notes from search (default: 10)" },
                            "graph_depth": { "type": "integer", "description": "BFS depth for graph expansion around seed notes (default: 1)" },
                            "max_tokens": { "type": "integer", "description": "Approximate token budget for context (default: 4096, 1 token ≈ 4 chars)" },
                            "fts_weight": { "type": "number", "description": "FTS vs semantic balance in hybrid mode, 0.0-1.0 (default: 0.5)" }
                        },
                        "required": ["query"]
                    }
                }
            ]
        }))
    }

    fn handle_tools_call(&self, params: &Value) -> Result<Value, (i32, String)> {
        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing tool name".to_string()))?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let result = match tool_name {
            "notes_create" => handlers::handle_notes_create(&self.db, &arguments),
            "notes_read" => handlers::handle_notes_read(&self.db, &arguments),
            "notes_search" => handlers::handle_notes_search(&self.db, &arguments),
            "notes_list" => handlers::handle_notes_list(&self.db, &arguments),
            "notes_graph" => handlers::handle_notes_graph(&self.db, &arguments),
            "memory_store" => handlers::handle_memory_store(&self.db, &arguments),
            "memory_retrieve" => handlers::handle_memory_retrieve(&self.db, &arguments),
            "memory_list" => handlers::handle_memory_list(&self.db, &arguments),
            "memory_history" => handlers::handle_memory_history(&self.db, &arguments),
            "notes_search_semantic" => handlers::handle_notes_search_semantic(&self.db, &arguments),
            "wiki_transaction_submit" => handlers::handle_wiki_transaction_submit(&self.db, &arguments),
            "wiki_transaction_commit" => handlers::handle_wiki_transaction_commit(&self.db, &arguments),
            "wiki_transaction_reject" => handlers::handle_wiki_transaction_reject(&self.db, &arguments),
            "wiki_transaction_list_pending" => handlers::handle_wiki_transaction_list_pending(&self.db, &arguments),
            "wiki_verify" => handlers::handle_wiki_verify(&self.db, &arguments),
            "contradictions_detect" => handlers::handle_contradictions_detect(&self.db, &arguments),
            "contradictions_list" => handlers::handle_contradictions_list(&self.db, &arguments),
            "notes_consolidate" => handlers::handle_notes_consolidate(&self.db, &arguments),
            "ingest_document" => handlers::handle_ingest_document(&self.db, &arguments),
            "retrieve_context" => handlers::handle_retrieve_context(&self.db, &arguments),
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        match result {
            Ok(value) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&value).unwrap_or_default()
                }]
            })),
            Err(e) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Error: {}", e)
                }],
                "isError": true
            })),
        }
    }

    fn handle_resources_list(&self) -> Result<Value, (i32, String)> {
        // List notes as resources
        let notes = self
            .db
            .list_notes(&crate::models::NoteListQuery {
                limit: 100,
                offset: 0,
                sort: crate::models::SortOrder::UpdatedDesc,
                tag: None,
            })
            .map_err(|e| (-32000, e.to_string()))?;

        let resources: Vec<Value> = notes
            .iter()
            .map(|n| {
                json!({
                    "uri": format!("note://{}", n.id),
                    "name": n.title,
                    "description": n.preview,
                    "mimeType": "text/markdown"
                })
            })
            .collect();

        Ok(json!({ "resources": resources }))
    }

    fn handle_resources_read(&self, params: &Value) -> Result<Value, (i32, String)> {
        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing uri".to_string()))?;

        let note_id = uri
            .strip_prefix("note://")
            .ok_or((-32602, "Invalid URI format".to_string()))?;

        let note = self
            .db
            .get_note(note_id)
            .map_err(|e| (-32000, e.to_string()))?;

        Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "text/markdown",
                "text": note.content
            }]
        }))
    }

    /// Dispatch a single JSON-RPC call by method name and return the result.
    ///
    /// Used by the HTTP MCP endpoint (`POST /mcp`) to reuse the same routing
    /// logic as the stdio transport, keeping both transports in sync.
    pub fn dispatch_http(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, (i32, String)> {
        match method {
            "initialize" => self.handle_initialize(&params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(&params),
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(&params),
            "notifications/initialized" => Ok(json!({})),
            other => Err((-32601, format!("Method not found: {}", other))),
        }
    }
}
