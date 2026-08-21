#!/bin/bash
# ==============================================================================
# Smriti Agent Test Runner
# ==============================================================================
# Runs a series of test queries against both agents to verify they work.
# Requires setup.sh to have been run first.
#
# Usage:
#   chmod +x test_agents.sh
#   ./test_agents.sh [--db /path/to/smriti.db]
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

# Parse optional --db flag
DB_FLAG=""
if [ "$1" = "--db" ] && [ -n "$2" ]; then
    DB_FLAG="--db $2"
    echo "Using database: $2"
fi

# Activate venv
if [ -d "$VENV_DIR" ]; then
    source "$VENV_DIR/bin/activate"
else
    echo "ERROR: Virtual environment not found. Run setup.sh first."
    exit 1
fi

# Check API key
if [ -z "$ANTHROPIC_API_KEY" ]; then
    if [ -f "$SCRIPT_DIR/.env" ]; then
        export $(grep -v '^#' "$SCRIPT_DIR/.env" | xargs)
    fi
    if [ -z "$ANTHROPIC_API_KEY" ]; then
        echo "ERROR: ANTHROPIC_API_KEY not set."
        exit 1
    fi
fi

PASS=0
FAIL=0
TOTAL=0

run_test() {
    local agent="$1"
    local description="$2"
    local query="$3"
    TOTAL=$((TOTAL + 1))

    echo "------------------------------------------------------------"
    echo "TEST $TOTAL: $description"
    echo "  Agent: $agent"
    echo "  Query: $query"
    echo "------------------------------------------------------------"

    if timeout 120 python "$SCRIPT_DIR/$agent" $DB_FLAG -q "$query" 2>&1; then
        echo
        echo "  RESULT: PASS"
        PASS=$((PASS + 1))
    else
        echo
        echo "  RESULT: FAIL (exit code: $?)"
        FAIL=$((FAIL + 1))
    fi
    echo
}

echo "============================================"
echo "  Smriti Agent Test Suite"
echo "============================================"
echo

# ---- General-Purpose Agent Tests ----

echo ">>> GENERAL-PURPOSE AGENT TESTS <<<"
echo

run_test "smriti_agent.py" \
    "List notes in the knowledge graph" \
    "List all notes in the knowledge graph. Show me the titles."

run_test "smriti_agent.py" \
    "Full-text search" \
    "Search for notes about 'strategy' or 'decision'. Summarize what you find."

run_test "smriti_agent.py" \
    "Graph traversal" \
    "Pick any note and explore its graph neighborhood (depth 2). What connections do you find?"

run_test "smriti_agent.py" \
    "Memory store and retrieve" \
    "Store a memory entry with key 'test-run' and value 'Agent test at $(date)'. Then retrieve it to confirm it was saved."

run_test "smriti_agent.py" \
    "Create and link a note" \
    "Create a test note titled 'Agent Test Note' with content 'This note was created by the Smriti agent test runner. [[Q3 B2B Pivot Decision]]' and tag it with #test. Then read it back to confirm."

# ---- PI Research Agent Tests ----

echo ">>> PI RESEARCH AGENT TESTS <<<"
echo

run_test "pi_research_agent.py" \
    "Cross-project search" \
    "Search across all notes for anything related to 'market' or 'competitive'. What patterns do you see?"

run_test "pi_research_agent.py" \
    "Graph exploration" \
    "Explore the full knowledge graph structure. How many nodes and edges are there? What are the main clusters?"

run_test "pi_research_agent.py" \
    "Note creation with conventions" \
    "Create a note titled 'Test Research Protocol v1' tagged #protocol #test with content describing a mock experiment that links to [[Agent Test Note]]."

# ---- Results ----

echo "============================================"
echo "  TEST RESULTS"
echo "============================================"
echo
echo "  Total:  $TOTAL"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo

if [ "$FAIL" -eq 0 ]; then
    echo "  ALL TESTS PASSED!"
else
    echo "  SOME TESTS FAILED — check output above."
    exit 1
fi
