# Smriti documentation

Product docs for people who just installed the binary. Research papers live in [`papers/`](papers/).

| Page | What it covers |
|------|----------------|
| [Quickstart](quickstart.md) | `cargo install` → first MCP tool call in under 5 minutes |
| [MCP tools](mcp.md) | All 18 tools with example JSON-RPC calls |
| [MCP tools (long)](mcp-tools.md) | Parameter-level reference |
| [REST API](rest-api.md) | HTTP endpoints, including consolidation review |
| [SQLite schema](sqlite-schema.md) | Tables, FTS5, sqlite-vec, audit trail |
| [Why local-first](why-local-first.md) | Positioning vs Mem0 / Zep / Neo4j |
| [Schema formation](schema-formation.md) | WikiSkill-mapped consolidation and isolation |
| [Consolidation proxy](consolidation-proxy.md) | Retrieve-context gate (not task-accuracy) |
| [Isolation notes](isolation-notes.md) | Why proposals are events, not notes |

Schema formation is a differentiator, not a packaging blocker. You do not need it to use Smriti as a knowledge graph + MCP memory layer.
