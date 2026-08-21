# Smriti Agent Testing Guide

## Prerequisites

Before testing, make sure you have:

1. **Smriti binary** — already built and in your PATH (you confirmed this)
2. **Python 3.10+** — check with `python3 --version`
3. **Anthropic API key** — get one at https://platform.claude.com/

## Quick Start (5 minutes)

### Step 1: Set your API key

```bash
export ANTHROPIC_API_KEY=sk-ant-your-key-here
```

Or create a `.env` file in the agents directory:

```bash
cd ~/path/to/smriti/agents
echo "ANTHROPIC_API_KEY=sk-ant-your-key-here" > .env
```

### Step 2: Run setup

```bash
cd agents/
chmod +x setup.sh
./setup.sh
```

This creates a virtual environment, installs the Claude Agent SDK, and verifies everything works.

### Step 3: Activate the environment

```bash
source .venv/bin/activate
```

### Step 4: Test with a quick query

```bash
python smriti_agent.py -q "List all notes in the knowledge graph"
```

If you see note titles printed, everything is working.

## Running Both Agents

### General-Purpose Agent (interactive)

```bash
python smriti_agent.py
```

Try these queries:

- `What notes do we have about strategy?`
- `Show me the graph around the B2B pivot decision`
- `Search for anything about competitive intelligence`
- `Store a memory: my preference is for concise summaries`
- `Create a note about today's standup meeting`

### PI Research Agent (interactive)

```bash
python pi_research_agent.py
```

Try these queries:

- `What preliminary data do we have across all projects?`
- `How are the market analysis and product roadmap connected?`
- `Record this experiment: tested new embedding model, 15% better recall`
- `Reviewer asks: why did you choose SQLite over Postgres?`

### Single Query Mode

Both agents support one-shot queries (useful for scripting):

```bash
# General agent
python smriti_agent.py -q "Summarize all notes tagged #strategy"

# PI agent
python pi_research_agent.py -q "What is the strongest evidence chain for our AI investment?"
```

### Custom Database

Point to any Smriti database:

```bash
python smriti_agent.py --db ~/projects/research.db
python smriti_agent.py --db ~/.smriti/smriti.db
```

## Automated Test Suite

Run all tests at once:

```bash
chmod +x test_agents.sh
./test_agents.sh
```

Or with a specific database:

```bash
./test_agents.sh --db ~/my-smriti.db
```

The test suite runs 8 tests covering: listing, searching, graph traversal, memory operations, note creation, and cross-project queries.

## What Each Agent Does

### smriti_agent.py — General Purpose

A conversational agent that can query, create, and explore the Smriti knowledge graph. It has 3 subagents it can delegate to:

| Subagent | Purpose |
|----------|---------|
| researcher | Exhaustive multi-keyword search + graph traversal |
| note-taker | Creates well-linked notes avoiding duplicates |
| graph-explorer | Maps graph structure, finds clusters and bridges |

### pi_research_agent.py — PI Research

A domain-specialized agent for academic Principal Investigators. It follows knowledge graph conventions (#project, #paper, #experiment, etc.) and has 4 subagents:

| Subagent | Purpose |
|----------|---------|
| literature-agent | Citation graphs, contradictions, gaps |
| grant-agent | Traces preliminary data to grant aims |
| lab-memory-agent | Captures protocols and failed experiments |
| reviewer-agent | Answers reviewer questions with evidence chains |

## Demo Data

Your Smriti instance already has 22 interconnected notes forming a CSO strategic decision graph. These include strategic decisions, market intelligence, and internal capability assessments — all heavily wiki-linked.

Run this to see them:

```bash
python smriti_agent.py -q "List all notes and categorize them by type"
```

## Troubleshooting

### "ANTHROPIC_API_KEY not set"

```bash
export ANTHROPIC_API_KEY=sk-ant-your-key-here
```

### "Smriti binary not found"

Make sure `smriti` is in your PATH:

```bash
which smriti
# If not found, add cargo bin:
export PATH="$HOME/.cargo/bin:$PATH"
```

### "Database does not exist"

Smriti creates the database automatically on first use. If you want to use your existing database:

```bash
python smriti_agent.py --db /path/to/your/smriti.db
```

### Agent times out or hangs

The agents use `bypassPermissions` mode so they shouldn't prompt for approval. If an agent seems stuck, it's likely making many tool calls. Press Ctrl+C and try a simpler query.

### Import errors

Make sure the virtual environment is activated:

```bash
source .venv/bin/activate
pip install -r requirements.txt
```

## Cost Notes

Each query costs Anthropic API tokens. Typical costs:

- Simple list/search query: ~$0.01-0.03
- Graph traversal with subagents: ~$0.05-0.15
- Complex multi-step query: ~$0.10-0.30

The `max_budget_usd` option can cap spending per session if needed.
