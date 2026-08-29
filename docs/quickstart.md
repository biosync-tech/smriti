# Quickstart

Zero prior context. Goal: a working MCP tool call in under five minutes.

```bash
cargo install smriti
smriti init
```

`smriti init` creates `~/.smriti/smriti.db` (override with `--db` or `SMRITI_DB`) and prints an MCP client config block. Same file for CLI and every agent.

```bash
smriti create "Hello Smriti" \
  -c "First note. Linked later via [[wiki-links]]. #intro"

smriti search Hello
```

Restart the MCP client. Ask: *Create a Smriti note titled Onboarding with a one-line hello.*

That is `notes_create`. Follow with *Search Smriti for Onboarding* (`notes_search`).

```
$ smriti init
✓ Initialized Smriti database at: /Users/you/.smriti/smriti.db

{
  "mcpServers": {
    "smriti": {
      "command": "smriti",
      "args": ["mcp"]
    }
  }
}
```
