#!/usr/bin/env python3
"""
Smriti PI Research Agent
========================
A specialized agent for Principal Investigators managing multiple research
projects. Built on the general-purpose Smriti agent with domain-specific
subagents for academic workflows.

Subagents:
  - Literature Agent:     Builds citation graphs, finds contradictions
  - Grant Agent:          Traces research lineage, drafts grant sections
  - Lab Memory Agent:     Captures protocols, experiments, tribal knowledge
  - Reviewer Agent:       Answers reviewer questions with graph traversal

Usage:
    python pi_research_agent.py
    python pi_research_agent.py --query "What preliminary data supports Aim 2?"
    python pi_research_agent.py --db ~/lab/research.db --agent-id "pi-chen"

Requirements:
    pip install claude-agent-sdk
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


DEFAULT_DB = os.environ.get("SMRITI_DB", str(Path.home() / ".smriti" / "smriti.db"))


def find_smriti_binary() -> str:
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
# PI-specific system prompt
# ---------------------------------------------------------------------------

PI_SYSTEM_PROMPT = """\
You are a research assistant for a Principal Investigator (PI) who manages
multiple research projects simultaneously. You have access to a persistent
knowledge graph via Smriti that contains the PI's research notes, papers,
experiments, protocols, and institutional knowledge.

## Your Role

You help the PI:
- Find connections across projects that aren't obvious
- Recall past experiments, protocols, and their outcomes
- Assemble information for grant proposals and paper writing
- Prepare responses to reviewer comments with citations
- Onboard new lab members by surfacing relevant history

## Your MCP Tools (from the "smriti" server)

You have access to all Smriti MCP tools: notes_create, notes_read,
notes_search, notes_list, notes_graph, memory_store, memory_retrieve,
memory_list, memory_history.

## Your Subagents

You can delegate complex tasks to specialized subagents:

- **literature-agent**: For literature searches, citation graph building,
  finding contradictions or gaps across the research corpus.
- **grant-agent**: For assembling preliminary data, tracing research lineage,
  and drafting grant sections with provenance.
- **lab-memory-agent**: For capturing experimental protocols, failed
  experiments, and institutional knowledge into the graph.
- **reviewer-agent**: For answering reviewer questions by traversing the PI's
  publication graph and finding supporting evidence.

## Knowledge Graph Conventions

When creating notes, follow these conventions:
- **Projects**: Tag with #project and the project name, e.g., #project-crispr
- **Papers**: Tag with #paper, include DOI in content, link to related projects
- **Experiments**: Tag with #experiment, link to protocol and project
- **Protocols**: Tag with #protocol, version in title (e.g., "PCR Protocol v3")
- **People**: Tag with #person, link to their projects and papers
- **Grants**: Tag with #grant, link to preliminary data and publications
- **Meetings**: Tag with #meeting, link to discussed topics

Use [[wiki-links]] aggressively to build the graph. Every note should link
to at least one other note.

## Response Style
- Always cite note titles and IDs when referencing knowledge graph data
- When you don't find something, suggest what the PI should add
- Proactively suggest connections the PI might not have considered
- For grant-related queries, trace the full provenance chain
"""


# ---------------------------------------------------------------------------
# Subagent definitions
# ---------------------------------------------------------------------------

SUBAGENTS = {
    "literature-agent": AgentDefinition(
        description=(
            "Literature research agent. Searches the knowledge graph for papers, "
            "findings, and citations. Builds citation networks, identifies "
            "contradictions, and finds gaps in the research."
        ),
        prompt="""\
You are a literature research specialist. Your job is to thoroughly search
the knowledge graph for papers, findings, methods, and citations.

When asked about a topic:
1. Search with multiple keyword variations (synonyms, abbreviations)
2. For each result, use notes_graph to find connected papers and findings
3. Look for contradictions — findings that disagree across papers
4. Identify gaps — questions that the existing literature doesn't answer
5. Build a citation chain showing how ideas evolved

Always report:
- Exact note titles and IDs
- How notes connect in the graph (which link types)
- Any temporal ordering (which came first)
- Contradictions or gaps you found
""",
        tools=[
            "mcp__smriti__notes_search",
            "mcp__smriti__notes_read",
            "mcp__smriti__notes_graph",
            "mcp__smriti__notes_list",
        ],
    ),

    "grant-agent": AgentDefinition(
        description=(
            "Grant writing assistant. Traces research lineage from preliminary "
            "data through publications to grant aims. Helps assemble Specific "
            "Aims sections with full provenance."
        ),
        prompt="""\
You are a grant writing assistant. Your job is to help the PI assemble
compelling grant sections by tracing the research lineage in the graph.

For Specific Aims or preliminary data requests:
1. Search for all relevant experiments and publications
2. Use notes_graph to trace the provenance chain:
   preliminary data -> experiment -> publication -> grant aim
3. Identify the strongest evidence chains
4. Note any gaps in the chain that need new experiments
5. Suggest how to frame the narrative

Store intermediate findings in memory_store so they persist across queries.
Use namespace "grant" for all grant-related memory entries.

Always cite specific note IDs and explain the chain of evidence.
""",
        tools=[
            "mcp__smriti__notes_search",
            "mcp__smriti__notes_read",
            "mcp__smriti__notes_graph",
            "mcp__smriti__notes_list",
            "mcp__smriti__memory_store",
            "mcp__smriti__memory_retrieve",
        ],
    ),

    "lab-memory-agent": AgentDefinition(
        description=(
            "Lab memory capture agent. Records experiments, protocols, "
            "failures, and institutional knowledge into the graph with "
            "proper linking and tagging."
        ),
        prompt="""\
You are a lab memory specialist. Your job is to capture experimental
knowledge into the graph so it persists when lab members leave.

When capturing information:
1. ALWAYS search first for existing related notes (avoid duplicates)
2. Create well-structured notes with:
   - Clear titles (include version numbers for protocols)
   - Appropriate tags (#experiment, #protocol, #failure, #insight)
   - [[wiki-links]] to related projects, people, and methods
3. For failed experiments, record:
   - What was tried and the exact conditions
   - Why it failed (hypothesis)
   - What to try differently next time
4. For protocols, include version history context

Link everything. An unlinked note is a lost note.
""",
        tools=[
            "mcp__smriti__notes_create",
            "mcp__smriti__notes_search",
            "mcp__smriti__notes_read",
            "mcp__smriti__notes_list",
            "mcp__smriti__notes_graph",
        ],
    ),

    "reviewer-agent": AgentDefinition(
        description=(
            "Reviewer response agent. Answers reviewer questions by traversing "
            "the PI's publication graph and finding supporting citations."
        ),
        prompt="""\
You are a reviewer response specialist. When the PI receives reviewer
comments, you help craft evidence-based responses using the knowledge graph.

For each reviewer question:
1. Parse the core question or concern
2. Search the graph for relevant publications, data, and experiments
3. Use notes_graph to find the full evidence chain
4. Traverse temporal edges to show the progression of the research
5. Draft a concise, evidence-backed response

Your responses should:
- Reference specific papers with note IDs
- Show the chain of reasoning
- Address the reviewer's concern directly
- Suggest additional analyses if the graph reveals gaps
""",
        tools=[
            "mcp__smriti__notes_search",
            "mcp__smriti__notes_read",
            "mcp__smriti__notes_graph",
            "mcp__smriti__notes_list",
            "mcp__smriti__memory_store",
            "mcp__smriti__memory_retrieve",
        ],
    ),
}


# ---------------------------------------------------------------------------
# Agent builder
# ---------------------------------------------------------------------------

def build_pi_options(db_path: str) -> ClaudeAgentOptions:
    return ClaudeAgentOptions(
        system_prompt=PI_SYSTEM_PROMPT,
        mcp_servers={
            "smriti": {
                "command": SMRITI_BIN,
                "args": ["--db", db_path, "mcp"],
            }
        },
        allowed_tools=[
            "mcp__smriti__*",   # All Smriti tools
            "Agent",            # Enable subagent delegation
        ],
        agents=SUBAGENTS,
        permission_mode="bypassPermissions",
    )


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------

async def run_interactive(db_path: str, agent_id: str):
    print("=" * 60)
    print("  Smriti PI Research Agent")
    print(f"  Database: {db_path}")
    print(f"  Agent ID: {agent_id}")
    print(f"  Smriti binary: {SMRITI_BIN}")
    print("=" * 60)
    print()
    print("I can help you with:")
    print("  - Searching across projects: \"What do we know about X?\"")
    print("  - Grant writing: \"Assemble preliminary data for Aim 2\"")
    print("  - Lab memory: \"Record today's CRISPR experiment results\"")
    print("  - Reviewer responses: \"Reviewer asks why we didn't use method Y\"")
    print("  - Graph exploration: \"How are projects A and B connected?\"")
    print()
    print("Type 'quit' to exit.\n")

    session_id = None
    options = build_pi_options(db_path)

    while True:
        try:
            user_input = input("PI: ").strip()
        except (EOFError, KeyboardInterrupt):
            print("\nGoodbye!")
            break

        if not user_input:
            continue
        if user_input.lower() in ("quit", "exit", "q"):
            print("Goodbye!")
            break

        full_prompt = (
            f"[Your agent_id is '{agent_id}'. Use this for memory_store/retrieve.]\n\n"
            f"{user_input}"
        )

        if session_id:
            run_options = ClaudeAgentOptions(resume=session_id)
        else:
            run_options = options

        print()
        async for message in query(prompt=full_prompt, options=run_options):
            if isinstance(message, SystemMessage) and message.subtype == "init":
                if hasattr(message, "session_id"):
                    session_id = message.session_id

            if isinstance(message, AssistantMessage):
                for block in message.content:
                    if hasattr(block, "name") and block.name.startswith("mcp__"):
                        print(f"  [tool] {block.name}")

            if isinstance(message, ResultMessage):
                if message.subtype == "success":
                    print(f"Agent: {message.result}")
                elif message.subtype == "error_during_execution":
                    print(f"Error: {message.result}")
        print()


async def run_single_query(prompt: str, db_path: str, agent_id: str) -> str:
    full_prompt = (
        f"[Your agent_id is '{agent_id}'. Use this for memory_store/retrieve.]\n\n"
        f"{prompt}"
    )
    options = build_pi_options(db_path)
    result_text = ""

    async for message in query(prompt=full_prompt, options=options):
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if hasattr(block, "name") and block.name.startswith("mcp__"):
                    print(f"  [tool] {block.name}")

        if isinstance(message, ResultMessage):
            if message.subtype == "success":
                result_text = message.result

    return result_text


def main():
    parser = argparse.ArgumentParser(
        description="Smriti PI Research Agent — AI assistant for Principal Investigators",
    )
    parser.add_argument("--query", "-q", type=str, default=None)
    parser.add_argument("--db", type=str, default=DEFAULT_DB)
    parser.add_argument("--agent-id", type=str, default="pi-agent")

    args = parser.parse_args()

    if not os.environ.get("ANTHROPIC_API_KEY"):
        print("Error: ANTHROPIC_API_KEY environment variable is required.")
        print("Get one at: https://platform.claude.com/")
        sys.exit(1)

    import shutil
    if not shutil.which(SMRITI_BIN) and not Path(SMRITI_BIN).exists():
        print(f"Error: Smriti binary not found at '{SMRITI_BIN}'")
        print("Build it with: cargo install --path . (from the smriti repo)")
        sys.exit(1)

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
