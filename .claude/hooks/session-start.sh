#!/bin/bash
# SessionStart hook for Claude Code on the web: make a fresh container a
# working overlaid newbound instance before the session begins.
#
# Runs tools/setup.sh against the newbound checkout beside this repo —
# overlay symlinks, the three-step first build, the agent/kb/scratch
# dylibs, git hygiene for builder-written local state, and the overlay
# probe. Idempotent: on a container whose state was cached from a prior
# session it passes through in seconds.
set -euo pipefail

# Web sessions only; local checkouts are the owner's own arrangement.
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

if [ ! -d "$CLAUDE_PROJECT_DIR/../newbound/data" ]; then
  echo "session-start: no newbound checkout beside this repo — skipping setup (add the newbound repo to the session's sources)"
  exit 0
fi

"$CLAUDE_PROJECT_DIR/tools/setup.sh" "$CLAUDE_PROJECT_DIR/../newbound"
