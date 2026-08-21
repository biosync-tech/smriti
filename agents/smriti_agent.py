#!/usr/bin/env python3
"""
Smriti General-Purpose Query Agent
===================================
A conversational agent built on the Claude Agent SDK that uses Smriti
as its persistent knowledge graph and memory backend via MCP.

This agent can:
  - Create, search, and retrieve notes in the knowledge graph
  - Store and recall agent memory (key-value with namespaces)
  - Traverse the knowledge graph (BFS, subgraphs, backlinks)
  - Build connections between concepts using wiki-links and tags
  - Answer questions by combining graph traversal with full-text search

Usage:
    # Interactive mode (asks questions in a loop)
    python smriti_agent.py

    # Single query mode
    python smriti_agent.py --query "What do we know about CRISPR across all projects?"

    # With a custom Smriti database
    python smriti_agent.py --db ~/research/lab.db

    # With a specific agent identity (for memory namespacing)
    python smriti_agent.py --agent-id "lab-assistant"

Requirements:
    pip install claude-agent-sdk

Environment:
    ANTHROPIC_API_KEY  — your Anthropic API key
    SMRITI_DB          — path to Smriti database (default: ~/.smriti/smriti.db)
"""

import asyncio
import argparse
import os
import sys
from pathlib import Path

from claude_agent_sdk import (
    query,
    ClaudeAgentOptions,
    AgentDefinition,
    AssistantMessage,
    ResultMessage,
    SystemMessage,
)


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

DEFAULT_DB = os.environ.get("SMRITI_DB", str(Path.home() / ".smriti" / "smriti.db"))


def find_smriti_binary() -> str:
    """Locate the smriti binary on the system."""
    import shutil
    found = shutil.which("smriti")
    if found:
        return found
    cargo_bin = Path.home() / ".cargo" / "bin" / "smriti"
    if cargo_bin.exists():
        return str(cargo_bin)
    return "smriti"


SMRITI_BIN = find_smriti_binary()


# ---------------------------------------------------------------------------
# System prompt — this is what makes the agent "smart" about Smriti
# ---------------------------------------------------------------------------

SYSTEM_PROMPT = """\
You are a research assistant powered by Smriti, a knowledge graph and memory system.
You have access to a persistent knowledge graph via MCP tools. Use them proactively.

## Your MCP Tools (from the "smriti" server)

### Notes & Knowledge Graph
- **notes_create**: Create a note with a title, content (markdown), and tags.
  Content can include [[wiki-links]] to other notes and #tags — these are
  auto-detected and create graph edges.
- **notes_read**: Read a note by its ID or title.
- **notes_search**: Full-text search across all notes (FTS5 keyword matching).
- **notes_list**: List recent notes, optionally filtered by tag.
- **notes_graph**: Get the knowledge graph or a subgraph around a specific note.
  Use this for discovering connections, tracing relationships, and understanding
  how concepts relate. Supports depth control and link-type filtering
  (semantic, temporal, causal, wikilink).

### Agent Memory (Key-Value Store)
- **memory_store**: Store a key-value pair with optional namespace, TTL, and
  conflict resolution policy (overwrite, reject, version_and_keep, invalidate).
  Use this to remember preferences, session state, intermediate results, or
  any structured data you want to persist across conversations.
- **memory_retrieve**: Get a stored value by key and namespace.
- **memory_list**: List all memory entries for your agent.
- **memory_history**: View past (superseded) values for a key — useful for
  understanding how knowledge evolved over time.

## How to Think

1. **Search before creating** — always check if a note already exists before
   creating a duplicate. Use notes_search or notes_list first.
2. **Link aggressively** — when creating notes, use [[wiki-links]] to connect
   to existing notes. This builds the graph.
3. **Use memory for state** — store intermediate findings, user preferences,
   and session context in memory_store so you can recall them later.
4. **Traverse the graph** — when answering complex questions, use notes_graph
   to explore connections. BFS traversal can reveal relationships that keyword
   search alone would miss.
5. **Be specific about sources** — when answering from the knowledge graph,
   cite which notes or memory entries you used.
6. **Build knowledge incrementally** — if the user shares new information,
   create notes and links to capture it in the graph for future use.

## Response Style
- Be concise and direct
- Cite your sources (note titles/IDs)
- If the knowledge graph doesn't have the answer, say so clearly
- Suggest what information could be added to make the graph more useful
"""


# ---------------------------------------------------------------------------
# Agent builder
# ---------------------------------------------------------------------------

def build_agent_options(
    db_path: str,
    agent_id: str,
    enable_subagents: bool = True,
) -> ClaudeAgentOptions:
    """
    Build ClaudeAgentOptions that connect to Smriti via MCP (stdio transport).

    The smriti binary is launched as a child process speaking JSON-RPC over stdio.
    All 8+ MCP tools become available to the agent automatically.
    """

    # Core MCP connection to Smriti
    mcp_servers = {
        "smriti": {
            "command": SMRITI_BIN,
            "args": ["--db", db_path, "mcp"],
        }
    }

    # Allow all Smriti MCP tools
    allowed_tools = [
        "mcp__smriti__*",   # All Smriti tools (notes_*, memory_*, etc.)
    ]

    # Optional: define specialized subagents for complex workflows
    agents = None
    if enable_subagents:
        agents = {
            "researcher": AgentDefinition(
                description="Deep research agent that searches the knowledge graph exhaustively.",
                prompt=(
                    "You are a thorough researcher. Given a question, use notes_search "
                    "and notes_graph extensively to find all relevant information in the "
                    "knowledge graph. Search with multiple keyword variations. Traverse "
                    "the graph from each result to find connected notes. Return a "
                    "comprehensive summary with all note IDs and titles cited."
                ),
                tools=["mcp__smriti__notes_search", "mcp__smriti__notes_read",
                       "mcp__smriti__notes_graph", "mcp__smriti__notes_list"],
            ),
            "note-taker": AgentDefinition(
                description="Captures information into the knowledge graph with proper links and tags.",
                prompt=(
                    "You are a meticulous note-taker. Given information to capture, "
                    "first search the existing graph to find related notes. Then create "
                    "well-structured notes with [[wiki-links]] to existing notes and "
                    "appropriate #tags. Always search before creating to avoid duplicates. "
                    "Report what you created and how it connects to existing knowledge."
                ),
                tools=["mcp__smriti__notes_create", "mcp__smriti__notes_search",
                       "mcp__smriti__notes_read", "mcp__smriti__notes_list"],
            ),
            "graph-explorer": AgentDefinition(
                description="Explores and maps the knowledge graph structure.",
                prompt=(
                    "You are a graph analyst. Use notes_graph to explore the knowledge "
                    "graph structure. Map connections, identify clusters, find bridge "
                    "nodes, and trace paths between concepts. You can filter by link "
                    "type (semantic, temporal, causal, wikilink) to analyze different "
                    "relationship layers. Report your findings as a clear summary."
                ),
                tools=["mcp__smriti__notes_graph", "mcp__smriti__notes_read",
                       "mcp__smriti__notes_list"],
            ),
        }
        allowed_tools.append("Agent")  # Enable subagent spawning

    return ClaudeAgentOptions(
        system_prompt=SYSTEM_PROMPT,
        mcp_servers=mcp_servers,
        allowed_tools=allowed_tools,
        agents=agents,
        permission_mode="bypassPermissions",
    )


# ---------------------------------------------------------------------------
# Agent runner
# ---------------------------------------------------------------------------

async def run_single_query(prompt: str, db_path: str, agent_id: str) -> str:
    """Run a single query against the Smriti knowledge graph and return the result."""

    full_prompt = (
        f"[Your agent_id is '{agent_id}'. Use this when calling memory_store "
        f"and memory_retrieve.]\n\n{prompt}"
    )

    options = build_agent_options(db_path, agent_id)
    result_text = ""

    async for message in query(prompt=full_prompt, options=options):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if hasattr(block, "text"):
                    print(f"  [thinking] {block.text[:120]}...")
                elif hasattr(block, "name"):
                    print(f"  [tool] {block.name}")

        if isinstance(message, ResultMessage):
            if message.subtype == "success":
                result_text = message.result
            elif message.subtype == "error_during_execution":
                result_text = f"Error: {message.result}"

    return result_text


async def run_interactive(db_path: str, agent_id: str):
    """Run an interactive conversation loop with the Smriti agent."""

    print("=" * 60)
    print("  Smriti Query Agent")
    print(f"  Database: {db_path}")
    print(f"  Agent ID: {agent_id}")
    print(f"  Smriti binary: {SMRITI_BIN}")
    print("=" * 60)
    print()
    print("Ask me anything about your knowledge graph.")
    print("Type 'quit' or 'exit' to stop.\n")

    session_id = None

    while True:
        try:
            user_input = input("You: ").strip()
        except (EOFError, KeyboardInterrupt):
            print("\nGoodbye!")
            break

        if not user_input:
            continue
        if user_input.lower() in ("quit", "exit", "q"):
            print("Goodbye!")
            break

        # Inject agent_id context
        full_prompt = (
            f"[Your agent_id is '{agent_id}'. Use this when calling memory_store "
            f"and memory_retrieve.]\n\n{user_input}"
        )

        # Resume session if we have one (maintains conversation context)
        if session_id:
            options = ClaudeAgentOptions(resume=session_id)
        else:
            options = build_agent_options(db_path, agent_id)

        print()
        async for message in query(prompt=full_prompt, options=options):
            # Capture session ID for conversation continuity
            if isinstance(message, SystemMessage) and message.subtype == "init":
                if hasattr(message, "session_id"):
                    session_id = message.session_id

            # Show tool calls for transparency
            if isinstance(message, AssistantMessage):
                for block in message.content:
                    if hasattr(block, "name") and block.name.startswith("mcp__"):
                        print(f"  [tool] {block.name}")

            # Print the final result
            if isinstance(message, ResultMessage):
                if message.subtype == "success":
                    print(f"Agent: {message.result}")
                elif message.subtype == "error_during_execution":
                    print(f"Error: {message.result}")
        print()


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Smriti General-Purpose Query Agent",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
Examples:
  # Interactive mode
  python smriti_agent.py

  # Single query
  python smriti_agent.py --query "What projects are related to gene editing?"

  # Custom database and agent identity
  python smriti_agent.py --db ~/lab/research.db --agent-id "pi-assistant"

  # Pipe a query
  echo "Summarize all notes tagged #experiment" | python smriti_agent.py --query -
""",
    )
    parser.add_argument(
        "--query", "-q",
        type=str,
        default=None,
        help="Run a single query and exit. Use '-' to read from stdin.",
    )
    parser.add_argument(
        "--db",
        type=str,
        default=DEFAULT_DB,
        help=f"Path to Smriti database (default: {DEFAULT_DB})",
    )
    parser.add_argument(
        "--agent-id",
        type=str,
        default="smriti-agent",
        help="Agent identity for memory namespacing (default: smriti-agent)",
    )

    args = parser.parse_args()

    # Validate API key
    if not os.environ.get("ANTHROPIC_API_KEY"):
        print("Error: ANTHROPIC_API_KEY environment variable is required.")
        print("Get one at: https://platform.claude.com/")
        sys.exit(1)

    # Validate smriti binary
    import shutil
    if not shutil.which(SMRITI_BIN) and not Path(SMRITI_BIN).exists():
        print(f"Error: Smriti binary not found at '{SMRITI_BIN}'")
        print("Make sure 'smriti' is in your PATH or set via --db flag.")
        print("Build it with: cargo install --path . (from the smriti repo)")
        sys.exit(1)

    # Validate database
    db_path = Path(args.db)
    if not db_path.exists():
        print(f"Warning: Database '{args.db}' does not exist.")
        print("Smriti will create it on first use.\n")

    if args.query:
        prompt = sys.stdin.read().strip() if args.query == "-" else args.query
        result = asyncio.run(run_single_query(prompt, args.db, args.agent_id))
        print(result)
    else:
        asyncio.run(run_interactive(args.db, args.agent_id))


if __name__ == "__main__":
    main()
