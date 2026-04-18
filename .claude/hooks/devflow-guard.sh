#!/bin/bash
# devflow-guard.sh — PreToolUse hook for Bash commands
# Blocks git commit / gh pr merge when devflow-state.md exists
# and the current stage doesn't allow these operations.
# Note: `gh pr create` is always allowed (incremental/draft PRs are fine);
# only `gh pr merge` is gated on Phase=complete.

set -euo pipefail

# Read tool input from stdin
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // ""')

# Determine which operation is attempted
IS_GIT_COMMIT=false
IS_GH_PR_MERGE=false

if echo "$COMMAND" | grep -qE '(^|\s|&&|\|)git\s+(commit|add\s+.*&&\s*git\s+commit)'; then
  IS_GIT_COMMIT=true
fi

if echo "$COMMAND" | grep -qE '(^|\s|&&|\|)gh\s+pr\s+merge'; then
  IS_GH_PR_MERGE=true
fi

# If neither, pass through
if [ "$IS_GIT_COMMIT" = false ] && [ "$IS_GH_PR_MERGE" = false ]; then
  exit 0
fi

# Find devflow-state.md (check both project root and worktree)
STATE_FILE=""
for candidate in \
  "devflow-docs/devflow-state.md" \
  "../devflow-docs/devflow-state.md" \
  "../../devflow-docs/devflow-state.md"; do
  if [ -f "$candidate" ]; then
    STATE_FILE="$candidate"
    break
  fi
done

# No devflow-state.md — pass through
if [ -z "$STATE_FILE" ]; then
  exit 0
fi

# Parse current phase and stage
PHASE=$(grep -E '^## Current Phase' "$STATE_FILE" -A1 | tail -1 | tr -d '[:space:]')
STAGE=$(grep -E '^## Current Stage' "$STATE_FILE" -A1 | tail -1 | tr -d '[:space:]')

# Exception: docs-only commits (devflow-docs, .claude/hooks, Cargo.lock) allowed in any phase
if [ "$IS_GIT_COMMIT" = true ]; then
  # Check if git add targets are all docs/config (no src/ changes)
  if echo "$COMMAND" | grep -qE 'git\s+add' && \
     ! echo "$COMMAND" | grep -qE 'git\s+add\s+(-A|\.|\s+src/)'; then
    # If staged files are only devflow-docs, .claude, Cargo.lock — allow
    STAGED_SRC=$(git diff --cached --name-only 2>/dev/null | grep -v '^devflow-docs/' | grep -v '^\.claude/' | grep -v '^Cargo\.lock$' | head -1)
    if [ -z "$STAGED_SRC" ]; then
      exit 0
    fi
  fi
fi

# Rule 1: git commit allowed during CONSTRUCTION (any stage) or when complete
if [ "$IS_GIT_COMMIT" = true ]; then
  if [ "$PHASE" = "CONSTRUCTION" ]; then
    exit 0
  fi
  if [ "$PHASE" = "complete" ] || [ "$PHASE" = "finished" ]; then
    exit 0
  fi
  echo '{"decision":"block","reason":"devflow 위반: git commit은 CONSTRUCTION 또는 완료 단계에서만 허용됩니다. 현재: Phase='"$PHASE"', Stage='"$STAGE"'. devflow 오케스트레이터의 안내를 따라주세요."}'
  exit 0
fi

# Rule 2: gh pr merge allowed only when phase is complete
# (gh pr create is unrestricted — use --draft for incremental PRs)
if [ "$IS_GH_PR_MERGE" = true ]; then
  if [ "$PHASE" = "complete" ] || [ "$PHASE" = "finished" ]; then
    exit 0
  fi
  echo '{"decision":"block","reason":"devflow 위반: gh pr merge는 Phase=complete일 때만 허용됩니다. 현재: Phase='"$PHASE"', Stage='"$STAGE"'. aidlc-finishing-a-development-branch 스킬을 먼저 실행해 머지 준비를 완료하세요. (gh pr create는 중간 단계에서도 가능합니다.)"}'
  exit 0
fi
