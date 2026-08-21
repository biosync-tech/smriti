#!/bin/bash
# ==============================================================================
# Smriti Agent Setup Script
# ==============================================================================
# Sets up a Python virtual environment with the Claude Agent SDK
# and verifies all dependencies for running Smriti agents.
#
# Usage:
#   chmod +x setup.sh
#   ./setup.sh
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

echo "============================================"
echo "  Smriti Agent Setup"
echo "============================================"
echo

# --- Check Python ---
echo "[1/5] Checking Python..."
if command -v python3 &>/dev/null; then
    PY=$(command -v python3)
    PY_VERSION=$($PY --version 2>&1)
    echo "  Found: $PY_VERSION at $PY"
else
    echo "  ERROR: Python 3.10+ is required but not found."
    echo "  Install from: https://www.python.org/downloads/"
    exit 1
fi

# Check Python version >= 3.10
PY_MINOR=$($PY -c "import sys; print(sys.version_info.minor)")
PY_MAJOR=$($PY -c "import sys; print(sys.version_info.major)")
if [ "$PY_MAJOR" -lt 3 ] || ([ "$PY_MAJOR" -eq 3 ] && [ "$PY_MINOR" -lt 10 ]); then
    echo "  ERROR: Python 3.10+ required, found $PY_MAJOR.$PY_MINOR"
    exit 1
fi

# --- Check Smriti binary ---
echo "[2/5] Checking Smriti binary..."
if command -v smriti &>/dev/null; then
    SMRITI_PATH=$(command -v smriti)
    echo "  Found: $SMRITI_PATH"
    smriti --version 2>/dev/null || echo "  (version check not supported)"
elif [ -f "$HOME/.cargo/bin/smriti" ]; then
    echo "  Found: $HOME/.cargo/bin/smriti"
    echo "  TIP: Add ~/.cargo/bin to your PATH:"
    echo "    export PATH=\"\$HOME/.cargo/bin:\$PATH\""
else
    echo "  ERROR: Smriti binary not found."
    echo "  Build it from the repo:"
    echo "    cd /path/to/smriti && cargo install --path ."
    exit 1
fi

# --- Check API key ---
echo "[3/5] Checking ANTHROPIC_API_KEY..."
if [ -n "$ANTHROPIC_API_KEY" ]; then
    echo "  Found: ${ANTHROPIC_API_KEY:0:8}...${ANTHROPIC_API_KEY: -4}"
elif [ -f "$SCRIPT_DIR/.env" ]; then
    echo "  Found .env file — will be loaded at runtime."
    echo "  Make sure it contains: ANTHROPIC_API_KEY=sk-ant-..."
elif [ -f "$HOME/.env" ]; then
    echo "  Found ~/.env file."
else
    echo "  WARNING: ANTHROPIC_API_KEY not set."
    echo "  Set it before running agents:"
    echo "    export ANTHROPIC_API_KEY=sk-ant-your-key-here"
    echo "  Or create a .env file in this directory."
fi

# --- Create virtual environment ---
echo "[4/5] Creating Python virtual environment..."
if [ -d "$VENV_DIR" ]; then
    echo "  Virtual environment already exists at $VENV_DIR"
    echo "  Updating packages..."
else
    $PY -m venv "$VENV_DIR"
    echo "  Created at $VENV_DIR"
fi

# Activate and install
source "$VENV_DIR/bin/activate"
pip install --quiet --upgrade pip

echo "[5/5] Installing dependencies..."
pip install --quiet -r "$SCRIPT_DIR/requirements.txt"
echo "  Installed: $(pip show claude-agent-sdk 2>/dev/null | grep Version || echo 'claude-agent-sdk')"

# --- Verify installation ---
echo
echo "============================================"
echo "  Verification"
echo "============================================"
echo

python3 -c "
from claude_agent_sdk import query, ClaudeAgentOptions, AgentDefinition
print('  claude-agent-sdk: OK')
print('  - query: available')
print('  - ClaudeAgentOptions: available')
print('  - AgentDefinition: available')
" 2>&1 || {
    echo "  ERROR: Failed to import claude-agent-sdk"
    exit 1
}

echo
echo "============================================"
echo "  Setup Complete!"
echo "============================================"
echo
echo "To activate the environment:"
echo "  source $VENV_DIR/bin/activate"
echo
echo "To run the general-purpose agent:"
echo "  python $SCRIPT_DIR/smriti_agent.py"
echo
echo "To run the PI research agent:"
echo "  python $SCRIPT_DIR/pi_research_agent.py"
echo
echo "Quick test (single query):"
echo "  python $SCRIPT_DIR/smriti_agent.py -q 'List all notes in the graph'"
echo
